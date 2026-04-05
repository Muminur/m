use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use crate::error::{AppError, AudioErrorCode};

/// Maximum recording duration: 4 hours in seconds.
const MAX_RECORDING_DURATION_SECS: u64 = 4 * 60 * 60;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_input: bool,
}

pub fn list_input_devices() -> Result<Vec<AudioDeviceInfo>, AppError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut devices = Vec::new();
    let input_devices = host.input_devices().map_err(|e| AppError::AudioError {
        code: AudioErrorCode::DeviceNotFound,
        message: format!("Failed to enumerate input devices: {}", e),
    })?;

    for (idx, device) in input_devices.enumerate() {
        let name = device.name().unwrap_or_else(|_| format!("Device {}", idx));
        devices.push(AudioDeviceInfo {
            id: format!("input_{}", idx),
            name: name.clone(),
            is_default: name == default_name,
            is_input: true,
        });
    }

    Ok(devices)
}

pub fn get_device_by_id(device_id: Option<&str>) -> Result<Device, AppError> {
    let host = cpal::default_host();

    if let Some(id) = device_id {
        if !id.is_empty() {
            let devices: Vec<Device> = host
                .input_devices()
                .map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::DeviceNotFound,
                    message: format!("Failed to enumerate devices: {}", e),
                })?
                .collect();

            // Try index-based lookup first, then fall back to name matching
            let mut index_match: Option<usize> = None;
            if let Some(idx_str) = id.strip_prefix("input_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if idx < devices.len() {
                        index_match = Some(idx);
                    }
                }
            }

            if let Some(idx) = index_match {
                return Ok(devices.into_iter().nth(idx).unwrap());
            }

            // Fallback: match by device name (handles hot-plug index shifts)
            for device in devices {
                if device.name().ok().as_deref() == Some(id) {
                    return Ok(device);
                }
            }

            tracing::warn!("Device '{}' not found, falling back to default", id);
        }
    }

    host.default_input_device().ok_or(AppError::AudioError {
        code: AudioErrorCode::DeviceNotFound,
        message: "No default input device available".into(),
    })
}

pub struct MicRecorder {
    stream: Option<cpal::Stream>,
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    is_paused: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    sample_count: Arc<Mutex<u64>>,
    level_db: Arc<Mutex<f32>>,
    sample_rate: u32,
    channels: u16,
    output_path: PathBuf,
}

impl MicRecorder {
    pub fn new(device: &Device, output_path: PathBuf) -> Result<Self, AppError> {
        let config = device
            .default_input_config()
            .map_err(|e| AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: format!("Failed to get default input config: {}", e),
            })?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = WavWriter::create(&output_path, spec).map_err(|e| AppError::AudioError {
            code: AudioErrorCode::CaptureFailure,
            message: format!("Failed to create WAV file: {}", e),
        })?;

        let writer = Arc::new(Mutex::new(Some(writer)));
        let is_paused = Arc::new(AtomicBool::new(false));
        let is_recording = Arc::new(AtomicBool::new(false));
        let sample_count = Arc::new(Mutex::new(0u64));
        let level_db = Arc::new(Mutex::new(-60.0f32));

        let max_samples = MAX_RECORDING_DURATION_SECS * sample_rate as u64 * channels as u64;

        let writer_clone = Arc::clone(&writer);
        let is_paused_clone = Arc::clone(&is_paused);
        let is_recording_clone = Arc::clone(&is_recording);
        let sample_count_clone = Arc::clone(&sample_count);
        let level_db_clone = Arc::clone(&level_db);

        let stream_config: StreamConfig = config.clone().into();

        let err_fn = |err: cpal::StreamError| {
            tracing::error!("Audio stream error: {}", err);
        };

        let stream = match config.sample_format() {
            SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if !is_recording_clone.load(Ordering::Relaxed)
                        || is_paused_clone.load(Ordering::Relaxed)
                    {
                        return;
                    }

                    // Calculate RMS level
                    let rms = if data.is_empty() {
                        0.0f32
                    } else {
                        let sum: f64 = data.iter().map(|&s| (s as f64).powi(2)).sum();
                        (sum / data.len() as f64).sqrt() as f32
                    };
                    let db = if rms > 0.0 {
                        20.0 * (rms / i16::MAX as f32).log10()
                    } else {
                        -60.0
                    };
                    if let Ok(mut level) = level_db_clone.lock() {
                        *level = db.max(-60.0);
                    }

                    // Check duration limit before writing
                    let current = sample_count_clone.lock().map(|c| *c).unwrap_or(0);
                    if current >= max_samples {
                        return;
                    }

                    // Write samples
                    if let Ok(mut guard) = writer_clone.lock() {
                        if let Some(ref mut w) = *guard {
                            for &sample in data {
                                let _ = w.write_sample(sample);
                            }
                        }
                    }

                    if let Ok(mut count) = sample_count_clone.lock() {
                        *count += data.len() as u64;
                    }
                },
                err_fn,
                None,
            ),
            SampleFormat::F32 => device.build_input_stream(
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

                    // Check duration limit before writing
                    let current = sample_count_clone.lock().map(|c| *c).unwrap_or(0);
                    if current >= max_samples {
                        return;
                    }

                    if let Ok(mut guard) = writer_clone.lock() {
                        if let Some(ref mut w) = *guard {
                            for &sample in data {
                                let s16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
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
            ),
            format => {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::UnsupportedFormat,
                    message: format!("Unsupported sample format: {:?}", format),
                });
            }
        }
        .map_err(|e| AppError::AudioError {
            code: AudioErrorCode::CaptureFailure,
            message: format!("Failed to build input stream: {}", e),
        })?;

        Ok(Self {
            stream: Some(stream),
            writer,
            is_paused,
            is_recording,
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
                message: format!("Failed to start recording: {}", e),
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

        // Drop the stream to stop capturing
        drop(self.stream.take());

        // Finalize the WAV file
        if let Ok(mut guard) = self.writer.lock() {
            if let Some(writer) = guard.take() {
                writer.finalize().map_err(|e| AppError::AudioError {
                    code: AudioErrorCode::CaptureFailure,
                    message: format!("Failed to finalize WAV file: {}", e),
                })?;
            }
        }

        Ok(self.output_path.clone())
    }
}
