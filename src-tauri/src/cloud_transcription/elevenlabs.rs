use crate::cloud_transcription::{CloudSegment, CloudTranscriptResult, CloudTranscriptionProvider};
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use serde::Deserialize;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

const API_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

/// ElevenLabs cloud transcription provider.
pub struct ElevenLabsProvider {
    api_key: String,
    guard: Arc<NetworkGuard>,
}

#[derive(Deserialize)]
struct ElevenLabsResponse {
    text: Option<String>,
    words: Option<Vec<ElevenLabsWord>>,
    language_code: Option<String>,
}

#[derive(Deserialize)]
struct ElevenLabsWord {
    text: String,
    start: Option<f64>,
    end: Option<f64>,
    speaker_id: Option<String>,
}

impl ElevenLabsProvider {
    /// Create a new ElevenLabs provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        Self { api_key, guard }
    }
}

impl CloudTranscriptionProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn cost_per_minute_usd(&self) -> f64 {
        0.01
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
                .text("model_id", "scribe_v1");

            if let Some(ref lang_code) = lang {
                form = form.text("language_code", lang_code.clone());
            }

            let req = self
                .guard
                .client()
                .post(API_URL)
                .header("xi-api-key", &self.api_key)
                .multipart(form);

            let response = self.guard.request(req).await?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::InvalidApiKey,
                    message: "Invalid ElevenLabs API key".into(),
                });
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("ElevenLabs error ({}): {}", status, body),
                });
            }

            let resp: ElevenLabsResponse =
                response.json().await.map_err(|e| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Failed to parse response: {}", e),
                })?;

            let text = resp.text.unwrap_or_default();
            let segments = group_words(resp.words.unwrap_or_default());
            let language = resp.language_code.unwrap_or_else(|| "en".into());

            Ok(CloudTranscriptResult {
                text,
                segments,
                language,
            })
        })
    }
}

/// Group ElevenLabs words into segments by speaker.
fn group_words(words: Vec<ElevenLabsWord>) -> Vec<CloudSegment> {
    if words.is_empty() {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_start: f64 = words[0].start.unwrap_or(0.0);
    let mut current_speaker = words[0].speaker_id.clone();
    let mut current_end: f64 = 0.0;

    for word in &words {
        if word.speaker_id != current_speaker && !current_text.is_empty() {
            segments.push(CloudSegment {
                start_ms: (current_start * 1000.0) as u64,
                end_ms: (current_end * 1000.0) as u64,
                text: current_text.trim().to_string(),
                speaker: current_speaker.clone(),
            });
            current_text = String::new();
            current_start = word.start.unwrap_or(0.0);
            current_speaker = word.speaker_id.clone();
        }
        if !current_text.is_empty() {
            current_text.push(' ');
        }
        current_text.push_str(&word.text);
        current_end = word.end.unwrap_or(current_end);
    }

    if !current_text.is_empty() {
        segments.push(CloudSegment {
            start_ms: (current_start * 1000.0) as u64,
            end_ms: (current_end * 1000.0) as u64,
            text: current_text.trim().to_string(),
            speaker: current_speaker,
        });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_elevenlabs_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = ElevenLabsProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "elevenlabs");
    }
}
