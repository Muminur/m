use crate::database::undo::{UndoManager, UndoOperation};
use crate::database::{search, segments, smart_folders, transcripts, Database};
use crate::error::{AppError, StorageErrorCode};
use chrono::Utc;
use rusqlite::params;
use std::sync::Arc;
use tauri::{command, State};
use uuid::Uuid;

// ─── Folder commands ─────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
}

#[command]
pub async fn list_folders(db: State<'_, Arc<Database>>) -> Result<Vec<FolderInfo>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, parent_id, color, sort_order FROM folders ORDER BY sort_order, name",
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to list folders: {}", e),
        })?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FolderInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    Ok(rows)
}

#[command]
pub async fn create_folder(
    name: String,
    parent_id: Option<String>,
    color: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    let conn = db.get()?;
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO folders (id, name, parent_id, color) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, parent_id, color],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to create folder: {}", e),
    })?;
    Ok(id)
}

#[command]
pub async fn rename_folder(
    id: String,
    name: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute(
        "UPDATE folders SET name = ?2 WHERE id = ?1",
        params![id, name],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to rename folder: {}", e),
    })?;
    Ok(())
}

#[command]
pub async fn delete_folder(id: String, db: State<'_, Arc<Database>>) -> Result<(), AppError> {
    let mut conn = db.get()?;
    let tx = conn.transaction().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    tx.execute(
        "UPDATE transcripts SET folder_id = NULL WHERE folder_id = ?1",
        params![id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    tx.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("Failed to delete folder: {}", e),
        })?;
    tx.commit().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to commit delete_folder: {}", e),
    })?;
    Ok(())
}

#[command]
pub async fn move_to_folder(
    transcript_id: String,
    folder_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    transcripts::update(
        &conn,
        &transcript_id,
        &transcripts::TranscriptUpdate {
            title: None,
            language: None,
            is_starred: None,
            folder_id,
            word_count: None,
            speaker_count: None,
        },
    )
}

// ─── Tag commands ────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub id: String,
    pub name: String,
}

#[command]
pub async fn list_tags(db: State<'_, Arc<Database>>) -> Result<Vec<TagInfo>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn
        .prepare("SELECT id, name FROM tags ORDER BY name")
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TagInfo {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    Ok(rows)
}

#[command]
pub async fn create_tag(name: String, db: State<'_, Arc<Database>>) -> Result<String, AppError> {
    let conn = db.get()?;
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tags (id, name) VALUES (?1, ?2)",
        params![id, name],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to create tag: {}", e),
    })?;
    Ok(id)
}

#[command]
pub async fn delete_tag(id: String, db: State<'_, Arc<Database>>) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    Ok(())
}

