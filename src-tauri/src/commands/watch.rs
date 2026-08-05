use crate::database::{recordings, Database};
use crate::error::AppError;
use crate::watch::WatchFolderManager;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn add_watch_folder(
    manager: State<'_, Arc<WatchFolderManager>>,
    folder_path: String,
) -> Result<(), AppError> {
    manager.add_folder(&folder_path).await
}

#[tauri::command]
pub async fn remove_watch_folder(
    manager: State<'_, Arc<WatchFolderManager>>,
    folder_path: String,
) -> Result<(), AppError> {
    manager.remove_folder(&folder_path).await
}

#[tauri::command]
pub async fn list_watch_folders(
    manager: State<'_, Arc<WatchFolderManager>>,
) -> Result<Vec<String>, AppError> {
    Ok(manager.list_watched().await)
}

#[tauri::command]
pub async fn update_watch_folder_event_status(
    event_id: String,
    status: String,
    transcript_id: Option<String>,
    error_message: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    if !matches!(status.as_str(), "queued" | "transcribed" | "failed") {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Invalid watch-folder event status: {}", status),
        });
    }

    let conn = db.get()?;
    recordings::update_watch_event_status(
        &conn,
        &event_id,
        &status,
        transcript_id.as_deref(),
        error_message.as_deref(),
    )
}
