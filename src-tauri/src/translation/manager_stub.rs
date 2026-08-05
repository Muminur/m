//! Unsupported-platform stand-in for the macOS CTranslate2 manager.
//!
//! Keeping the same public API means Tauri can register the offline translation
//! commands on every desktop target without linking the macOS-only `ct2rs`
//! runtime.

use crate::error::{AppError, ModelErrorCode};
use std::path::Path;

pub struct TranslationEngineManager;

impl TranslationEngineManager {
    pub fn new() -> Self {
        Self
    }

    pub fn ensure_supported(&self) -> Result<(), AppError> {
        Err(unsupported_platform_error())
    }

    pub fn ensure_loaded(&self, _models_root: &Path) -> Result<(), AppError> {
        Err(unsupported_platform_error())
    }

    pub fn translate(
        &self,
        _texts: &[String],
        _src: &str,
        _tgt: &str,
    ) -> Result<Vec<String>, AppError> {
        Err(unsupported_platform_error())
    }
}

impl Default for TranslationEngineManager {
    fn default() -> Self {
        Self::new()
    }
}

fn unsupported_platform_error() -> AppError {
    AppError::ModelError {
        code: ModelErrorCode::UnsupportedPlatform,
        message: format!(
            "Offline translation is not supported on {}; it is currently available on macOS only",
            std::env::consts::OS
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_typed_unsupported_platform_error() {
        let error = TranslationEngineManager::new()
            .ensure_supported()
            .expect_err("non-macOS targets must reject offline translation");
        let json = serde_json::to_value(error).expect("AppError should serialize");

        assert_eq!(json["kind"], "ModelError");
        assert_eq!(json["detail"]["code"], "UnsupportedPlatform");
        assert!(json["detail"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("macOS only")));
    }
}
