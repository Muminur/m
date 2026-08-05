use crate::database::Database;
use crate::error::{AppError, BatchErrorCode};

use super::queue::{BatchItemStatus, BatchJob, BatchJobItem, BatchJobStatus};

/// Fetch a single batch job by ID.
pub fn get_job(db: &Database, job_id: &str) -> Result<BatchJob, AppError> {
    let conn = db.get()?;
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

/// List all batch jobs ordered by creation time (newest first).
pub fn list_jobs(db: &Database) -> Result<Vec<BatchJob>, AppError> {
    let conn = db.get()?;
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

/// Fetch all items for a batch job, ordered by sort_order.
pub fn get_job_items(db: &Database, job_id: &str) -> Result<Vec<BatchJobItem>, AppError> {
    let conn = db.get()?;
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
        let (id, job_id_col, file_path, transcript_id, status_str, error, progress, processing_ms) =
            row?;
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

/// Update a job's status (and related timestamps).
pub fn set_job_status(db: &Database, job_id: &str, status: BatchJobStatus) -> Result<(), AppError> {
    let conn = db.get()?;
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

/// Update a batch item's status, error, and progress.
pub fn set_item_status(
    db: &Database,
    item_id: &str,
    status: BatchItemStatus,
    error: Option<String>,
    progress: f32,
) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute(
        "UPDATE batch_job_items SET status = ?1, error = ?2, progress = ?3 WHERE id = ?4",
        rusqlite::params![status.to_string(), error, progress as f64, item_id],
    )?;
    Ok(())
}

/// Link a transcript to a batch item.
pub fn set_item_transcript_id(
    db: &Database,
    item_id: &str,
    transcript_id: &str,
) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute(
        "UPDATE batch_job_items SET transcript_id = ?1 WHERE id = ?2",
        rusqlite::params![transcript_id, item_id],
    )?;
    Ok(())
}
