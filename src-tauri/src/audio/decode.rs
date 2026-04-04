use std::path::Path;
use crate::error::{AppError, AudioErrorCode};

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
}

/// Supported audio formats. WMA intentionally excluded — symphonia does not support WMA.
pub fn is_supported_format(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("mp3") | Some("wav") | Some("m4a") | Some("flac") | Some("ogg") | Some("oga")
    )
}

pub fn decode_file(path: &Path) -> Result<DecodedAudio, AppError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    if !is_supported_format(path) {
        return Err(AppError::AudioError {
            code: AudioErrorCode::UnsupportedFormat,
            message: format!(
                "Unsupported audio format: {:?}. Supported: mp3, wav, m4a, flac, ogg",
                path.extension().and_then(|e| e.to_str()).unwrap_or("unknown")
            ),
        });
    }

    let file = std::fs::File::open(path).map_err(|e| AppError::AudioError {
        code: AudioErrorCode::DecodeFailure,
        message: format!("Failed to open audio file: {}", e),
    })?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| AppError::AudioError {
            code: AudioErrorCode::DecodeFailure,
            message: format!("Failed to probe audio format: {}", e),
        })?;

    let mut format = probed.format;

    // Select the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::AudioError {
            code: AudioErrorCode::DecodeFailure,
            message: "No audio tracks found in file".into(),
        })?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AppError::AudioError {
            code: AudioErrorCode::DecodeFailure,
            message: format!("Failed to create audio decoder: {}", e),
        })?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::ResetRequired) => break,
            Err(e) => {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::DecodeFailure,
                    message: format!("Failed to read audio packet: {}", e),
                });
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sample_buf.copy_interleaved_ref(decoded);
                all_samples.extend_from_slice(sample_buf.samples());
            }
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::DecodeError(e)) => {
                tracing::warn!("Decode error (skipping frame): {}", e);
                continue;
            }
            Err(e) => {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::DecodeFailure,
                    message: format!("Decode error: {}", e),
                });
            }
        }
    }

    let duration_ms = if sample_rate > 0 && channels > 0 {
        let total_samples_per_channel = all_samples.len() as u64 / channels as u64;
        (total_samples_per_channel * 1000) / sample_rate as u64
    } else {
        0
    };

    // Zero-length validation
    if duration_ms < 100 {
        return Err(AppError::AudioError {
            code: AudioErrorCode::InvalidAudioFormat,
            message: format!(
                "Audio too short ({}ms). Minimum is 100ms.",
                duration_ms
            ),
        });
    }

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
        channels,
        duration_ms,
    })
}

/// Resample and convert to mono 16kHz f32 PCM for whisper-rs.
pub fn resample_to_whisper(decoded: &DecodedAudio) -> Result<Vec<f32>, AppError> {
    // Step 1: Convert to mono
    let mono: Vec<f32> = if decoded.channels == 1 {
        decoded.samples.clone()
    } else {
        decoded
            .samples
            .chunks(decoded.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    // Step 2: Resample to 16000 Hz if needed
    const TARGET_RATE: u32 = 16000;
    if decoded.sample_rate == TARGET_RATE {
        return Ok(mono);
    }

    let ratio = TARGET_RATE as f64 / decoded.sample_rate as f64;
    let output_len = (mono.len() as f64 * ratio) as usize;
    let mut resampled = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let s0 = mono.get(src_idx).copied().unwrap_or(0.0);
        let s1 = mono.get(src_idx + 1).copied().unwrap_or(s0);
        resampled.push(s0 + (s1 - s0) * frac as f32);
    }

    Ok(resampled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_supported_formats() {
        assert!(is_supported_format(&PathBuf::from("test.mp3")));
        assert!(is_supported_format(&PathBuf::from("test.wav")));
        assert!(is_supported_format(&PathBuf::from("test.m4a")));
        assert!(is_supported_format(&PathBuf::from("test.flac")));
        assert!(is_supported_format(&PathBuf::from("test.ogg")));
        assert!(is_supported_format(&PathBuf::from("test.oga")));
    }

    #[test]
    fn test_wma_not_supported() {
        assert!(!is_supported_format(&PathBuf::from("test.wma")));
    }

    #[test]
    fn test_unknown_extension_not_supported() {
        assert!(!is_supported_format(&PathBuf::from("test.xyz")));
        assert!(!is_supported_format(&PathBuf::from("test.doc")));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(is_supported_format(&PathBuf::from("test.MP3")));
        assert!(is_supported_format(&PathBuf::from("test.WAV")));
        assert!(!is_supported_format(&PathBuf::from("test.WMA")));
    }

    #[test]
    fn test_decode_nonexistent_file() {
        let result = decode_file(&PathBuf::from("/nonexistent/audio.mp3"));
        assert!(result.is_err());
    }

    #[test]
    fn test_resample_noop_at_16khz() {
        let audio = DecodedAudio {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            sample_rate: 16000,
            channels: 1,
            duration_ms: 250,
        };
        let result = resample_to_whisper(&audio).unwrap();
        assert_eq!(result, audio.samples);
    }

    #[test]
    fn test_mono_conversion() {
        // Stereo: L=1.0, R=0.0 for each frame -> mono should average to 0.5
        let audio = DecodedAudio {
            samples: vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
            sample_rate: 16000,
            channels: 2,
            duration_ms: 250,
        };
        let result = resample_to_whisper(&audio).unwrap();
        assert_eq!(result.len(), 4);
        for &s in &result {
            assert!((s - 0.5).abs() < 1e-6);
        }
    }
}
