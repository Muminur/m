pub mod handler;

use crate::database::Database;
use crate::error::{AppError, StorageErrorCode};
use crate::settings::WatchFolderConfig;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchFolderEvent {
    pub event_id: Option<String>,
    pub folder_path: String,
    pub file_path: String,
    pub file_name: String,
    pub status: String,
}

pub struct WatchFolderManager {
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    app_handle: Mutex<Option<AppHandle>>,
}

impl Default for WatchFolderManager {
    fn default() -> Self {
        Self::new()
    }
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
        let pending_paths = Arc::new(StdMutex::new(HashSet::<PathBuf>::new()));

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if is_file_arrival_event(&event.kind) {
                    for path in &event.paths {
                        if handler::is_audio_file(path) {
                            let is_new = pending_paths
                                .lock()
                                .map(|mut pending| pending.insert(path.clone()))
                                .unwrap_or(false);
                            if !is_new {
                                tracing::debug!(
                                    "Ignoring duplicate watch-folder create event: {:?}",
                                    path
                                );
                                continue;
                            }

                            tracing::info!("Watch folder detected new audio file: {:?}", path);

                            if let Some(ref app) = app_handle {
                                let app = app.clone();
                                let folder = folder_str.clone();
                                let detected_path = path.clone();
                                let pending_paths = Arc::clone(&pending_paths);
                                tauri::async_runtime::spawn(async move {
                                    match handler::wait_until_file_stable(&detected_path).await {
                                        Ok(true) => publish_detected_file(&app, &folder, &detected_path),
                                        Ok(false) => tracing::warn!(
                                            "Watch folder file did not stabilize before timeout: {:?}",
                                            detected_path
                                        ),
                                        Err(error) => tracing::warn!(
                                            "Watch folder file disappeared or became unreadable: {:?}: {}",
                                            detected_path,
                                            error
                                        ),
                                    }
                                    if let Ok(mut pending) = pending_paths.lock() {
                                        pending.remove(&detected_path);
                                    }
                                });
                            } else if let Ok(mut pending) = pending_paths.lock() {
                                pending.remove(path);
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

fn is_file_arrival_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
    )
}

fn publish_detected_file(app: &AppHandle, folder_path: &str, path: &Path) {
    let file_path = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    let event_id = if let Some(db) = app.try_state::<Arc<Database>>() {
        match db.get() {
            Ok(conn) => match crate::database::recordings::insert_watch_event(
                &conn,
                folder_path,
                &file_path,
                &file_name,
            ) {
                Ok(id) => Some(id),
                Err(error) => {
                    tracing::error!(
                        "Failed to record watch folder event for {:?}: {}",
                        path,
                        error
                    );
                    None
                }
            },
            Err(error) => {
                tracing::error!("Failed to get DB connection for watch event: {}", error);
                None
            }
        }
    } else {
        None
    };

    let _ = app.emit(
        "watch:file-detected",
        WatchFolderEvent {
            event_id,
            folder_path: folder_path.to_string(),
            file_path,
            file_name,
            status: "detected".into(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, ModifyKind, RenameMode};

    #[test]
    fn recognizes_created_and_renamed_files_as_arrivals() {
        assert!(is_file_arrival_event(&EventKind::Create(CreateKind::File)));
        assert!(is_file_arrival_event(&EventKind::Modify(ModifyKind::Name(
            RenameMode::To
        ))));
        assert!(!is_file_arrival_event(&EventKind::Modify(
            ModifyKind::Data(DataChange::Content)
        )));
        assert!(!is_file_arrival_event(&EventKind::Remove(
            notify::event::RemoveKind::File
        )));
    }
}
