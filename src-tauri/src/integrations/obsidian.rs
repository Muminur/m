use crate::database::segments::SegmentRow;
use crate::error::{AppError, IntegrationErrorCode};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Configuration for Obsidian vault export.
pub struct ObsidianConfig {
    pub vault_path: String,
}

/// Format milliseconds as HH:MM:SS.
fn format_duration(ms: i64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Format a segment timestamp as [HH:MM:SS].
fn format_timestamp(ms: i64) -> String {
    format!("[{}]", format_duration(ms))
}

/// Sanitize a filename by removing characters invalid on any OS.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Build YAML frontmatter for the Obsidian markdown file.
fn build_frontmatter(
    duration_ms: i64,
    language: &str,
    speakers: &[String],
    created_at: i64,
) -> String {
    let date = chrono::DateTime::from_timestamp(created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let duration = format_duration(duration_ms);

    let speakers_yaml: Vec<String> = speakers.iter().map(|s| format!("\"{}\"", s)).collect();
    let speakers_str = format!("[{}]", speakers_yaml.join(", "));

    format!(
        "---\ndate: {}\nduration: {}\nlanguage: {}\nspeakers: {}\nsource: WhisperDesk\n---\n",
        date, duration, language, speakers_str
    )
}

/// Build markdown body from transcript segments with speaker headers.
fn build_body(segments: &[SegmentRow]) -> String {
    let mut body = String::new();
    let mut current_speaker: Option<&str> = None;

    for seg in segments {
        let speaker = seg.speaker_id.as_deref();

        // Add speaker header when speaker changes
        if speaker != current_speaker {
            if let Some(s) = speaker {
                body.push_str(&format!("\n## {}\n\n", s));
            }
            current_speaker = speaker;
        }

        let timestamp = format_timestamp(seg.start_ms);
        body.push_str(&format!("{} {}\n\n", timestamp, seg.text.trim()));
    }

    body
}

/// Write a transcript to an Obsidian vault as a markdown file.
///
/// Returns the absolute path of the written file.
pub fn write_to_obsidian(
    config: &ObsidianConfig,
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
    created_at: i64,
) -> Result<String, AppError> {
    let vault_path = Path::new(&config.vault_path);

    if !vault_path.is_dir() {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Obsidian vault path does not exist: {}", config.vault_path),
        });
    }

    let safe_title = sanitize_filename(title);
    if safe_title.is_empty() {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "Title cannot be empty".into(),
        });
    }

    // Verify file stays within vault (defense in depth)
    let safe_filename = format!("{}.md", safe_title);
    if safe_filename.contains("..") || safe_filename.contains('/') || safe_filename.contains('\\') {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "Invalid transcript title for file output".into(),
        });
    }

    let file_path: PathBuf = vault_path.join(&safe_filename);

    // Collect unique speakers from segments
    let speakers: Vec<String> = segments
        .iter()
        .filter_map(|s| s.speaker_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let frontmatter = build_frontmatter(duration_ms, language, &speakers, created_at);
    let body = build_body(segments);

    let content = format!("{}\n# {}\n\n{}", frontmatter, title, body);

    std::fs::write(&file_path, &content).map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ApiError,
        message: format!("Failed to write Obsidian file: {}", e),
    })?;

    Ok(file_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello/world"), "hello_world");
        assert_eq!(sanitize_filename("a:b*c?d"), "a_b_c_d");
        assert_eq!(sanitize_filename("normal"), "normal");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(754_000), "00:12:34");
    }

    #[test]
    fn test_build_frontmatter() {
        let fm = build_frontmatter(
            754_000,
            "en",
            &["Speaker 1".to_string(), "Speaker 2".to_string()],
            1705276800, // 2024-01-15 approx
        );
        assert!(fm.contains("duration: 00:12:34"));
        assert!(fm.contains("language: en"));
        assert!(fm.contains("source: WhisperDesk"));
        assert!(fm.contains("Speaker 1"));
    }

    #[test]
    fn test_build_body_with_speakers() {
        let segments = vec![
            SegmentRow {
                id: "s1".into(),
                transcript_id: "t1".into(),
                index_num: 0,
                start_ms: 0,
                end_ms: 5000,
                text: "Hello there".into(),
                speaker_id: Some("Alice".into()),
                confidence: None,
                is_deleted: false,
            },
            SegmentRow {
                id: "s2".into(),
                transcript_id: "t1".into(),
                index_num: 1,
                start_ms: 5000,
                end_ms: 10000,
                text: "Hi Alice".into(),
                speaker_id: Some("Bob".into()),
                confidence: None,
                is_deleted: false,
            },
        ];
        let body = build_body(&segments);
        assert!(body.contains("## Alice"));
        assert!(body.contains("## Bob"));
        assert!(body.contains("[00:00:00] Hello there"));
        assert!(body.contains("[00:00:05] Hi Alice"));
    }

    #[test]
    fn test_write_to_obsidian_invalid_vault() {
        let config = ObsidianConfig {
            vault_path: "/nonexistent/vault/path".into(),
        };
        let result = write_to_obsidian(&config, "test", &[], 0, "en", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_to_obsidian_empty_title() {
        let config = ObsidianConfig {
            vault_path: ".".into(),
        };
        let result = write_to_obsidian(&config, "", &[], 0, "en", 0);
        assert!(result.is_err());
    }
}
