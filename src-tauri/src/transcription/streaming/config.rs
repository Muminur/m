//! Configuration types and constants for the streaming transcription pipeline.

use serde::{Deserialize, Serialize};

use crate::transcription::vad::VadConfig;

/// Configuration for the streaming transcription pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingConfig {
    /// Step / advance size in milliseconds (default: 3000).
    pub step_size_ms: u32,
    /// Total context window in milliseconds (default: 10000).
    pub window_size_ms: u32,
    /// Overlap between consecutive windows in milliseconds (default: 200).
    pub overlap_ms: u32,
    /// Audio sample rate (default: 16000).
    pub sample_rate: u32,
    /// Whether VAD gating is enabled.
    pub vad_enabled: bool,
    /// VAD configuration (used only when `vad_enabled` is true).
    pub vad_config: VadConfig,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            step_size_ms: 3000,
            window_size_ms: 10000,
            overlap_ms: 200,
            sample_rate: 16000,
            vad_enabled: true,
            vad_config: VadConfig::default(),
        }
    }
}

/// A caption segment emitted during streaming transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionSegment {
    /// The recognized text for this window.
    pub text: String,
    /// Start time in milliseconds (relative to stream start).
    pub start_ms: u64,
    /// End time in milliseconds (relative to stream start).
    pub end_ms: u64,
    /// Whether this is a final (committed) or interim segment.
    pub is_final: bool,
    /// Average confidence score.
    pub confidence: f32,
}

/// Trait abstracting whisper inference so the streaming logic can be tested
/// without loading a real model.
pub trait InferenceProvider: Send {
    /// Run whisper on the given PCM window and return segments.
    fn infer(&self, pcm_window: &[f32]) -> Result<Vec<crate::transcription::engine::SegmentResult>, crate::error::AppError>;
}

/// State of the streaming transcription pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingState {
    /// Ready to receive audio but not yet started.
    Idle,
    /// Actively receiving and processing audio.
    Running,
    /// Stopped; must create a new instance to restart.
    Stopped,
}

/// Convert milliseconds to sample count at the given sample rate.
pub(crate) fn ms_to_samples(ms: u32, sample_rate: u32) -> usize {
    (sample_rate as usize * ms as usize) / 1000
}
