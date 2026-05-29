use crate::database::Database;
use crate::error::{AppError, BatchErrorCode};
use crate::models::manager::ModelManager;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::{db, worker};

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

fn lock_controls(
    controls: &Mutex<std::collections::HashMap<String, JobControl>>,
) -> Result<std::sync::MutexGuard<'_, std::collections::HashMap<String, JobControl>>, AppError> {
    controls.lock().map_err(|_| AppError::BatchError {
        code: BatchErrorCode::InvalidState,
        message: "batch controls mutex poisoned".into(),
    })
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
        let concurrency = concurrency.clamp(1, 4);
        let job_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.db.get()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO batch_jobs (id, status, concurrency, model_id, language, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![job_id, BatchJobStatus::Pending.to_string(), concurrency as i64, &model_id, &language, now, now],
        )?;

        for (i, file_path) in files.iter().enumerate() {
            let item_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO batch_job_items (id, job_id, file_path, status, progress, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![item_id, job_id, file_path, BatchItemStatus::Queued.to_string(), 0.0_f64, i as i64],
            )?;
        }
        tx.commit()?;

        lock_controls(&self.controls)?.insert(
            job_id.clone(),
            JobControl {
                paused: false,
                cancelled: false,
            },
        );

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

    /// Transition a Pending/Paused job to Running and spawn async workers.
    pub fn start_job(
        self: &Arc<Self>,
        job_id: &str,
        app_handle: AppHandle,
    ) -> Result<(), AppError> {
        let job = self.get_job(job_id)?;

        match job.status {
            BatchJobStatus::Pending | BatchJobStatus::Paused => {}
            s => {
                return Err(AppError::BatchError {
                    code: BatchErrorCode::InvalidState,
                    message: format!("Cannot start a job in {:?} state", s),
                });
            }
        }

        db::set_job_status(&self.db, job_id, BatchJobStatus::Running)?;

        // Ensure control slot is present and not cancelled
        {
            let mut ctrl = lock_controls(&self.controls)?;
            let entry = ctrl.entry(job_id.to_string()).or_insert(JobControl {
                paused: false,
                cancelled: false,
            });
            entry.paused = false;
            if entry.cancelled {
                return Err(AppError::BatchError {
                    code: BatchErrorCode::InvalidState,
                    message: "Job has been cancelled".into(),
                });
            }
        }

        let queue = Arc::clone(self);
        let job_id_owned = job_id.to_string();

        tauri::async_runtime::spawn(async move {
            if let Err(e) = queue
                .run_job(
                    &job_id_owned,
                    job.concurrency,
                    job.model_id,
                    job.language,
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

            if self.is_cancelled(job_id) {
                break;
            }

            // Spin-wait on pause — check every 200 ms
            loop {
                if self.is_cancelled(job_id) || !self.is_paused(job_id) {
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
            let item_id = item.id.clone();
            let job_id_owned = job_id.to_string();
            let app_clone = app_handle.clone();
            let completed_clone = Arc::clone(&completed_count);
            let model_id_ref = model_id.clone();
            let language_ref = language.clone();

            let handle = tauri::async_runtime::spawn(async move {
                let _permit = permit;

                let _ = db::set_item_status(
                    &queue.db,
                    &item_id,
                    BatchItemStatus::Processing,
                    None,
                    0.0,
                );

                let done = completed_clone.load(std::sync::atomic::Ordering::Relaxed);
                let _ = app_clone.emit(
                    "batch:progress",
                    BatchProgressEvent {
                        job_id: job_id_owned.clone(),
                        item_id: item_id.clone(),
                        progress: 0.0,
                        completed: done,
                        total,
                    },
                );

                let (final_status, transcript_id, err_msg) = worker::process_item(
                    &item,
                    &queue.model_manager,
                    &queue.db,
                    model_id_ref.as_deref(),
                    language_ref.as_deref(),
                )
                .await;

                let _ = db::set_item_status(
                    &queue.db,
                    &item_id,
                    final_status.clone(),
                    err_msg.clone(),
                    1.0,
                );

                if let Some(ref tid) = transcript_id {
                    let _ = db::set_item_transcript_id(&queue.db, &item_id, tid);
                }

                let done = completed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                let _ = app_clone.emit(
                    "batch:item-complete",
                    BatchItemCompleteEvent {
                        job_id: job_id_owned.clone(),
                        item_id: item_id.clone(),
                        transcript_id,
                        status: final_status,
                        error: err_msg,
                    },
                );

                let _ = app_clone.emit(
                    "batch:progress",
                    BatchProgressEvent {
                        job_id: job_id_owned,
                        item_id,
                        progress: 1.0,
                        completed: done,
                        total,
                    },
                );
            });

            handles.push(handle);
        }

        for h in handles {
            let _ = h.await;
        }

        // Determine final job status
        let final_status = if self.is_cancelled(job_id) {
            BatchJobStatus::Cancelled
        } else {
            let items = self.get_job_items(job_id)?;
            let any_failed = items
                .iter()
                .any(|i| i.status == BatchItemStatus::Failed);
            let any_succeeded = items
                .iter()
                .any(|i| i.status == BatchItemStatus::Completed || i.status == BatchItemStatus::Skipped);

            if !any_failed {
                BatchJobStatus::Completed
            } else if !any_succeeded {
                BatchJobStatus::Failed
            } else {
                BatchJobStatus::Completed // partial success counts as Completed
            }
        };

        db::set_job_status(&self.db, job_id, final_status.clone())?;

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
        {
            let mut ctrl = lock_controls(&self.controls)?;
            if let Some(c) = ctrl.get_mut(job_id) {
                c.paused = true;
            }
        }
        db::set_job_status(&self.db, job_id, BatchJobStatus::Paused)
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
        {
            let mut ctrl = lock_controls(&self.controls)?;
            if let Some(c) = ctrl.get_mut(job_id) {
                c.paused = false;
            }
        }
        db::set_job_status(&self.db, job_id, BatchJobStatus::Running)
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

        {
            let mut ctrl = lock_controls(&self.controls)?;
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

    // ─── Delegated queries ────────────────────────────────────────────────────

    pub fn get_job(&self, job_id: &str) -> Result<BatchJob, AppError> {
        db::get_job(&self.db, job_id)
    }

    pub fn list_jobs(&self) -> Result<Vec<BatchJob>, AppError> {
        db::list_jobs(&self.db)
    }

    pub fn get_job_items(&self, job_id: &str) -> Result<Vec<BatchJobItem>, AppError> {
        db::get_job_items(&self.db, job_id)
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

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
    use crate::models::manager::ModelManager;
    use rusqlite::Connection;

    fn make_queue() -> Arc<BatchQueue> {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        let db = Arc::new(Database::new(conn));
        let model_manager = Arc::new(ModelManager::new(std::path::PathBuf::from("/tmp/models-test")));
        Arc::new(BatchQueue::new(db, model_manager))
    }

    #[test]
    fn test_create_job_with_files() {
        let q = make_queue();
        let files = vec!["a.mp3".to_string(), "b.wav".to_string()];
        let job = q.create_job(files, 2, None, None).unwrap();
        assert_eq!(job.status, BatchJobStatus::Pending);
        assert_eq!(job.concurrency, 2);
    }

    #[test]
    fn test_create_job_clamps_concurrency() {
        let q = make_queue();
        let job = q.create_job(vec![], 10, None, None).unwrap();
        assert_eq!(job.concurrency, 4);

        let job2 = q.create_job(vec![], 0, None, None).unwrap();
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
        let job = q.create_job(files, 1, None, None).unwrap();
        let items = q.get_job_items(&job.id).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.status == BatchItemStatus::Queued));
    }

    #[test]
    fn test_get_job_roundtrip() {
        let q = make_queue();
        let job = q.create_job(vec!["f.mp3".to_string()], 1, None, None).unwrap();
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
        q.create_job(vec![], 1, None, None).unwrap();
        q.create_job(vec![], 2, None, None).unwrap();
        let jobs = q.list_jobs().unwrap();
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn test_cancel_job_marks_items_skipped() {
        let q = make_queue();
        let files = vec!["a.mp3".to_string(), "b.mp3".to_string()];
        let job = q.create_job(files, 1, None, None).unwrap();
        q.cancel_job(&job.id).unwrap();

        let fetched = q.get_job(&job.id).unwrap();
        assert_eq!(fetched.status, BatchJobStatus::Cancelled);

        let items = q.get_job_items(&job.id).unwrap();
        assert!(items.iter().all(|i| i.status == BatchItemStatus::Skipped));
    }

    #[test]
    fn test_pause_requires_running_state() {
        let q = make_queue();
        let job = q.create_job(vec![], 1, None, None).unwrap();
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
        let job = q.create_job(vec!["f.mp4".to_string()], 1, None, None).unwrap();
        assert_eq!(job.status, BatchJobStatus::Pending);
        q.cancel_job(&job.id).unwrap();
        let fetched = q.get_job(&job.id).unwrap();
        assert_eq!(fetched.status, BatchJobStatus::Cancelled);
    }

    #[test]
    fn test_concurrency_limit_enforced() {
        let q = make_queue();
        let job = q.create_job(vec![], 0, None, None).unwrap();
        assert_eq!(job.concurrency, 1);
        let job2 = q.create_job(vec![], 4, None, None).unwrap();
        assert_eq!(job2.concurrency, 4);
        let job3 = q.create_job(vec![], 5, None, None).unwrap();
        assert_eq!(job3.concurrency, 4);
    }
}
