pub mod audio;
pub mod commands;
pub mod database;
pub mod error;
pub mod export;
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

            // Initialize undo manager
            let undo_manager = crate::database::undo::UndoManager::new();
            app.manage(undo_manager);

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
            // Library
            commands::library::list_folders,
            commands::library::create_folder,
            commands::library::rename_folder,
            commands::library::delete_folder,
            commands::library::move_to_folder,
            commands::library::list_tags,
            commands::library::create_tag,
            commands::library::delete_tag,
            commands::library::tag_transcript,
            commands::library::untag_transcript,
            commands::library::get_transcript_tags,
            commands::library::toggle_star,
            commands::library::trash_transcript,
            commands::library::restore_transcript,
            commands::library::list_trash,
            commands::library::permanently_delete_transcript,
            commands::library::purge_old_trash,
            commands::library::search_transcripts,
            commands::library::list_smart_folders,
            commands::library::create_smart_folder,
            commands::library::update_smart_folder,
            commands::library::delete_smart_folder,
            commands::library::query_smart_folder_transcripts,
            commands::library::merge_segments,
            commands::library::split_segment,
            commands::library::delete_segment,
            commands::library::undo,
            commands::library::redo,
            commands::library::can_undo,
            // Export
            commands::export::export_transcript,
            commands::export::export_to_file,
            commands::export::copy_transcript_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
