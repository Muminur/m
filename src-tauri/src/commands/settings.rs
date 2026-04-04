use tauri::{AppHandle, State};
use std::sync::Mutex;
use crate::error::AppError;
use crate::settings::AppSettings;

#[tauri::command]
pub async fn get_settings(
    settings_state: State<'_, Mutex<AppSettings>>,
) -> Result<AppSettings, AppError> {
    let settings = settings_state.lock().map_err(|_| {
        crate::error::AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: "Failed to acquire settings lock".into(),
        }
    })?;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    updates: serde_json::Value,
    settings_state: State<'_, Mutex<AppSettings>>,
) -> Result<AppSettings, AppError> {
    let mut settings = settings_state.lock().map_err(|_| {
        crate::error::AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: "Failed to acquire settings lock".into(),
        }
    })?;

    // Merge updates into existing settings via JSON round-trip
    let mut current_json = serde_json::to_value(settings.clone())?;
    if let (Some(obj), Some(updates_obj)) = (current_json.as_object_mut(), updates.as_object()) {
        for (key, value) in updates_obj {
            obj.insert(key.clone(), value.clone());
        }
    }

    let new_settings: AppSettings = serde_json::from_value(current_json)?;
    new_settings.save(&app)?;
    *settings = new_settings.clone();

    tracing::info!("Settings updated");
    Ok(new_settings)
}
