use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use serde::Serialize;

use crate::database::Database;
use crate::dictation::accessibility;
use crate::dictation::history::{DictationHistory, HistoryEntry};
use crate::dictation::postprocess::PostProcessor;
use crate::dictation::{DictationManager, DictationState};
use crate::error::AppError;

/// Status response for the dictation system.
#[derive(Debug, Clone, Serialize)]
pub struct DictationStatusResponse {
    pub state: DictationState,
    pub accessibility_granted: bool,
}

/// Response after dictation produces text.
#[derive(Debug, Clone, Serialize)]
pub struct DictationTextResponse {
    pub raw_text: String,
    pub processed_text: String,
    pub history_id: String,
}

#[tauri::command]
pub async fn start_dictation(
    app: AppHandle,
    manager: State<'_, Arc<DictationManager>>,
) -> Result<(), AppError> {
    let current = manager.state();
    if current != DictationState::Idle {
        tracing::warn!(state = ?current, "Cannot start dictation: not idle");
        return Err(AppError::AudioError {
            code: crate::error::AudioErrorCode::CaptureFailure,
            message: format!("Cannot start dictation from state: {:?}", current),
        });
    }

    manager.transition(DictationState::Listening);
    let _ = app.emit("dictation:started", ());
    tracing::info!("Dictation started");
    Ok(())
}

#[tauri::command]
pub async fn stop_dictation(
    app: AppHandle,
    manager: State<'_, Arc<DictationManager>>,
    db: State<'_, Arc<Database>>,
    raw_text: Option<String>,
) -> Result<Option<DictationTextResponse>, AppError> {
    let current = manager.state();
    if current == DictationState::Idle {
        tracing::warn!("Dictation already idle, nothing to stop");
        return Ok(None);
    }

    manager.transition(DictationState::Processing);

    let result = if let Some(text) = raw_text {
        if text.is_empty() {
            None
        } else {
            // Post-process the text
            let processor = PostProcessor::new();
            let processed = processor.process(&text);

            // Insert into target app
            manager.transition(DictationState::Inserting);
            let inserter = accessibility::create_text_inserter();
            let app_target: Option<String> = inserter.get_focused_app()?;

            if let Err(e) = inserter.insert_text(&processed) {
                tracing::warn!(error = %e, "Text insertion failed");
                let _ = app.emit("dictation:error", format!("Text insertion failed: {}", e));
            }

            // Save to history
            let conn = db.get()?;
            let history_id = DictationHistory::add_entry(
                &conn,
                &processed,
                app_target.as_deref(),
            )?;

            let response = DictationTextResponse {
                raw_text: text,
                processed_text: processed.clone(),
                history_id,
            };

            let _ = app.emit("dictation:text", processed);

            Some(response)
        }
    } else {
        None
    };

    manager.transition(DictationState::Idle);
    let _ = app.emit("dictation:stopped", ());
    tracing::info!("Dictation stopped");
    Ok(result)
}

#[tauri::command]
pub async fn get_dictation_status(
    manager: State<'_, Arc<DictationManager>>,
) -> Result<DictationStatusResponse, AppError> {
    let accessibility_granted = accessibility::request_accessibility_permission()?;
    Ok(DictationStatusResponse {
        state: manager.state(),
        accessibility_granted,
    })
}

#[tauri::command]
pub async fn toggle_dictation(
    app: AppHandle,
    manager: State<'_, Arc<DictationManager>>,
    db: State<'_, Arc<Database>>,
    raw_text: Option<String>,
) -> Result<Option<DictationTextResponse>, AppError> {
    match manager.state() {
        DictationState::Idle => {
            start_dictation(app, manager).await?;
            Ok(None)
        }
        DictationState::Listening => {
            stop_dictation(app, manager, db, raw_text).await
        }
        other => {
            tracing::warn!(state = ?other, "Cannot toggle dictation in current state");
            Ok(None)
        }
    }
}

// -- History commands --

#[tauri::command]
pub async fn list_dictation_history(
    db: State<'_, Arc<Database>>,
    limit: Option<usize>,
) -> Result<Vec<HistoryEntry>, AppError> {
    let conn = db.get()?;
    DictationHistory::list_recent(&conn, limit.unwrap_or(50))
}

#[tauri::command]
pub async fn delete_dictation_history_entry(
    db: State<'_, Arc<Database>>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.get()?;
    DictationHistory::delete_entry(&conn, &id)
}

#[tauri::command]
pub async fn clear_dictation_history(
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    let conn = db.get()?;
    DictationHistory::clear(&conn)
}
