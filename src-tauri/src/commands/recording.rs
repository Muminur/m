use std::sync::Arc;
use tauri::{AppHandle, State};
use crate::audio::mic;
use crate::audio::recording::{AudioSource, RecordingManager, RecordingLevelEvent, RecordingStatus};
use crate::error::AppError;

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<mic::AudioDeviceInfo>, AppError> {
    mic::list_input_devices()
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    manager: State<'_, Arc<RecordingManager>>,
    source: String,
    device_id: Option<String>,
) -> Result<String, AppError> {
    let audio_source: AudioSource = source.parse()?;
    manager.start(&app, audio_source, device_id)
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    manager: State<'_, Arc<RecordingManager>>,
) -> Result<String, AppError> {
    manager.stop(&app)
}

#[tauri::command]
pub async fn pause_recording(
    app: AppHandle,
    manager: State<'_, Arc<RecordingManager>>,
) -> Result<(), AppError> {
    manager.pause(&app)
}

#[tauri::command]
pub async fn resume_recording(
    app: AppHandle,
    manager: State<'_, Arc<RecordingManager>>,
) -> Result<(), AppError> {
    manager.resume(&app)
}

#[tauri::command]
pub async fn get_recording_level(
    manager: State<'_, Arc<RecordingManager>>,
) -> Result<RecordingLevelEvent, AppError> {
    Ok(manager.get_level())
}

#[tauri::command]
pub async fn get_recording_status(
    manager: State<'_, Arc<RecordingManager>>,
) -> Result<RecordingStatus, AppError> {
    Ok(manager.status())
}

#[tauri::command]
pub async fn is_system_audio_available() -> Result<bool, AppError> {
    Ok(crate::audio::system_audio::is_system_audio_available())
}
