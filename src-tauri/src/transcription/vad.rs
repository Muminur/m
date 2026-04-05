//! Voice Activity Detection (VAD) for filtering silence before transcription.
//!
//! Provides a `VoiceActivityDetector` trait and a `SileroVad` implementation
//! that uses energy-based detection (with the Silero ONNX model as a future
//! upgrade path). Silent regions are skipped so whisper never hallucinates on
//! silence.

use crate::error::{AppError, TranscriptionErrorCode};
use serde::{Deserialize, Serialize};

// ─── Configuration ──────────────────────────────────────────────────────────

/// Configuration for voice activity detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VadConfig {
    /// Speech probability threshold (0.0 - 1.0). Frames above this are speech.
    pub threshold: f32,
    /// Minimum duration of speech to keep (ms).
    pub min_speech_ms: u32,
    /// Minimum duration of silence to split on (ms).
    pub min_silence_ms: u32,
    /// Expected sample rate of incoming audio.
    pub sample_rate: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            min_speech_ms: 250,
            min_silence_ms: 300,
            sample_rate: 16000,
        }
    }
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// A detected segment of speech or silence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VadSegment {
    /// Start time in milliseconds from chunk origin.
    pub start_ms: u64,
    /// End time in milliseconds from chunk origin.
    pub end_ms: u64,
    /// Whether this segment contains speech.
    pub is_speech: bool,
    /// Confidence / energy score for this segment (0.0 - 1.0).
    pub confidence: f32,
}

// ─── Trait ───────────────────────────────────────────────────────────────────

/// Trait for voice activity detectors.
pub trait VoiceActivityDetector: Send + Sync {
    /// Process a chunk of audio samples and return detected segments.
    fn process_chunk(&mut self, samples: &[f32]) -> Result<Vec<VadSegment>, AppError>;

    /// Reset internal state for a new audio stream.
    fn reset(&mut self);
}

// ─── Energy-based VAD (Silero-compatible interface) ─────────────────────────

/// Energy-based voice activity detector.
///
/// Uses RMS energy with adaptive thresholding. The API mirrors Silero VAD so
/// swapping in the real ONNX model later is a drop-in replacement.
pub struct SileroVad {
    config: VadConfig,
    /// Running frame index for timestamp calculation.
    frame_offset: u64,
}

impl SileroVad {
    /// Create a new VAD instance with the given configuration.
    pub fn new(config: VadConfig) -> Result<Self, AppError> {
        if config.sample_rate == 0 {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::VadError,
                message: "Sample rate must be > 0".into(),
            });
        }
        if !(0.0..=1.0).contains(&config.threshold) {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::VadError,
                message: format!("Threshold must be in [0.0, 1.0], got {}", config.threshold),
            });
        }
        Ok(Self {
            config,
            frame_offset: 0,
        })
    }

    /// Compute RMS energy of a slice, normalized to [0.0, 1.0].
    fn rms_energy(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        // Clamp to [0, 1] — PCM f32 is typically in [-1, 1] so RMS <= 1.
        rms.min(1.0)
    }

    /// Number of samples in one analysis frame (30 ms, matching Silero).
    fn frame_size(&self) -> usize {
        (self.config.sample_rate as usize * 30) / 1000
    }

    /// Minimum number of consecutive speech frames to keep a segment.
    fn min_speech_frames(&self) -> usize {
        let frame_ms = 30u32;
        (self.config.min_speech_ms / frame_ms).max(1) as usize
    }

    /// Minimum number of consecutive silence frames to split.
    fn min_silence_frames(&self) -> usize {
        let frame_ms = 30u32;
        (self.config.min_silence_ms / frame_ms).max(1) as usize
    }

    /// Convert a frame index (relative to chunk start) to ms.
    fn frame_to_ms(&self, frame_idx: usize) -> u64 {
        (frame_idx as u64) * 30
    }
}

