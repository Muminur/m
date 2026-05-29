//! Dynamic SQL query builder for filtered/sorted transcript listing.

use crate::error::{AppError, StorageErrorCode};
use rusqlite::Connection;

use super::transcripts::TranscriptRow;

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub is_starred: Option<bool>,
    pub is_deleted: Option<bool>,
    pub folder_id: Option<String>,
    pub source_type: Option<String>,
    pub language: Option<String>,
    pub tag_id: Option<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ListSort {
    pub field: Option<String>,
    pub direction: Option<String>,
}

pub fn list_filtered(
    conn: &Connection,
    filter: &ListFilter,
    sort: &ListSort,
    page: u32,
    page_size: u32,
) -> Result<(Vec<TranscriptRow>, u64), AppError> {
    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1usize;

    // is_deleted default false
    conditions.push(format!("t.is_deleted = ?{}", idx));
    param_values.push(Box::new(filter.is_deleted.unwrap_or(false) as i64));
    idx += 1;

    if let Some(starred) = filter.is_starred {
        conditions.push(format!("t.is_starred = ?{}", idx));
        param_values.push(Box::new(starred as i64));
        idx += 1;
    }
    if let Some(ref fid) = filter.folder_id {
        conditions.push(format!("t.folder_id = ?{}", idx));
        param_values.push(Box::new(fid.clone()));
        idx += 1;
    }
    if let Some(ref st) = filter.source_type {
        conditions.push(format!("t.source_type = ?{}", idx));
        param_values.push(Box::new(st.clone()));
        idx += 1;
    }
    if let Some(ref lang) = filter.language {
        conditions.push(format!("t.language = ?{}", idx));
        param_values.push(Box::new(lang.clone()));
        idx += 1;
    }
    if let Some(ref tid) = filter.tag_id {
        conditions.push(format!(
            "t.id IN (SELECT transcript_id FROM transcript_tags WHERE tag_id = ?{})",
            idx
        ));
        param_values.push(Box::new(tid.clone()));
        idx += 1;
    }
    if let Some(from) = filter.date_from {
        conditions.push(format!("t.created_at >= ?{}", idx));
        param_values.push(Box::new(from));
        idx += 1;
    }
    if let Some(to) = filter.date_to {
        conditions.push(format!("t.created_at <= ?{}", idx));
        param_values.push(Box::new(to));
        idx += 1;
    }

    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let sort_field = match sort.field.as_deref() {
        Some("title") => "t.title",
        Some("duration_ms") => "t.duration_ms",
        Some("language") => "t.language",
        _ => "t.created_at",
    };
    let sort_dir = if sort.direction.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM transcripts t {}", where_clause);
    let refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let total: u64 = conn
        .query_row(&count_sql, refs.as_slice(), |row| row.get(0))
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Count failed: {}", e),
        })?;

    // Data
    let data_sql =
        format!(
        "SELECT t.id, t.title, t.created_at, t.updated_at, t.duration_ms, t.language, t.model_id,
                t.source_type, t.source_url, t.audio_path, t.folder_id, t.is_starred, t.is_deleted,
                t.deleted_at, t.speaker_count, t.word_count, t.metadata
         FROM transcripts t {} ORDER BY {} {} LIMIT ?{} OFFSET ?{}",
        where_clause, sort_field, sort_dir, idx, idx + 1
    );
    param_values.push(Box::new(page_size));
    param_values.push(Box::new(page * page_size));
    let refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&data_sql)
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Prepare failed: {}", e),
        })?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok(TranscriptRow {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                duration_ms: row.get(4)?,
                language: row.get(5)?,
                model_id: row.get(6)?,
                source_type: row.get(7)?,
                source_url: row.get(8)?,
                audio_path: row.get(9)?,
                folder_id: row.get(10)?,
                is_starred: row.get::<_, i64>(11)? != 0,
                is_deleted: row.get::<_, i64>(12)? != 0,
                deleted_at: row.get(13)?,
                speaker_count: row.get(14)?,
                word_count: row.get(15)?,
                metadata: row.get(16)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Query failed: {}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Collect failed: {}", e),
        })?;

    Ok((rows, total))
}
