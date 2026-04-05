use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::audio::mic::{self, MicRecorder};
use crate::database::{self, Database};
use crate::error::{AppError, AudioErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Idle,
    Recording,
    Paused,
    Stopping,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSource {
    Microphone,
    System,
    Both,
}

impl AudioSource {
    pub fn as_db_str(&self) -> &str {
        match self {
            AudioSource::Microphone => "mic",
            AudioSource::System => "system",
            AudioSource::Both => "both",
        }
    }
}

impl std::str::FromStr for AudioSource {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Microphone" | "mic" => Ok(AudioSource::Microphone),
            "System" | "system" => Ok(AudioSource::System),
            "Both" | "both" => Ok(AudioSource::Both),
            _ => Err(AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: format!("Unknown audio source: {}", s),
            }),
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingLevelEvent {
    pub level_db: f32,
    pub duration_ms: u64,
    pub status: RecordingStatus,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatusEvent {
    pub status: RecordingStatus,
    pub recording_id: Option<String>,
}

enum ActiveRecording {
    Mic(MicRecorder),
    #[cfg(target_os = "windows")]
    System(crate::audio::system_audio::wasapi_loopback::SystemAudioCapture),
    Combined(crate::audio::combined::CombinedCapture),
}

// Safety: RecordingManager protects all access to ActiveRecording through
// std::sync::Mutex. cpal::Stream is !Send because platform audio backends
// (WASAPI/COM on Windows, CoreAudio on macOS) may use thread-local state.
// In practice, Tauri command handlers run on a tokio thread pool and the
// Mutex ensures exclusive access. cpal internally initializes COM per-thread
// on Windows. The stream callbacks run on cpal's own audio thread, not
// through the Mutex. We only call play()/pause() and drop through the Mutex,
// which are safe across threads in cpal's WASAPI backend.
unsafe impl Send for RecordingManager {}
unsafe impl Sync for RecordingManager {}

pub struct RecordingManager {
    status: Mutex<RecordingStatus>,
    active: Mutex<Option<ActiveRecording>>,
    recording_id: Mutex<Option<String>>,
    source: Mutex<AudioSource>,
    device_id: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().expect("RecordingManager mutex poisoned — audio subsystem in inconsistent state")
}

impl RecordingManager {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(RecordingStatus::Idle),
            active: Mutex::new(None),
            recording_id: Mutex::new(None),
            source: Mutex::new(AudioSource::Microphone),
            device_id: Mutex::new(None),
        }
    }

    pub fn start(
        &self,
        app: &AppHandle,
        source: AudioSource,
        device_id: Option<String>,
    ) -> Result<String, AppError> {
        {
            let status = lock(&self.status);
            if *status != RecordingStatus::Idle {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Already recording".into(),
                });
            }
        }

        let rec_id = Uuid::new_v4().to_string();
        let recordings_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: "Failed to get app data dir".into(),
            })?
            .join("recordings");

        std::fs::create_dir_all(&recordings_dir)?;

        let active = match source {
            AudioSource::Microphone => {
                let device = mic::get_device_by_id(device_id.as_deref())?;
                let path = recordings_dir.join(format!("{}.wav", rec_id));
                let recorder = MicRecorder::new(&device, path)?;
                recorder.start()?;
                ActiveRecording::Mic(recorder)
            }
            #[cfg(target_os = "windows")]
            AudioSource::System => {
                let path = recordings_dir.join(format!("{}_system.wav", rec_id));
                let capture =
                    crate::audio::system_audio::wasapi_loopback::SystemAudioCapture::new(path)?;
                capture.start()?;
                ActiveRecording::System(capture)
            }
            #[cfg(not(target_os = "windows"))]
            AudioSource::System => {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "System audio capture not supported on this platform".into(),
                });
            }
            AudioSource::Both => {
                let mic_path = recordings_dir.join(format!("{}_mic.wav", rec_id));
                let sys_path = recordings_dir.join(format!("{}_system.wav", rec_id));
                let combined = crate::audio::combined::CombinedCapture::new(
                    device_id.as_deref(),
                    mic_path,
                    sys_path,
                )?;
                combined.start()?;
                ActiveRecording::Combined(combined)
            }
        };

        // Update state atomically — each lock acquired and released independently
        *lock(&self.active) = Some(active);
        *lock(&self.recording_id) = Some(rec_id.clone());
        *lock(&self.source) = source;
        *lock(&self.device_id) = device_id;
        *lock(&self.status) = RecordingStatus::Recording;

        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Recording,
                recording_id: Some(rec_id.clone()),
            },
        );

        Ok(rec_id)
    }

    pub fn stop(&self, app: &AppHandle) -> Result<String, AppError> {
        // Phase 1: Check and set status to Stopping (short lock scope)
        {
            let mut status = lock(&self.status);
            if *status != RecordingStatus::Recording && *status != RecordingStatus::Paused {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not recording".into(),
                });
            }
            *status = RecordingStatus::Stopping;
        }

        let rid = lock(&self.recording_id).clone();
        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Stopping,
                recording_id: rid,
            },
        );

        // Phase 2: Take ownership of active recording (short lock), then do I/O outside lock
        let active = lock(&self.active).take();
        let source = lock(&self.source).clone();
        let device_id = lock(&self.device_id).clone();

        // Phase 3: Stop recording and finalize WAV — NO locks held during I/O
        let (audio_path, duration_ms, sample_rate, channels) = match active {
            Some(ActiveRecording::Mic(recorder)) => {
                let dur = recorder.duration_ms();
                let sr = recorder.sample_rate();
                let ch = recorder.channels();
                let path = recorder.stop()?;
                (path.to_string_lossy().to_string(), dur, sr as i64, ch as i64)
            }
            #[cfg(target_os = "windows")]
            Some(ActiveRecording::System(capture)) => {
                let dur = capture.duration_ms();
                let sr = capture.sample_rate();
                let ch = capture.channels();
                let path = capture.stop()?;
                (path.to_string_lossy().to_string(), dur, sr as i64, ch as i64)
            }
            Some(ActiveRecording::Combined(combined)) => {
                let dur = combined.duration_ms();
                let sr = combined.mic_sample_rate();
                let ch = combined.mic_channels();
                let (mic_path, _sys_path) = combined.stop()?;
                (mic_path.to_string_lossy().to_string(), dur, sr as i64, ch as i64)
            }
            None => {
                // Restore idle status since we have no recording
                *lock(&self.status) = RecordingStatus::Idle;
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "No active recording".into(),
                });
            }
        };

        // Phase 4: Save to database
        let db = app.state::<Arc<Database>>();
        let conn = db.get()?;

        let _rec_id = lock(&self.recording_id).take().unwrap_or_default();

        database::recordings::insert(
            &conn,
            source.as_db_str(),
            device_id.as_deref(),
            None,
            &audio_path,
            duration_ms as i64,
            sample_rate,
            channels,
        )?;

        let title = format!(
            "Recording {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M")
        );
        let _transcript_id = database::transcripts::insert(
            &conn,
            &database::transcripts::NewTranscript {
                title,
                duration_ms: Some(duration_ms as i64),
                language: None,
                model_id: None,
                source_type: Some(source.as_db_str().to_string()),
                source_url: None,
                audio_path: Some(audio_path.clone()),
            },
        )?;

        // Phase 5: Set idle (short lock)
        *lock(&self.status) = RecordingStatus::Idle;
        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Idle,
                recording_id: None,
            },
        );

        Ok(audio_path)
    }

    pub fn pause(&self, app: &AppHandle) -> Result<(), AppError> {
        {
            let mut status = lock(&self.status);
            if *status != RecordingStatus::Recording {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not recording".into(),
                });
            }
            *status = RecordingStatus::Paused;
        }

        {
            let active = lock(&self.active);
            match active.as_ref() {
                Some(ActiveRecording::Mic(rec)) => rec.pause(),
                #[cfg(target_os = "windows")]
                Some(ActiveRecording::System(cap)) => cap.pause(),
                Some(ActiveRecording::Combined(combined)) => combined.pause(),
                _ => {}
            }
        }

        let rid = lock(&self.recording_id).clone();
        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Paused,
                recording_id: rid,
            },
        );

        Ok(())
    }

    pub fn resume(&self, app: &AppHandle) -> Result<(), AppError> {
        {
            let mut status = lock(&self.status);
            if *status != RecordingStatus::Paused {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not paused".into(),
                });
            }
            *status = RecordingStatus::Recording;
        }

        {
            let active = lock(&self.active);
            match active.as_ref() {
                Some(ActiveRecording::Mic(rec)) => rec.resume(),
                #[cfg(target_os = "windows")]
                Some(ActiveRecording::System(cap)) => cap.resume(),
                Some(ActiveRecording::Combined(combined)) => combined.resume(),
                _ => {}
            }
        }

        let rid = lock(&self.recording_id).clone();
        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Recording,
                recording_id: rid,
            },
        );

        Ok(())
    }

    pub fn get_level(&self) -> RecordingLevelEvent {
        let status = *lock(&self.status);
        let active = lock(&self.active);

        let (level_db, duration_ms) = match active.as_ref() {
            Some(ActiveRecording::Mic(rec)) => (rec.get_level_db(), rec.duration_ms()),
            #[cfg(target_os = "windows")]
            Some(ActiveRecording::System(cap)) => (cap.get_level_db(), cap.duration_ms()),
            Some(ActiveRecording::Combined(combined)) => {
                (combined.get_mic_level_db(), combined.duration_ms())
            }
            None => (-60.0, 0),
        };

        RecordingLevelEvent {
            level_db,
            duration_ms,
            status,
        }
    }

    pub fn status(&self) -> RecordingStatus {
        *lock(&self.status)
    }
}
