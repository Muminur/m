//! Streaming (real-time) transcription using a sliding window approach.
//!
//! `StreamingTranscriber` buffers incoming audio in a ring buffer, advances a
//! step every `step_size_ms`, and runs whisper inference on the current window.
//! An optional VAD gate skips inference when no speech is detected.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, TranscriptionErrorCode};
use crate::transcription::engine::SegmentResult;
use crate::transcription::vad::{SileroVad, VadConfig, VoiceActivityDetector};

// ─── Configuration ──────────────────────────────────────────────────────────

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

// ─── Event payloads ─────────────────────────────────────────────────────────

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

// ─── Inference callback ─────────────────────────────────────────────────────

/// Trait abstracting whisper inference so the streaming logic can be tested
/// without loading a real model.
pub trait InferenceProvider: Send {
    /// Run whisper on the given PCM window and return segments.
    fn infer(&self, pcm_window: &[f32]) -> Result<Vec<SegmentResult>, AppError>;
}

// ─── Ring buffer ────────────────────────────────────────────────────────────

/// A fixed-capacity ring buffer for f32 audio samples.
#[derive(Debug)]
struct RingBuffer {
    data: Vec<f32>,
    capacity: usize,
    write_pos: usize,
    len: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0.0; capacity],
            capacity,
            write_pos: 0,
            len: 0,
        }
    }

    /// Push samples into the buffer, overwriting oldest data when full.
    fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.data[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            if self.len < self.capacity {
                self.len += 1;
            }
        }
    }

    /// Read the last `count` samples in chronological order.
    /// Returns fewer if the buffer has fewer than `count` samples.
    fn read_last(&self, count: usize) -> Vec<f32> {
        let n = count.min(self.len);
        if n == 0 {
            return vec![];
        }
        let start = if self.len < self.capacity {
            // Buffer not yet wrapped.
            self.len.saturating_sub(n)
        } else {
            // Buffer has wrapped; write_pos points to the oldest sample.
            (self.write_pos + self.capacity - n) % self.capacity
        };

        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(self.data[(start + i) % self.capacity]);
        }
        out
    }

    fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.write_pos = 0;
        self.len = 0;
        // No need to zero-fill; push overwrites.
    }
}

// ─── StreamingTranscriber ───────────────────────────────────────────────────

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

/// Real-time sliding-window transcription engine.
///
/// Audio is pushed in via `push_audio`. When enough samples accumulate for one
/// step, inference is triggered on the current window. A VAD gate optionally
/// filters silence.
pub struct StreamingTranscriber {
    config: StreamingConfig,
    buffer: RingBuffer,
    /// Samples accumulated since the last step.
    step_accumulator: usize,
    /// Total samples pushed (for timestamp calculation).
    total_samples: u64,
    /// Number of steps completed.
    steps_completed: u64,
    state: StreamingState,
    abort_flag: Arc<AtomicBool>,
    vad: Option<SileroVad>,
    /// Collected caption segments.
    segments: Vec<CaptionSegment>,
}

