//! Tauri IPC commands for live translation.

use std::sync::Arc;
use tauri::State;

use crate::error::AppError;
use crate::network::guard::NetworkGuard;
use crate::transcription::translate::{
    SupportedLanguage, TranslationConfig, TranslationManager, TranslationResult,
};

#[tauri::command]
pub async fn translate_text(
    text: String,
    target_lang: String,
    manager: State<'_, Arc<TranslationManager>>,
    network: State<'_, NetworkGuard>,
) -> Result<TranslationResult, AppError> {
    // Attempt to retrieve API key from keychain (best-effort on non-macOS)
    let api_key = crate::keychain::get("deepl", "api_key").ok().flatten();

    manager
        .translate(&text, &target_lang, &network, api_key)
        .await
}

#[tauri::command]
pub async fn set_translation_config(
    config: TranslationConfig,
    manager: State<'_, Arc<TranslationManager>>,
) -> Result<(), AppError> {
    manager.set_config(config);
    tracing::info!("Translation config updated");
    Ok(())
}

#[tauri::command]
pub async fn get_translation_config(
    manager: State<'_, Arc<TranslationManager>>,
) -> Result<TranslationConfig, AppError> {
    Ok(manager.config())
}

#[tauri::command]
pub async fn get_supported_languages(
    manager: State<'_, Arc<TranslationManager>>,
) -> Result<Vec<SupportedLanguage>, AppError> {
    Ok(manager.supported_languages())
}
