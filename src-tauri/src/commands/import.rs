use std::sync::Arc;
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager, State};

use crate::database::Database;
use crate::error::AppError;
use crate::import::storage;
use crate::import::youtube::{YouTubeImportResult, YouTubeImporter};
use crate::import::ytdlp::{YtDlpManager, YtDlpStatus};
use crate::transcription::postprocess::{FillerConfig, FillerWordRemover};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeImportStatus {
    pub available: bool,
    pub yt_dlp: YtDlpStatus,
    pub ffmpeg_available: bool,
}

// ── Filler config state ───────────────────────────────────────────────────────

/// Application-managed filler configuration (stored in Tauri state).
pub struct FillerConfigState(pub Mutex<FillerConfig>);

impl Default for FillerConfigState {
    fn default() -> Self {
        Self(Mutex::new(FillerConfig::default()))
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Download audio from a YouTube URL and return import metadata.
///
/// The caller is responsible for queuing the resulting `audio_path` for
/// transcription using the existing transcription pipeline.
#[command]
pub async fn import_youtube(
    url: String,
    app: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<YouTubeImportResult, AppError> {
    // Download in a per-job temp directory, then promote only a completed WAV
    // into durable app data. This keeps interrupted jobs disposable without
    // leaving successful transcript audio on a 24-hour cleanup timer.
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Failed to resolve app data directory for YouTube imports".into(),
        })?;
    storage::prepare_youtube_storage(&app_data_dir)?;
    let durable_root = storage::youtube_import_root(&app_data_dir);
    {
        let conn = db.get()?;
        storage::cleanup_abandoned_imports(
            &app_data_dir,
            &conn,
            storage::ABANDONED_IMPORT_MAX_AGE,
        )?;
    }

    let job_id = uuid::Uuid::new_v4().to_string();
    let staging_root = storage::youtube_staging_root(&app_data_dir);
    storage::cleanup_stale_staging(&app_data_dir, storage::ABANDONED_IMPORT_MAX_AGE);
    let staging_dir = staging_root.join(&job_id);
    let durable_job_dir = durable_root.join(job_id);

    // Wrap in spawn_blocking because YouTubeImporter::import uses std::process::Command
    // which blocks the thread — must not block the tokio async runtime.
    tokio::task::spawn_blocking(move || {
        let result = match YouTubeImporter::import(&url, &staging_dir, Some(&app)) {
            Ok(mut imported) => {
                match storage::promote_audio(
                    std::path::Path::new(&imported.audio_path),
                    &staging_dir,
                    &durable_job_dir,
                ) {
                    Ok(durable_path) => {
                        imported.audio_path = durable_path.to_string_lossy().into_owned();
                        Ok(imported)
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };

        if let Err(error) = storage::remove_owned_staging_job(&app_data_dir, &staging_dir) {
            tracing::warn!(?error, ?staging_dir, "failed to clean YouTube staging job");
        }
        if result.is_err() {
            if let Err(error) = storage::remove_owned_import_job(&app_data_dir, &durable_job_dir) {
                tracing::warn!(
                    ?error,
                    ?durable_job_dir,
                    "failed to clean incomplete durable YouTube import"
                );
            }
        }

        result
    })
    .await
    .map_err(|e| AppError::ImportError {
        code: crate::error::ImportErrorCode::DownloadFailed,
        message: format!("Task join error: {}", e),
    })?
}

/// Return the current yt-dlp availability status.
#[command]
pub async fn check_ytdlp_status() -> Result<YtDlpStatus, AppError> {
    YtDlpManager::detect()
}

/// Check every external dependency required for YouTube audio import.
#[command]
pub async fn check_youtube_import_status() -> Result<YouTubeImportStatus, AppError> {
    let yt_dlp = YtDlpManager::detect()?;
    let ffmpeg_available = YtDlpManager::find_ffmpeg_path().is_some();
    Ok(YouTubeImportStatus {
        available: matches!(&yt_dlp, YtDlpStatus::Available { .. }) && ffmpeg_available,
        yt_dlp,
        ffmpeg_available,
    })
}

#[cfg(test)]
mod youtube_status_tests {
    use super::*;

    #[test]
    fn unavailable_status_serializes_for_the_frontend() {
        let status = YouTubeImportStatus {
            available: false,
            yt_dlp: YtDlpStatus::NotFound,
            ffmpeg_available: false,
        };
        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json["available"], false);
        assert_eq!(json["ytDlp"]["status"], "notFound");
        assert_eq!(json["ffmpegAvailable"], false);
    }

    #[test]
    fn youtube_imports_use_unique_subdirectories() {
        let root = std::env::temp_dir().join("whisperdesk_yt_imports");
        let first = root.join(uuid::Uuid::new_v4().to_string());
        let second = root.join(uuid::Uuid::new_v4().to_string());
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(root.as_path()));
    }
}

/// Remove filler words from `text`.
///
/// If `word_list` is supplied it overrides the application-level config for
/// this call only.  Pass `null` / `None` to use the configured word list.
#[command]
pub async fn remove_filler_words(
    text: String,
    word_list: Option<Vec<String>>,
    config_state: State<'_, FillerConfigState>,
) -> Result<String, AppError> {
    let config = config_state
        .0
        .lock()
        .map_err(|_| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Failed to acquire filler config lock".into(),
        })?
        .clone();

    if !config.enabled {
        return Ok(text);
    }

    let effective_list = word_list.unwrap_or(config.word_list);
    let remover = FillerWordRemover::new(effective_list);
    Ok(remover.remove(&text))
}

/// Return the current filler word configuration.
#[command]
pub async fn get_filler_config(
    config_state: State<'_, FillerConfigState>,
) -> Result<FillerConfig, AppError> {
    config_state
        .0
        .lock()
        .map(|c| c.clone())
        .map_err(|_| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Failed to acquire filler config lock".into(),
        })
}

/// Update the filler word configuration.
#[command]
pub async fn set_filler_config(
    config: FillerConfig,
    config_state: State<'_, FillerConfigState>,
) -> Result<(), AppError> {
    let mut guard = config_state.0.lock().map_err(|_| AppError::StorageError {
        code: crate::error::StorageErrorCode::IoError,
        message: "Failed to acquire filler config lock".into(),
    })?;
    *guard = config;
    Ok(())
}
