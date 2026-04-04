use crate::error::{AppError, StorageErrorCode};

#[cfg(target_os = "macos")]
const SERVICE_PREFIX: &str = "com.whisperdesk";

pub fn set(service: &str, key: &str, value: &str) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::set_generic_password;
        let full_service = format!("{}.{}", SERVICE_PREFIX, service);
        set_generic_password(&full_service, key, value.as_bytes()).map_err(|e| {
            AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: format!("Keychain set failed: {}", e),
            }
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, key, value);
        Err(AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: "Keychain is only available on macOS".into(),
        })
    }
}

pub fn get(service: &str, key: &str) -> Result<Option<String>, AppError> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::get_generic_password;
        let full_service = format!("{}.{}", SERVICE_PREFIX, service);
        match get_generic_password(&full_service, key) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(e) if e.code() == -25300 => Ok(None), // errSecItemNotFound
            Err(e) => Err(AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: format!("Keychain get failed: {}", e),
            }),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, key);
        Err(AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: "Keychain is only available on macOS".into(),
        })
    }
}

pub fn delete(service: &str, key: &str) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::delete_generic_password;
        let full_service = format!("{}.{}", SERVICE_PREFIX, service);
        delete_generic_password(&full_service, key).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Keychain delete failed: {}", e),
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, key);
        Err(AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: "Keychain is only available on macOS".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_keychain_returns_error_on_non_macos() {
        let result = set("test", "key", "value");
        assert!(result.is_err());
        let result = get("test", "key");
        assert!(result.is_err());
        let result = delete("test", "key");
        assert!(result.is_err());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_keychain_round_trip() {
        let service = "test_service";
        let key = "test_key_round_trip";
        let value = "test_secret_value";

        // Clean up first
        let _ = delete(service, key);

        // Set
        set(service, key, value).expect("Keychain set failed");

        // Get
        let retrieved = get(service, key).expect("Keychain get failed");
        assert_eq!(retrieved, Some(value.to_string()));

        // Delete
        delete(service, key).expect("Keychain delete failed");

        // Verify deleted
        let after_delete = get(service, key).expect("Keychain get after delete failed");
        assert_eq!(after_delete, None);
    }
}
