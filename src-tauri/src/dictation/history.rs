use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use crate::error::{AppError, StorageErrorCode};

/// A single dictation history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub text: String,
    pub app_target: Option<String>,
    pub created_at: i64,
}

/// Maximum number of entries to retain.
const MAX_HISTORY_ENTRIES: usize = 50;

/// Manages dictation history in the database.
pub struct DictationHistory;

impl DictationHistory {
    /// Add a new dictation history entry. Returns the generated ID.
    /// Auto-prunes to keep only the most recent MAX_HISTORY_ENTRIES.
    pub fn add_entry(
        conn: &Connection,
        text: &str,
        app_target: Option<&str>,
    ) -> Result<String, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO dictation_history (id, text, app_target, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, text, app_target, now],
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to insert dictation history: {}", e),
        })?;

        // Auto-prune: keep only the most recent MAX_HISTORY_ENTRIES
        Self::prune(conn)?;

        tracing::debug!(id = %id, "Added dictation history entry");
        Ok(id)
    }

    /// List the most recent dictation history entries.
    pub fn list_recent(conn: &Connection, limit: usize) -> Result<Vec<HistoryEntry>, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, text, app_target, created_at FROM dictation_history ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("Failed to prepare dictation history query: {}", e),
            })?;

        let entries = stmt
            .query_map(rusqlite::params![limit as i64], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    app_target: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("Failed to query dictation history: {}", e),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("Failed to read dictation history row: {}", e),
            })?;

        Ok(entries)
    }

    /// Delete a single history entry by ID.
    pub fn delete_entry(conn: &Connection, id: &str) -> Result<(), AppError> {
        let affected = conn
            .execute(
                "DELETE FROM dictation_history WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("Failed to delete dictation history entry: {}", e),
            })?;

        if affected == 0 {
            tracing::warn!(id = %id, "Dictation history entry not found for deletion");
        }
        Ok(())
    }

    /// Clear all dictation history.
    pub fn clear(conn: &Connection) -> Result<(), AppError> {
        conn.execute("DELETE FROM dictation_history", [])
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("Failed to clear dictation history: {}", e),
            })?;

        tracing::info!("Cleared all dictation history");
        Ok(())
    }

    /// Prune old entries, keeping only the most recent MAX_HISTORY_ENTRIES.
    fn prune(conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "DELETE FROM dictation_history WHERE id NOT IN (
                SELECT id FROM dictation_history ORDER BY created_at DESC LIMIT ?1
            )",
            rusqlite::params![MAX_HISTORY_ENTRIES as i64],
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to prune dictation history: {}", e),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dictation_history (
                id TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                app_target TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX idx_dictation_history_created ON dictation_history(created_at DESC);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_add_entry_returns_id() {
        let conn = setup_db();
        let id = DictationHistory::add_entry(&conn, "hello world", None).unwrap();
        assert!(!id.is_empty());
        // Verify it's a valid UUID
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_add_entry_with_app_target() {
        let conn = setup_db();
        let id = DictationHistory::add_entry(&conn, "test", Some("Visual Studio Code")).unwrap();
        let entries = DictationHistory::list_recent(&conn, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].app_target.as_deref(), Some("Visual Studio Code"));
    }

    #[test]
    fn test_list_recent_empty() {
        let conn = setup_db();
        let entries = DictationHistory::list_recent(&conn, 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_recent_respects_limit() {
        let conn = setup_db();
        for i in 0..5 {
            DictationHistory::add_entry(&conn, &format!("entry {}", i), None).unwrap();
        }
        let entries = DictationHistory::list_recent(&conn, 3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_list_recent_ordered_by_created_at_desc() {
        let conn = setup_db();
        // Insert with explicit timestamps to control ordering
        for i in 0..3 {
            conn.execute(
                "INSERT INTO dictation_history (id, text, app_target, created_at) VALUES (?1, ?2, NULL, ?3)",
                rusqlite::params![format!("id-{}", i), format!("entry {}", i), 1000 + i as i64],
            )
            .unwrap();
        }
        let entries = DictationHistory::list_recent(&conn, 10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text, "entry 2"); // most recent first
        assert_eq!(entries[2].text, "entry 0");
    }

    #[test]
    fn test_delete_entry() {
        let conn = setup_db();
        let id = DictationHistory::add_entry(&conn, "to delete", None).unwrap();
        DictationHistory::delete_entry(&conn, &id).unwrap();
        let entries = DictationHistory::list_recent(&conn, 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_entry_succeeds() {
        let conn = setup_db();
        // Should not error even if entry does not exist
        assert!(DictationHistory::delete_entry(&conn, "nonexistent").is_ok());
    }

    #[test]
    fn test_clear() {
        let conn = setup_db();
        for i in 0..5 {
            DictationHistory::add_entry(&conn, &format!("entry {}", i), None).unwrap();
        }
        DictationHistory::clear(&conn).unwrap();
        let entries = DictationHistory::list_recent(&conn, 100).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_auto_prune_keeps_max_entries() {
        let conn = setup_db();
        // Insert more than MAX_HISTORY_ENTRIES
        for i in 0..60 {
            conn.execute(
                "INSERT INTO dictation_history (id, text, app_target, created_at) VALUES (?1, ?2, NULL, ?3)",
                rusqlite::params![format!("id-{}", i), format!("entry {}", i), 1000 + i as i64],
            )
            .unwrap();
        }
        // Trigger prune via add_entry
        DictationHistory::add_entry(&conn, "final entry", None).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM dictation_history", [], |row| row.get(0))
            .unwrap();
        assert!(count <= MAX_HISTORY_ENTRIES as i64, "Expected <= {} entries, got {}", MAX_HISTORY_ENTRIES, count);
    }

    #[test]
    fn test_entry_serialization() {
        let entry = HistoryEntry {
            id: "test-id".into(),
            text: "hello world".into(),
            app_target: Some("Notepad".into()),
            created_at: 1700000000,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["id"], "test-id");
        assert_eq!(json["text"], "hello world");
        assert_eq!(json["app_target"], "Notepad");
        assert_eq!(json["created_at"], 1700000000);
    }

    #[test]
    fn test_entry_serialization_null_app_target() {
        let entry = HistoryEntry {
            id: "test-id".into(),
            text: "hello".into(),
            app_target: None,
            created_at: 1700000000,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json["app_target"].is_null());
    }
}
