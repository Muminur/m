use crate::error::AppError;

/// Store an API key in the system keychain.
#[tauri::command]
pub async fn set_api_key(service: String, key: String) -> Result<(), AppError> {
    // Validate inputs
    if service.is_empty() {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Service name cannot be empty".into(),
        });
    }
    if key.is_empty() {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "API key cannot be empty".into(),
        });
    }

    tokio::task::spawn_blocking(move || crate::keychain::set(&service, "api_key", &key))
        .await
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: format!("Keychain task failed: {}", e),
        })?
}

/// Check whether an API key exists in the system keychain.
///
/// Returns `true` if a key is stored for the given service, `false` otherwise.
/// The actual key value is never sent to the renderer process; internal Rust
/// code should call `crate::keychain::get()` directly.
#[tauri::command]
pub async fn check_api_key_set(service: String) -> Result<bool, AppError> {
    if service.is_empty() {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Service name cannot be empty".into(),
        });
    }

    let result = tokio::task::spawn_blocking(move || crate::keychain::get(&service, "api_key"))
        .await
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: format!("Keychain task failed: {}", e),
        })??;

    Ok(result.is_some())
}

/// Delete an API key from the system keychain.
#[tauri::command]
pub async fn delete_api_key(service: String) -> Result<(), AppError> {
    if service.is_empty() {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: "Service name cannot be empty".into(),
        });
    }

    tokio::task::spawn_blocking(move || crate::keychain::delete(&service, "api_key"))
        .await
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::IoError,
            message: format!("Keychain task failed: {}", e),
        })?
}
