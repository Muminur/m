use std::sync::Arc;
use tauri::State;
use crate::watch::WatchFolderManager;
use crate::error::AppError;

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
