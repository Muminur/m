use crate::database::{segments, transcripts, Database};
use crate::error::AppError;
use crate::models::{manager::ModelManager, registry::ModelInfo};
use crate::settings::{AccelerationBackend, AppSettings};
use crate::transcription::{engine::TranscriptionParams, pipeline::TranscriptionManager};
use rusqlite::{Connection, OptionalExtension};
use std::sync::Arc;
use tauri::{command, AppHandle, State};

/// Whether Whisper Metal inference is safe on this build target.
///
/// whisper.cpp Metal is supported only on Apple Silicon in WhisperDesk. Intel
/// Macs must use CPU inference to avoid native aborts and invalid output.
#[command]
pub fn is_metal_available() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
    pub id: String,
    pub label: String,
    pub color: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDetail {
    pub transcript: transcripts::TranscriptRow,
    pub segments: Vec<segments::SegmentRow>,
    pub speakers: Vec<Speaker>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTranscriptionResult {
    pub job_id: String,
    pub transcript_id: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionPerformance {
    pub realtime_factor: f64,
    pub backend_used: String,
    pub wall_time_ms: i64,
    pub transcript_id: String,
}

fn load_transcription_performance(
    conn: &Connection,
    transcript_id: &str,
) -> Result<Option<TranscriptionPerformance>, AppError> {
    conn.query_row(
        "SELECT realtime_factor, backend, wall_time_ms, transcript_id
         FROM acceleration_stats
         WHERE transcript_id = ?1
         ORDER BY recorded_at DESC
         LIMIT 1",
        [transcript_id],
        |row| {
            Ok(TranscriptionPerformance {
                realtime_factor: row.get(0)?,
                backend_used: row.get(1)?,
                wall_time_ms: row.get(2)?,
                transcript_id: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

// ─── Model commands ───────────────────────────────────────────────────────────

#[command]
pub async fn list_models(db: State<'_, Arc<Database>>) -> Result<Vec<ModelInfo>, AppError> {
    ModelManager::list_models(&db)
}

#[command]
pub async fn download_model(
    model_id: String,
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<(), AppError> {
    // Fetch download metadata from DB
    let (download_url, sha256, file_size_mb) = {
        let conn = db.get()?;
        conn.query_row(
            "SELECT download_url, sha256, file_size_mb FROM whisper_models WHERE id = ?1",
            rusqlite::params![model_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(|_| AppError::ModelError {
            code: crate::error::ModelErrorCode::NotFound,
            message: format!("Model '{}' not found in registry", model_id),
        })?
    };

    ModelManager::start_download(
        Arc::clone(&model_manager),
        model_id,
        download_url,
        sha256,
        file_size_mb as u64,
        app_handle,
        Arc::clone(&db),
    )
}

#[command]
pub async fn cancel_model_download(
    model_id: String,
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<(), AppError> {
    model_manager.cancel_download(&model_id)
}

#[command]
pub async fn delete_model(
    model_id: String,
    db: State<'_, Arc<Database>>,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), AppError> {
    model_manager.delete_model(&model_id, &db, &transcription_manager)
}

#[command]
pub async fn set_default_model(
    model_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    ModelManager::set_default_model(&model_id, &db)
}

// ─── Transcription commands ───────────────────────────────────────────────────

#[command]
#[allow(clippy::too_many_arguments)]
pub async fn transcribe_file(
    audio_path: String,
    model_id: String,
    params: Option<TranscriptionParams>,
    existing_transcript_id: Option<String>,
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_manager: State<'_, Arc<ModelManager>>,
    settings_state: State<'_, std::sync::Mutex<AppSettings>>,
) -> Result<StartTranscriptionResult, AppError> {
    let params = params.unwrap_or_default();

    let backend = settings_state
        .lock()
        .map(|s| s.acceleration_backend.clone())
        .unwrap_or(AccelerationBackend::Auto);

    let (job_id, transcript_id) = TranscriptionManager::start_transcription(
        Arc::clone(&transcription_manager),
        audio_path,
        model_id,
        params,
        backend,
        app_handle,
        Arc::clone(&db),
        Arc::clone(&model_manager),
        existing_transcript_id,
    )?;

    Ok(StartTranscriptionResult {
        job_id,
        transcript_id,
    })
}

#[command]
pub async fn get_transcription_performance(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<TranscriptionPerformance>, AppError> {
    let conn = db.get()?;
    load_transcription_performance(&conn, &transcript_id)
}

#[command]
pub async fn cancel_transcription(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), AppError> {
    transcription_manager.abort();
    Ok(())
}

// ─── Transcript query commands ────────────────────────────────────────────────

#[command]
pub async fn get_transcript(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<TranscriptDetail, AppError> {
    let conn = db.get()?;

    let transcript = transcripts::get_by_id(&conn, &id)?.ok_or_else(|| AppError::StorageError {
        code: crate::error::StorageErrorCode::DatabaseError,
        message: format!("Transcript '{}' not found", id),
    })?;

    let segs = segments::get_by_transcript(&conn, &id)?;

    // Query speakers for this transcript
    let mut stmt = conn
        .prepare(
            "SELECT id, label, color FROM speakers WHERE transcript_id = ?1 ORDER BY rowid ASC",
        )
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Failed to query speakers: {}", e),
        })?;

    let speakers: Vec<Speaker> = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok(Speaker {
                id: row.get(0)?,
                label: row.get(1)?,
                color: row.get(2)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Failed to map speakers: {}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Failed to collect speakers: {}", e),
        })?;

    Ok(TranscriptDetail {
        transcript,
        segments: segs,
        speakers,
    })
}

#[command]
pub async fn list_transcripts(
    page: Option<u32>,
    page_size: Option<u32>,
    filter: Option<transcripts::ListFilter>,
    sort: Option<transcripts::ListSort>,
    db: State<'_, Arc<Database>>,
) -> Result<serde_json::Value, AppError> {
    let page = page.unwrap_or(0);
    let page_size = page_size.unwrap_or(50);
    let filter = filter.unwrap_or_default();
    let sort = sort.unwrap_or_default();

    let conn = db.get()?;
    let (rows, total) = transcripts::list_filtered(&conn, &filter, &sort, page, page_size)?;

    Ok(serde_json::json!({
        "items": rows,
        "total": total,
        "page": page,
        "pageSize": page_size,
    }))
}

#[command]
pub async fn update_segment(
    segment_id: String,
    text: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    segments::update_text(&conn, &segment_id, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;

    #[test]
    fn metal_capability_matches_build_target() {
        assert_eq!(
            is_metal_available(),
            cfg!(all(target_os = "macos", target_arch = "aarch64"))
        );
    }

    #[test]
    fn loads_latest_performance_for_transcript() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        let transcript_id = transcripts::insert(
            &conn,
            &transcripts::NewTranscript {
                title: "Performance test".into(),
                duration_ms: None,
                language: None,
                model_id: None,
                source_type: None,
                source_url: None,
                audio_path: None,
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO acceleration_stats
             (id, transcript_id, model_id, backend, audio_duration_ms, wall_time_ms, realtime_factor)
             VALUES ('stat-1', ?1, 'base', 'cpu', 6000, 3000, 2.0)",
            [&transcript_id],
        )
        .unwrap();

        let stats = load_transcription_performance(&conn, &transcript_id)
            .unwrap()
            .unwrap();
        assert_eq!(stats.transcript_id, transcript_id);
        assert_eq!(stats.backend_used, "cpu");
        assert_eq!(stats.wall_time_ms, 3000);
        assert_eq!(stats.realtime_factor, 2.0);
        assert!(load_transcription_performance(&conn, "missing")
            .unwrap()
            .is_none());
    }
}
