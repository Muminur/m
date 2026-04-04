pub mod audio;
pub mod commands;
pub mod database;
pub mod error;
pub mod keychain;
pub mod logging;
pub mod models;
pub mod network;
pub mod settings;
pub mod transcription;

use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    // Initialize logging first
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize database (wrapped in Arc for sharing across async tasks)
            let db = database::init(&app_handle)?;
            app.manage(Arc::new(db));

            // Initialize settings
            let settings = settings::AppSettings::load(&app_handle)?;

            // Initialize NetworkGuard from network policy (must be before settings is managed)
            let network_guard =
                network::guard::NetworkGuard::new(settings.network_policy.clone())?;
            app.manage(network_guard);

            app.manage(std::sync::Mutex::new(settings));

            // Initialize model manager with per-user models directory
            let models_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|_| crate::error::AppError::StorageError {
                    code: crate::error::StorageErrorCode::IoError,
                    message: "Failed to resolve app data directory for models".into(),
                })?
                .join("models");
            let model_manager = Arc::new(models::manager::ModelManager::new(models_dir));
            app.manage(Arc::clone(&model_manager));

            // Initialize transcription manager
            let transcription_manager =
                Arc::new(transcription::pipeline::TranscriptionManager::new());
            app.manage(Arc::clone(&transcription_manager));

            tracing::info!("WhisperDesk initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Models
            commands::transcription::list_models,
            commands::transcription::download_model,
            commands::transcription::cancel_model_download,
            commands::transcription::delete_model,
            commands::transcription::set_default_model,
            // Transcription
            commands::transcription::transcribe_file,
            commands::transcription::cancel_transcription,
            // Transcripts
            commands::transcription::get_transcript,
            commands::transcription::list_transcripts,
            commands::transcription::update_segment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
