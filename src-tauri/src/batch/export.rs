use std::path::Path;
use std::sync::Arc;
use crate::database::{Database, transcripts, segments};
use crate::error::{AppError, BatchErrorCode, ExportErrorCode};
use crate::export;

/// Supported export formats for batch export.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Txt,
    Srt,
    Vtt,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Txt => "txt",
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
        }
    }
}

impl std::str::FromStr for ExportFormat {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "txt" => Ok(ExportFormat::Txt),
            "srt" => Ok(ExportFormat::Srt),
            "vtt" => Ok(ExportFormat::Vtt),
            _ => Err(AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Unsupported export format: {}", s),
            }),
        }
    }
}

pub struct BatchExporter {
    db: Arc<Database>,
}

impl BatchExporter {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Export all completed items in a batch job to `dest_folder` in the given format.
    ///
    /// Returns the list of absolute file paths that were written.
    pub fn export_completed(
        &self,
        job_id: &str,
        format: ExportFormat,
        dest_folder: &str,
    ) -> Result<Vec<String>, AppError> {
        let dest = Path::new(dest_folder);
        if !dest.exists() {
            std::fs::create_dir_all(dest)?;
        }

        let conn = self.db.get()?;

        // Fetch completed items that have a transcript_id
        let mut stmt = conn.prepare(
            "SELECT id, file_path, transcript_id FROM batch_job_items \
             WHERE job_id = ?1 AND status = 'Completed' AND transcript_id IS NOT NULL \
             ORDER BY sort_order",
        )?;

        let rows: Vec<(String, String, String)> = stmt
            .query_map(rusqlite::params![job_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<_, _>>()?;

        if rows.is_empty() {
            return Err(AppError::BatchError {
                code: BatchErrorCode::ExportFailed,
                message: format!("No completed items with transcripts found for job '{}'", job_id),
            });
        }

        let mut exported_paths = Vec::new();

        for (_item_id, file_path, transcript_id) in rows {
            let transcript = transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| {
                AppError::BatchError {
                    code: BatchErrorCode::ExportFailed,
                    message: format!("Transcript '{}' not found during batch export", transcript_id),
                }
            })?;

            let segs = segments::get_by_transcript(&conn, &transcript_id)?;

            let content = match format {
                ExportFormat::Txt => export::text::render(&transcript, &segs, true, true),
                ExportFormat::Srt => export::srt::render(&segs),
                ExportFormat::Vtt => export::vtt::render(&segs),
            };

            // Derive output file name from the source file stem
            let stem = Path::new(&file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("transcript");

            let out_name = format!("{}.{}", stem, format.extension());
            let out_path = dest.join(&out_name);

            std::fs::write(&out_path, &content).map_err(|e| AppError::ExportError {
                code: ExportErrorCode::IoError,
                message: format!("Failed to write export file '{}': {}", out_path.display(), e),
            })?;

            exported_paths.push(out_path.to_string_lossy().into_owned());
        }

        Ok(exported_paths)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use crate::database::migrations;

    fn make_db() -> Arc<Database> {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        Arc::new(Database::new(conn))
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Txt.extension(), "txt");
        assert_eq!(ExportFormat::Srt.extension(), "srt");
        assert_eq!(ExportFormat::Vtt.extension(), "vtt");
    }

    #[test]
    fn test_export_format_parse() {
        assert_eq!("txt".parse::<ExportFormat>().unwrap(), ExportFormat::Txt);
        assert_eq!("SRT".parse::<ExportFormat>().unwrap(), ExportFormat::Srt);
        assert_eq!("vtt".parse::<ExportFormat>().unwrap(), ExportFormat::Vtt);
        assert!("mp3".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn test_export_no_completed_items_returns_error() {
        let db = make_db();
        let exporter = BatchExporter::new(Arc::clone(&db));

        // Insert a batch job with no completed items
        {
            let conn = db.get().unwrap();
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO batch_jobs (id, status, concurrency, created_at, updated_at) VALUES ('job1', 'Completed', 1, ?1, ?1)",
                rusqlite::params![now],
            ).unwrap();
        }

        let dir = std::env::temp_dir().join("batch_export_test_empty");
        let result = exporter.export_completed("job1", ExportFormat::Txt, dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::BatchError { code: BatchErrorCode::ExportFailed, .. }));
    }
}
