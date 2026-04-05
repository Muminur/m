use rusqlite::{Connection, params};
use uuid::Uuid;
use chrono::Utc;
use crate::error::{AppError, StorageErrorCode};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingRow {
    pub id: String,
    pub source: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub audio_path: String,
    pub duration_ms: i64,
    pub sample_rate: i64,
    pub channels: i64,
    pub transcript_id: Option<String>,
    pub created_at: i64,
}

pub fn insert(
    conn: &Connection,
    source: &str,
    device_id: Option<&str>,
    device_name: Option<&str>,
    audio_path: &str,
    duration_ms: i64,
    sample_rate: i64,
    channels: i64,
) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO recordings (id, source, device_id, device_name, audio_path, duration_ms, sample_rate, channels, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, source, device_id, device_name, audio_path, duration_ms, sample_rate, channels, now],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to insert recording: {}", e),
    })?;

    Ok(id)
}

pub fn link_transcript(conn: &Connection, recording_id: &str, transcript_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE recordings SET transcript_id = ?2 WHERE id = ?1",
        params![recording_id, transcript_id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to link transcript to recording: {}", e),
    })?;
    Ok(())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<RecordingRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, source, device_id, device_name, audio_path, duration_ms, sample_rate, channels, transcript_id, created_at
         FROM recordings WHERE id = ?1"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare query: {}", e),
    })?;

    let result = stmt.query_row(params![id], |row| {
        Ok(RecordingRow {
            id: row.get(0)?,
            source: row.get(1)?,
            device_id: row.get(2)?,
            device_name: row.get(3)?,
            audio_path: row.get(4)?,
            duration_ms: row.get(5)?,
            sample_rate: row.get(6)?,
            channels: row.get(7)?,
            transcript_id: row.get(8)?,
            created_at: row.get(9)?,
        })
    });

    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to get recording: {}", e),
        }),
    }
}

pub fn list_recent(conn: &Connection, limit: u32) -> Result<Vec<RecordingRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, source, device_id, device_name, audio_path, duration_ms, sample_rate, channels, transcript_id, created_at
         FROM recordings ORDER BY created_at DESC LIMIT ?1"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare query: {}", e),
    })?;

    let rows = stmt.query_map(params![limit], |row| {
        Ok(RecordingRow {
            id: row.get(0)?,
            source: row.get(1)?,
            device_id: row.get(2)?,
            device_name: row.get(3)?,
            audio_path: row.get(4)?,
            duration_ms: row.get(5)?,
            sample_rate: row.get(6)?,
            channels: row.get(7)?,
            transcript_id: row.get(8)?,
            created_at: row.get(9)?,
        })
    }).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to list recordings: {}", e),
    })?.collect::<Result<Vec<_>, _>>().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to collect recordings: {}", e),
    })?;

    Ok(rows)
}

pub fn update_duration(conn: &Connection, id: &str, duration_ms: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE recordings SET duration_ms = ?2 WHERE id = ?1",
        params![id, duration_ms],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to update recording duration: {}", e),
    })?;
    Ok(())
}

// Watch folder events

pub fn insert_watch_event(
    conn: &Connection,
    folder_path: &str,
    file_path: &str,
    file_name: &str,
) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO watch_folder_events (id, folder_path, file_path, file_name, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'detected', ?5)",
        params![id, folder_path, file_path, file_name, now],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to insert watch event: {}", e),
    })?;

    Ok(id)
}

pub fn update_watch_event_status(
    conn: &Connection,
    id: &str,
    status: &str,
    transcript_id: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE watch_folder_events SET status = ?2, transcript_id = ?3, error_message = ?4, processed_at = ?5 WHERE id = ?1",
        params![id, status, transcript_id, error_message, now],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to update watch event: {}", e),
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
    fn test_insert_and_get_recording() {
        let conn = test_db();
        let id = insert(&conn, "mic", Some("dev1"), Some("Built-in Mic"), "/tmp/rec.wav", 5000, 16000, 1).unwrap();
        assert!(!id.is_empty());

        let rec = get_by_id(&conn, &id).unwrap().unwrap();
        assert_eq!(rec.source, "mic");
        assert_eq!(rec.device_id.as_deref(), Some("dev1"));
        assert_eq!(rec.duration_ms, 5000);
        assert_eq!(rec.sample_rate, 16000);
        assert!(rec.transcript_id.is_none());
    }

    #[test]
    fn test_link_transcript() {
        let conn = test_db();
        let rec_id = insert(&conn, "mic", None, None, "/tmp/rec.wav", 1000, 16000, 1).unwrap();

        // Create a transcript to link
        let tid = crate::database::transcripts::insert(&conn, &crate::database::transcripts::NewTranscript {
            title: "Recording".into(),
            duration_ms: Some(1000),
            language: None, model_id: None,
            source_type: Some("mic".into()),
            source_url: None, audio_path: Some("/tmp/rec.wav".into()),
        }).unwrap();

        link_transcript(&conn, &rec_id, &tid).unwrap();
        let rec = get_by_id(&conn, &rec_id).unwrap().unwrap();
        assert_eq!(rec.transcript_id.as_deref(), Some(tid.as_str()));
    }

    #[test]
    fn test_list_recent() {
        let conn = test_db();
        for i in 0..5 {
            insert(&conn, "mic", None, None, &format!("/tmp/rec{}.wav", i), 1000, 16000, 1).unwrap();
        }
        let recs = list_recent(&conn, 3).unwrap();
        assert_eq!(recs.len(), 3);
    }

    #[test]
    fn test_update_duration() {
        let conn = test_db();
        let id = insert(&conn, "mic", None, None, "/tmp/rec.wav", 0, 16000, 1).unwrap();
        update_duration(&conn, &id, 30000).unwrap();
        let rec = get_by_id(&conn, &id).unwrap().unwrap();
        assert_eq!(rec.duration_ms, 30000);
    }

    #[test]
    fn test_watch_folder_events() {
        let conn = test_db();
        let id = insert_watch_event(&conn, "/watch", "/watch/test.mp3", "test.mp3").unwrap();
        assert!(!id.is_empty());

        update_watch_event_status(&conn, &id, "transcribed", Some("tid-123"), None).unwrap();
    }
}
