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

struct RecordingManagerInner {
    status: RecordingStatus,
    active: Option<ActiveRecording>,
    recording_id: Option<String>,
    source: AudioSource,
    device_id: Option<String>,
}

// Safety: RecordingManager protects all access to RecordingManagerInner through
// a single std::sync::Mutex, ensuring atomic state transitions. cpal::Stream
// is !Send because platform audio backends (WASAPI/COM on Windows, CoreAudio
// on macOS) may use thread-local state. In practice, Tauri command handlers run
// on a tokio thread pool and the Mutex ensures exclusive access. cpal internally
// initializes COM per-thread on Windows. The stream callbacks run on cpal's own
// audio thread, not through the Mutex. We only call play()/pause() and drop
// through the Mutex, which are safe across threads in cpal's WASAPI backend.
unsafe impl Send for RecordingManager {}
unsafe impl Sync for RecordingManager {}

pub struct RecordingManager {
    inner: Mutex<RecordingManagerInner>,
}

fn lock(m: &Mutex<RecordingManagerInner>) -> std::sync::MutexGuard<'_, RecordingManagerInner> {
    m.lock().expect("RecordingManager mutex poisoned — audio subsystem in inconsistent state")
}

impl RecordingManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RecordingManagerInner {
                status: RecordingStatus::Idle,
                active: None,
                recording_id: None,
                source: AudioSource::Microphone,
                device_id: None,
            }),
        }
    }

    pub fn start(
        &self,
        app: &AppHandle,
        source: AudioSource,
        device_id: Option<String>,
    ) -> Result<String, AppError> {
        let rec_id = {
            let mut inner = lock(&self.inner);
            if inner.status != RecordingStatus::Idle {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Already recording".into(),
                });
            }
            // Atomically claim the Recording state to prevent double-start
            let rid = Uuid::new_v4().to_string();
            inner.status = RecordingStatus::Recording;
            inner.recording_id = Some(rid.clone());
            inner.source = source.clone();
            inner.device_id = device_id.clone();
            rid
        };

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
                // Reset state before returning error
                let mut inner = lock(&self.inner);
                inner.status = RecordingStatus::Idle;
                inner.recording_id = None;
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

        // Store the active recording
        lock(&self.inner).active = Some(active);

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
        // Phase 1: Atomically check status, set to Stopping, and take ownership of state
        let (active, source, device_id, rec_id) = {
            let mut inner = lock(&self.inner);
            if inner.status != RecordingStatus::Recording && inner.status != RecordingStatus::Paused {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not recording".into(),
                });
            }
            inner.status = RecordingStatus::Stopping;
            (
                inner.active.take(),
                inner.source.clone(),
                inner.device_id.clone(),
                inner.recording_id.take().unwrap_or_default(),
            )
        };

        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Stopping,
                recording_id: Some(rec_id.clone()),
            },
        );

        // Phase 2: Stop recording and finalize WAV — NO locks held during I/O
        let (audio_path, system_audio_path, duration_ms, sample_rate, channels) = match active {
            Some(ActiveRecording::Mic(recorder)) => {
                let dur = recorder.duration_ms();
                let sr = recorder.sample_rate();
                let ch = recorder.channels();
                let path = recorder.stop()?;
                (path.to_string_lossy().to_string(), None, dur, sr as i64, ch as i64)
            }
            #[cfg(target_os = "windows")]
            Some(ActiveRecording::System(capture)) => {
                let dur = capture.duration_ms();
                let sr = capture.sample_rate();
                let ch = capture.channels();
                let path = capture.stop()?;
                (path.to_string_lossy().to_string(), None, dur, sr as i64, ch as i64)
            }
            Some(ActiveRecording::Combined(combined)) => {
                let dur = combined.duration_ms();
                let sr = combined.mic_sample_rate();
                let ch = combined.mic_channels();
                let (mic_path, sys_path) = combined.stop()?;
                let sys_str = sys_path.map(|p| p.to_string_lossy().to_string());
                (mic_path.to_string_lossy().to_string(), sys_str, dur, sr as i64, ch as i64)
            }
            None => {
                lock(&self.inner).status = RecordingStatus::Idle;
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "No active recording".into(),
                });
            }
        };

        // Phase 3: Save to database — reset to Idle regardless of success/failure
        let db_result = (|| -> Result<(), AppError> {
            let db = app.state::<Arc<Database>>();
            let conn = db.get()?;

            database::recordings::insert(
                &conn,
                &rec_id,
                source.as_db_str(),
                device_id.as_deref(),
                None,
                &audio_path,
                system_audio_path.as_deref(),
                duration_ms as i64,
                sample_rate,
                channels,
            )?;

            let title = format!(
                "Recording {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M")
            );
            database::transcripts::insert(
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

            Ok(())
        })();

        // Phase 4: Always reset to Idle — even if DB failed, audio is already saved to disk
        lock(&self.inner).status = RecordingStatus::Idle;
        let _ = app.emit(
            "recording:status",
            RecordingStatusEvent {
                status: RecordingStatus::Idle,
                recording_id: None,
            },
        );

        db_result?;
        Ok(audio_path)
    }

    pub fn pause(&self, app: &AppHandle) -> Result<(), AppError> {
        let rid = {
            let mut inner = lock(&self.inner);
            if inner.status != RecordingStatus::Recording {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not recording".into(),
                });
            }
            inner.status = RecordingStatus::Paused;

            match inner.active.as_ref() {
                Some(ActiveRecording::Mic(rec)) => rec.pause(),
                #[cfg(target_os = "windows")]
                Some(ActiveRecording::System(cap)) => cap.pause(),
                Some(ActiveRecording::Combined(combined)) => combined.pause(),
                _ => {}
            }

            inner.recording_id.clone()
        };

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
        let rid = {
            let mut inner = lock(&self.inner);
            if inner.status != RecordingStatus::Paused {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: "Not paused".into(),
                });
            }
            inner.status = RecordingStatus::Recording;

            match inner.active.as_ref() {
                Some(ActiveRecording::Mic(rec)) => rec.resume(),
                #[cfg(target_os = "windows")]
                Some(ActiveRecording::System(cap)) => cap.resume(),
                Some(ActiveRecording::Combined(combined)) => combined.resume(),
                _ => {}
            }

            inner.recording_id.clone()
        };

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
        let inner = lock(&self.inner);

        let (level_db, duration_ms) = match inner.active.as_ref() {
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
            status: inner.status,
        }
    }

    pub fn status(&self) -> RecordingStatus {
        lock(&self.inner).status
    }
}
