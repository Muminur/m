use crate::error::{AppError, StorageErrorCode};
use crate::transcription::engine::SegmentResult;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SegmentRow {
    pub id: String,
    pub transcript_id: String,
    pub index_num: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub speaker_id: Option<String>,
    pub confidence: Option<f64>,
    pub is_deleted: bool,
}

pub fn insert_batch(
    conn: &Connection,
    transcript_id: &str,
    segments: &[SegmentResult],
) -> Result<Vec<String>, AppError> {
    let mut ids = Vec::with_capacity(segments.len());

    for seg in segments {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO segments (id, transcript_id, index_num, start_ms, end_ms, text, confidence, is_deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![
                id, transcript_id,
                seg.index as i64, seg.start_ms, seg.end_ms,
                seg.text, seg.confidence as f64,
            ],
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to insert segment {}: {}", seg.index, e),
        })?;
        ids.push(id);
    }

    Ok(ids)
}

pub fn get_by_transcript(
    conn: &Connection,
    transcript_id: &str,
) -> Result<Vec<SegmentRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, transcript_id, index_num, start_ms, end_ms, text, speaker_id, confidence, is_deleted
             FROM segments
             WHERE transcript_id = ?1 AND is_deleted = 0
             ORDER BY index_num ASC",
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to prepare segments query: {}", e),
        })?;

    let rows = stmt
        .query_map(params![transcript_id], |row| {
            Ok(SegmentRow {
                id: row.get(0)?,
                transcript_id: row.get(1)?,
                index_num: row.get(2)?,
                start_ms: row.get(3)?,
                end_ms: row.get(4)?,
                text: row.get(5)?,
                speaker_id: row.get(6)?,
                confidence: row.get(7)?,
                is_deleted: row.get::<_, i64>(8)? != 0,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to query segments: {}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to collect segments: {}", e),
        })?;

    Ok(rows)
}

pub fn get_by_id(conn: &Connection, segment_id: &str) -> Result<SegmentRow, AppError> {
    conn.query_row(
        "SELECT id, transcript_id, index_num, start_ms, end_ms, text, speaker_id, confidence, is_deleted
         FROM segments WHERE id = ?1",
        params![segment_id],
        |row| Ok(SegmentRow {
            id: row.get(0)?,
            transcript_id: row.get(1)?,
            index_num: row.get(2)?,
            start_ms: row.get(3)?,
            end_ms: row.get(4)?,
            text: row.get(5)?,
            speaker_id: row.get(6)?,
            confidence: row.get(7)?,
            is_deleted: row.get::<_, i64>(8)? != 0,
        }),
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Segment '{}' not found: {}", segment_id, e),
    })
}

pub fn update_text(conn: &Connection, segment_id: &str, text: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE segments SET text = ?2 WHERE id = ?1 AND is_deleted = 0",
        params![segment_id, text],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to update segment text: {}", e),
    })?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, segment_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE segments SET is_deleted = 1 WHERE id = ?1",
        params![segment_id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to soft-delete segment: {}", e),
    })?;
    Ok(())
}

pub fn count_words(conn: &Connection, transcript_id: &str) -> Result<i64, AppError> {
    let count: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(text) - LENGTH(REPLACE(text, ' ', '')) + 1), 0)
             FROM segments WHERE transcript_id = ?1 AND is_deleted = 0",
            params![transcript_id],
            |row| row.get(0),
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to count words: {}", e),
        })?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;
    use crate::database::transcripts;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    fn make_segments(n: usize) -> Vec<SegmentResult> {
        (0..n)
            .map(|i| SegmentResult {
                index: i,
                start_ms: (i as i64) * 1000,
                end_ms: (i as i64) * 1000 + 900,
                text: format!("Segment {}", i),
                confidence: 0.9,
            })
            .collect()
    }

    #[test]
    fn test_insert_and_get_segments() {
        let conn = test_db();
        let tid = transcripts::insert(
            &conn,
            &transcripts::NewTranscript {
                title: "T".into(),
                duration_ms: None,
                language: None,
                model_id: None,
                source_type: None,
                source_url: None,
                audio_path: None,
            },
        )
        .unwrap();

        let segs = make_segments(3);
        insert_batch(&conn, &tid, &segs).unwrap();

        let rows = get_by_transcript(&conn, &tid).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text, "Segment 0");
        assert_eq!(rows[2].end_ms, 2900);
        assert!((rows[0].confidence.unwrap() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_update_segment_text() {
        let conn = test_db();
        let tid = transcripts::insert(
            &conn,
            &transcripts::NewTranscript {
                title: "T".into(),
                duration_ms: None,
                language: None,
                model_id: None,
                source_type: None,
                source_url: None,
                audio_path: None,
            },
        )
        .unwrap();
        let segs = make_segments(1);
        let ids = insert_batch(&conn, &tid, &segs).unwrap();

        update_text(&conn, &ids[0], "Updated text").unwrap();
        let rows = get_by_transcript(&conn, &tid).unwrap();
        assert_eq!(rows[0].text, "Updated text");
    }

    #[test]
    fn test_word_count() {
        let conn = test_db();
        let tid = transcripts::insert(
            &conn,
            &transcripts::NewTranscript {
                title: "T".into(),
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
                text: "hello world".into(),
                confidence: 0.9,
            },
            SegmentResult {
                index: 1,
                start_ms: 1000,
                end_ms: 2000,
                text: "foo bar baz".into(),
                confidence: 0.9,
            },
        ];
        insert_batch(&conn, &tid, &segs).unwrap();
        let count = count_words(&conn, &tid).unwrap();
        // "hello world": 1 space -> 2 words; "foo bar baz": 2 spaces -> 3 words; total = 5
        assert!(count >= 5);
    }
}
