pub mod audio;
pub mod batch;
pub mod commands;
pub mod database;
pub mod diarization;
pub mod dictation;
pub mod error;
pub mod export;
pub mod import;
pub mod keychain;
pub mod logging;
pub mod models;
pub mod network;
pub mod settings;
pub mod shortcuts;
pub mod transcription;
pub mod watch;

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
            let network_guard = network::guard::NetworkGuard::new(settings.network_policy.clone())?;
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

            // Initialize recording manager
            let recording_manager = Arc::new(audio::recording::RecordingManager::new());
            app.manage(Arc::clone(&recording_manager));

            // Initialize dictation manager
            let dictation_manager = Arc::new(dictation::DictationManager::new());
            app.manage(Arc::clone(&dictation_manager));

            // Initialize translation manager
            let translation_manager = Arc::new(transcription::translate::TranslationManager::new());
            app.manage(Arc::clone(&translation_manager));

            // Initialize shortcut manager
            let shortcut_manager = Arc::new(shortcuts::ShortcutManager::new());
            app.manage(Arc::clone(&shortcut_manager));

            // Initialize watch folder manager
            let watch_manager = Arc::new(watch::WatchFolderManager::new());
            app.manage(Arc::clone(&watch_manager));

            // Initialize filler word config state
            app.manage(commands::import::FillerConfigState::default());

            // Initialize captions state
            app.manage(commands::captions::CaptionsState::default());

            // Initialize batch queue
            let db_ref = app.state::<Arc<database::Database>>();
            let batch_queue = Arc::new(batch::queue::BatchQueue::new(Arc::clone(&db_ref)));
            app.manage(Arc::clone(&batch_queue));

            // Start watching configured folders
            let watch_handle = app_handle.clone();
            let settings_ref = app.state::<std::sync::Mutex<settings::AppSettings>>();
            let watch_configs: Vec<settings::WatchFolderConfig> = settings_ref
                .lock()
                .map(|s| s.watch_folders.clone())
                .unwrap_or_default();
            let wm = Arc::clone(&watch_manager);
            tauri::async_runtime::spawn(async move {
                wm.init(watch_handle, &watch_configs).await;
            });

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
            // Recording
            commands::recording::get_audio_devices,
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::pause_recording,
            commands::recording::resume_recording,
            commands::recording::get_recording_level,
            commands::recording::get_recording_status,
            commands::recording::is_system_audio_available,
            // Dictation
            commands::dictation::start_dictation,
            commands::dictation::stop_dictation,
            commands::dictation::get_dictation_status,
            commands::dictation::toggle_dictation,
            commands::dictation::list_dictation_history,
            commands::dictation::delete_dictation_history_entry,
            commands::dictation::clear_dictation_history,
            // Translation
            commands::translate::translate_text,
            commands::translate::set_translation_config,
            commands::translate::get_translation_config,
            commands::translate::get_supported_languages,
            // Shortcuts
            commands::shortcuts::register_shortcut,
            commands::shortcuts::unregister_shortcut,
            commands::shortcuts::list_shortcuts,
            commands::shortcuts::update_shortcut,
            commands::shortcuts::check_shortcut_conflict,
            // Watch folders
            commands::watch::add_watch_folder,
            commands::watch::remove_watch_folder,
            commands::watch::list_watch_folders,
            // Import
            commands::import::import_youtube,
            commands::import::check_ytdlp_status,
            commands::import::remove_filler_words,
            commands::import::get_filler_config,
            commands::import::set_filler_config,
            // Batch
            commands::batch::create_batch_job,
            commands::batch::start_batch_job,
            commands::batch::pause_batch_job,
            commands::batch::resume_batch_job,
            commands::batch::cancel_batch_job,
            commands::batch::get_batch_job,
            commands::batch::list_batch_jobs,
            commands::batch::get_batch_job_items,
            commands::batch::export_batch_job,
            // Diarization
            commands::diarization::diarize_transcript,
            commands::diarization::get_diarization_providers,
            commands::diarization::update_speaker_label,
            // Captions
            commands::captions::start_captions,
            commands::captions::stop_captions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
