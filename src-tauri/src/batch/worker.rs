use crate::audio::decode;
use crate::database::{segments, transcripts, Database};
use crate::error::AppError;
use crate::models::manager::ModelManager;
use crate::settings::AccelerationBackend;
use crate::transcription::engine::{TranscriptionParams, WhisperEngine};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::queue::{BatchItemStatus, BatchJobItem};

/// Process a single batch item by running the full transcription pipeline.
/// Returns (final_status, transcript_id, error_msg).
pub async fn process_item(
    item: &BatchJobItem,
    model_manager: &ModelManager,
    db: &Arc<Database>,
    model_id: Option<&str>,
    language: Option<&str>,
) -> (BatchItemStatus, Option<String>, Option<String>) {
    // Validate the file exists
    let audio_path = std::path::PathBuf::from(&item.file_path);
    if !audio_path.exists() {
        return (
            BatchItemStatus::Failed,
            None,
            Some(format!("File not found: {}", item.file_path)),
        );
    }

    // Resolve model_id — required for transcription
    let model_id = match model_id {
        Some(id) => id.to_string(),
        None => {
            return (
                BatchItemStatus::Failed,
                None,
                Some("No model_id specified for batch job".into()),
            );
        }
    };

    // Verify the model is downloaded
    if !model_manager.is_downloaded(&model_id) {
        return (
            BatchItemStatus::Failed,
            None,
            Some(format!("Model '{}' is not downloaded", model_id)),
        );
    }

    let model_path = model_manager.model_path(&model_id);
    let db = Arc::clone(db);
    let language_owned = language.map(|s| s.to_string());
    let file_path = item.file_path.clone();

    // Run transcription on a blocking thread — whisper-rs is CPU-bound
    let result = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        // Step 1: Decode audio
        let decoded = decode::decode_file(&audio_path)?;
        let duration_ms = decoded.duration_ms;

        // Step 2: Resample to whisper format (mono 16kHz)
        let pcm = decode::resample_to_whisper(&decoded)?;

        // Step 3: Build transcription params
        let params = TranscriptionParams {
            language: language_owned,
            ..TranscriptionParams::default()
        };

        // Step 4: Load engine and run inference
        let engine = WhisperEngine::new(&model_path, AccelerationBackend::Auto)?;
        let abort_flag = Arc::new(AtomicBool::new(false));
        let output = engine.transcribe(
            &params,
            &pcm,
            |_progress| {
                // Batch items don't emit per-item whisper progress
            },
            abort_flag,
        )?;

        let segments_result = output.segments;

        // Step 5: Create transcript record
        let title = std::path::Path::new(&file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let transcript_id = {
            let conn = db.get()?;
            transcripts::insert(
                &conn,
                &transcripts::NewTranscript {
                    title,
                    duration_ms: Some(duration_ms as i64),
                    language: params.language.clone(),
                    model_id: Some(model_id),
                    // Batch items are still ordinary imported files. Keep the
                    // storage value within transcripts.source_type's contract.
                    source_type: Some("file".to_string()),
                    source_url: None,
                    audio_path: Some(file_path),
                },
            )?
        };

        // Step 6: Insert segments
        {
            let conn = db.get()?;
            segments::insert_batch(&conn, &transcript_id, &segments_result)?;

            let word_count: i64 = segments_result
                .iter()
                .map(|s| s.text.split_whitespace().count() as i64)
                .sum();

            conn.execute(
                "UPDATE transcripts SET word_count = ?1, updated_at = strftime('%s','now') WHERE id = ?2",
                rusqlite::params![word_count, transcript_id],
            )?;
        }

        Ok(transcript_id)
    })
    .await;

    match result {
        Ok(Ok(transcript_id)) => (BatchItemStatus::Completed, Some(transcript_id), None),
        Ok(Err(e)) => (
            BatchItemStatus::Failed,
            None,
            Some(format!("Transcription failed: {}", e)),
        ),
        Err(e) => (
            BatchItemStatus::Failed,
            None,
            Some(format!("Task panicked: {}", e)),
        ),
    }
}
