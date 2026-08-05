use crate::error::{AppError, AudioErrorCode};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::path::Path;

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
                "Unsupported audio format: {:?}. Supported: mp3, wav, m4a, flac, ogg, oga",
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
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
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
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
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(1);

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
            message: format!("Audio too short ({}ms). Minimum is 100ms.", duration_ms),
        });
    }

    log_pcm_stats("decoded raw (interleaved)", &all_samples);

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
        channels,
        duration_ms,
    })
}

/// Resample and convert to mono 16kHz f32 PCM for whisper-rs.
///
/// Whisper was trained on properly resampled 16kHz mono audio. Feeding it
/// audio that's been downsampled with naive linear interpolation (which has
/// NO anti-aliasing filter) yields garbage transcripts — frequencies above
/// the new Nyquist (8kHz) fold back into the speech band as aliasing noise.
/// We use rubato's sinc-windowed resampler which includes a proper low-pass
/// anti-alias filter.
pub fn resample_to_whisper(decoded: &DecodedAudio) -> Result<Vec<f32>, AppError> {
    // Step 1: Convert to mono by averaging channels
    let mono: Vec<f32> = if decoded.channels == 1 {
        decoded.samples.clone()
    } else {
        decoded
            .samples
            .chunks(decoded.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    log_pcm_stats("post-mono mix", &mono);

    // Step 2: Resample to 16000 Hz if needed (skip if already 16kHz)
    const TARGET_RATE: u32 = 16000;
    if decoded.sample_rate == TARGET_RATE {
        log_pcm_stats("resample_to_whisper (no resample)", &mono);
        return Ok(mono);
    }

    let mut resampled = resample_sinc(&mono, decoded.sample_rate, TARGET_RATE)?;
    // Sinc interpolation can ring/overshoot past [-1, 1] (Gibbs phenomenon).
    // Whisper expects normalized samples in [-1, 1]; out-of-range values can
    // degrade transcription. Hard-clip to be safe — also report how many
    // samples were clipped so we can spot pathological input later.
    let mut clipped = 0usize;
    for s in &mut resampled {
        if *s > 1.0 {
            *s = 1.0;
            clipped += 1;
        } else if *s < -1.0 {
            *s = -1.0;
            clipped += 1;
        }
    }
    if clipped > 0 {
        tracing::info!(
            target = "audio.pcm",
            "resample_to_whisper: clipped {} samples to [-1, 1]",
            clipped
        );
    }
    log_pcm_stats("resample_to_whisper (sinc, clamped)", &resampled);

    // Peak-normalize quiet recordings. Whisper's mel-spectrogram is somewhat
    // level-invariant but very quiet input (peak < -6 dBFS) consistently
    // transcribes worse than properly-leveled audio. Real-world evidence:
    // recordings with peak -12 to -18 dBFS produce 1 garbage segment per
    // 10s of speech. Lift the peak to ~-1 dBFS (0.9) when safe.
    normalize_peak(&mut resampled);
    log_pcm_stats("resample_to_whisper (normalized)", &resampled);

    Ok(resampled)
}

/// If the peak amplitude is below a comfortable level, scale the whole
/// buffer up so the peak hits ~-1 dBFS. Does nothing if peak is already
/// above the target (we never reduce — that risks pumping the noise floor).
fn normalize_peak(pcm: &mut [f32]) {
    if pcm.is_empty() {
        return;
    }
    const TARGET_PEAK: f32 = 0.9; // ~-0.9 dBFS, safely below clipping
    const MIN_PEAK_TO_BOOST: f32 = 0.001; // below this is silence; don't amplify noise

    let mut peak: f32 = 0.0;
    for &s in pcm.iter() {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }

    if peak < MIN_PEAK_TO_BOOST {
        tracing::warn!(
            target = "audio.pcm",
            "normalize_peak: input is effectively silent (peak={:.6}) — skipping normalization. \
             Check microphone permission and device selection.",
            peak
        );
        return;
    }

    if peak >= TARGET_PEAK {
        // Already at a healthy level (or louder); don't change.
        return;
    }

    let gain = TARGET_PEAK / peak;
    tracing::info!(
        target = "audio.pcm",
        "normalize_peak: peak={:.4} -> gain {:.2}x (target peak {:.2})",
        peak,
        gain,
        TARGET_PEAK
    );
    for s in pcm.iter_mut() {
        *s *= gain;
    }
}

/// High-quality resampling via rubato's windowed-sinc interpolator with a
/// built-in anti-alias low-pass filter. Input is mono f32 in `source_rate`,
/// output is mono f32 in `target_rate`.
fn resample_sinc(input: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, AppError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // Process in moderate chunks so we don't allocate huge intermediate
    // buffers for long recordings.
    let chunk_size: usize = 1024;
    let resample_ratio = target_rate as f64 / source_rate as f64;

    let mut resampler = SincFixedIn::<f32>::new(
        resample_ratio,
        2.0,
        params,
        chunk_size,
        1, // mono after step 1
    )
    .map_err(|e| AppError::AudioError {
        code: AudioErrorCode::InvalidAudioFormat,
        message: format!("Failed to construct rubato resampler: {}", e),
    })?;

    let mut output: Vec<f32> = Vec::with_capacity((input.len() as f64 * resample_ratio) as usize);
    let mut idx = 0usize;

    while idx < input.len() {
        let needed = resampler.input_frames_next();
        let remaining = input.len() - idx;
        let take = remaining.min(needed);

        // Build the chunk (zero-pad the tail of the final chunk if short)
        let mut chunk = Vec::with_capacity(needed);
        chunk.extend_from_slice(&input[idx..idx + take]);
        if chunk.len() < needed {
            chunk.resize(needed, 0.0);
        }

        let waves_out = resampler
            .process(&[chunk], None)
            .map_err(|e| AppError::AudioError {
                code: AudioErrorCode::InvalidAudioFormat,
                message: format!("Resampler process failed: {}", e),
            })?;

        if remaining < needed {
            // Final partial chunk — trim padding from output.
            let expected = (take as f64 * resample_ratio).round() as usize;
            let out = &waves_out[0];
            output.extend_from_slice(&out[..expected.min(out.len())]);
            break;
        } else {
            output.extend_from_slice(&waves_out[0]);
            idx += needed;
        }
    }

    Ok(output)
}

/// Log basic stats about a PCM buffer so we can spot silent / saturated /
/// corrupted audio before it reaches whisper.
fn log_pcm_stats(tag: &str, pcm: &[f32]) {
    if pcm.is_empty() {
        tracing::warn!(target = "audio.pcm", "{}: EMPTY pcm buffer", tag);
        return;
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sumsq = 0.0_f64;
    let mut nan_count = 0usize;
    for &s in pcm {
        if s.is_nan() {
            nan_count += 1;
            continue;
        }
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
        sumsq += (s as f64) * (s as f64);
    }
    let rms = (sumsq / pcm.len() as f64).sqrt();
    tracing::info!(
        target = "audio.pcm",
        "{}: samples={} min={:.4} max={:.4} rms={:.4} nan={}",
        tag,
        pcm.len(),
        min,
        max,
        rms,
        nan_count
    );
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
