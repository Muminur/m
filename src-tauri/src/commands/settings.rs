use crate::error::AppError;
use crate::network::guard::NetworkGuard;
use crate::settings::AppSettings;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_settings(
    settings_state: State<'_, Mutex<AppSettings>>,
) -> Result<AppSettings, AppError> {
    let settings = settings_state
        .lock()
        .map_err(|_| crate::error::AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: "Failed to acquire settings lock".into(),
        })?;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    updates: serde_json::Value,
    settings_state: State<'_, Mutex<AppSettings>>,
    network_guard: State<'_, Arc<NetworkGuard>>,
) -> Result<AppSettings, AppError> {
    let mut settings = settings_state
        .lock()
        .map_err(|_| crate::error::AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: "Failed to acquire settings lock".into(),
        })?;

    // Merge updates into existing settings via JSON round-trip
    let mut current_json = serde_json::to_value(settings.clone())?;
    if let (Some(obj), Some(updates_obj)) = (current_json.as_object_mut(), updates.as_object()) {
        for (key, value) in updates_obj {
            obj.insert(key.clone(), value.clone());
        }
    }

    let new_settings: AppSettings = serde_json::from_value(current_json)?;
    let network_policy_changed = new_settings.network_policy != settings.network_policy;
    new_settings.save(&app)?;

    // Persist first; once that succeeds, update the managed guard before
    // publishing the matching in-memory settings snapshot.
    if network_policy_changed {
        network_guard.set_policy(new_settings.network_policy.clone());
    }
    *settings = new_settings.clone();

    tracing::info!("Settings updated");
    Ok(new_settings)
}
