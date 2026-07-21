//! Owns the lazily-loaded NllbEngine (load weights once).
use std::path::Path;
use std::sync::Mutex;
use crate::error::{AppError, StorageErrorCode};
use super::engine::NllbEngine;
use super::model;

pub struct TranslationEngineManager {
    engine: Mutex<Option<NllbEngine>>,
}

impl TranslationEngineManager {
    pub fn new() -> Self {
        Self { engine: Mutex::new(None) }
    }

    /// Load the engine once. Errors if the model isn't downloaded.
    pub fn ensure_loaded(&self, models_root: &Path) -> Result<(), AppError> {
        let mut guard = self.engine.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "translation engine mutex poisoned".into(),
        })?;
        if guard.is_some() {
            return Ok(());
        }
        if !model::is_downloaded(models_root) {
            return Err(AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: "translation model not downloaded".into(),
            });
        }
        let engine = NllbEngine::load(&model::model_dir(models_root))?;
        *guard = Some(engine);
        Ok(())
    }

    /// Translate with the loaded engine. Call ensure_loaded first.
    pub fn translate(&self, texts: &[String], src: &str, tgt: &str) -> Result<Vec<String>, AppError> {
        let guard = self.engine.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "translation engine mutex poisoned".into(),
        })?;
        let engine = guard.as_ref().ok_or_else(|| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "translation engine not loaded".into(),
        })?;
        engine.translate(texts, src, tgt)
    }
}

impl Default for TranslationEngineManager {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ensure_loaded_errors_when_model_missing() {
        let mgr = TranslationEngineManager::new();
        let err = mgr.ensure_loaded(&PathBuf::from("/tmp/nope-xyz"));
        assert!(err.is_err());
    }
}
