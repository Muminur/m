use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, ImportErrorCode};
use crate::import::ytdlp::YtDlpManager;

// ── Result types ───────────────────────────────────────────────────────────────

/// Result of a successful YouTube audio download.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeImportResult {
    /// Absolute path to the downloaded WAV file.
    pub audio_path: String,
    /// Video title as reported by yt-dlp.
    pub title: String,
    /// Duration in milliseconds (if available).
    pub duration_ms: Option<u64>,
    /// Original YouTube URL.
    pub source_url: String,
    /// Thumbnail URL (if available).
    pub thumbnail_url: Option<String>,
}

/// Progress event payload emitted as `youtube:download-progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeDownloadProgress {
    /// 0.0 – 100.0
    pub percent: f64,
    /// Human-readable ETA string from yt-dlp (e.g. "00:12").
    pub eta: Option<String>,
    /// Current download speed string from yt-dlp (e.g. "1.23MiB/s").
    pub speed: Option<String>,
}

// ── Importer ──────────────────────────────────────────────────────────────────

/// Handles downloading audio from YouTube URLs via yt-dlp.
pub struct YouTubeImporter;

impl YouTubeImporter {
    /// Validate the URL, invoke yt-dlp to extract audio as WAV, and return
    /// metadata about the downloaded file.
    ///
    /// Progress events (`youtube:download-progress`) are emitted on `app`.
    pub fn import(
        url: &str,
        output_dir: &Path,
        app: Option<&AppHandle>,
    ) -> Result<YouTubeImportResult, AppError> {
        // 1. Validate URL
        Self::validate_url(url)?;

        // 2. Locate yt-dlp
        let binary = YtDlpManager::get_binary_path()?;

        // 3. Build output template — yt-dlp will substitute %(title)s etc.
        std::fs::create_dir_all(output_dir).map_err(|e| AppError::ImportError {
            code: ImportErrorCode::DownloadFailed,
            message: format!("Cannot create output directory: {}", e),
        })?;

        let output_template = output_dir.join("%(title)s.%(ext)s");
        let output_template_str = output_template.to_string_lossy().into_owned();

        // 4. Emit initial progress
        if let Some(handle) = app {
            let _ = handle.emit(
                "youtube:download-progress",
                YouTubeDownloadProgress {
                    percent: 0.0,
                    eta: None,
                    speed: None,
                },
            );
        }

        // 5. Run yt-dlp
        //    -x              extract audio only
        //    --audio-format  convert to wav
        //    --audio-quality 0  best quality
        //    --print-json    write metadata JSON to stdout
        //    --no-playlist   never download entire playlists by accident
        let output = Command::new(&binary)
            .args([
                "-x",
                "--audio-format",
                "wav",
                "--audio-quality",
                "0",
                "--no-playlist",
                "--print-json",
                "-o",
                &output_template_str,
                url,
            ])
            .output()
            .map_err(|e| AppError::ImportError {
                code: ImportErrorCode::DownloadFailed,
                message: format!("Failed to spawn yt-dlp: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let code = Self::classify_error(&stderr);
            return Err(AppError::ImportError {
                code,
                message: format!(
                    "yt-dlp failed: {}",
                    stderr.lines().last().unwrap_or("unknown error")
                ),
            });
        }

        // 6. Parse JSON metadata from stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        let meta: serde_json::Value =
            serde_json::from_str(stdout.trim()).map_err(|_| AppError::ImportError {
                code: ImportErrorCode::DownloadFailed,
                message: "yt-dlp did not return valid JSON metadata".into(),
            })?;

        let title = meta["title"].as_str().unwrap_or("Untitled").to_string();

        let duration_ms = meta["duration"].as_f64().map(|secs| (secs * 1000.0) as u64);

        let thumbnail_url = meta["thumbnail"].as_str().map(str::to_string);

        // 7. Locate the actual output file
        //    yt-dlp writes the final filename into the "filename" key after
        //    post-processing; fall back to searching the directory.
        let audio_path = Self::resolve_output_file(&meta, output_dir, &title)?;

        // 8. Emit completion progress
        if let Some(handle) = app {
            let _ = handle.emit(
                "youtube:download-progress",
                YouTubeDownloadProgress {
                    percent: 100.0,
                    eta: Some("00:00".into()),
                    speed: None,
                },
            );
        }

        Ok(YouTubeImportResult {
            audio_path: audio_path.to_string_lossy().into_owned(),
            title,
            duration_ms,
            source_url: url.to_string(),
            thumbnail_url,
        })
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Accept only youtube.com (and www/music subdomains) and youtu.be URLs.
    pub fn validate_url(url: &str) -> Result<(), AppError> {
        let lower = url.to_lowercase();
        let is_valid = lower.starts_with("https://www.youtube.com/")
            || lower.starts_with("http://www.youtube.com/")
            || lower.starts_with("https://youtube.com/")
            || lower.starts_with("http://youtube.com/")
            || lower.starts_with("https://music.youtube.com/")
            || lower.starts_with("https://youtu.be/")
            || lower.starts_with("http://youtu.be/");

        if !is_valid {
            return Err(AppError::ImportError {
                code: ImportErrorCode::InvalidUrl,
                message: format!(
                    "Invalid YouTube URL '{}'. Must be from youtube.com or youtu.be.",
                    url
                ),
            });
        }
        Ok(())
    }

    /// Map yt-dlp stderr output to an appropriate error code.
    fn classify_error(stderr: &str) -> ImportErrorCode {
        let lower = stderr.to_lowercase();
        if lower.contains("unable to download") || lower.contains("http error") {
            ImportErrorCode::DownloadFailed
        } else if lower.contains("network")
            || lower.contains("connection")
            || lower.contains("timeout")
        {
            ImportErrorCode::DownloadFailed
        } else if lower.contains("unavailable")
            || lower.contains("private")
            || lower.contains("removed")
        {
            ImportErrorCode::DownloadFailed
        } else {
            ImportErrorCode::DownloadFailed
        }
    }

    /// Determine the final WAV file path from yt-dlp metadata or by scanning
    /// the output directory for a .wav file that contains the title.
    fn resolve_output_file(
        meta: &serde_json::Value,
        output_dir: &Path,
        title: &str,
    ) -> Result<PathBuf, AppError> {
        // yt-dlp sets "filename" to the post-processed path
        if let Some(filename) = meta["filename"].as_str() {
            let p = PathBuf::from(filename);
            // After audio extraction the extension is replaced with .wav
            let wav = p.with_extension("wav");
            if wav.exists() {
                return Ok(wav);
            }
            if p.exists() {
                return Ok(p);
            }
        }

        // Fallback: find a .wav in the output directory whose name contains the title
        let safe_title: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        if let Ok(entries) = std::fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.ends_with(".wav") && name.contains(safe_title.trim()) {
                    return Ok(entry.path());
                }
            }
            // Last resort: any .wav in the dir
            if let Ok(entries2) = std::fs::read_dir(output_dir) {
                for entry in entries2.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.ends_with(".wav") {
                        return Ok(entry.path());
                    }
                }
            }
        }

        Err(AppError::ImportError {
            code: ImportErrorCode::DownloadFailed,
            message: format!(
                "Could not locate downloaded WAV file in '{}'",
                output_dir.display()
            ),
        })
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_accepts_youtube_com() {
        assert!(
            YouTubeImporter::validate_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").is_ok()
        );
    }

    #[test]
    fn test_validate_url_accepts_youtu_be() {
        assert!(YouTubeImporter::validate_url("https://youtu.be/dQw4w9WgXcQ").is_ok());
    }

    #[test]
    fn test_validate_url_accepts_music_youtube() {
        assert!(YouTubeImporter::validate_url("https://music.youtube.com/watch?v=abc123").is_ok());
    }

    #[test]
    fn test_validate_url_rejects_vimeo() {
        let err = YouTubeImporter::validate_url("https://vimeo.com/123456789").unwrap_err();
        match err {
            AppError::ImportError {
                code: ImportErrorCode::InvalidUrl,
                ..
            } => {}
            other => panic!("Expected InvalidUrl, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_url_rejects_arbitrary_domain() {
        assert!(YouTubeImporter::validate_url("https://evil.com/watch?v=x").is_err());
    }

    #[test]
    fn test_validate_url_rejects_empty() {
        assert!(YouTubeImporter::validate_url("").is_err());
    }

    #[test]
    fn test_youtube_import_result_serializes_camel_case() {
        let result = YouTubeImportResult {
            audio_path: "/tmp/video.wav".into(),
            title: "Test Video".into(),
            duration_ms: Some(180_000),
            source_url: "https://youtu.be/abc".into(),
            thumbnail_url: Some("https://i.ytimg.com/vi/abc/hq.jpg".into()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("audioPath").is_some(),
            "expected camelCase audioPath"
        );
        assert!(
            json.get("sourceUrl").is_some(),
            "expected camelCase sourceUrl"
        );
        assert!(
            json.get("durationMs").is_some(),
            "expected camelCase durationMs"
        );
        assert!(
            json.get("thumbnailUrl").is_some(),
            "expected camelCase thumbnailUrl"
        );
        assert_eq!(json["durationMs"], 180_000u64);
    }

    #[test]
    fn test_youtube_import_result_optional_fields_null() {
        let result = YouTubeImportResult {
            audio_path: "/tmp/video.wav".into(),
            title: "Test".into(),
            duration_ms: None,
            source_url: "https://youtu.be/abc".into(),
            thumbnail_url: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json["durationMs"].is_null());
        assert!(json["thumbnailUrl"].is_null());
    }
}
