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
}
