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

    fn as_transcript_source_type(&self) -> &str {
        match self {
            AudioSource::Both => "meeting",
            _ => self.as_db_str(),
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

/// Result of stopping a recording — includes the placeholder transcript id
/// so the frontend can pass it to `transcribe_file` to reuse the same row
/// instead of creating a duplicate.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRecordingResult {
    pub audio_path: String,
    pub transcript_id: String,
    pub recording_id: String,
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
    m.lock()
        .expect("RecordingManager mutex poisoned — audio subsystem in inconsistent state")
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
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
        let recordings_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: "Failed to get app data dir".into(),
            })?
            .join("recordings");

        std::fs::create_dir_all(&recordings_dir)?;

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

        let active_result: Result<ActiveRecording, AppError> = (|| match source {
            AudioSource::Microphone => {
                let device = mic::get_device_by_id(device_id.as_deref())?;
                let path = recordings_dir.join(format!("{}.wav", rec_id));
                let recorder = MicRecorder::new(&device, path)?;
                recorder.start()?;
                Ok(ActiveRecording::Mic(recorder))
            }
            #[cfg(target_os = "windows")]
            AudioSource::System => {
                let path = recordings_dir.join(format!("{}_system.wav", rec_id));
                let capture =
                    crate::audio::system_audio::wasapi_loopback::SystemAudioCapture::new(path)?;
                capture.start()?;
                Ok(ActiveRecording::System(capture))
            }
            #[cfg(not(target_os = "windows"))]
            AudioSource::System => Err(AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: "System audio capture not supported on this platform".into(),
            }),
            AudioSource::Both => {
                let mic_path = recordings_dir.join(format!("{}_mic.wav", rec_id));
                let sys_path = recordings_dir.join(format!("{}_system.wav", rec_id));
                let combined = crate::audio::combined::CombinedCapture::new(
                    device_id.as_deref(),
                    mic_path,
                    sys_path,
                )?;
                combined.start()?;
                Ok(ActiveRecording::Combined(combined))
            }
        })();

        let active = match active_result {
            Ok(a) => a,
            Err(e) => {
                // Reset to Idle — initialization failed, no audio stream was created
                let mut inner = lock(&self.inner);
                inner.status = RecordingStatus::Idle;
                inner.recording_id = None;
                drop(inner);
                publish_status(app, RecordingStatus::Idle, None);
                return Err(e);
            }
        };

        // Store the active recording
        lock(&self.inner).active = Some(active);

        publish_status(app, RecordingStatus::Recording, Some(rec_id.clone()));

        Ok(rec_id)
    }

    pub fn stop(&self, app: &AppHandle) -> Result<StopRecordingResult, AppError> {
        // Phase 1: Atomically check status, set to Stopping, and take ownership of state
        let (active, source, device_id, rec_id) = {
            let mut inner = lock(&self.inner);
            if inner.status != RecordingStatus::Recording && inner.status != RecordingStatus::Paused
            {
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

        publish_status(app, RecordingStatus::Stopping, Some(rec_id.clone()));

        // Phase 2: Stop recording and finalize WAV — NO locks held during I/O
        let capture_result: Result<(String, Option<String>, u64, i64, i64), AppError> = match active
        {
            Some(ActiveRecording::Mic(recorder)) => {
                let dur = recorder.duration_ms();
                let sr = recorder.sample_rate();
                let ch = recorder.channels();
                recorder.stop().map(|path| {
                    (
                        path.to_string_lossy().to_string(),
                        None,
                        dur,
                        sr as i64,
                        ch as i64,
                    )
                })
            }
            #[cfg(target_os = "windows")]
            Some(ActiveRecording::System(capture)) => {
                let dur = capture.duration_ms();
                let sr = capture.sample_rate();
                let ch = capture.channels();
                capture.stop().map(|path| {
                    (
                        path.to_string_lossy().to_string(),
                        None,
                        dur,
                        sr as i64,
                        ch as i64,
                    )
                })
            }
            Some(ActiveRecording::Combined(combined)) => {
                let dur = combined.duration_ms();
                let sr = combined.mic_sample_rate();
                let ch = combined.mic_channels();
                combined.stop().map(|(mic_path, sys_path)| {
                    let sys_str = sys_path.map(|p| p.to_string_lossy().to_string());
                    (
                        mic_path.to_string_lossy().to_string(),
                        sys_str,
                        dur,
                        sr as i64,
                        ch as i64,
                    )
                })
            }
            None => Err(AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: "No active recording".into(),
            }),
        };

        let (audio_path, system_audio_path, duration_ms, sample_rate, channels) =
            match capture_result {
                Ok(result) => result,
                Err(error) => {
                    // A device/finalization failure must not leave the manager
                    // or tray stuck in the Stopping state.
                    let mut inner = lock(&self.inner);
                    inner.status = RecordingStatus::Idle;
                    inner.recording_id = None;
                    drop(inner);
                    publish_status(app, RecordingStatus::Idle, None);
                    return Err(error);
                }
            };

        // Phase 3: Save to database — reset to Idle regardless of success/failure.
        // Returns the placeholder transcript id so callers (e.g. tray "Stop and
        // Transcribe" → transcribe_file) can REUSE this row instead of creating
        // a duplicate.
        let db_result = (|| -> Result<String, AppError> {
            let db = app.state::<Arc<Database>>();
            let mut conn = db.get()?;
            let tx = conn.transaction().map_err(|error| AppError::StorageError {
                code: crate::error::StorageErrorCode::DatabaseError,
                message: format!("Failed to begin recording save: {error}"),
            })?;

            database::recordings::insert(
                &tx,
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
            let transcript_id = database::transcripts::insert(
                &tx,
                &database::transcripts::NewTranscript {
                    title,
                    duration_ms: Some(duration_ms as i64),
                    language: None,
                    model_id: None,
                    source_type: Some(source.as_transcript_source_type().to_string()),
                    source_url: None,
                    audio_path: Some(audio_path.clone()),
                },
            )?;

            // Link the recording row to the placeholder transcript so
            // get-recording-with-transcript queries work later.
            database::recordings::link_transcript(&tx, &rec_id, &transcript_id)?;
            tx.commit().map_err(|error| AppError::StorageError {
                code: crate::error::StorageErrorCode::DatabaseError,
                message: format!("Failed to commit recording save: {error}"),
            })?;

            Ok(transcript_id)
        })();

        // Phase 4: Always reset to Idle — even if DB failed, audio is already saved to disk
        lock(&self.inner).status = RecordingStatus::Idle;
        publish_status(app, RecordingStatus::Idle, None);

        let transcript_id = db_result?;
        Ok(StopRecordingResult {
            audio_path,
            transcript_id,
            recording_id: rec_id,
        })
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

        publish_status(app, RecordingStatus::Paused, rid);

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

        publish_status(app, RecordingStatus::Recording, rid);

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

fn publish_status(app: &AppHandle, status: RecordingStatus, recording_id: Option<String>) {
    let _ = app.emit(
        "recording:status",
        RecordingStatusEvent {
            status,
            recording_id,
        },
    );

    let tray_state = match status {
        RecordingStatus::Recording => crate::tray::TrayState::Recording,
        RecordingStatus::Paused => crate::tray::TrayState::Paused,
        RecordingStatus::Idle | RecordingStatus::Stopping => crate::tray::TrayState::Idle,
    };
    crate::tray::update_tray_state(app, tray_state);
}

#[cfg(test)]
mod tests {
    use super::AudioSource;

    #[test]
    fn combined_capture_uses_valid_transcript_source_type() {
        assert_eq!(AudioSource::Microphone.as_transcript_source_type(), "mic");
        assert_eq!(AudioSource::System.as_transcript_source_type(), "system");
        assert_eq!(AudioSource::Both.as_db_str(), "both");
        assert_eq!(AudioSource::Both.as_transcript_source_type(), "meeting");
    }
}