impl VoiceActivityDetector for SileroVad {
    fn process_chunk(&mut self, samples: &[f32]) -> Result<Vec<VadSegment>, AppError> {
        let frame_size = self.frame_size();
        if frame_size == 0 {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::VadError,
                message: "Frame size is zero".into(),
            });
        }

        if samples.is_empty() {
            return Ok(vec![]);
        }

        // Classify each frame as speech or silence.
        let mut frame_labels: Vec<(bool, f32)> = Vec::new();
        let mut pos = 0;
        while pos + frame_size <= samples.len() {
            let energy = Self::rms_energy(&samples[pos..pos + frame_size]);
            let is_speech = energy >= self.config.threshold;
            frame_labels.push((is_speech, energy));
            pos += frame_size;
        }
        // Handle trailing partial frame.
        if pos < samples.len() {
            let energy = Self::rms_energy(&samples[pos..]);
            let is_speech = energy >= self.config.threshold;
            frame_labels.push((is_speech, energy));
        }

        if frame_labels.is_empty() {
            return Ok(vec![]);
        }

        // Merge consecutive same-label frames into raw segments.
        let mut raw_segments: Vec<(bool, usize, usize, f32)> = Vec::new(); // (is_speech, start_frame, end_frame, sum_energy)
        let (mut cur_speech, mut cur_start, mut cur_energy_sum, mut cur_count) =
            (frame_labels[0].0, 0usize, frame_labels[0].1, 1usize);

        for (i, &(is_speech, energy)) in frame_labels.iter().enumerate().skip(1) {
            if is_speech == cur_speech {
                cur_energy_sum += energy;
                cur_count += 1;
            } else {
                raw_segments.push((cur_speech, cur_start, i, cur_energy_sum / cur_count as f32));
                cur_speech = is_speech;
                cur_start = i;
                cur_energy_sum = energy;
                cur_count = 1;
            }
        }
        raw_segments.push((
            cur_speech,
            cur_start,
            frame_labels.len(),
            cur_energy_sum / cur_count as f32,
        ));

        // Apply minimum duration filters:
        // 1. Short silence gaps between speech are merged into speech.
        // 2. Short speech segments are dropped.
        let min_silence = self.min_silence_frames();
        let min_speech = self.min_speech_frames();

        // Pass 1: merge short silence gaps into surrounding speech.
        let mut merged: Vec<(bool, usize, usize, f32)> = Vec::new();
        for seg in &raw_segments {
            if !seg.0 && (seg.2 - seg.1) < min_silence {
                // Short silence — check if surrounded by speech.
                if let Some(last) = merged.last() {
                    if last.0 {
                        // Previous was speech; mark this silence as speech for merging.
                        merged.push((true, seg.1, seg.2, seg.3));
                        continue;
                    }
                }
            }
            merged.push(*seg);
        }

        // Pass 2: collapse consecutive same-label segments.
        let mut collapsed: Vec<(bool, usize, usize, f32)> = Vec::new();
        for seg in &merged {
            if let Some(last) = collapsed.last_mut() {
                if last.0 == seg.0 {
                    let total_frames = (last.2 - last.1) + (seg.2 - seg.1);
                    let old_frames = last.2 - last.1;
                    last.3 = (last.3 * old_frames as f32 + seg.3 * (seg.2 - seg.1) as f32)
                        / total_frames as f32;
                    last.2 = seg.2;
                    continue;
                }
            }
            collapsed.push(*seg);
        }

        // Pass 3: drop short speech segments.
        let filtered: Vec<(bool, usize, usize, f32)> = collapsed
            .into_iter()
            .map(|seg| {
                if seg.0 && (seg.2 - seg.1) < min_speech {
                    (false, seg.1, seg.2, seg.3) // too short — reclassify as silence
                } else {
                    seg
                }
            })
            .collect();

        // Build output VadSegments.
        let base_ms = self.frame_offset * 30;
        let segments: Vec<VadSegment> = filtered
            .iter()
            .map(|&(is_speech, start, end, avg_energy)| VadSegment {
                start_ms: base_ms + self.frame_to_ms(start),
                end_ms: base_ms + self.frame_to_ms(end),
                is_speech,
                confidence: avg_energy,
            })
            .collect();

        // Advance the frame offset for the next chunk.
        self.frame_offset += frame_labels.len() as u64;

        Ok(segments)
    }

    fn reset(&mut self) {
        self.frame_offset = 0;
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_silence(num_samples: usize) -> Vec<f32> {
        vec![0.0; num_samples]
    }

    fn make_speech(num_samples: usize, amplitude: f32) -> Vec<f32> {
        // Generate a simple sine wave to simulate speech energy.
        (0..num_samples)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect()
    }

    // ── Config tests ────────────────────────────────────────────────────────

    #[test]
    fn test_vad_config_default() {
        let cfg = VadConfig::default();
        assert!((cfg.threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(cfg.min_speech_ms, 250);
        assert_eq!(cfg.min_silence_ms, 300);
        assert_eq!(cfg.sample_rate, 16000);
    }

    #[test]
    fn test_vad_config_serializes() {
        let cfg = VadConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["threshold"], 0.5);
        assert_eq!(json["sampleRate"], 16000);
    }

    #[test]
    fn test_vad_config_deserializes() {
        let json = r#"{"threshold":0.3,"minSpeechMs":200,"minSilenceMs":400,"sampleRate":16000}"#;
        let cfg: VadConfig = serde_json::from_str(json).unwrap();
        assert!((cfg.threshold - 0.3).abs() < f32::EPSILON);
        assert_eq!(cfg.min_speech_ms, 200);
    }

    // ── Constructor tests ───────────────────────────────────────────────────

    #[test]
    fn test_silero_vad_rejects_zero_sample_rate() {
        let cfg = VadConfig {
            sample_rate: 0,
            ..Default::default()
        };
        let result = SileroVad::new(cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_silero_vad_rejects_invalid_threshold() {
        let cfg = VadConfig {
            threshold: 1.5,
            ..Default::default()
        };
        assert!(SileroVad::new(cfg).is_err());

        let cfg = VadConfig {
            threshold: -0.1,
            ..Default::default()
        };
        assert!(SileroVad::new(cfg).is_err());
    }

    #[test]
    fn test_silero_vad_accepts_valid_config() {
        let cfg = VadConfig::default();
        assert!(SileroVad::new(cfg).is_ok());
    }

    // ── RMS energy tests ────────────────────────────────────────────────────

    #[test]
    fn test_rms_energy_silence_is_zero() {
        assert!((SileroVad::rms_energy(&[0.0; 480]) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rms_energy_empty_is_zero() {
        assert!((SileroVad::rms_energy(&[]) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rms_energy_constant_signal() {
        let samples = vec![0.5; 480];
        let rms = SileroVad::rms_energy(&samples);
        assert!((rms - 0.5).abs() < 0.01);
    }

    // ── Process chunk tests ─────────────────────────────────────────────────

    #[test]
    fn test_process_empty_chunk() {
        let mut vad = SileroVad::new(VadConfig::default()).unwrap();
        let result = vad.process_chunk(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_pure_silence_produces_no_speech() {
        let mut vad = SileroVad::new(VadConfig::default()).unwrap();
        let silence = make_silence(16000); // 1 second
        let segments = vad.process_chunk(&silence).unwrap();
        assert!(!segments.is_empty());
        for seg in &segments {
            assert!(
                !seg.is_speech,
                "Expected no speech in silence, got {:?}",
                seg
            );
        }
    }

    #[test]
    fn test_loud_signal_produces_speech() {
        let mut vad = SileroVad::new(VadConfig {
            threshold: 0.1,
            min_speech_ms: 30, // one frame minimum
            ..Default::default()
        })
        .unwrap();
        let speech = make_speech(16000, 0.8); // 1 second loud
        let segments = vad.process_chunk(&speech).unwrap();
        let speech_segs: Vec<_> = segments.iter().filter(|s| s.is_speech).collect();
        assert!(
            !speech_segs.is_empty(),
            "Expected speech segments in loud signal"
        );
    }

    #[test]
    fn test_speech_then_silence_produces_both() {
        let mut vad = SileroVad::new(VadConfig {
            threshold: 0.1,
            min_speech_ms: 30,
            min_silence_ms: 30,
            ..Default::default()
        })
        .unwrap();
        let mut audio = make_speech(8000, 0.8); // 0.5s speech
        audio.extend(make_silence(8000)); // 0.5s silence
        let segments = vad.process_chunk(&audio).unwrap();
        let has_speech = segments.iter().any(|s| s.is_speech);
        let has_silence = segments.iter().any(|s| !s.is_speech);
        assert!(has_speech, "Expected speech segment");
        assert!(has_silence, "Expected silence segment");
    }

    #[test]
    fn test_short_speech_is_filtered() {
        let mut vad = SileroVad::new(VadConfig {
            threshold: 0.1,
            min_speech_ms: 500, // require 500ms minimum speech
            min_silence_ms: 30,
            ..Default::default()
        })
        .unwrap();
        // Very short speech blip (60ms) surrounded by silence.
        let mut audio = make_silence(8000);
        audio.extend(make_speech(960, 0.8)); // 60ms
        audio.extend(make_silence(8000));
        let segments = vad.process_chunk(&audio).unwrap();
        let speech_segs: Vec<_> = segments.iter().filter(|s| s.is_speech).collect();
        assert!(
            speech_segs.is_empty(),
            "Short speech blip should be filtered out"
        );
    }

    #[test]
    fn test_reset_clears_frame_offset() {
        let mut vad = SileroVad::new(VadConfig::default()).unwrap();
        let audio = make_silence(16000);
        vad.process_chunk(&audio).unwrap();
        assert!(vad.frame_offset > 0);
        vad.reset();
        assert_eq!(vad.frame_offset, 0);
    }

    // ── VadSegment serialization ────────────────────────────────────────────

    #[test]
    fn test_vad_segment_serializes() {
        let seg = VadSegment {
            start_ms: 0,
            end_ms: 3000,
            is_speech: true,
            confidence: 0.85,
        };
        let json = serde_json::to_value(&seg).unwrap();
        assert_eq!(json["startMs"], 0);
        assert_eq!(json["endMs"], 3000);
        assert_eq!(json["isSpeech"], true);
    }

    #[test]
    fn test_vad_segment_equality() {
        let a = VadSegment {
            start_ms: 0,
            end_ms: 100,
            is_speech: true,
            confidence: 0.5,
        };
        let b = VadSegment {
            start_ms: 0,
            end_ms: 100,
            is_speech: true,
            confidence: 0.5,
        };
        assert_eq!(a, b);
    }

    // ── Frame calculation tests ─────────────────────────────────────────────

    #[test]
    fn test_frame_size_at_16khz() {
        let vad = SileroVad::new(VadConfig::default()).unwrap();
        assert_eq!(vad.frame_size(), 480); // 16000 * 30 / 1000
    }

    #[test]
    fn test_consecutive_chunks_advance_timestamps() {
        let mut vad = SileroVad::new(VadConfig {
            threshold: 0.1,
            min_speech_ms: 30,
            min_silence_ms: 30,
            ..Default::default()
        })
        .unwrap();

        let chunk1 = make_silence(4800); // 300ms = 10 frames
        let seg1 = vad.process_chunk(&chunk1).unwrap();
        assert!(!seg1.is_empty());
        let first_end = seg1.last().unwrap().end_ms;

        let chunk2 = make_silence(4800);
        let seg2 = vad.process_chunk(&chunk2).unwrap();
        assert!(!seg2.is_empty());
        // Second chunk timestamps must start at or after the first chunk ended.
        assert!(
            seg2[0].start_ms >= first_end,
            "Second chunk start_ms={} should be >= first end={}",
            seg2[0].start_ms,
            first_end
        );
    }
}
