use rusqlite::{Connection, params};
use uuid::Uuid;
use chrono::Utc;
use crate::error::{AppError, StorageErrorCode};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptRow {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,
    pub model_id: Option<String>,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub audio_path: Option<String>,
    pub folder_id: Option<String>,
    pub is_starred: bool,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub speaker_count: i64,
    pub word_count: i64,
    pub metadata: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NewTranscript {
    pub title: String,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,
    pub model_id: Option<String>,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub audio_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptUpdate {
    pub title: Option<String>,
    pub language: Option<String>,
    pub is_starred: Option<bool>,
    pub folder_id: Option<String>,
    pub word_count: Option<i64>,
    pub speaker_count: Option<i64>,
}

pub fn insert(conn: &Connection, new: &NewTranscript) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO transcripts (id, title, created_at, updated_at, duration_ms, language, model_id, source_type, source_url, audio_path, is_starred, is_deleted, speaker_count, word_count, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, 0, 0, '{}')",
        params![
            id, new.title, now, now,
            new.duration_ms, new.language, new.model_id,
            new.source_type, new.source_url, new.audio_path,
        ],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to insert transcript: {}", e),
    })?;

    Ok(id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<TranscriptRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, duration_ms, language, model_id,
                source_type, source_url, audio_path, folder_id, is_starred, is_deleted,
                deleted_at, speaker_count, word_count, metadata
         FROM transcripts WHERE id = ?1 AND is_deleted = 0"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare query: {}", e),
    })?;

    let result = stmt.query_row(params![id], |row| {
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
    });

    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to get transcript: {}", e),
        }),
    }
}

pub fn list(
    conn: &Connection,
    is_deleted: bool,
    page: u32,
    page_size: u32,
) -> Result<(Vec<TranscriptRow>, u64), AppError> {
    let offset = page * page_size;
    let deleted_int = is_deleted as i64;

    let total: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transcripts WHERE is_deleted = ?1",
            params![deleted_int],
            |row| row.get(0),
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to count transcripts: {}", e),
        })?;

    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, duration_ms, language, model_id,
                source_type, source_url, audio_path, folder_id, is_starred, is_deleted,
                deleted_at, speaker_count, word_count, metadata
         FROM transcripts
         WHERE is_deleted = ?1
         ORDER BY created_at DESC
         LIMIT ?2 OFFSET ?3"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare list query: {}", e),
    })?;

    let rows = stmt
        .query_map(params![deleted_int, page_size, offset], |row| {
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
            message: format!("Failed to list transcripts: {}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to collect transcripts: {}", e),
        })?;

    Ok((rows, total))
}

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
        conditions.push(format!("t.id IN (SELECT transcript_id FROM transcript_tags WHERE tag_id = ?{})", idx));
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
    let sort_dir = if sort.direction.as_deref() == Some("asc") { "ASC" } else { "DESC" };

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM transcripts t {}", where_clause);
    let refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let total: u64 = conn.query_row(&count_sql, refs.as_slice(), |row| row.get(0))
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Count failed: {}", e),
        })?;

    // Data
    let data_sql = format!(
        "SELECT t.id, t.title, t.created_at, t.updated_at, t.duration_ms, t.language, t.model_id,
                t.source_type, t.source_url, t.audio_path, t.folder_id, t.is_starred, t.is_deleted,
                t.deleted_at, t.speaker_count, t.word_count, t.metadata
         FROM transcripts t {} ORDER BY {} {} LIMIT ?{} OFFSET ?{}",
        where_clause, sort_field, sort_dir, idx, idx + 1
    );
    param_values.push(Box::new(page_size));
    param_values.push(Box::new(page * page_size));
    let refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&data_sql).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Prepare failed: {}", e),
    })?;
    let rows = stmt.query_map(refs.as_slice(), |row| {
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
    }).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Query failed: {}", e),
    })?.collect::<Result<Vec<_>, _>>().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Collect failed: {}", e),
    })?;

    Ok((rows, total))
}

pub fn update(conn: &Connection, id: &str, upd: &TranscriptUpdate) -> Result<(), AppError> {
    let now = Utc::now().timestamp();

    conn.execute(
        "UPDATE transcripts SET
            title = COALESCE(?2, title),
            language = COALESCE(?3, language),
            is_starred = COALESCE(?4, is_starred),
            folder_id = COALESCE(?5, folder_id),
            word_count = COALESCE(?6, word_count),
            speaker_count = COALESCE(?7, speaker_count),
            updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            upd.title, upd.language,
            upd.is_starred.map(|b| b as i64),
            upd.folder_id,
            upd.word_count, upd.speaker_count,
            now,
        ],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to update transcript: {}", e),
    })?;

    Ok(())
}

