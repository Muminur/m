use crate::audio::decode;
use crate::database::{segments, transcripts, Database};
use crate::error::{AppError, BatchErrorCode};
use crate::models::manager::ModelManager;
use crate::settings::AccelerationBackend;
use crate::transcription::engine::{TranscriptionParams, WhisperEngine};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use uuid::Uuid;

// ─── Status enums ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchJobStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for BatchJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BatchJobStatus::Pending => "Pending",
            BatchJobStatus::Running => "Running",
            BatchJobStatus::Paused => "Paused",
            BatchJobStatus::Completed => "Completed",
            BatchJobStatus::Failed => "Failed",
            BatchJobStatus::Cancelled => "Cancelled",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for BatchJobStatus {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(BatchJobStatus::Pending),
            "Running" => Ok(BatchJobStatus::Running),
            "Paused" => Ok(BatchJobStatus::Paused),
            "Completed" => Ok(BatchJobStatus::Completed),
            "Failed" => Ok(BatchJobStatus::Failed),
            "Cancelled" => Ok(BatchJobStatus::Cancelled),
            _ => Err(AppError::BatchError {
                code: BatchErrorCode::InvalidState,
                message: format!("Unknown job status: {}", s),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchItemStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for BatchItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BatchItemStatus::Queued => "Queued",
            BatchItemStatus::Processing => "Processing",
            BatchItemStatus::Completed => "Completed",
            BatchItemStatus::Failed => "Failed",
            BatchItemStatus::Skipped => "Skipped",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for BatchItemStatus {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Queued" => Ok(BatchItemStatus::Queued),
            "Processing" => Ok(BatchItemStatus::Processing),
            "Completed" => Ok(BatchItemStatus::Completed),
            "Failed" => Ok(BatchItemStatus::Failed),
            "Skipped" => Ok(BatchItemStatus::Skipped),
            _ => Err(AppError::BatchError {
                code: BatchErrorCode::InvalidState,
                message: format!("Unknown item status: {}", s),
            }),
        }
    }
}

// ─── Data structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJob {
    pub id: String,
    pub status: BatchJobStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub concurrency: u8,
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJobItem {
    pub id: String,
    pub job_id: String,
    pub file_path: String,
    pub transcript_id: Option<String>,
    pub status: BatchItemStatus,
    pub error: Option<String>,
    pub progress: f32,
    pub processing_ms: Option<i64>,
}

// ─── Event payloads ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProgressEvent {
    job_id: String,
    item_id: String,
    progress: f32,
    completed: usize,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchItemCompleteEvent {
    job_id: String,
    item_id: String,
    transcript_id: Option<String>,
    status: BatchItemStatus,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchJobCompleteEvent {
    job_id: String,
    status: BatchJobStatus,
    completed: usize,
    failed: usize,
    total: usize,
}

// ─── Queue ────────────────────────────────────────────────────────────────────

/// Tracks in-memory pause/cancel signals per job.
struct JobControl {
    paused: bool,
    cancelled: bool,
}

pub struct BatchQueue {
    db: Arc<Database>,
    model_manager: Arc<ModelManager>,
    /// Per-job control signals (job_id -> control)
    controls: Mutex<std::collections::HashMap<String, JobControl>>,
}

impl BatchQueue {
    pub fn new(db: Arc<Database>, model_manager: Arc<ModelManager>) -> Self {
        Self {
            db,
            model_manager,
            controls: Mutex::new(std::collections::HashMap::new()),
        }
    }

    // ─── Create ───────────────────────────────────────────────────────────────

    /// Create a new batch job and queue all provided file paths as items.
    pub fn create_job(
        &self,
        files: Vec<String>,
        concurrency: u8,
        model_id: Option<String>,
        language: Option<String>,
    ) -> Result<BatchJob, AppError> {
        // Clamp concurrency to 1-4
        let concurrency = concurrency.clamp(1, 4);

        let job_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.db.get()?;

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO batch_jobs (id, status, concurrency, model_id, language, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![job_id, BatchJobStatus::Pending.to_string(), concurrency as i64, model_id, language, now, now],
        )?;

        for (i, file_path) in files.iter().enumerate() {
            let item_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO batch_job_items (id, job_id, file_path, status, progress, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![item_id, job_id, file_path, BatchItemStatus::Queued.to_string(), 0.0_f64, i as i64],
            )?;
        }
        tx.commit()?;

        // Register control slot
        if let Ok(mut ctrl) = self.controls.lock() {
            ctrl.insert(
                job_id.clone(),
                JobControl {
                    paused: false,
                    cancelled: false,
                },
            );
        }

        Ok(BatchJob {
            id: job_id,
            status: BatchJobStatus::Pending,
            created_at: now,
            updated_at: now,
            concurrency,
            model_id,
            language,
            started_at: None,
            completed_at: None,
        })
    }

    // ─── Start ────────────────────────────────────────────────────────────────

    /// Transition a Pending job to Running and spawn async workers up to concurrency limit.
    pub fn start_job(
        self: &Arc<Self>,
        job_id: &str,
        app_handle: AppHandle,
    ) -> Result<(), AppError> {
        let job = self.get_job(job_id)?;

        match job.status {
            BatchJobStatus::Pending => {}
            BatchJobStatus::Paused => {}
            s => {
                return Err(AppError::BatchError {
                    code: BatchErrorCode::InvalidState,
                    message: format!("Cannot start a job in {:?} state", s),
                });
            }
        }

        self.set_job_status(job_id, BatchJobStatus::Running)?;

        // Ensure control slot is present and not cancelled
        if let Ok(mut ctrl) = self.controls.lock() {
            let entry = ctrl.entry(job_id.to_string()).or_insert(JobControl {
                paused: false,
                cancelled: false,
            });
            entry.paused = false;
            // preserve cancelled flag — if already cancelled don't re-run
            if entry.cancelled {
                return Err(AppError::BatchError {
                    code: BatchErrorCode::InvalidState,
                    message: "Job has been cancelled".into(),
                });
            }
        }

        let queue = Arc::clone(self);
        let job_id_owned = job_id.to_string();
        let model_id = job.model_id.clone();
        let language = job.language.clone();

        // Spawn the orchestrator on the Tauri async runtime
        tauri::async_runtime::spawn(async move {
            if let Err(e) = queue
                .run_job(
                    &job_id_owned,
                    job.concurrency,
                    model_id,
                    language,
                    app_handle,
                )
                .await
            {
                tracing::error!("Batch job {} failed: {}", job_id_owned, e);
            }
        });

        Ok(())
    }

    /// Internal async orchestrator — processes items respecting concurrency and pause/cancel.
    async fn run_job(
        self: &Arc<Self>,
        job_id: &str,
        concurrency: u8,
        model_id: Option<String>,
        language: Option<String>,
        app_handle: AppHandle,
    ) -> Result<(), AppError> {
        let semaphore = Arc::new(Semaphore::new(concurrency as usize));

        let items = self.get_job_items(job_id)?;
        let total = items.len();
        let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut handles = Vec::new();

        for item in items {
            if item.status == BatchItemStatus::Completed || item.status == BatchItemStatus::Skipped
            {
                completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                continue;
            }

            // Check cancelled before acquiring permit
            if self.is_cancelled(job_id) {
                break;
            }

            // Spin-wait on pause — check every 200 ms
            loop {
                if self.is_cancelled(job_id) {
                    break;
                }
                if !self.is_paused(job_id) {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            if self.is_cancelled(job_id) {
                break;
            }

            let permit =
                Arc::clone(&semaphore)
                    .acquire_owned()
                    .await
                    .map_err(|_| AppError::BatchError {
                        code: BatchErrorCode::ConcurrencyLimit,
                        message: "Semaphore closed".into(),
                    })?;

            let queue = Arc::clone(self);
            let item_clone = item.clone();
            let job_id_clone = job_id.to_string();
            let app_clone = app_handle.clone();
            let completed_clone = Arc::clone(&completed_count);
            let model_id_clone = model_id.clone();
            let language_clone = language.clone();

            let handle = tauri::async_runtime::spawn(async move {
                let _permit = permit; // Released when this task completes

                // Mark as Processing
                let _ =
                    queue.set_item_status(&item_clone.id, BatchItemStatus::Processing, None, 0.0);

                // Emit progress start
                let done = completed_clone.load(std::sync::atomic::Ordering::Relaxed);
                let _ = app_clone.emit(
                    "batch:progress",
                    BatchProgressEvent {
                        job_id: job_id_clone.clone(),
                        item_id: item_clone.id.clone(),
                        progress: 0.0,
                        completed: done,
                        total,
                    },
                );

                // Run real transcription pipeline for this item
                let (final_status, transcript_id, err_msg) = queue
                    .process_item(
                        &item_clone,
                        &job_id_clone,
                        model_id_clone.as_deref(),
                        language_clone.as_deref(),
                        &app_clone,
                    )
                    .await;

                let _ = queue.set_item_status(
                    &item_clone.id,
                    final_status.clone(),
                    err_msg.clone(),
                    1.0,
                );

                let done = completed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                let _ = app_clone.emit(
                    "batch:item-complete",
                    BatchItemCompleteEvent {
                        job_id: job_id_clone.clone(),
                        item_id: item_clone.id.clone(),
                        transcript_id: transcript_id.clone(),
                        status: final_status,
                        error: err_msg,
                    },
                );

                let _ = app_clone.emit(
                    "batch:progress",
                    BatchProgressEvent {
                        job_id: job_id_clone.clone(),
                        item_id: item_clone.id.clone(),
                        progress: 1.0,
                        completed: done,
                        total,
                    },
                );
            });

            handles.push(handle);
        }

        // Wait for all spawned tasks
        for h in handles {
            let _ = h.await;
        }

        // Determine final job status
        let final_status = if self.is_cancelled(job_id) {
            BatchJobStatus::Cancelled
        } else {
            let items = self.get_job_items(job_id)?;
            let failed = items
                .iter()
                .filter(|i| i.status == BatchItemStatus::Failed)
                .count();
            let done = items
                .iter()
                .filter(|i| i.status == BatchItemStatus::Completed)
                .count();
            let skipped = items
                .iter()
                .filter(|i| i.status == BatchItemStatus::Skipped)
                .count();

            if failed == 0 {
                BatchJobStatus::Completed
            } else if done + skipped == 0 {
                BatchJobStatus::Failed
            } else {
                BatchJobStatus::Completed // partial success counts as Completed
            }
        };

        self.set_job_status(job_id, final_status.clone())?;

        let items = self.get_job_items(job_id)?;
        let failed = items
            .iter()
            .filter(|i| i.status == BatchItemStatus::Failed)
            .count();
        let done = items
            .iter()
            .filter(|i| i.status == BatchItemStatus::Completed)
            .count();

        let _ = app_handle.emit(
            "batch:job-complete",
            BatchJobCompleteEvent {
                job_id: job_id.to_string(),
                status: final_status,
                completed: done,
                failed,
                total,
            },
        );

        Ok(())
    }

    /// Process a single batch item by running the full transcription pipeline.
    /// Returns (final_status, transcript_id, error_msg).
    async fn process_item(
        &self,
        item: &BatchJobItem,
        _job_id: &str,
        model_id: Option<&str>,
        language: Option<&str>,
        _app_handle: &AppHandle,
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
        if !self.model_manager.is_downloaded(&model_id) {
            return (
                BatchItemStatus::Failed,
                None,
                Some(format!("Model '{}' is not downloaded", model_id)),
            );
        }

        let model_path = self.model_manager.model_path(&model_id);
        let db = Arc::clone(&self.db);
        let language_owned = language.map(|s| s.to_string());
        let item_id = item.id.clone();
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
                    // Batch items don't emit per-item whisper progress (job-level progress is handled by the orchestrator)
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
                        source_type: Some("batch".to_string()),
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
            Ok(Ok(transcript_id)) => {
                // Update the batch item's transcript_id in the database
                let _ = self.set_item_transcript_id(&item_id, &transcript_id);
                (BatchItemStatus::Completed, Some(transcript_id), None)
            }
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

    // ─── Pause ────────────────────────────────────────────────────────────────

    pub fn pause_job(&self, job_id: &str) -> Result<(), AppError> {
        let job = self.get_job(job_id)?;
        if job.status != BatchJobStatus::Running {
            return Err(AppError::BatchError {
                code: BatchErrorCode::InvalidState,
                message: format!(
                    "Job must be Running to pause; current state: {:?}",
                    job.status
                ),
            });
        }
        if let Ok(mut ctrl) = self.controls.lock() {
            if let Some(c) = ctrl.get_mut(job_id) {
                c.paused = true;
            }
        }
        self.set_job_status(job_id, BatchJobStatus::Paused)
    }

    // ─── Resume ───────────────────────────────────────────────────────────────

    /// Resume a paused job by clearing the pause flag.
    ///
    /// The existing `run_job` orchestrator is spin-waiting on the pause flag
    /// and will resume processing automatically — we must NOT call `start_job`
    /// here, as that would spawn a second orchestrator causing duplicate work.
    pub fn resume_job(&self, job_id: &str) -> Result<(), AppError> {
        let job = self.get_job(job_id)?;
        if job.status != BatchJobStatus::Paused {
            return Err(AppError::BatchError {
                code: BatchErrorCode::InvalidState,
                message: format!(
                    "Job must be Paused to resume; current state: {:?}",
                    job.status
                ),
            });
        }
        if let Ok(mut ctrl) = self.controls.lock() {
            if let Some(c) = ctrl.get_mut(job_id) {
                c.paused = false;
            }
        }
        self.set_job_status(job_id, BatchJobStatus::Running)
    }

    // ─── Cancel ───────────────────────────────────────────────────────────────

    pub fn cancel_job(&self, job_id: &str) -> Result<(), AppError> {
        let job = self.get_job(job_id)?;
        match job.status {
            BatchJobStatus::Completed | BatchJobStatus::Failed | BatchJobStatus::Cancelled => {
                return Err(AppError::BatchError {
                    code: BatchErrorCode::InvalidState,
                    message: format!("Cannot cancel a job in {:?} state", job.status),
                });
            }
            _ => {}
        }

        if let Ok(mut ctrl) = self.controls.lock() {
            let entry = ctrl.entry(job_id.to_string()).or_insert(JobControl {
                paused: false,
                cancelled: false,
            });
            entry.cancelled = true;
            entry.paused = false; // unblock any paused spin-wait
        }

        // Mark all Queued/Processing items as Skipped
        let conn = self.db.get()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE batch_job_items SET status = 'Skipped' WHERE job_id = ?1 AND status IN ('Queued', 'Processing')",
            rusqlite::params![job_id],
        )?;
        conn.execute(
            "UPDATE batch_jobs SET status = 'Cancelled', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, job_id],
        )?;
        Ok(())
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn get_job(&self, job_id: &str) -> Result<BatchJob, AppError> {
        let conn = self.db.get()?;
        conn.query_row(
            "SELECT id, status, concurrency, created_at, updated_at, model_id, language, started_at, completed_at FROM batch_jobs WHERE id = ?1",
            rusqlite::params![job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::BatchError {
                code: BatchErrorCode::JobNotFound,
                message: format!("Batch job '{}' not found", job_id),
            },
            other => AppError::from(other),
        })
        .and_then(|(id, status_str, concurrency, created_at, updated_at, model_id, language, started_at, completed_at)| {
            let status: BatchJobStatus = status_str.parse()?;
            Ok(BatchJob {
                id,
                status,
                created_at,
                updated_at,
                concurrency: concurrency as u8,
                model_id,
                language,
                started_at,
                completed_at,
            })
        })
    }

    pub fn list_jobs(&self) -> Result<Vec<BatchJob>, AppError> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, status, concurrency, created_at, updated_at, model_id, language, started_at, completed_at FROM batch_jobs ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
            ))
        })?;

        let mut jobs = Vec::new();
        for row in rows {
            let (
                id,
                status_str,
                concurrency,
                created_at,
                updated_at,
                model_id,
                language,
                started_at,
                completed_at,
            ) = row?;
            let status: BatchJobStatus = status_str.parse()?;
            jobs.push(BatchJob {
                id,
                status,
                created_at,
                updated_at,
                concurrency: concurrency as u8,
                model_id,
                language,
                started_at,
                completed_at,
            });
        }
        Ok(jobs)
    }

    pub fn get_job_items(&self, job_id: &str) -> Result<Vec<BatchJobItem>, AppError> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, job_id, file_path, transcript_id, status, error, progress, processing_ms FROM batch_job_items WHERE job_id = ?1 ORDER BY sort_order",
        )?;
        let rows = stmt.query_map(rusqlite::params![job_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?;

        let mut items = Vec::new();
        for row in rows {
            let (
                id,
                job_id_col,
                file_path,
                transcript_id,
                status_str,
                error,
                progress,
                processing_ms,
            ) = row?;
            let status: BatchItemStatus = status_str.parse()?;
            items.push(BatchJobItem {
                id,
                job_id: job_id_col,
                file_path,
                transcript_id,
                status,
                error,
                progress: progress as f32,
                processing_ms,
            });
        }
        Ok(items)
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

    fn set_job_status(&self, job_id: &str, status: BatchJobStatus) -> Result<(), AppError> {
        let conn = self.db.get()?;
        let now = chrono::Utc::now().timestamp();
        match status {
            BatchJobStatus::Running => {
                conn.execute(
                    "UPDATE batch_jobs SET status = ?1, updated_at = ?2, started_at = ?2 WHERE id = ?3",
                    rusqlite::params![status.to_string(), now, job_id],
                )?;
            }
            BatchJobStatus::Completed | BatchJobStatus::Failed | BatchJobStatus::Cancelled => {
                conn.execute(
                    "UPDATE batch_jobs SET status = ?1, updated_at = ?2, completed_at = ?2 WHERE id = ?3",
                    rusqlite::params![status.to_string(), now, job_id],
                )?;
            }
            _ => {
                conn.execute(
                    "UPDATE batch_jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![status.to_string(), now, job_id],
                )?;
            }
        }
        Ok(())
    }

    fn set_item_transcript_id(&self, item_id: &str, transcript_id: &str) -> Result<(), AppError> {
        let conn = self.db.get()?;
        conn.execute(
            "UPDATE batch_job_items SET transcript_id = ?1 WHERE id = ?2",
            rusqlite::params![transcript_id, item_id],
        )?;
        Ok(())
    }

    fn set_item_status(
        &self,
        item_id: &str,
        status: BatchItemStatus,
        error: Option<String>,
        progress: f32,
    ) -> Result<(), AppError> {
        let conn = self.db.get()?;
        conn.execute(
            "UPDATE batch_job_items SET status = ?1, error = ?2, progress = ?3 WHERE id = ?4",
            rusqlite::params![status.to_string(), error, progress as f64, item_id],
        )?;
        Ok(())
    }

    fn is_paused(&self, job_id: &str) -> bool {
        self.controls
            .lock()
            .map(|ctrl| ctrl.get(job_id).map(|c| c.paused).unwrap_or(false))
            .unwrap_or(false)
    }

    fn is_cancelled(&self, job_id: &str) -> bool {
        self.controls
            .lock()
            .map(|ctrl| ctrl.get(job_id).map(|c| c.cancelled).unwrap_or(false))
            .unwrap_or(false)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;
    use rusqlite::Connection;

    fn make_queue() -> Arc<BatchQueue> {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        let db = Arc::new(Database::new(conn));
        Arc::new(BatchQueue::new(db))
    }

    #[test]
    fn test_create_job_with_files() {
        let q = make_queue();
        let files = vec!["a.mp3".to_string(), "b.wav".to_string()];
        let job = q.create_job(files, 2).unwrap();
        assert_eq!(job.status, BatchJobStatus::Pending);
        assert_eq!(job.concurrency, 2);
    }

    #[test]
    fn test_create_job_clamps_concurrency() {
        let q = make_queue();
        let job = q.create_job(vec![], 10).unwrap();
        assert_eq!(job.concurrency, 4);

        let job2 = q.create_job(vec![], 0).unwrap();
        assert_eq!(job2.concurrency, 1);
    }

    #[test]
    fn test_items_added_to_db() {
        let q = make_queue();
        let files = vec![
            "x.mp3".to_string(),
            "y.mp3".to_string(),
            "z.mp3".to_string(),
        ];
        let job = q.create_job(files, 1).unwrap();
        let items = q.get_job_items(&job.id).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.status == BatchItemStatus::Queued));
    }

    #[test]
    fn test_get_job_roundtrip() {
        let q = make_queue();
        let job = q.create_job(vec!["f.mp3".to_string()], 1).unwrap();
        let fetched = q.get_job(&job.id).unwrap();
        assert_eq!(fetched.id, job.id);
        assert_eq!(fetched.status, BatchJobStatus::Pending);
    }

    #[test]
    fn test_get_job_not_found() {
        let q = make_queue();
        let err = q.get_job("nonexistent-id").unwrap_err();
        assert!(matches!(
            err,
            AppError::BatchError {
                code: BatchErrorCode::JobNotFound,
                ..
            }
        ));
    }

    #[test]
    fn test_list_jobs() {
        let q = make_queue();
        q.create_job(vec![], 1).unwrap();
        q.create_job(vec![], 2).unwrap();
        let jobs = q.list_jobs().unwrap();
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn test_cancel_job_marks_items_skipped() {
        let q = make_queue();
        let files = vec!["a.mp3".to_string(), "b.mp3".to_string()];
        let job = q.create_job(files, 1).unwrap();
        q.cancel_job(&job.id).unwrap();

        let fetched = q.get_job(&job.id).unwrap();
        assert_eq!(fetched.status, BatchJobStatus::Cancelled);

        let items = q.get_job_items(&job.id).unwrap();
        assert!(items.iter().all(|i| i.status == BatchItemStatus::Skipped));
    }

    #[test]
    fn test_pause_requires_running_state() {
        let q = make_queue();
        let job = q.create_job(vec![], 1).unwrap();
        // Job is Pending — pause must fail
        let err = q.pause_job(&job.id).unwrap_err();
        assert!(matches!(
            err,
            AppError::BatchError {
                code: BatchErrorCode::InvalidState,
                ..
            }
        ));
    }

    #[test]
    fn test_status_transitions_pending_to_cancelled() {
        let q = make_queue();
        let job = q.create_job(vec!["f.mp4".to_string()], 1).unwrap();
        assert_eq!(job.status, BatchJobStatus::Pending);
        q.cancel_job(&job.id).unwrap();
        let fetched = q.get_job(&job.id).unwrap();
        assert_eq!(fetched.status, BatchJobStatus::Cancelled);
    }

    #[test]
    fn test_concurrency_limit_enforced() {
        let q = make_queue();
        // Concurrency 0 → clamps to 1
        let job = q.create_job(vec![], 0).unwrap();
        assert_eq!(job.concurrency, 1);
        // Concurrency 4 → max allowed
        let job2 = q.create_job(vec![], 4).unwrap();
        assert_eq!(job2.concurrency, 4);
        // Concurrency 5 → clamps to 4
        let job3 = q.create_job(vec![], 5).unwrap();
        assert_eq!(job3.concurrency, 4);
    }
}