#[command]
pub async fn tag_transcript(
    transcript_id: String,
    tag_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute(
        "INSERT OR IGNORE INTO transcript_tags (transcript_id, tag_id) VALUES (?1, ?2)",
        params![transcript_id, tag_id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    Ok(())
}

#[command]
pub async fn untag_transcript(
    transcript_id: String,
    tag_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    conn.execute(
        "DELETE FROM transcript_tags WHERE transcript_id = ?1 AND tag_id = ?2",
        params![transcript_id, tag_id],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    Ok(())
}

#[command]
pub async fn get_transcript_tags(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<TagInfo>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn.prepare("SELECT t.id, t.name FROM tags t JOIN transcript_tags tt ON t.id = tt.tag_id WHERE tt.transcript_id = ?1 ORDER BY t.name")
        .map_err(|e| AppError::StorageError { code: StorageErrorCode::DatabaseError, message: format!("{}", e) })?;
    let rows = stmt
        .query_map(params![transcript_id], |row| {
            Ok(TagInfo {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    Ok(rows)
}

// ─── Star/Favorite ───────────────────────────────────────────────────────────

#[command]
pub async fn toggle_star(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<bool, AppError> {
    let conn = db.get()?;
    let current: bool = conn
        .query_row(
            "SELECT is_starred FROM transcripts WHERE id = ?1",
            params![transcript_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    let new_val = !current;
    transcripts::update(
        &conn,
        &transcript_id,
        &transcripts::TranscriptUpdate {
            title: None,
            language: None,
            is_starred: Some(new_val),
            folder_id: None,
            word_count: None,
            speaker_count: None,
        },
    )?;
    Ok(new_val)
}

// ─── Trash ───────────────────────────────────────────────────────────────────

#[command]
pub async fn trash_transcript(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    transcripts::soft_delete(&conn, &transcript_id)
}

#[command]
pub async fn restore_transcript(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    transcripts::restore(&conn, &transcript_id)
}

#[command]
pub async fn list_trash(
    page: Option<u32>,
    page_size: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<serde_json::Value, AppError> {
    let conn = db.get()?;
    let (rows, total) = transcripts::list(&conn, true, page.unwrap_or(0), page_size.unwrap_or(50))?;
    Ok(
        serde_json::json!({ "items": rows, "total": total, "page": page.unwrap_or(0), "pageSize": page_size.unwrap_or(50) }),
    )
}

#[command]
pub async fn permanently_delete_transcript(
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    transcripts::hard_delete(&conn, &transcript_id)
}

#[command]
pub async fn purge_old_trash(db: State<'_, Arc<Database>>) -> Result<u64, AppError> {
    let conn = db.get()?;
    let thirty_days_ago = Utc::now().timestamp() - (30 * 24 * 3600);
    let count = conn
        .execute(
            "DELETE FROM transcripts WHERE is_deleted = 1 AND deleted_at < ?1",
            params![thirty_days_ago],
        )
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("{}", e),
        })?;
    Ok(count as u64)
}

// ─── Search ──────────────────────────────────────────────────────────────────

#[command]
pub async fn search_transcripts(
    query: String,
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<search::SearchResult>, AppError> {
    let conn = db.get()?;
    search::search(&conn, &query, limit.unwrap_or(50))
}

// ─── Smart Folders ───────────────────────────────────────────────────────────

#[command]
pub async fn list_smart_folders(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<smart_folders::SmartFolder>, AppError> {
    let conn = db.get()?;
    smart_folders::list(&conn)
}

#[command]
pub async fn create_smart_folder(
    name: String,
    filter_json: String,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    let conn = db.get()?;
    smart_folders::insert(&conn, &name, &filter_json)
}

#[command]
pub async fn update_smart_folder(
    id: String,
    name: String,
    filter_json: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    smart_folders::update(&conn, &id, &name, &filter_json)
}

#[command]
pub async fn delete_smart_folder(id: String, db: State<'_, Arc<Database>>) -> Result<(), AppError> {
    let conn = db.get()?;
    smart_folders::delete(&conn, &id)
}

#[command]
pub async fn query_smart_folder_transcripts(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<serde_json::Value, AppError> {
    let conn = db.get()?;
    let (rows, total) = smart_folders::query_transcripts(&conn, &id)?;
    Ok(serde_json::json!({ "items": rows, "total": total }))
}

// ─── Segment editing ─────────────────────────────────────────────────────────

#[command]
pub async fn merge_segments(
    kept_id: String,
    removed_id: String,
    db: State<'_, Arc<Database>>,
    undo_mgr: State<'_, UndoManager>,
) -> Result<(), AppError> {
    let mut conn = db.get()?;
    let kept = segments::get_by_id(&conn, &kept_id)?;
    let removed = segments::get_by_id(&conn, &removed_id)?;
    let merged_text = format!("{} {}", kept.text.trim(), removed.text.trim());
    let new_end_ms = removed.end_ms;
    let tx = conn.transaction().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    tx.execute(
        "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
        params![kept_id, merged_text, new_end_ms],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    segments::soft_delete(&tx, &removed_id)?;
    tx.commit().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to commit merge_segments: {}", e),
    })?;
    undo_mgr.push(UndoOperation::MergeSegments {
        kept_id,
        removed_id,
        old_kept_text: kept.text,
        old_removed_text: removed.text,
        merged_text,
        old_kept_end_ms: kept.end_ms,
        new_kept_end_ms: new_end_ms,
    })?;
    Ok(())
}

#[command]
pub async fn split_segment(
    segment_id: String,
    split_pos: usize,
    split_ms: i64,
    db: State<'_, Arc<Database>>,
    undo_mgr: State<'_, UndoManager>,
) -> Result<String, AppError> {
    let mut conn = db.get()?;
    let seg = segments::get_by_id(&conn, &segment_id)?;
    let first_text = seg.text.chars().take(split_pos).collect::<String>();
    let second_text = seg.text.chars().skip(split_pos).collect::<String>();
    let new_id = Uuid::new_v4().to_string();
    let tx = conn.transaction().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    tx.execute(
        "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
        params![segment_id, first_text.trim(), split_ms],
    )
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("{}", e),
    })?;
    tx.execute("INSERT INTO segments (id, transcript_id, index_num, start_ms, end_ms, text, speaker_id, confidence, is_deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
        params![new_id, seg.transcript_id, seg.index_num + 1, split_ms, seg.end_ms, second_text.trim(), seg.speaker_id, seg.confidence])
        .map_err(|e| AppError::StorageError { code: StorageErrorCode::DatabaseError, message: format!("{}", e) })?;
    tx.commit().map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("Failed to commit split_segment: {}", e),
    })?;
    undo_mgr.push(UndoOperation::SplitSegment {
        original_id: segment_id,
        new_id: new_id.clone(),
        old_text: seg.text,
        first_text,
        second_text,
        split_ms,
    })?;
    Ok(new_id)
}

#[command]
pub async fn delete_segment(
    segment_id: String,
    db: State<'_, Arc<Database>>,
    undo_mgr: State<'_, UndoManager>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    let seg = segments::get_by_id(&conn, &segment_id)?;
    segments::soft_delete(&conn, &segment_id)?;
    undo_mgr.push(UndoOperation::DeleteSegment {
        segment_id,
        transcript_id: seg.transcript_id,
        index_num: seg.index_num,
        start_ms: seg.start_ms,
        end_ms: seg.end_ms,
        text: seg.text,
        speaker_id: seg.speaker_id,
        confidence: seg.confidence,
    })?;
    Ok(())
}

// ─── Undo/Redo ───────────────────────────────────────────────────────────────

#[command]
pub async fn undo(
    db: State<'_, Arc<Database>>,
    undo_mgr: State<'_, UndoManager>,
) -> Result<bool, AppError> {
    let op = match undo_mgr.pop_undo()? {
        Some(op) => op,
        None => return Ok(false),
    };
    let conn = db.get()?;
    match &op {
        UndoOperation::UpdateText {
            segment_id,
            old_text,
            ..
        } => {
            segments::update_text(&conn, segment_id, old_text)?;
        }
        UndoOperation::DeleteSegment { segment_id, .. } => {
            conn.execute(
                "UPDATE segments SET is_deleted = 0 WHERE id = ?1",
                params![segment_id],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
        }
        UndoOperation::MergeSegments {
            kept_id,
            removed_id,
            old_kept_text,
            old_kept_end_ms,
            ..
        } => {
            conn.execute(
                "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
                params![kept_id, old_kept_text, old_kept_end_ms],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
            conn.execute(
                "UPDATE segments SET is_deleted = 0 WHERE id = ?1",
                params![removed_id],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
        }
        UndoOperation::SplitSegment {
            original_id,
            new_id,
            old_text,
            ..
        } => {
            let end_ms: i64 = conn
                .query_row(
                    "SELECT end_ms FROM segments WHERE id = ?1",
                    params![new_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::StorageError {
                    code: StorageErrorCode::DatabaseError,
                    message: format!("{}", e),
                })?;
            conn.execute(
                "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
                params![original_id, old_text, end_ms],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
            conn.execute("DELETE FROM segments WHERE id = ?1", params![new_id])
                .map_err(|e| AppError::StorageError {
                    code: StorageErrorCode::DatabaseError,
                    message: format!("{}", e),
                })?;
        }
    }
    undo_mgr.push_redo(op)?;
    Ok(true)
}

#[command]
pub async fn redo(
    db: State<'_, Arc<Database>>,
    undo_mgr: State<'_, UndoManager>,
) -> Result<bool, AppError> {
    let op = match undo_mgr.pop_redo()? {
        Some(op) => op,
        None => return Ok(false),
    };
    let conn = db.get()?;
    match &op {
        UndoOperation::UpdateText {
            segment_id,
            new_text,
            ..
        } => {
            segments::update_text(&conn, segment_id, new_text)?;
        }
        UndoOperation::DeleteSegment { segment_id, .. } => {
            segments::soft_delete(&conn, segment_id)?;
        }
        UndoOperation::MergeSegments {
            kept_id,
            removed_id,
            merged_text,
            new_kept_end_ms,
            ..
        } => {
            conn.execute(
                "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
                params![kept_id, merged_text, new_kept_end_ms],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
            segments::soft_delete(&conn, removed_id)?;
        }
        UndoOperation::SplitSegment {
            original_id,
            new_id,
            first_text,
            second_text,
            split_ms,
            ..
        } => {
            conn.execute(
                "UPDATE segments SET text = ?2, end_ms = ?3 WHERE id = ?1",
                params![original_id, first_text.trim(), split_ms],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
            conn.execute(
                "UPDATE segments SET is_deleted = 0, text = ?2, start_ms = ?3 WHERE id = ?1",
                params![new_id, second_text.trim(), split_ms],
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("{}", e),
            })?;
        }
    }
    undo_mgr.push(op)?;
    Ok(true)
}

#[command]
pub async fn can_undo(undo_mgr: State<'_, UndoManager>) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({ "canUndo": undo_mgr.can_undo(), "canRedo": undo_mgr.can_redo() }))
}
