use crate::cloud_transcription::{CloudSegment, CloudTranscriptResult, CloudTranscriptionProvider};
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use serde::Deserialize;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

const API_URL: &str = "https://api.deepgram.com/v1/listen";

/// Deepgram Nova-2 cloud transcription provider.
pub struct DeepgramProvider {
    api_key: String,
    guard: Arc<NetworkGuard>,
}

#[derive(Deserialize)]
struct DeepgramResponse {
    results: Option<DeepgramResults>,
}

#[derive(Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
    detected_language: Option<String>,
}

#[derive(Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    words: Option<Vec<DeepgramWord>>,
}

#[derive(Deserialize)]
struct DeepgramWord {
    word: String,
    start: f64,
    end: f64,
    speaker: Option<u32>,
}

impl DeepgramProvider {
    /// Create a new Deepgram provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        Self { api_key, guard }
    }
}

impl CloudTranscriptionProvider for DeepgramProvider {
    fn name(&self) -> &str {
        "deepgram"
    }

    fn cost_per_minute_usd(&self) -> f64 {
        0.0043
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

            let mut url =
                format!("{}?model=nova-2&smart_format=true&diarize=true", API_URL);

            if let Some(ref lang_code) = lang {
                url.push_str(&format!("&language={}", lang_code));
            }

            let req = self
                .guard
                .client()
                .post(&url)
                .header("Authorization", format!("Token {}", self.api_key))
                .header("Content-Type", "audio/wav")
                .body(file_bytes);

            let response = self.guard.request(req).await?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::InvalidApiKey,
                    message: "Invalid Deepgram API key".into(),
                });
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Deepgram error ({}): {}", status, body),
                });
            }

            let resp: DeepgramResponse =
                response.json().await.map_err(|e| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Failed to parse Deepgram response: {}", e),
                })?;

            let channel = resp
                .results
                .and_then(|r| r.channels.into_iter().next())
                .ok_or_else(|| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: "No transcription results".into(),
                })?;

            let detected_lang = channel.detected_language.clone().unwrap_or_else(|| "en".into());
            let alt = channel
                .alternatives
                .into_iter()
                .next()
                .unwrap_or(DeepgramAlternative {
                    transcript: String::new(),
                    words: None,
                });

            // Group words into segments by speaker or sentence boundaries
            let segments = group_words_into_segments(alt.words.unwrap_or_default());

            Ok(CloudTranscriptResult {
                text: alt.transcript,
                segments,
                language: detected_lang,
            })
        })
    }
}

/// Group Deepgram words into segments based on speaker changes.
fn group_words_into_segments(words: Vec<DeepgramWord>) -> Vec<CloudSegment> {
    if words.is_empty() {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_start = words[0].start;
    let mut current_speaker = words[0].speaker;

    for word in &words {
        if word.speaker != current_speaker && !current_text.is_empty() {
            segments.push(CloudSegment {
                start_ms: (current_start * 1000.0) as u64,
                end_ms: (word.start * 1000.0) as u64,
                text: current_text.trim().to_string(),
                speaker: current_speaker.map(|s| format!("Speaker {}", s)),
            });
            current_text = String::new();
            current_start = word.start;
            current_speaker = word.speaker;
        }
        if !current_text.is_empty() {
            current_text.push(' ');
        }
        current_text.push_str(&word.word);
    }

    // Push the last segment
    if !current_text.is_empty() {
        if let Some(last_word) = words.last() {
            segments.push(CloudSegment {
                start_ms: (current_start * 1000.0) as u64,
                end_ms: (last_word.end * 1000.0) as u64,
                text: current_text.trim().to_string(),
                speaker: current_speaker.map(|s| format!("Speaker {}", s)),
            });
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_deepgram_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = DeepgramProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "deepgram");
    }

    #[test]
    fn test_group_words_empty() {
        let segments = group_words_into_segments(vec![]);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_group_words_single_speaker() {
        let words = vec![
            DeepgramWord { word: "Hello".into(), start: 0.0, end: 0.5, speaker: Some(0) },
            DeepgramWord { word: "world".into(), start: 0.5, end: 1.0, speaker: Some(0) },
        ];
        let segments = group_words_into_segments(words);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello world");
    }

    #[test]
    fn test_group_words_speaker_change() {
        let words = vec![
            DeepgramWord { word: "Hello".into(), start: 0.0, end: 0.5, speaker: Some(0) },
            DeepgramWord { word: "Hi".into(), start: 1.0, end: 1.5, speaker: Some(1) },
        ];
        let segments = group_words_into_segments(words);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker, Some("Speaker 0".into()));
        assert_eq!(segments[1].speaker, Some("Speaker 1".into()));
    }
}
