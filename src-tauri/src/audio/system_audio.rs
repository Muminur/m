use crate::error::{AppError, AudioErrorCode};
use std::path::PathBuf;

/// System audio capture abstraction.
///
/// Platform support:
/// - Windows: WASAPI loopback via cpal output device (supported)
/// - macOS: ScreenCaptureKit via Swift plugin (future M5.2)
/// - Linux: PulseAudio monitor source via cpal (future)

#[cfg(target_os = "windows")]
pub mod wasapi_loopback {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use hound::{WavSpec, WavWriter};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    pub struct SystemAudioCapture {
        stream: Option<cpal::Stream>,
        writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
        is_recording: Arc<AtomicBool>,
        is_paused: Arc<AtomicBool>,
        sample_count: Arc<Mutex<u64>>,
        level_db: Arc<Mutex<f32>>,
        sample_rate: u32,
        channels: u16,
        output_path: PathBuf,
    }

    impl SystemAudioCapture {
        pub fn new(output_path: PathBuf) -> Result<Self, AppError> {
            let host = cpal::default_host();

            let device = host.default_output_device().ok_or(AppError::AudioError {
                code: AudioErrorCode::DeviceNotFound,
                message: "No default output device for loopback capture".into(),
            })?;

            let config = device
                .default_output_config()
                .map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: format!("Failed to get output config: {}", e),
                })?;

            let sample_rate = config.sample_rate().0;
            let channels = config.channels();

            let spec = WavSpec {
                channels,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };

            let writer =
                WavWriter::create(&output_path, spec).map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: format!("Failed to create WAV file: {}", e),
                })?;

            let writer = Arc::new(Mutex::new(Some(writer)));
            let is_recording = Arc::new(AtomicBool::new(false));
            let is_paused = Arc::new(AtomicBool::new(false));
            let sample_count = Arc::new(Mutex::new(0u64));
            let level_db = Arc::new(Mutex::new(-60.0f32));

            let writer_clone = Arc::clone(&writer);
            let is_recording_clone = Arc::clone(&is_recording);
            let is_paused_clone = Arc::clone(&is_paused);
            let sample_count_clone = Arc::clone(&sample_count);
            let level_db_clone = Arc::clone(&level_db);

            let stream_config: cpal::StreamConfig = config.into();
            let err_fn = |err: cpal::StreamError| {
                tracing::error!("System audio stream error: {}", err);
            };

            let stream = device
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !is_recording_clone.load(Ordering::Relaxed)
                            || is_paused_clone.load(Ordering::Relaxed)
                        {
                            return;
                        }

                        let rms = if data.is_empty() {
                            0.0f32
                        } else {
                            let sum: f64 = data.iter().map(|&s| (s as f64).powi(2)).sum();
                            (sum / data.len() as f64).sqrt() as f32
                        };
                        let db = if rms > 0.0 {
                            20.0 * rms.log10()
                        } else {
                            -60.0
                        };
                        if let Ok(mut level) = level_db_clone.lock() {
                            *level = db.max(-60.0);
                        }

                        if let Ok(mut guard) = writer_clone.lock() {
                            if let Some(ref mut w) = *guard {
                                for &sample in data {
                                    let s16 =
                                        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                    let _ = w.write_sample(s16);
                                }
                            }
                        }

                        if let Ok(mut count) = sample_count_clone.lock() {
                            *count += data.len() as u64;
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: format!("Failed to build loopback stream: {}", e),
                })?;

            Ok(Self {
                stream: Some(stream),
                writer,
                is_recording,
                is_paused,
                sample_count,
                level_db,
                sample_rate,
                channels,
                output_path,
            })
        }

        pub fn start(&self) -> Result<(), AppError> {
            self.is_recording.store(true, Ordering::Relaxed);
            if let Some(ref stream) = self.stream {
                stream.play().map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: format!("Failed to start system audio capture: {}", e),
                })?;
            }
            Ok(())
        }

        pub fn pause(&self) {
            self.is_paused.store(true, Ordering::Relaxed);
        }

        pub fn resume(&self) {
            self.is_paused.store(false, Ordering::Relaxed);
        }

        pub fn get_level_db(&self) -> f32 {
            self.level_db.lock().map(|l| *l).unwrap_or(-60.0)
        }

        pub fn duration_ms(&self) -> u64 {
            let count = self.sample_count.lock().map(|c| *c).unwrap_or(0);
            if self.sample_rate == 0 || self.channels == 0 {
                return 0;
            }
            (count * 1000) / (self.sample_rate as u64 * self.channels as u64)
        }

        pub fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        pub fn channels(&self) -> u16 {
            self.channels
        }

        pub fn stop(mut self) -> Result<PathBuf, AppError> {
            self.is_recording.store(false, Ordering::Relaxed);
            drop(self.stream.take());

            if let Ok(mut guard) = self.writer.lock() {
                if let Some(writer) = guard.take() {
                    writer.finalize().map_err(|e| AppError::AudioError {
                        code: AudioErrorCode::CaptureFailure,
                        message: format!("Failed to finalize system audio WAV: {}", e),
                    })?;
                }
            }

            Ok(self.output_path.clone())
        }
    }
}

/// Check if system audio capture is available on this platform.
pub fn is_system_audio_available() -> bool {
    cfg!(target_os = "windows")
}