impl StreamingTranscriber {
    /// Create a new streaming transcriber.
    pub fn new(config: StreamingConfig) -> Result<Self, AppError> {
        if config.sample_rate == 0 {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Sample rate must be > 0".into(),
            });
        }
        if config.step_size_ms == 0 {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Step size must be > 0".into(),
            });
        }
        if config.window_size_ms < config.step_size_ms {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Window size must be >= step size".into(),
            });
        }

        let window_samples = ms_to_samples(config.window_size_ms, config.sample_rate);

        let vad = if config.vad_enabled {
            Some(SileroVad::new(config.vad_config.clone())?)
        } else {
            None
        };

        Ok(Self {
            config,
            buffer: RingBuffer::new(window_samples),
            step_accumulator: 0,
            total_samples: 0,
            steps_completed: 0,
            state: StreamingState::Idle,
            abort_flag: Arc::new(AtomicBool::new(false)),
            vad,
            segments: Vec::new(),
        })
    }

    /// Get the current state.
    pub fn state(&self) -> StreamingState {
        self.state
    }

    /// Get the abort flag (for external cancellation).
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_flag)
    }

    /// Push audio samples into the pipeline. Returns caption segments produced
    /// during this push (may be empty if no step boundary was crossed or VAD
    /// filtered the audio).
    pub fn push_audio(
        &mut self,
        samples: &[f32],
        inference: &dyn InferenceProvider,
    ) -> Result<Vec<CaptionSegment>, AppError> {
        if self.state == StreamingState::Stopped {
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::InferenceFailure,
                message: "Transcriber is stopped".into(),
            });
        }

        if self.abort_flag.load(Ordering::Relaxed) {
            self.state = StreamingState::Stopped;
            return Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::Cancelled,
                message: "Streaming transcription was cancelled".into(),
            });
        }

        self.state = StreamingState::Running;
        self.buffer.push(samples);
        self.total_samples += samples.len() as u64;
        self.step_accumulator += samples.len();

        let step_samples = ms_to_samples(self.config.step_size_ms, self.config.sample_rate);
        let mut new_segments = Vec::new();

        while self.step_accumulator >= step_samples {
            self.step_accumulator -= step_samples;

            if self.abort_flag.load(Ordering::Relaxed) {
                self.state = StreamingState::Stopped;
                return Err(AppError::TranscriptionError {
                    code: TranscriptionErrorCode::Cancelled,
                    message: "Cancelled during step processing".into(),
                });
            }

            // Extract the current window.
            let window_samples_count =
                ms_to_samples(self.config.window_size_ms, self.config.sample_rate);
            let window = self.buffer.read_last(window_samples_count);

            // VAD gate: skip inference if no speech detected.
            if let Some(ref mut vad) = self.vad {
                let vad_segments = vad.process_chunk(&window)?;
                let has_speech = vad_segments.iter().any(|s| s.is_speech);
                if !has_speech {
                    tracing::debug!("VAD: no speech detected in window, skipping inference");
                    self.steps_completed += 1;
                    continue;
                }
            }

            // Run inference on the window.
            let results = inference.infer(&window)?;

            // Convert engine segments to caption segments with global timestamps.
            let step_start_ms = self.steps_completed * self.config.step_size_ms as u64;
            for seg in &results {
                let caption = CaptionSegment {
                    text: seg.text.clone(),
                    start_ms: step_start_ms + seg.start_ms as u64,
                    end_ms: step_start_ms + seg.end_ms as u64,
                    is_final: true,
                    confidence: seg.confidence,
                };
                new_segments.push(caption);
            }

            self.steps_completed += 1;
        }

        self.segments.extend(new_segments.clone());
        Ok(new_segments)
    }

    /// Flush remaining audio (final partial window). Forces inference on
    /// whatever is left in the buffer.
    pub fn flush(
        &mut self,
        inference: &dyn InferenceProvider,
    ) -> Result<Vec<CaptionSegment>, AppError> {
        if self.state == StreamingState::Stopped {
            return Ok(vec![]);
        }

        let available = self.buffer.len();
        if available == 0 {
            self.state = StreamingState::Stopped;
            return Ok(vec![]);
        }

        let window = self.buffer.read_last(available);

        // VAD gate on flush too.
        if let Some(ref mut vad) = self.vad {
            let vad_segments = vad.process_chunk(&window)?;
            let has_speech = vad_segments.iter().any(|s| s.is_speech);
            if !has_speech {
                self.state = StreamingState::Stopped;
                return Ok(vec![]);
            }
        }

        let results = inference.infer(&window)?;
        let step_start_ms = self.steps_completed * self.config.step_size_ms as u64;
        let mut final_segments = Vec::new();

        for seg in &results {
            let caption = CaptionSegment {
                text: seg.text.clone(),
                start_ms: step_start_ms + seg.start_ms as u64,
                end_ms: step_start_ms + seg.end_ms as u64,
                is_final: true,
                confidence: seg.confidence,
            };
            final_segments.push(caption);
        }

        self.segments.extend(final_segments.clone());
        self.state = StreamingState::Stopped;
        Ok(final_segments)
    }

    /// Stop the transcriber. No more audio can be pushed after this.
    pub fn stop(&mut self) {
        self.abort_flag.store(true, Ordering::Relaxed);
        self.state = StreamingState::Stopped;
    }

    /// Get all caption segments produced so far.
    pub fn all_segments(&self) -> &[CaptionSegment] {
        &self.segments
    }

    /// Get the number of completed inference steps.
    pub fn steps_completed(&self) -> u64 {
        self.steps_completed
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert milliseconds to sample count at the given sample rate.
fn ms_to_samples(ms: u32, sample_rate: u32) -> usize {
    (sample_rate as usize * ms as usize) / 1000
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock inference provider ─────────────────────────────────────────────

    struct MockInference {
        /// Fixed segments to return on every infer call.
        segments: Vec<SegmentResult>,
    }

    impl MockInference {
        fn new(text: &str) -> Self {
            Self {
                segments: vec![SegmentResult {
                    index: 0,
                    start_ms: 0,
                    end_ms: 3000,
                    text: text.to_string(),
                    confidence: 0.95,
                }],
            }
        }

        fn empty() -> Self {
            Self { segments: vec![] }
        }
    }

    impl InferenceProvider for MockInference {
        fn infer(&self, _pcm_window: &[f32]) -> Result<Vec<SegmentResult>, AppError> {
            Ok(self.segments.clone())
        }
    }

    /// An inference provider that counts how many times infer() was called.
    struct CountingInference {
        call_count: std::cell::Cell<u32>,
    }

    impl CountingInference {
        fn new() -> Self {
            Self {
                call_count: std::cell::Cell::new(0),
            }
        }
        fn count(&self) -> u32 {
            self.call_count.get()
        }
    }

    impl InferenceProvider for CountingInference {
        fn infer(&self, _pcm_window: &[f32]) -> Result<Vec<SegmentResult>, AppError> {
            self.call_count.set(self.call_count.get() + 1);
            Ok(vec![SegmentResult {
                index: 0,
                start_ms: 0,
                end_ms: 3000,
                text: "test".to_string(),
                confidence: 0.9,
            }])
        }
    }

    fn make_speech_samples(num: usize, amplitude: f32) -> Vec<f32> {
        (0..num)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect()
    }

    // ── Config tests ────────────────────────────────────────────────────────

    #[test]
    fn test_streaming_config_default() {
        let cfg = StreamingConfig::default();
        assert_eq!(cfg.step_size_ms, 3000);
        assert_eq!(cfg.window_size_ms, 10000);
        assert_eq!(cfg.overlap_ms, 200);
        assert_eq!(cfg.sample_rate, 16000);
        assert!(cfg.vad_enabled);
    }

    #[test]
    fn test_streaming_config_serializes() {
        let cfg = StreamingConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["stepSizeMs"], 3000);
        assert_eq!(json["windowSizeMs"], 10000);
        assert_eq!(json["vadEnabled"], true);
    }

    // ── Constructor validation ──────────────────────────────────────────────

    #[test]
    fn test_rejects_zero_sample_rate() {
        let cfg = StreamingConfig {
            sample_rate: 0,
            ..Default::default()
        };
        assert!(StreamingTranscriber::new(cfg).is_err());
    }

    #[test]
    fn test_rejects_zero_step_size() {
        let cfg = StreamingConfig {
            step_size_ms: 0,
            ..Default::default()
        };
        assert!(StreamingTranscriber::new(cfg).is_err());
    }

    #[test]
    fn test_rejects_window_smaller_than_step() {
        let cfg = StreamingConfig {
            window_size_ms: 1000,
            step_size_ms: 5000,
            ..Default::default()
        };
        assert!(StreamingTranscriber::new(cfg).is_err());
    }

    #[test]
    fn test_creates_with_valid_config() {
        let st = StreamingTranscriber::new(StreamingConfig::default());
        assert!(st.is_ok());
        assert_eq!(st.unwrap().state(), StreamingState::Idle);
    }

    // ── Ring buffer tests ───────────────────────────────────────────────────

    #[test]
    fn test_ring_buffer_basic() {
        let mut rb = RingBuffer::new(10);
        assert_eq!(rb.len(), 0);
        rb.push(&[1.0, 2.0, 3.0]);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.read_last(3), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_ring_buffer_wraps() {
        let mut rb = RingBuffer::new(4);
        rb.push(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(rb.read_last(4), vec![1.0, 2.0, 3.0, 4.0]);
        rb.push(&[5.0, 6.0]);
        // Should now contain [3, 4, 5, 6]
        assert_eq!(rb.read_last(4), vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_ring_buffer_read_more_than_available() {
        let mut rb = RingBuffer::new(10);
        rb.push(&[1.0, 2.0]);
        let out = rb.read_last(5);
        assert_eq!(out, vec![1.0, 2.0]);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut rb = RingBuffer::new(10);
        rb.push(&[1.0, 2.0, 3.0]);
        rb.clear();
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.read_last(10), Vec::<f32>::new());
    }

    // ── Streaming pipeline tests ────────────────────────────────────────────

    #[test]
    fn test_push_audio_no_step_yet() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("hello");
        // Push less than one step (3000ms = 48000 samples).
        let audio = vec![0.0; 1000];
        let result = st.push_audio(&audio, &inference).unwrap();
        assert!(result.is_empty(), "No step boundary crossed yet");
        assert_eq!(st.state(), StreamingState::Running);
    }

    #[test]
    fn test_push_audio_triggers_step() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("hello world");
        // Push exactly one step worth of audio (3000ms * 16 samples/ms = 48000).
        let audio = vec![0.1; 48000];
        let result = st.push_audio(&audio, &inference).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "hello world");
        assert!(result[0].is_final);
        assert_eq!(st.steps_completed(), 1);
    }

    #[test]
    fn test_push_audio_multiple_steps() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("chunk");
        // Push 2 steps worth (96000 samples).
        let audio = vec![0.1; 96000];
        let result = st.push_audio(&audio, &inference).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(st.steps_completed(), 2);
    }

    #[test]
    fn test_vad_skips_silent_windows() {
        let cfg = StreamingConfig {
            vad_enabled: true,
            vad_config: VadConfig {
                threshold: 0.1,
                min_speech_ms: 30,
                min_silence_ms: 30,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let counting = CountingInference::new();
        // Push one step of pure silence.
        let silence = vec![0.0; 48000];
        let result = st.push_audio(&silence, &counting).unwrap();
        assert!(
            result.is_empty(),
            "Silent window should not trigger inference"
        );
        assert_eq!(counting.count(), 0, "Inference should not have been called");
        assert_eq!(st.steps_completed(), 1, "Step should still be counted");
    }

    #[test]
    fn test_vad_allows_speech_windows() {
        let cfg = StreamingConfig {
            vad_enabled: true,
            vad_config: VadConfig {
                threshold: 0.1,
                min_speech_ms: 30,
                min_silence_ms: 30,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let counting = CountingInference::new();
        // Push one step of loud audio.
        let speech = make_speech_samples(48000, 0.8);
        let result = st.push_audio(&speech, &counting).unwrap();
        assert!(!result.is_empty(), "Speech window should trigger inference");
        assert_eq!(counting.count(), 1);
    }

    #[test]
    fn test_flush_processes_remaining() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("final");
        // Push less than a full step.
        let audio = vec![0.1; 10000];
        let _ = st.push_audio(&audio, &inference).unwrap();
        let final_segs = st.flush(&inference).unwrap();
        assert_eq!(final_segs.len(), 1);
        assert_eq!(final_segs[0].text, "final");
        assert_eq!(st.state(), StreamingState::Stopped);
    }

    #[test]
    fn test_stop_prevents_further_push() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        st.stop();
        assert_eq!(st.state(), StreamingState::Stopped);
        let inference = MockInference::new("test");
        let result = st.push_audio(&[0.1; 100], &inference);
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_flag_cancels() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let flag = st.abort_flag();
        flag.store(true, Ordering::Relaxed);
        let inference = MockInference::new("test");
        let result = st.push_audio(&[0.1; 100], &inference);
        assert!(result.is_err());
        assert_eq!(st.state(), StreamingState::Stopped);
    }

    #[test]
    fn test_all_segments_accumulate() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("seg");
        // Push 2 steps.
        let audio = vec![0.1; 96000];
        st.push_audio(&audio, &inference).unwrap();
        assert_eq!(st.all_segments().len(), 2);
    }

    #[test]
    fn test_timestamps_advance_with_steps() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::new("test");
        // 2 steps
        let audio = vec![0.1; 96000];
        let segs = st.push_audio(&audio, &inference).unwrap();
        assert_eq!(segs.len(), 2);
        // First step starts at 0.
        assert_eq!(segs[0].start_ms, 0);
        // Second step starts at step_size_ms.
        assert_eq!(segs[1].start_ms, 3000);
    }

    #[test]
    fn test_empty_inference_produces_no_segments() {
        let cfg = StreamingConfig {
            vad_enabled: false,
            ..Default::default()
        };
        let mut st = StreamingTranscriber::new(cfg).unwrap();
        let inference = MockInference::empty();
        let audio = vec![0.1; 48000];
        let result = st.push_audio(&audio, &inference).unwrap();
        assert!(result.is_empty());
    }

    // ── Caption segment serialization ───────────────────────────────────────

    #[test]
    fn test_caption_segment_serializes() {
        let seg = CaptionSegment {
            text: "hello".into(),
            start_ms: 0,
            end_ms: 3000,
            is_final: true,
            confidence: 0.95,
        };
        let json = serde_json::to_value(&seg).unwrap();
        assert_eq!(json["text"], "hello");
        assert_eq!(json["startMs"], 0);
        assert_eq!(json["isFinal"], true);
    }

    // ── ms_to_samples helper ────────────────────────────────────────────────

    #[test]
    fn test_ms_to_samples() {
        assert_eq!(ms_to_samples(1000, 16000), 16000);
        assert_eq!(ms_to_samples(3000, 16000), 48000);
        assert_eq!(ms_to_samples(30, 16000), 480);
        assert_eq!(ms_to_samples(0, 16000), 0);
    }
}
