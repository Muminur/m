//! Tauri IPC commands for global shortcut management.

use std::sync::Arc;
use tauri::State;

use crate::error::AppError;
use crate::shortcuts::{ShortcutBinding, ShortcutConflict, ShortcutManager};

#[tauri::command]
pub async fn register_shortcut(
    id: String,
    accelerator: String,
    description: Option<String>,
    manager: State<'_, Arc<ShortcutManager>>,
) -> Result<(), AppError> {
    let desc = description.unwrap_or_default();
    manager.register(&id, &accelerator, &desc)
}

#[tauri::command]
pub async fn unregister_shortcut(
    id: String,
    manager: State<'_, Arc<ShortcutManager>>,
) -> Result<(), AppError> {
    manager.unregister(&id)
}

#[tauri::command]
pub async fn list_shortcuts(
    manager: State<'_, Arc<ShortcutManager>>,
) -> Result<Vec<ShortcutBinding>, AppError> {
    Ok(manager.list_registered())
}

#[tauri::command]
pub async fn update_shortcut(
    id: String,
    accelerator: String,
    manager: State<'_, Arc<ShortcutManager>>,
) -> Result<(), AppError> {
    manager.update_binding(&id, &accelerator)
}

#[tauri::command]
pub async fn check_shortcut_conflict(
    accelerator: String,
    manager: State<'_, Arc<ShortcutManager>>,
) -> Result<Vec<ShortcutConflict>, AppError> {
    Ok(manager.detect_conflicts(&accelerator))
}
