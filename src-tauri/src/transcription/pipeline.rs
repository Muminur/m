use crate::audio::decode;
use crate::database::{segments, transcripts, Database};
use crate::error::{AppError, TranscriptionErrorCode};
use crate::models::manager::ModelManager;
use crate::settings::AccelerationBackend;
use crate::transcription::engine::{SegmentResult, TranscriptionParams, WhisperEngine};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

// ─── Active job ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ActiveJob {
    pub job_id: String,
    pub model_id: String,
    pub abort_flag: Arc<AtomicBool>,
}

// ─── Event payloads ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentEvent {
    pub job_id: String,
    pub transcript_id: String,
    pub segment: SegmentResult,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProgressEvent {
    pub job_id: String,
    pub progress: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionCompleteEvent {
    pub job_id: String,
    pub transcript_id: String,
    pub segment_count: usize,
    pub duration_ms: u64,
    pub backend_used: String,
    pub realtime_factor: f64,
    pub wall_time_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendFallbackEvent {
    pub job_id: String,
    pub requested_backend: String,
    pub actual_backend: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionErrorEvent {
    pub job_id: String,
    pub transcript_id: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionCancelledEvent {
    pub job_id: String,
    pub transcript_id: String,
    pub segments_saved: usize,
}

// ─── Manager ─────────────────────────────────────────────────────────────────

pub struct TranscriptionManager {
    pub active_job: Mutex<Option<ActiveJob>>,
}

impl TranscriptionManager {
    pub fn new() -> Self {
        Self {
            active_job: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.active_job.lock().map(|j| j.is_some()).unwrap_or(false)
    }

    pub fn active_model_id(&self) -> Option<String> {
        self.active_job
            .lock()
            .ok()
            .and_then(|j| j.as_ref().map(|aj| aj.model_id.clone()))
    }

    pub fn abort(&self) {
        if let Ok(job) = self.active_job.lock() {
            if let Some(aj) = job.as_ref() {
                aj.abort_flag.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Kick off transcription. Returns (job_id, transcript_id) immediately.
    /// Progress and results are delivered via Tauri events.
    #[allow(clippy::too_many_arguments)]
    pub fn start_transcription(
        manager: Arc<TranscriptionManager>,
        audio_path: String,
        model_id: String,
        params: TranscriptionParams,
        backend: AccelerationBackend,
        app_handle: AppHandle,
        db: Arc<Database>,
        model_manager: Arc<ModelManager>,
    ) -> Result<(String, String), AppError> {
        // Guard: only one job at a time
        {
            let lock = manager
                .active_job
                .lock()
                .map_err(|_| AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: "Failed to acquire transcription job lock".into(),
                })?;
            if lock.is_some() {
                return Err(AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: "A transcription job is already running".into(),
                });
            }
        }

        // Validate model is available on disk
        if !model_manager.is_downloaded(&model_id) {
            return Err(AppError::ModelError {
                code: crate::error::ModelErrorCode::NotFound,
                message: format!("Model '{}' is not downloaded", model_id),
            });
        }

        let model_path = model_manager.model_path(&model_id);
        let audio_path_buf = std::path::PathBuf::from(&audio_path);

        // Create a transcript record immediately so the UI can navigate to it
        let title = audio_path_buf
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
                    duration_ms: None,
                    language: params.language.clone(),
                    model_id: Some(model_id.clone()),
                    source_type: Some("file".to_string()),
                    source_url: None,
                    audio_path: Some(audio_path.clone()),
                },
            )?
        };

        let job_id = uuid::Uuid::new_v4().to_string();
        let abort_flag = Arc::new(AtomicBool::new(false));

        {
            let mut lock = manager
                .active_job
                .lock()
                .map_err(|_| AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: "Failed to register active job".into(),
                })?;
            *lock = Some(ActiveJob {
                job_id: job_id.clone(),
                model_id,
                abort_flag: Arc::clone(&abort_flag),
            });
        }

        let manager_clone = Arc::clone(&manager);
        let job_id_event = job_id.clone();
        let transcript_id_event = transcript_id.clone();
        let model_id_for_stats = manager_clone
            .active_job
            .lock()
            .ok()
            .and_then(|j| j.as_ref().map(|aj| aj.model_id.clone()))
            .unwrap_or_default();

        // Spawn a dedicated OS thread — whisper-rs is CPU-bound and synchronous
        std::thread::spawn(move || {
            let result = run_transcription_thread(
                &audio_path_buf,
                &model_path,
                &params,
                backend,
                &job_id_event,
                &transcript_id_event,
                &model_id_for_stats,
                Arc::clone(&abort_flag),
                &app_handle,
                &db,
            );

            // Always clear the active job slot when the thread exits
            if let Ok(mut lock) = manager_clone.active_job.lock() {
                *lock = None;
            }

            if let Err(e) = result {
                // Cancelled is not an error — the event was already emitted inside the thread
                let is_cancelled = matches!(
                    &e,
                    AppError::TranscriptionError {
                        code: TranscriptionErrorCode::Cancelled,
                        ..
                    }
                );
                if !is_cancelled {
                    let _ = app_handle.emit(
                        "transcription:error",
                        TranscriptionErrorEvent {
                            job_id: job_id_event,
                            transcript_id: Some(transcript_id_event),
                            error: e.to_string(),
                        },
                    );
                }
            }
        });

        Ok((job_id, transcript_id))
    }
}

impl Default for TranscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Thread implementation ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn run_transcription_thread(
    audio_path: &std::path::Path,
    model_path: &std::path::Path,
    params: &TranscriptionParams,
    backend: AccelerationBackend,
    job_id: &str,
    transcript_id: &str,
    model_id: &str,
    abort_flag: Arc<AtomicBool>,
    app_handle: &AppHandle,
    db: &Database,
) -> Result<(), AppError> {
    tracing::info!(
        "Starting transcription job={} transcript={} backend={}",
        job_id,
        transcript_id,
        backend
    );

    // Step 1: Decode audio file
    let decoded = decode::decode_file(audio_path)?;
    let duration_ms = decoded.duration_ms;

    // Step 2: Convert to mono 16kHz PCM for whisper
    let pcm = decode::resample_to_whisper(&decoded)?;

    if abort_flag.load(Ordering::SeqCst) {
        emit_cancelled(app_handle, job_id, transcript_id, 0);
        return Err(AppError::TranscriptionError {
            code: TranscriptionErrorCode::Cancelled,
            message: "Cancelled before inference".into(),
        });
    }

    // Step 3: Load engine and run inference with fallback
    let run_inference = |actual_backend: AccelerationBackend| {
        let engine = WhisperEngine::new(model_path, actual_backend)?;
        let app_for_progress = app_handle.clone();
        let job_id_for_progress = job_id.to_string();
        engine.transcribe(
            params,
            &pcm,
            move |progress| {
                let _ = app_for_progress.emit(
                    "transcription:progress",
                    TranscriptionProgressEvent {
                        job_id: job_id_for_progress.clone(),
                        progress,
                    },
                );
            },
            Arc::clone(&abort_flag),
        )
    };

    let output = match run_inference(backend.clone()) {
        Ok(out) => out,
        Err(AppError::TranscriptionError {
            code: TranscriptionErrorCode::Cancelled,
            ..
        }) => {
            emit_cancelled(app_handle, job_id, transcript_id, 0);
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::Cancelled,
                message: "Cancelled during inference".into(),
            });
        }
        Err(AppError::TranscriptionError {
            code: TranscriptionErrorCode::BackendUnavailable,
            message: ref reason,
        }) => {
            // Fallback: retry with CPU
            let reason_str = reason.clone();
            let _ = app_handle.emit(
                "transcription:backend_fallback",
                BackendFallbackEvent {
                    job_id: job_id.to_string(),
                    requested_backend: backend.to_string(),
                    actual_backend: AccelerationBackend::Cpu.to_string(),
                    reason: reason_str,
                },
            );
            tracing::warn!("Backend {} unavailable, falling back to CPU", backend);
            match run_inference(AccelerationBackend::Cpu) {
                Ok(out) => out,
                Err(AppError::TranscriptionError {
                    code: TranscriptionErrorCode::Cancelled,
                    ..
                }) => {
                    emit_cancelled(app_handle, job_id, transcript_id, 0);
                    return Err(AppError::TranscriptionError {
                        code: TranscriptionErrorCode::Cancelled,
                        message: "Cancelled during CPU fallback inference".into(),
                    });
                }
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    };

    let segments: Vec<SegmentResult> = output.segments;
    let backend_used = output.backend_used;
    let wall_time_ms = output.wall_time_ms;

    let segment_count = segments.len();

    // Step 4: Persist segments and update transcript metadata
    {
        let conn = db.get()?;
        segments::insert_batch(&conn, transcript_id, &segments)?;

        let word_count: i64 = segments
            .iter()
            .map(|s| s.text.split_whitespace().count() as i64)
            .sum();

        conn.execute(
            "UPDATE transcripts
             SET duration_ms = ?1, word_count = ?2, updated_at = strftime('%s','now')
             WHERE id = ?3",
            rusqlite::params![duration_ms as i64, word_count, transcript_id],
        )
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Failed to update transcript metadata: {}", e),
        })?;
    }

    // Step 5: Stream segments to frontend as individual events
    // (whisper processes all at once; we emit them sequentially for real-time feel)
    for seg in &segments {
        let _ = app_handle.emit(
            "transcription:segment",
            SegmentEvent {
                job_id: job_id.to_string(),
                transcript_id: transcript_id.to_string(),
                segment: seg.clone(),
            },
        );
    }

    // Step 6: Insert acceleration stats (fire-and-forget — log errors but don't fail)
    let realtime_factor = if wall_time_ms > 0 {
        duration_ms as f64 / wall_time_ms as f64
    } else {
        0.0
    };
    {
        let stat_id = uuid::Uuid::new_v4().to_string();
        let backend_name = backend_used.to_string();
        if let Ok(conn) = db.get() {
            let _ = conn.execute(
                "INSERT INTO acceleration_stats (id, model_id, backend, audio_duration_ms, wall_time_ms, realtime_factor) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![stat_id, model_id, backend_name, duration_ms as i64, wall_time_ms as i64, realtime_factor],
            );
        }
    }

    // Step 7: Emit completion
    let _ = app_handle.emit(
        "transcription:complete",
        TranscriptionCompleteEvent {
            job_id: job_id.to_string(),
            transcript_id: transcript_id.to_string(),
            segment_count,
            duration_ms,
            backend_used: backend_used.to_string(),
            realtime_factor,
            wall_time_ms,
        },
    );

    tracing::info!(
        "Transcription done: job={} segments={} duration_ms={} backend={} realtime_factor={:.2}x",
        job_id,
        segment_count,
        duration_ms,
        backend_used,
        realtime_factor
    );

    Ok(())
}

