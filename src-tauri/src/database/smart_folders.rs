use rusqlite::{Connection, params};
use uuid::Uuid;
use chrono::Utc;
use crate::error::{AppError, StorageErrorCode};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    pub filter_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn insert(conn: &Connection, name: &str, filter_json: &str) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO smart_folders (id, name, filter_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, filter_json, now, now],
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to insert smart folder: {}", e),
    })?;
    Ok(id)
}

pub fn list(conn: &Connection) -> Result<Vec<SmartFolder>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, filter_json, created_at, updated_at FROM smart_folders ORDER BY name"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to list smart folders: {}", e),
    })?;
    let rows = stmt.query_map([], |row| {
        Ok(SmartFolder {
            id: row.get(0)?,
            name: row.get(1)?,
            filter_json: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to query smart folders: {}", e),
    })?.collect::<Result<Vec<_>, _>>().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to collect smart folders: {}", e),
    })?;
    Ok(rows)
}

pub fn update(conn: &Connection, id: &str, name: &str, filter_json: &str) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE smart_folders SET name = ?2, filter_json = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, name, filter_json, now],
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to update smart folder: {}", e),
    })?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM smart_folders WHERE id = ?1", params![id])
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to delete smart folder: {}", e),
        })?;
    Ok(())
}

pub fn query_transcripts(conn: &Connection, id: &str) -> Result<(Vec<crate::database::transcripts::TranscriptRow>, u64), AppError> {
    let folder = conn.query_row(
        "SELECT filter_json FROM smart_folders WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Smart folder not found: {}", e),
    })?;

    let filter: crate::database::transcripts::ListFilter = serde_json::from_str(&folder)
        .unwrap_or_default();
    let sort = crate::database::transcripts::ListSort::default();
    crate::database::transcripts::list_filtered(conn, &filter, &sort, 0, 100)
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
    fn test_crud_smart_folder() {
        let conn = test_db();
        let id = insert(&conn, "Important", r#"{"is_starred": true}"#).unwrap();
        let folders = list(&conn).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Important");

        update(&conn, &id, "Very Important", r#"{"is_starred": true}"#).unwrap();
        let folders = list(&conn).unwrap();
        assert_eq!(folders[0].name, "Very Important");

        delete(&conn, &id).unwrap();
        let folders = list(&conn).unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn test_query_smart_folder_transcripts() {
        let conn = test_db();
        // Insert a transcript and star it
        let tid = crate::database::transcripts::insert(&conn, &crate::database::transcripts::NewTranscript {
            title: "Starred Meeting".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();
        crate::database::transcripts::update(&conn, &tid, &crate::database::transcripts::TranscriptUpdate {
            title: None, language: None, is_starred: Some(true),
            folder_id: None, word_count: None, speaker_count: None,
        }).unwrap();
        // Insert unstarred
        crate::database::transcripts::insert(&conn, &crate::database::transcripts::NewTranscript {
            title: "Other".into(), duration_ms: None, language: None,
            model_id: None, source_type: None, source_url: None, audio_path: None,
        }).unwrap();

        // Create smart folder filtering starred
        let sf_id = insert(&conn, "Starred", r#"{"isStarred": true}"#).unwrap();
        let (rows, total) = query_transcripts(&conn, &sf_id).unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].title, "Starred Meeting");
    }
}
