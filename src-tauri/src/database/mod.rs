pub mod migrations;
pub mod segments;
pub mod transcripts;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use crate::error::{AppError, StorageErrorCode};

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

    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to set WAL mode: {}", e),
    })?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to enable foreign keys: {}", e),
    })?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| AppError::StorageError {
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
}
