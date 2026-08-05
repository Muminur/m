use crate::error::{AppError, StorageErrorCode};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub transcript_id: String,
    pub title: String,
    pub excerpt: String,
    pub match_count: i64,
}

pub fn search(conn: &Connection, query: &str, limit: u32) -> Result<Vec<SearchResult>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT s.transcript_id, t.title, snippet(transcripts_fts, 0, '<mark>', '</mark>', '...', 32) as excerpt
         FROM transcripts_fts f
         JOIN segments s ON s.rowid = f.rowid
         JOIN transcripts t ON t.id = s.transcript_id
         WHERE transcripts_fts MATCH ?1 AND t.is_deleted = 0 AND s.is_deleted = 0
         ORDER BY rank"
    ).map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to prepare search: {}", e),
    })?;

    let rows = stmt
        .query_map(params![query], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
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

    // FTS5 auxiliary functions such as snippet() cannot be evaluated in a
    // GROUP BY query. Aggregate matching segments in Rust while preserving
    // the FTS rank order and first (best-ranked) excerpt per transcript.
    let mut results: Vec<SearchResult> = Vec::new();
    for (transcript_id, title, excerpt) in rows {
        if let Some(existing) = results
            .iter_mut()
            .find(|result| result.transcript_id == transcript_id)
        {
            existing.match_count += 1;
        } else if results.len() < limit as usize {
            results.push(SearchResult {
                transcript_id,
                title,
                excerpt,
                match_count: 1,
            });
        }
    }

    Ok(results)
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
