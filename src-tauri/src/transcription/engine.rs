#[cfg(target_os = "macos")]
use crate::error::AudioErrorCode;
use crate::error::{AppError, TranscriptionErrorCode};
use crate::settings::AccelerationBackend;
use std::path::Path;
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionParams {
    pub language: Option<String>,
    pub translate: bool,
    pub beam_size: i32,
    pub temperature: f32,
    pub n_threads: i32,
    pub word_timestamps: bool,
    pub initial_prompt: Option<String>,
    pub no_speech_threshold: Option<f32>,
}

impl Default for TranscriptionParams {
    fn default() -> Self {
        Self {
            language: None,
            translate: false,
            beam_size: 5,
            temperature: 0.0,
            n_threads: 4,
            word_timestamps: false,
            initial_prompt: None,
            no_speech_threshold: Some(0.6),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SegmentResult {
    pub index: usize,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptionOutput {
    pub segments: Vec<SegmentResult>,
    pub backend_used: AccelerationBackend,
    pub wall_time_ms: u64,
}

#[derive(Debug)]
pub struct WhisperEngine {
    #[allow(dead_code)] // Stored for future use (benchmarks, logging)
    model_path: std::path::PathBuf,
    pub backend: AccelerationBackend,
}

impl WhisperEngine {
    pub fn new(model_path: &Path, backend: AccelerationBackend) -> Result<Self, AppError> {
        if !model_path.exists() {
            return Err(AppError::ModelError {
                code: crate::error::ModelErrorCode::NotFound,
                message: format!("Model file not found: {:?}", model_path),
            });
        }
        Ok(Self {
            model_path: model_path.to_path_buf(),
            backend,
        })
    }

    pub fn transcribe(
        &self,
        params: &TranscriptionParams,
        pcm_data: &[f32],
        progress_callback: impl Fn(f32) + Send + 'static,
        abort_flag: Arc<AtomicBool>,
    ) -> Result<TranscriptionOutput, AppError> {
        #[cfg(target_os = "macos")]
        {
            use whisper_rs::{
                FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
            };

            // CoreML is unimplemented — no .mlmodelc packages available on CDN yet
            if self.backend == AccelerationBackend::CoreMl {
                return Err(AppError::TranscriptionError {
                    code: TranscriptionErrorCode::BackendUnavailable,
                    message: "CoreML backend requires .mlmodelc model packages (not yet available)"
                        .into(),
                });
            }

            if pcm_data.is_empty() {
                return Err(AppError::AudioError {
                    code: AudioErrorCode::InvalidAudioFormat,
                    message: "PCM data is empty".into(),
                });
            }

            // Build context parameters based on requested backend
            let ctx_params = {
                let mut p = WhisperContextParameters::default();
                if self.backend == AccelerationBackend::Cpu {
                    p.use_gpu(false);
                }
                // Auto and Metal: use_gpu stays true (default with metal feature)
                p
            };

            let ctx = WhisperContext::new_with_params(
                self.model_path.to_str().unwrap_or_default(),
                ctx_params,
            )
            .map_err(|e| AppError::TranscriptionError {
                code: TranscriptionErrorCode::ModelNotLoaded,
                message: format!("Failed to load whisper model: {}", e),
            })?;

            let mut full_params = FullParams::new(if params.beam_size > 1 {
                SamplingStrategy::BeamSearch {
                    beam_size: params.beam_size,
                    patience: -1.0,
                }
            } else {
                SamplingStrategy::Greedy { best_of: 1 }
            });

            full_params.set_n_threads(params.n_threads);
            full_params.set_translate(params.translate);
            full_params.set_print_special(false);
            full_params.set_print_progress(false);
            full_params.set_print_realtime(false);
            full_params.set_print_timestamps(true);

            if let Some(lang) = &params.language {
                full_params.set_language(Some(lang.as_str()));
            }

            if let Some(prompt) = &params.initial_prompt {
                full_params.set_initial_prompt(prompt.as_str());
            }

            if let Some(threshold) = params.no_speech_threshold {
                full_params.set_no_speech_thold(threshold);
            }

            // Progress callback — also checks abort flag
            let abort_clone = Arc::clone(&abort_flag);
            full_params.set_progress_callback_safe(move |progress| {
                if abort_clone.load(Ordering::Relaxed) {
                    return;
                }
                progress_callback(progress as f32 / 100.0);
            });

            let mut state = ctx
                .create_state()
                .map_err(|e| AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: format!("Failed to create whisper state: {}", e),
                })?;

            let wall_start = std::time::Instant::now();

            state
                .full(full_params, pcm_data)
                .map_err(|e| AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: format!("Whisper inference failed: {}", e),
                })?;

            let wall_time_ms = wall_start.elapsed().as_millis() as u64;

            if abort_flag.load(Ordering::Relaxed) {
                return Err(AppError::TranscriptionError {
                    code: TranscriptionErrorCode::Cancelled,
                    message: "Transcription was cancelled".into(),
                });
            }

            // whisper-rs 0.16.0: full_n_segments() returns c_int directly (not Result)
            let n_segments = state.full_n_segments();

            let mut results = Vec::with_capacity(n_segments as usize);

            for i in 0..n_segments {
                // whisper-rs 0.16.0: use get_segment(i) -> Option<WhisperSegment>
                let segment = match state.get_segment(i) {
                    Some(s) => s,
                    None => continue,
                };

                let text = segment.to_str().map_err(|e| AppError::TranscriptionError {
                    code: TranscriptionErrorCode::InferenceFailure,
                    message: format!("Failed to get segment text: {}", e),
                })?;

                // start/end_timestamp() returns centiseconds; multiply by 10 → milliseconds
                let start_ms = segment.start_timestamp() * 10;
                let end_ms = segment.end_timestamp() * 10;

                let n_tokens = segment.n_tokens();
                let confidence = if n_tokens > 0 {
                    let sum: f32 = (0..n_tokens)
                        .filter_map(|t| segment.get_token(t))
                        .map(|tok| tok.token_probability())
                        .sum();
                    sum / n_tokens as f32
                } else {
                    0.0
                };

                results.push(SegmentResult {
                    index: i as usize,
                    start_ms,
                    end_ms,
                    text: text.trim().to_string(),
                    confidence,
                });
            }

            let backend_used = if self.backend == AccelerationBackend::Auto
                || self.backend == AccelerationBackend::Metal
            {
                AccelerationBackend::Metal
            } else {
                AccelerationBackend::Cpu
            };

            return Ok(TranscriptionOutput {
                segments: results,
                backend_used,
                wall_time_ms,
            });
        }

        // Non-macOS: whisper-rs is not compiled in
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (params, pcm_data, progress_callback, abort_flag);
            Err(AppError::TranscriptionError {
                code: TranscriptionErrorCode::ModelNotLoaded,
                message: "Whisper transcription requires macOS (Metal backend)".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AccelerationBackend;
    use std::path::PathBuf;

    #[test]
    fn test_whisper_engine_rejects_missing_model() {
        let result = WhisperEngine::new(
            &PathBuf::from("/nonexistent/model.bin"),
            AccelerationBackend::Auto,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ModelError {
                code: crate::error::ModelErrorCode::NotFound,
                ..
            } => {}
            other => panic!("Expected ModelNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_transcription_params_default() {
        let params = TranscriptionParams::default();
        assert_eq!(params.beam_size, 5);
        assert_eq!(params.n_threads, 4);
        assert!(!params.translate);
        assert!(params.language.is_none());
    }

    #[test]
    fn test_segment_result_serializes() {
        let seg = SegmentResult {
            index: 0,
            start_ms: 0,
            end_ms: 1500,
            text: "Hello world".into(),
            confidence: 0.95,
        };
        let json = serde_json::to_value(&seg).unwrap();
        let confidence = json["confidence"].as_f64().unwrap();
        assert!(
            (confidence - 0.95).abs() < 1e-3,
            "confidence={}",
            confidence
        );
        assert_eq!(json["text"], "Hello world");
    }
}
