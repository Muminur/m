use crate::cloud_transcription::{CloudSegment, CloudTranscriptResult, CloudTranscriptionProvider};
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use serde::Deserialize;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

const API_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Groq Whisper cloud transcription provider.
///
/// Uses the OpenAI-compatible whisper endpoint on Groq infrastructure.
pub struct GroqWhisperProvider {
    api_key: String,
    guard: Arc<NetworkGuard>,
}

#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
    segments: Option<Vec<WhisperSegment>>,
    language: Option<String>,
}

#[derive(Deserialize)]
struct WhisperSegment {
    start: f64,
    end: f64,
    text: String,
}

impl GroqWhisperProvider {
    /// Create a new Groq Whisper provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        Self { api_key, guard }
    }
}

impl CloudTranscriptionProvider for GroqWhisperProvider {
    fn name(&self) -> &str {
        "groq_whisper"
    }

    fn cost_per_minute_usd(&self) -> f64 {
        0.0011
    }

    fn transcribe(
        &self,
        audio_path: &Path,
        language: Option<&str>,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<CloudTranscriptResult, AppError>> + Send + '_>,
    > {
        let path = audio_path.to_path_buf();
        let lang = language.map(|s| s.to_string());

        Box::pin(async move {
            let file_bytes = tokio::fs::read(&path).await.map_err(|e| {
                AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::UploadFailed,
                    message: format!("Failed to read audio file: {}", e),
                }
            })?;

            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "audio.wav".into());

            let file_part = reqwest::multipart::Part::bytes(file_bytes)
                .file_name(file_name)
                .mime_str("audio/wav")
                .map_err(|e| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::UploadFailed,
                    message: format!("Failed to create multipart: {}", e),
                })?;

            let mut form = reqwest::multipart::Form::new()
                .part("file", file_part)
                .text("model", "whisper-large-v3")
                .text("response_format", "verbose_json");

            if let Some(ref lang_code) = lang {
                form = form.text("language", lang_code.clone());
            }

            let req = self
                .guard
                .client()
                .post(API_URL)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .multipart(form);

            let response = self.guard.request(req).await?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::InvalidApiKey,
                    message: "Invalid Groq API key".into(),
                });
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Groq Whisper error ({}): {}", status, body),
                });
            }

            let resp: WhisperResponse =
                response.json().await.map_err(|e| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Failed to parse response: {}", e),
                })?;

            let segments = resp
                .segments
                .unwrap_or_default()
                .into_iter()
                .map(|s| CloudSegment {
                    start_ms: (s.start * 1000.0) as u64,
                    end_ms: (s.end * 1000.0) as u64,
                    text: s.text.trim().to_string(),
                    speaker: None,
                })
                .collect();

            Ok(CloudTranscriptResult {
                text: resp.text,
                segments,
                language: resp.language.unwrap_or_else(|| "en".into()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_groq_whisper_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = GroqWhisperProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "groq_whisper");
        assert!((provider.cost_per_minute_usd() - 0.0011).abs() < 0.0001);
    }
}
