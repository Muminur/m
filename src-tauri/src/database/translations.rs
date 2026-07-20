use crate::error::{AppError, StorageErrorCode};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRow {
    pub id: String,
    pub transcript_id: String,
    pub segment_id: String,
    pub target_lang: String,
    pub source_lang: Option<String>,
    pub text: String,
    pub engine: String,
    pub created_at: String,
}

pub fn insert_batch(
    conn: &Connection,
    transcript_id: &str,
    target_lang: &str,
    source_lang: Option<&str>,
    rows: &[(String, String)],
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    for (segment_id, text) in rows {
        conn.execute(
            "INSERT OR REPLACE INTO translations
                (id, transcript_id, segment_id, target_lang, source_lang, text, engine, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'nllb-600m', ?7)",
            params![
                uuid::Uuid::new_v4().to_string(),
                transcript_id,
                segment_id,
                target_lang,
                source_lang,
                text,
                now,
            ],
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("insert translation: {e}"),
        })?;
    }
    Ok(())
}

pub fn get_by_transcript_lang(
    conn: &Connection,
    transcript_id: &str,
    target_lang: &str,
) -> Result<Vec<TranslationRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, transcript_id, segment_id, target_lang, source_lang, text, engine, created_at
             FROM translations WHERE transcript_id = ?1 AND target_lang = ?2
             ORDER BY rowid ASC",
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("prepare get translations: {e}"),
        })?;
    let rows = stmt
        .query_map(params![transcript_id, target_lang], |r| {
            Ok(TranslationRow {
                id: r.get(0)?,
                transcript_id: r.get(1)?,
                segment_id: r.get(2)?,
                target_lang: r.get(3)?,
                source_lang: r.get(4)?,
                text: r.get(5)?,
                engine: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("query translations: {e}"),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("collect translations: {e}"),
        })?;
    Ok(rows)
}

pub fn delete_by_transcript(conn: &Connection, transcript_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM translations WHERE transcript_id = ?1",
        params![transcript_id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("delete translations: {e}"),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE transcripts (id TEXT PRIMARY KEY);
             CREATE TABLE segments (id TEXT PRIMARY KEY, transcript_id TEXT);
             CREATE TABLE translations (
                id TEXT PRIMARY KEY, transcript_id TEXT NOT NULL, segment_id TEXT NOT NULL,
                target_lang TEXT NOT NULL, source_lang TEXT, text TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT 'nllb-600m', created_at TEXT NOT NULL,
                UNIQUE(segment_id, target_lang));
             INSERT INTO transcripts (id) VALUES ('t1');
             INSERT INTO segments (id, transcript_id) VALUES ('s1','t1'),('s2','t1');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn insert_and_fetch_roundtrip() {
        let conn = setup();
        insert_batch(&conn, "t1", "ben_Beng", Some("eng_Latn"),
            &[("s1".into(), "একটি".into()), ("s2".into(), "দুই".into())]).unwrap();
        let rows = get_by_transcript_lang(&conn, "t1", "ben_Beng").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text, "একটি");
    }

    #[test]
    fn insert_or_replace_caches() {
        let conn = setup();
        insert_batch(&conn, "t1", "ben_Beng", None, &[("s1".into(), "old".into())]).unwrap();
        insert_batch(&conn, "t1", "ben_Beng", None, &[("s1".into(), "new".into())]).unwrap();
        let rows = get_by_transcript_lang(&conn, "t1", "ben_Beng").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "new");
    }

    #[test]
    fn delete_removes_all() {
        let conn = setup();
        insert_batch(&conn, "t1", "ben_Beng", None, &[("s1".into(), "x".into())]).unwrap();
        delete_by_transcript(&conn, "t1").unwrap();
        assert!(get_by_transcript_lang(&conn, "t1", "ben_Beng").unwrap().is_empty());
    }
}
