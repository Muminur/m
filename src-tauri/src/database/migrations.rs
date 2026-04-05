use rusqlite::Connection;
use crate::error::{AppError, StorageErrorCode};

pub fn run(conn: &mut Connection) -> Result<(), AppError> {
    let migrations = [
        ("V001", include_str!("../../migrations/V001__initial_schema.sql")),
        ("V002", include_str!("../../migrations/V002__fts5_search.sql")),
        ("V003", include_str!("../../migrations/V003__ai_templates.sql")),
        ("V004", include_str!("../../migrations/V004__integrations.sql")),
        ("V005", include_str!("../../migrations/V005__export_presets.sql")),
        ("V006", include_str!("../../migrations/V006__whisper_jobs.sql")),
        ("V007", include_str!("../../migrations/V007__acceleration_stats.sql")),
        ("V008", include_str!("../../migrations/V008__smart_folders.sql")),
        ("V009", include_str!("../../migrations/V009__segments_fts_population.sql")),
        ("V010", include_str!("../../migrations/V010__recordings.sql")),
        ("V011", include_str!("../../migrations/V011__recordings_system_audio_path.sql")),
        ("V012", include_str!("../../migrations/V012__dictation_history.sql")),
    ];

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        )",
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::MigrationFailed,
        message: format!("Failed to create migrations table: {}", e),
    })?;

    for (version, sql) in &migrations {
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM _migrations WHERE version = ?1",
                rusqlite::params![version],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !already_applied {
            tracing::info!("Applying migration {}", version);

            // Run migration and record it atomically in a single transaction
            let tx = conn.transaction().map_err(|e| AppError::StorageError {
                code: StorageErrorCode::MigrationFailed,
                message: format!("Failed to begin transaction for {}: {}", version, e),
            })?;

            tx.execute_batch(sql).map_err(|e| AppError::StorageError {
                code: StorageErrorCode::MigrationFailed,
                message: format!("Migration {} failed: {}", version, e),
            })?;

            tx.execute(
                "INSERT INTO _migrations (version) VALUES (?1)",
                rusqlite::params![version],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::MigrationFailed,
                message: format!("Failed to record migration {}: {}", version, e),
            })?;

            tx.commit().map_err(|e| AppError::StorageError {
                code: StorageErrorCode::MigrationFailed,
                message: format!("Failed to commit migration {}: {}", version, e),
            })?;

            tracing::info!("Migration {} applied successfully", version);
        }
    }

    Ok(())
}
