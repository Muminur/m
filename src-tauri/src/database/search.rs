use crate::error::{AppError, StorageErrorCode};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub transcript_id: String,
    pub title: String,
    pub excerpt: String,
    pub match_count: i64,
}

pub fn search(conn: &Connection, query: &str, limit: u32) -> Result<Vec<SearchResult>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT s.transcript_id, t.title, snippet(transcripts_fts, 0, '<mark>', '</mark>', '...', 32) as excerpt, COUNT(*) as match_count
         FROM transcripts_fts f
         JOIN segments s ON s.rowid = f.rowid
         JOIN transcripts t ON t.id = s.transcript_id
         WHERE transcripts_fts MATCH ?1 AND t.is_deleted = 0 AND s.is_deleted = 0
         GROUP BY s.transcript_id
         ORDER BY rank
         LIMIT ?2"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare search: {}", e),
    })?;

    let rows = stmt
        .query_map(params![query, limit], |row| {
            Ok(SearchResult {
                transcript_id: row.get(0)?,
                title: row.get(1)?,
                excerpt: row.get(2)?,
                match_count: row.get(3)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Search failed: {}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to collect search results: {}", e),
        })?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{migrations, segments, transcripts};
    use crate::transcription::engine::SegmentResult;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_search_returns_matching_transcripts() {
        let conn = test_db();
        let tid = transcripts::insert(
            &conn,
            &transcripts::NewTranscript {
                title: "Meeting Notes".into(),
                duration_ms: None,
                language: None,
                model_id: None,
                source_type: None,
                source_url: None,
                audio_path: None,
            },
        )
        .unwrap();
        let segs = vec![
            SegmentResult {
                index: 0,
                start_ms: 0,
                end_ms: 1000,
                text: "hello world testing search".into(),
                confidence: 0.9,
            },
            SegmentResult {
                index: 1,
                start_ms: 1000,
                end_ms: 2000,
                text: "another segment here".into(),
                confidence: 0.9,
            },
        ];
        segments::insert_batch(&conn, &tid, &segs).unwrap();

        let results = search(&conn, "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].transcript_id, tid);
    }

    #[test]
    fn test_search_no_results() {
        let conn = test_db();
        let results = search(&conn, "nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }
}