fn emit_cancelled(
    app_handle: &AppHandle,
    job_id: &str,
    transcript_id: &str,
    segments_saved: usize,
) {
    let _ = app_handle.emit(
        "transcription:cancelled",
        TranscriptionCancelledEvent {
            job_id: job_id.to_string(),
            transcript_id: transcript_id.to_string(),
            segments_saved,
        },
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_not_running_initially() {
        let m = TranscriptionManager::new();
        assert!(!m.is_running());
        assert!(m.active_model_id().is_none());
    }

    #[test]
    fn test_abort_with_no_job_does_not_panic() {
        let m = TranscriptionManager::new();
        m.abort(); // Must not panic
    }

    #[test]
    fn test_abort_sets_flag() {
        let m = TranscriptionManager::new();
        let flag = Arc::new(AtomicBool::new(false));
        *m.active_job.lock().unwrap() = Some(ActiveJob {
            job_id: "j1".into(),
            model_id: "tiny".into(),
            abort_flag: Arc::clone(&flag),
        });
        m.abort();
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_active_model_id() {
        let m = TranscriptionManager::new();
        assert!(m.active_model_id().is_none());
        *m.active_job.lock().unwrap() = Some(ActiveJob {
            job_id: "j1".into(),
            model_id: "base.en".into(),
            abort_flag: Arc::new(AtomicBool::new(false)),
        });
        assert_eq!(m.active_model_id(), Some("base.en".into()));
    }
}
