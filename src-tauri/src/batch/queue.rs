use crate::database::Database;
use crate::error::{AppError, BatchErrorCode};
use serde::{Deserialize, Serialize};
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
    /// Per-job control signals (job_id -> control)
    controls: Mutex<std::collections::HashMap<String, JobControl>>,
}

impl BatchQueue {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            controls: Mutex::new(std::collections::HashMap::new()),
        }
    }

    // ─── Create ───────────────────────────────────────────────────────────────

    /// Create a new batch job and queue all provided file paths as items.
    pub fn create_job(&self, files: Vec<String>, concurrency: u8) -> Result<BatchJob, AppError> {
        // Clamp concurrency to 1-4
        let concurrency = concurrency.clamp(1, 4);

        let job_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.db.get()?;

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO batch_jobs (id, status, concurrency, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![job_id, BatchJobStatus::Pending.to_string(), concurrency as i64, now, now],
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

        // Spawn the orchestrator on the Tauri async runtime
        tauri::async_runtime::spawn(async move {
            if let Err(e) = queue
                .run_job(&job_id_owned, job.concurrency, app_handle)
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

                // Simulate processing — real integration would call TranscriptionManager here.
                // We mark Processing→Completed immediately; callers wire the real pipeline.
                // This stub ensures the state machine and events are correct.
                let (final_status, transcript_id, err_msg) = queue
                    .process_item(&item_clone, &job_id_clone, &app_clone)
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

    /// Process a single item. Returns (final_status, transcript_id, error_msg).
    /// This is the extension point for real transcription integration.
    async fn process_item(
        &self,
        item: &BatchJobItem,
        _job_id: &str,
        _app_handle: &AppHandle,
    ) -> (BatchItemStatus, Option<String>, Option<String>) {
        // Validate the file exists before claiming success
        if !std::path::Path::new(&item.file_path).exists() {
            return (
                BatchItemStatus::Failed,
                None,
                Some(format!("File not found: {}", item.file_path)),
            );
        }
        // Real implementation would invoke TranscriptionManager here.
        // Return Completed with no transcript_id — callers supply the wired version.
        (BatchItemStatus::Completed, None, None)
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
            "SELECT id, status, concurrency, created_at, updated_at FROM batch_jobs WHERE id = ?1",
            rusqlite::params![job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
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
        .and_then(|(id, status_str, concurrency, created_at, updated_at)| {
            let status: BatchJobStatus = status_str.parse()?;
            Ok(BatchJob {
                id,
                status,
                created_at,
                updated_at,
                concurrency: concurrency as u8,
            })
        })
    }

    pub fn list_jobs(&self) -> Result<Vec<BatchJob>, AppError> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, status, concurrency, created_at, updated_at FROM batch_jobs ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;

        let mut jobs = Vec::new();
        for row in rows {
            let (id, status_str, concurrency, created_at, updated_at) = row?;
            let status: BatchJobStatus = status_str.parse()?;
            jobs.push(BatchJob {
                id,
                status,
                created_at,
                updated_at,
                concurrency: concurrency as u8,
            });
        }
        Ok(jobs)
    }

    pub fn get_job_items(&self, job_id: &str) -> Result<Vec<BatchJobItem>, AppError> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, job_id, file_path, transcript_id, status, error, progress FROM batch_job_items WHERE job_id = ?1 ORDER BY sort_order",
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
            ))
        })?;

        let mut items = Vec::new();
        for row in rows {
            let (id, job_id_col, file_path, transcript_id, status_str, error, progress) = row?;
            let status: BatchItemStatus = status_str.parse()?;
            items.push(BatchJobItem {
                id,
                job_id: job_id_col,
                file_path,
                transcript_id,
                status,
                error,
                progress: progress as f32,
            });
        }
        Ok(items)
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

    fn set_job_status(&self, job_id: &str, status: BatchJobStatus) -> Result<(), AppError> {
        let conn = self.db.get()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE batch_jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![status.to_string(), now, job_id],
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
