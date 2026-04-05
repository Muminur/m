use crate::batch::export::{BatchExporter, ExportFormat};
use crate::batch::queue::{BatchJob, BatchJobItem, BatchQueue};
use crate::database::Database;
use crate::error::AppError;
use std::sync::Arc;
use tauri::{command, AppHandle, State};

// ─── Job commands ─────────────────────────────────────────────────────────────

#[command]
pub async fn create_batch_job(
    files: Vec<String>,
    concurrency: u8,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<BatchJob, AppError> {
    batch_queue.create_job(files, concurrency)
}

#[command]
pub async fn start_batch_job(
    job_id: String,
    app_handle: AppHandle,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<(), AppError> {
    let queue = Arc::clone(&batch_queue);
    queue.start_job(&job_id, app_handle)
}

#[command]
pub async fn pause_batch_job(
    job_id: String,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<(), AppError> {
    batch_queue.pause_job(&job_id)
}

#[command]
pub async fn resume_batch_job(
    job_id: String,
    app_handle: AppHandle,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<(), AppError> {
    let queue = Arc::clone(&batch_queue);
    queue.resume_job(&job_id, app_handle)
}

#[command]
pub async fn cancel_batch_job(
    job_id: String,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<(), AppError> {
    batch_queue.cancel_job(&job_id)
}

#[command]
pub async fn get_batch_job(
    job_id: String,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<BatchJob, AppError> {
    batch_queue.get_job(&job_id)
}

#[command]
pub async fn list_batch_jobs(
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<Vec<BatchJob>, AppError> {
    batch_queue.list_jobs()
}

#[command]
pub async fn get_batch_job_items(
    job_id: String,
    batch_queue: State<'_, Arc<BatchQueue>>,
) -> Result<Vec<BatchJobItem>, AppError> {
    batch_queue.get_job_items(&job_id)
}

// ─── Export command ───────────────────────────────────────────────────────────

#[command]
pub async fn export_batch_job(
    job_id: String,
    format: String,
    dest_folder: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<String>, AppError> {
    let fmt: ExportFormat = format.parse()?;
    let exporter = BatchExporter::new(Arc::clone(&db));
    exporter.export_completed(&job_id, fmt, &dest_folder)
}