pub fn soft_delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE transcripts SET is_deleted = 1, deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to soft-delete transcript: {}", e),
    })?;
    Ok(())
}

pub fn hard_delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM transcripts WHERE id = ?1", params![id])
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to delete transcript: {}", e),
        })?;
    Ok(())
}

pub fn restore(conn: &Connection, id: &str) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE transcripts SET is_deleted = 0, deleted_at = NULL, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to restore transcript: {}", e),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use crate::database::migrations;

    fn test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_get_transcript() {
        let conn = test_db();
        let new = NewTranscript {
            title: "Test Transcript".into(),
            duration_ms: Some(60000),
            language: Some("en".into()),
            model_id: Some("tiny.en".into()),
            source_type: Some("file".into()),
            source_url: None,
            audio_path: Some("/tmp/test.mp3".into()),
        };
        let id = insert(&conn, &new).unwrap();
        assert!(!id.is_empty());

        let fetched = get_by_id(&conn, &id).unwrap();
        assert!(fetched.is_some());
        let t = fetched.unwrap();
        assert_eq!(t.title, "Test Transcript");
        assert_eq!(t.duration_ms, Some(60000));
        assert_eq!(t.language.as_deref(), Some("en"));
    }

    #[test]
    fn test_list_transcripts() {
        let conn = test_db();
        for i in 0..3 {
            insert(&conn, &NewTranscript {
                title: format!("Transcript {}", i),
                duration_ms: None,
                language: None,
                model_id: None,
                source_type: None,
                source_url: None,
                audio_path: None,
            }).unwrap();
        }
        let (rows, total) = list(&conn, false, 0, 10).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(total, 3);
    }

    #[test]
    fn test_soft_delete_and_restore() {
        let conn = test_db();
        let id = insert(&conn, &NewTranscript {
            title: "To Delete".into(),
            duration_ms: None, language: None, model_id: None,
            source_type: None, source_url: None, audio_path: None,
        }).unwrap();

        soft_delete(&conn, &id).unwrap();
        assert!(get_by_id(&conn, &id).unwrap().is_none()); // soft-deleted, not returned

        restore(&conn, &id).unwrap();
        assert!(get_by_id(&conn, &id).unwrap().is_some());
    }

    #[test]
    fn test_update_transcript() {
        let conn = test_db();
        let id = insert(&conn, &NewTranscript {
            title: "Original".into(),
            duration_ms: None, language: None, model_id: None,
            source_type: None, source_url: None, audio_path: None,
        }).unwrap();

        update(&conn, &id, &TranscriptUpdate {
            title: Some("Updated".into()),
            language: Some("nl".into()),
            is_starred: Some(true),
            folder_id: None,
            word_count: Some(42),
            speaker_count: None,
        }).unwrap();

        let t = get_by_id(&conn, &id).unwrap().unwrap();
        assert_eq!(t.title, "Updated");
        assert_eq!(t.language.as_deref(), Some("nl"));
        assert!(t.is_starred);
        assert_eq!(t.word_count, 42);
    }

    #[test]
    fn test_list_filtered_starred() {
        let conn = test_db();
        let id1 = insert(&conn, &NewTranscript {
            title: "Starred".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();
        let _id2 = insert(&conn, &NewTranscript {
            title: "Not starred".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();
        update(&conn, &id1, &TranscriptUpdate {
            title: None, language: None, is_starred: Some(true),
            folder_id: None, word_count: None, speaker_count: None,
        }).unwrap();

        let filter = ListFilter { is_starred: Some(true), ..Default::default() };
        let sort = ListSort::default();
        let (rows, total) = list_filtered(&conn, &filter, &sort, 0, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Starred");
    }

    #[test]
    fn test_list_filtered_sort_title_asc() {
        let conn = test_db();
        insert(&conn, &NewTranscript {
            title: "Bravo".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();
        insert(&conn, &NewTranscript {
            title: "Alpha".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();

        let filter = ListFilter::default();
        let sort = ListSort { field: Some("title".into()), direction: Some("asc".into()) };
        let (rows, _) = list_filtered(&conn, &filter, &sort, 0, 50).unwrap();
        assert_eq!(rows[0].title, "Alpha");
        assert_eq!(rows[1].title, "Bravo");
    }
}
