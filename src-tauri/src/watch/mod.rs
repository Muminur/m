pub mod handler;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use tauri::{AppHandle, Emitter, Manager};
use crate::database::Database;
use crate::error::{AppError, StorageErrorCode};
use crate::settings::WatchFolderConfig;

#[derive(Clone, serde::Serialize)]
pub struct WatchFolderEvent {
    pub folder_path: String,
    pub file_path: String,
    pub file_name: String,
    pub status: String,
}

pub struct WatchFolderManager {
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    app_handle: Mutex<Option<AppHandle>>,
}

impl WatchFolderManager {
    pub fn new() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
            app_handle: Mutex::new(None),
        }
    }

    pub async fn init(&self, app: AppHandle, configs: &[WatchFolderConfig]) {
        *self.app_handle.lock().await = Some(app);
        for config in configs {
            if config.enabled {
                if let Err(e) = self.add_folder(&config.path).await {
                    tracing::warn!("Failed to watch folder {}: {}", config.path, e);
                }
            }
        }
    }

    pub async fn add_folder(&self, folder_path: &str) -> Result<(), AppError> {
        let path = PathBuf::from(folder_path);
        if !path.exists() {
            return Err(AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: format!("Watch folder does not exist: {}", folder_path),
            });
        }

        let app_handle = self.app_handle.lock().await.clone();
        let folder_str = folder_path.to_string();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if let EventKind::Create(_) = event.kind {
                    for path in &event.paths {
                        if handler::is_audio_file(path) {
                            let file_name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();

                            tracing::info!("Watch folder detected new audio file: {:?}", path);

                            if let Some(ref app) = app_handle {
                                let _ = app.emit(
                                    "watch:file-detected",
                                    WatchFolderEvent {
                                        folder_path: folder_str.clone(),
                                        file_path: path.to_string_lossy().to_string(),
                                        file_name,
                                        status: "detected".into(),
                                    },
                                );

                                // Record in database
                                if let Some(db) = app.try_state::<Arc<Database>>() {
                                    if let Ok(conn) = db.get() {
                                        let _ = crate::database::recordings::insert_watch_event(
                                            &conn,
                                            &folder_str,
                                            &path.to_string_lossy(),
                                            &path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Failed to create file watcher: {}", e),
        })?;

        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: format!("Failed to watch folder: {}", e),
            })?;

        self.watchers
            .lock()
            .await
            .insert(folder_path.to_string(), watcher);

        tracing::info!("Watching folder: {}", folder_path);
        Ok(())
    }

    pub async fn remove_folder(&self, folder_path: &str) -> Result<(), AppError> {
        self.watchers.lock().await.remove(folder_path);
        tracing::info!("Stopped watching folder: {}", folder_path);
        Ok(())
    }

    pub async fn list_watched(&self) -> Vec<String> {
        self.watchers.lock().await.keys().cloned().collect()
    }
}
