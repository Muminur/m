//! Tauri commands for the live caption pipeline.
//!
//! `start_captions` opens a microphone input stream, feeds audio into
//! `StreamingTranscriber`, and emits `caption:segment` events to the frontend
//! as each segment is recognised. `stop_captions` tears the pipeline down.
//!
//! # Threading model
//!
//! `cpal::Stream` is `!Send` on Windows (WASAPI uses a `PhantomData<*mut ()>`
//! marker).  To keep it on the thread that created it we build *and* play the
//! stream inside the dedicated OS thread that also runs the drain loop.  A
//! `std::sync::mpsc::sync_channel` carries `Vec<f32>` chunks from the cpal
//! callback (which runs on the same thread) to the drain loop that feeds
//! `StreamingTranscriber`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::SampleFormat;
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AudioErrorCode, TranscriptionErrorCode};
use crate::transcription::engine::SegmentResult;
use crate::transcription::streaming::{InferenceProvider, StreamingConfig, StreamingTranscriber};

// ─── No-op inference provider ────────────────────────────────────────────────
//
// Used when no model path is supplied (UI preview / testing). Returns an empty
// segment list so the transcriber pipeline exercises correctly without a real
// Whisper model loaded.

struct NoopInference;

impl InferenceProvider for NoopInference {
    fn infer(&self, _pcm_window: &[f32]) -> Result<Vec<SegmentResult>, AppError> {
        Ok(vec![])
    }
}

// ─── Managed state ────────────────────────────────────────────────────────────

/// Per-session caption state held in Tauri's state map.
pub struct CaptionsState {
    inner: Mutex<Option<CaptionsSession>>,
}

impl CaptionsState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

impl Default for CaptionsState {
    fn default() -> Self {
        Self::new()
    }
}

struct CaptionsSession {
    stop_flag: Arc<AtomicBool>,
    /// Handle to the OS thread that owns the cpal stream and drain loop.
    thread: Option<std::thread::JoinHandle<()>>,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Start live captions from a microphone input device.
///
/// Audio is captured via cpal, converted to mono f32 at the device's native
/// sample rate, and fed into `StreamingTranscriber`. Each caption segment
/// produced is emitted as a `caption:segment` event on the Tauri event bus.
///
/// If captions are already running the command returns an error rather than
/// starting a second session.
#[tauri::command]
pub async fn start_captions(
    app: AppHandle,
    state: State<'_, CaptionsState>,
    device_id: Option<String>,
) -> Result<(), AppError> {
    // Guard: only one session at a time.
    {
        let guard = state
            .inner
            .lock()
            .map_err(|_| AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Caption state lock poisoned".into(),
            })?;
        if guard.is_some() {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Captions already running".into(),
            });
        }
    }

    // Resolve the device *name* now (on the calling thread, which is fine) so
    // the worker thread can look it up by name without needing to send the
    // Device handle across threads.
    let device_name: Option<String> = {
        use cpal::traits::HostTrait;
        let host = cpal::default_host();
        match device_id.as_deref() {
            None | Some("") => None,
            Some(id) => {
                // Re-use the same index/name resolution logic as get_device_by_id
                // but capture just the name so we can re-open on the worker thread.
                let devices: Vec<cpal::Device> = host
                    .input_devices()
                    .map_err(|e| AppError::AudioError {
                        code: AudioErrorCode::DeviceNotFound,
                        message: format!("Failed to enumerate devices: {}", e),
                    })?
                    .collect();

                let mut found: Option<String> = None;

                // Index-based lookup (e.g. "input_0")
                if let Some(idx_str) = id.strip_prefix("input_") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        found = devices.get(idx).and_then(|d| d.name().ok());
                    }
                }

                // Fallback: treat the id itself as a device name
                if found.is_none() {
                    found = devices
                        .iter()
                        .find(|d| d.name().ok().as_deref() == Some(id))
                        .and_then(|d| d.name().ok());
                }

                found
            }
        }
    };

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = Arc::clone(&stop_flag);
    let app_thread = app.clone();

    // Spawn a plain OS thread.  The cpal::Stream (which is !Send on Windows)
    // is constructed *inside* this thread so it never crosses a thread
    // boundary.
    let thread = std::thread::spawn(move || {
        run_caption_thread(app_thread, stop_flag_thread, device_name);
    });

    // Store the session.
    let mut guard = state
        .inner
        .lock()
        .map_err(|_| AppError::TranscriptionError {
            code: TranscriptionErrorCode::InferenceFailure,
            message: "Caption state lock poisoned".into(),
        })?;
    *guard = Some(CaptionsSession {
        stop_flag,
        thread: Some(thread),
    });

    tracing::info!("Captions started (device_id={:?})", device_id);
    Ok(())
}

