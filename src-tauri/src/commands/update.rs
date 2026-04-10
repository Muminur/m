//! Auto-update commands using tauri-plugin-updater.

use crate::error::{AppError, NetworkErrorCode};
use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_updater::UpdaterExt;

/// Information about an available update.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

/// Return the current application version from package info.
#[command]
pub async fn get_app_version(app: AppHandle) -> Result<String, AppError> {
    Ok(app.package_info().version.to_string())
}

/// Check whether an update is available.
///
/// Returns `Some(UpdateInfo)` when a newer version exists, `None` when up to date.
#[command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let updater = app.updater_builder().build().map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Failed to build updater: {}", e),
    })?;

    let update = updater.check().await.map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Update check failed: {}", e),
    })?;

    match update {
        Some(u) => Ok(Some(UpdateInfo {
            version: u.version.clone(),
            body: u.body.clone(),
            date: u.date.map(|d| d.to_string()),
        })),
        None => Ok(None),
    }
}

/// Download and install the pending update, then restart the application.
#[command]
pub async fn download_and_install_update(app: AppHandle) -> Result<(), AppError> {
    let updater = app.updater_builder().build().map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Failed to build updater: {}", e),
    })?;

    let update = updater.check().await.map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Update check failed: {}", e),
    })?;

    let update = update.ok_or_else(|| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: "No update available to install".into(),
    })?;

    update
        .download_and_install(|_chunk_length, _content_length| {}, || {})
        .await
        .map_err(|e| AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: format!("Download/install failed: {}", e),
        })?;

    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_info_serialization() {
        let info = UpdateInfo {
            version: "1.2.0".into(),
            body: Some("Bug fixes".into()),
            date: Some("2025-01-15".into()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["version"], "1.2.0");
        assert_eq!(json["body"], "Bug fixes");
        assert_eq!(json["date"], "2025-01-15");
    }

    #[test]
    fn test_update_info_serialization_none_fields() {
        let info = UpdateInfo {
            version: "1.0.0".into(),
            body: None,
            date: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["version"], "1.0.0");
        assert!(json["body"].is_null());
        assert!(json["date"].is_null());
    }
}
