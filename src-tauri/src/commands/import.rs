use std::sync::Mutex;
use tauri::{command, AppHandle, State};

use crate::error::AppError;
use crate::import::youtube::{YouTubeImportResult, YouTubeImporter};
use crate::import::ytdlp::{YtDlpManager, YtDlpStatus};
use crate::transcription::postprocess::{FillerConfig, FillerWordRemover};

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
pub async fn import_youtube(url: String, app: AppHandle) -> Result<YouTubeImportResult, AppError> {
    // Resolve a per-session temp directory for the download.
    // Wrap in spawn_blocking because YouTubeImporter::import uses std::process::Command
    // which blocks the thread — must not block the tokio async runtime.
    let output_dir = std::env::temp_dir().join("whisperdesk_yt_imports");
    tokio::task::spawn_blocking(move || YouTubeImporter::import(&url, &output_dir, Some(&app)))
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
