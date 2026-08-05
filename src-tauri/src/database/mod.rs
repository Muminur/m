pub mod migrations;
pub mod recordings;
pub mod search;
pub mod segments;
pub mod smart_folders;
pub mod transcript_queries;
pub mod transcripts;
pub mod translations;
pub mod undo;

use crate::error::{AppError, StorageErrorCode};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn get(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.conn.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "Failed to acquire database lock".into(),
        })
    }
}

pub fn init(app: &AppHandle) -> Result<Database, AppError> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: "Failed to get app data directory".into(),
        })?;

    std::fs::create_dir_all(&app_dir).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::IoError,
        message: format!("Failed to create app data directory: {}", e),
    })?;

    let db_path = app_dir.join("whisperdesk.db");
    tracing::info!("Opening database at: {:?}", db_path);

    let mut conn = Connection::open(&db_path).map_err(|e| {
        tracing::error!("Failed to open database: {}", e);
        AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to open database: {}", e),
        }
    })?;

    let journal_mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to set WAL mode: {}", e),
        })?;
    tracing::debug!("SQLite journal_mode set to: {}", journal_mode);
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to enable foreign keys: {}", e),
        })?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to set synchronous mode: {}", e),
        })?;

    migrations::run(&mut conn)?;

    tracing::info!("Database initialized successfully");
    Ok(Database::new(conn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_migrations_run_successfully() {
        let conn = in_memory_db();
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert!(tables.contains(&"transcripts".to_string()));
        assert!(tables.contains(&"segments".to_string()));
        assert!(tables.contains(&"speakers".to_string()));
        assert!(tables.contains(&"whisper_models".to_string()));
        assert!(tables.contains(&"folders".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"transcript_tags".to_string()));
        assert!(tables.contains(&"ai_templates".to_string()));
        assert!(tables.contains(&"integrations".to_string()));
        assert!(tables.contains(&"export_presets".to_string()));
        assert!(tables.contains(&"dictation_history".to_string()));
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = in_memory_db();
        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_fts5_table_exists() {
        let conn = in_memory_db();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='transcripts_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_whisper_model_sha256_migration() {
        let conn = in_memory_db();
        let expected = [
            (
                "tiny",
                "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
            ),
            (
                "tiny.en",
                "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
            ),
            (
                "base",
                "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
            ),
            (
                "base.en",
                "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
            ),
            (
                "small",
                "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
            ),
            (
                "small.en",
                "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
            ),
            (
                "medium",
                "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
            ),
            (
                "large-v2",
                "9a423fe4d40c82774b6af34115b8b935f34152246eb19e80e376071d3f999487",
            ),
            (
                "large-v3",
                "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
            ),
        ];

        for (model_id, expected_sha256) in expected {
            let actual: String = conn
                .query_row(
                    "SELECT sha256 FROM whisper_models WHERE id = ?1",
                    [model_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(actual, expected_sha256, "unexpected SHA-256 for {model_id}");
        }

        let applied: bool = conn
            .query_row(
                "SELECT COUNT(*) = 1 FROM _migrations WHERE version = 'V019'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(applied);
    }
}