/// Stop live captions and tear down the audio pipeline.
///
/// Safe to call when captions are not running — returns `Ok(())` in that case.
#[tauri::command]
pub async fn stop_captions(state: State<'_, CaptionsState>) -> Result<(), AppError> {
    let session = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Caption state lock poisoned".into(),
            })?;
        guard.take()
    };

    if let Some(mut s) = session {
        s.stop_flag.store(true, Ordering::Relaxed);
        if let Some(thread) = s.thread.take() {
            let _ = thread.join();
        }
        tracing::info!("Captions stopped");
    }

    Ok(())
}

// ─── Worker thread ────────────────────────────────────────────────────────────

/// Runs entirely on one OS thread: opens the cpal stream, drains audio chunks
/// through `StreamingTranscriber`, and emits events.
fn run_caption_thread(app: AppHandle, stop_flag: Arc<AtomicBool>, device_name: Option<String>) {
    use cpal::traits::HostTrait;

    let host = cpal::default_host();

    // Open the device by name (or default).
    let device = match device_name {
        Some(ref name) => {
            let found = host
                .input_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(name.as_str())));
            match found {
                Some(d) => d,
                None => {
                    tracing::warn!("Caption device '{}' not found, using default", name);
                    match host.default_input_device() {
                        Some(d) => d,
                        None => {
                            tracing::error!("No default input device for captions");
                            return;
                        }
                    }
                }
            }
        }
        None => match host.default_input_device() {
            Some(d) => d,
            None => {
                tracing::error!("No default input device for captions");
                return;
            }
        },
    };

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get input config for captions: {}", e);
            return;
        }
    };

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let streaming_config = StreamingConfig {
        sample_rate,
        vad_enabled: false,
        ..StreamingConfig::default()
    };

    // Channel from cpal callback to drain loop — both live on this thread.
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);

    let stop_stream = Arc::clone(&stop_flag);
    let stream_config: cpal::StreamConfig = config.clone().into();
    let err_fn = |e: cpal::StreamError| {
        tracing::error!("Caption audio stream error: {}", e);
    };

    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            let stop = Arc::clone(&stop_stream);
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let mono = to_mono_f32(data, channels);
                    let _ = tx.try_send(mono);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let stop = Arc::clone(&stop_stream);
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let f32_data: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    let mono = to_mono_f32(&f32_data, channels);
                    let _ = tx.try_send(mono);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U8 => {
            let stop = Arc::clone(&stop_stream);
            device.build_input_stream(
                &stream_config,
                move |data: &[u8], _: &cpal::InputCallbackInfo| {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let f32_data: Vec<f32> =
                        data.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect();
                    let mono = to_mono_f32(&f32_data, channels);
                    let _ = tx.try_send(mono);
                },
                err_fn,
                None,
            )
        }
        fmt => {
            tracing::error!("Unsupported sample format for captions: {:?}", fmt);
            return;
        }
    };

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to build caption input stream: {}", e);
            return;
        }
    };

    if let Err(e) = stream.play() {
        tracing::error!("Failed to start caption audio stream: {}", e);
        return;
    }

    let mut transcriber = match StreamingTranscriber::new(streaming_config) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to create StreamingTranscriber: {}", e);
            return;
        }
    };
    let inference = NoopInference;

    // Drain loop — runs on this thread alongside the cpal callbacks.
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        // Block briefly on the first chunk, then drain the rest.
        let first = match rx.recv_timeout(std::time::Duration::from_millis(20)) {
            Ok(chunk) => chunk,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        let mut flat = first;
        loop {
            match rx.try_recv() {
                Ok(chunk) => flat.extend(chunk),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    stop_flag.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        match transcriber.push_audio(&flat, &inference) {
            Ok(segments) => {
                for segment in segments {
                    let _ = app.emit("caption:segment", &segment);
                }
            }
            Err(e) => {
                tracing::warn!("StreamingTranscriber error: {}", e);
                break;
            }
        }
    }

    // `stream` is dropped here, stopping audio capture.
    tracing::debug!("Caption drain thread exiting");
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Downmix multi-channel f32 audio to mono by averaging channels.
fn to_mono_f32(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_mono_f32_passthrough_mono() {
        let samples = vec![0.1, 0.2, 0.3];
        assert_eq!(to_mono_f32(&samples, 1), samples);
    }

    #[test]
    fn test_to_mono_f32_stereo_averages() {
        // Two stereo frames: [L=0.0, R=1.0] and [L=0.5, R=0.5]
        let samples = vec![0.0f32, 1.0, 0.5, 0.5];
        let mono = to_mono_f32(&samples, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_captions_state_default_is_none() {
        let s = CaptionsState::default();
        let guard = s.inner.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn test_noop_inference_returns_empty() {
        let provider = NoopInference;
        let result = provider.infer(&[0.1; 100]).unwrap();
        assert!(result.is_empty());
    }
}
