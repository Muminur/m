//! Cloud diarization via the ElevenLabs Speech-to-Text (Scribe) API.
//!
//! The provider submits pre-transcribed segments to the ElevenLabs API with
//! diarization enabled and maps returned speaker IDs to sequential labels.
//!
//! Network access is gated through [`NetworkGuard`] so offline/local-only
//! policies are honoured automatically.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{AppError, DiarizationErrorCode, NetworkErrorCode};
use crate::network::guard::NetworkGuard;

use super::{DiarizationProvider, DiarizedSegment, TranscriptSegment};

/// ElevenLabs Scribe speech-to-text endpoint.
const ELEVENLABS_STT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

// ─── API request / response types ────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ElevenLabsRequest<'a> {
    /// Raw text to re-analyse (used when we already have segments).
    text: &'a str,
    diarize: bool,
}

#[derive(Debug, Deserialize)]
struct ElevenLabsResponse {
    words: Option<Vec<ElevenLabsWord>>,
}

#[derive(Debug, Deserialize)]
struct ElevenLabsWord {
    text: String,
    start: f64,
    end: f64,
    #[serde(rename = "speaker_id")]
    speaker_id: Option<String>,
}

// ─── Provider ────────────────────────────────────────────────────────────────

/// Cloud diarization backed by ElevenLabs Scribe.
pub struct ElevenLabsProvider {
    guard: NetworkGuard,
}

impl ElevenLabsProvider {
    /// Create from an already-initialised [`NetworkGuard`].
    pub fn new(guard: NetworkGuard) -> Self {
        Self { guard }
    }
}

impl DiarizationProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn diarize(&self, segments: &[TranscriptSegment]) -> Result<Vec<DiarizedSegment>, AppError> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        // Retrieve API key from keychain (macOS) or return an error on other
        // platforms so the caller knows this provider is unavailable.
        let api_key = crate::keychain::get("elevenlabs", "api_key")?.ok_or_else(|| {
            AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: "ElevenLabs API key not found in keychain".into(),
            }
        })?;

        // Concatenate all segment texts into a single string for the API call.
        let full_text: String = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // Build the request via NetworkGuard so offline/local-only policies
        // are enforced before any bytes leave the machine.
        let req = self
            .guard
            .client()
            .post(ELEVENLABS_STT_URL)
            .header("xi-api-key", &api_key)
            .header("Content-Type", "application/json")
            .json(&ElevenLabsRequest {
                text: &full_text,
                diarize: true,
            });

        // Execute synchronously inside a blocking context.
        let response = tauri::async_runtime::block_on(self.guard.request(req))?;

        let status = response.status();
        if status == 401 || status == 403 {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: format!(
                    "ElevenLabs authentication failed (HTTP {})",
                    status.as_u16()
                ),
            });
        }
        if status == 429 {
            return Err(AppError::NetworkError {
                code: NetworkErrorCode::HttpError {
                    status: status.as_u16(),
                },
                message: "ElevenLabs rate limit exceeded".into(),
            });
        }
        if !status.is_success() {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: format!("ElevenLabs API error: HTTP {}", status.as_u16()),
            });
        }

        let body: ElevenLabsResponse =
            tauri::async_runtime::block_on(response.json()).map_err(|e| {
                AppError::DiarizationError {
                    code: DiarizationErrorCode::ApiError,
                    message: format!("Failed to parse ElevenLabs response: {}", e),
                }
            })?;

        let words = body.words.unwrap_or_default();
        if words.is_empty() {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::NoSpeakersDetected,
                message: "ElevenLabs returned no words".into(),
            });
        }

        map_words_to_diarized_segments(&words, segments)
    }
}

// ─── Mapping helpers ─────────────────────────────────────────────────────────

/// Convert ElevenLabs word-level output into [`DiarizedSegment`]s that align
/// with the original segments' time boundaries.
fn map_words_to_diarized_segments(
    words: &[ElevenLabsWord],
    original_segments: &[TranscriptSegment],
) -> Result<Vec<DiarizedSegment>, AppError> {
    // Build a stable speaker_id → label mapping so labels are sequential.
    let mut speaker_map: HashMap<String, (String, String)> = HashMap::new();
    let mut speaker_counter: u32 = 0;

    // Group words back into the original time windows.
    let mut result = Vec::with_capacity(original_segments.len());

    for orig in original_segments {
        let start_s = orig.start_ms as f64 / 1000.0;
        let end_s = orig.end_ms as f64 / 1000.0;

        // Collect words whose midpoint falls inside this segment's window.
        let seg_words: Vec<&ElevenLabsWord> = words
            .iter()
            .filter(|w| {
                let mid = (w.start + w.end) / 2.0;
                mid >= start_s && mid < end_s
            })
            .collect();

        if seg_words.is_empty() {
            // Fall back: attribute to the majority speaker across all words.
            let fallback_speaker = dominant_speaker(words, &mut speaker_map, &mut speaker_counter);
            result.push(DiarizedSegment {
                text: orig.text.clone(),
                start_ms: orig.start_ms,
                end_ms: orig.end_ms,
                speaker_id: fallback_speaker.0,
                speaker_label: fallback_speaker.1,
                confidence: orig.confidence.unwrap_or(1.0),
            });
            continue;
        }

        // Dominant speaker for this window.
        let raw_speaker_id = dominant_speaker_in_slice(&seg_words);
        let (speaker_id, speaker_label) =
            resolve_speaker(&raw_speaker_id, &mut speaker_map, &mut speaker_counter);

        let text = seg_words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        debug!(
            speaker_id = %speaker_id,
            words = seg_words.len(),
            "elevenlabs: segment [{}, {}] assigned",
            orig.start_ms,
            orig.end_ms
        );

        result.push(DiarizedSegment {
            text,
            start_ms: orig.start_ms,
            end_ms: orig.end_ms,
            speaker_id,
            speaker_label,
            confidence: orig.confidence.unwrap_or(1.0),
        });
    }

    Ok(result)
}

/// Return the `(speaker_id, speaker_label)` that appears most in `words`.
fn dominant_speaker(
    words: &[ElevenLabsWord],
    speaker_map: &mut HashMap<String, (String, String)>,
    counter: &mut u32,
) -> (String, String) {
    let words_ref: Vec<&ElevenLabsWord> = words.iter().collect();
    let raw = dominant_speaker_in_slice(&words_ref);
    resolve_speaker(&raw, speaker_map, counter)
}

fn dominant_speaker_in_slice(words: &[&ElevenLabsWord]) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for w in words {
        let id = w.speaker_id.as_deref().unwrap_or("unknown");
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(id, _)| id.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Resolve (or create) an internal speaker id / label pair.
fn resolve_speaker(
    raw_id: &str,
    speaker_map: &mut HashMap<String, (String, String)>,
    counter: &mut u32,
) -> (String, String) {
    if let Some(pair) = speaker_map.get(raw_id) {
        return pair.clone();
    }
    *counter += 1;
    let internal_id = format!("speaker_{}", counter);
    let label = format!("Speaker {}", counter);
    speaker_map.insert(raw_id.to_string(), (internal_id.clone(), label.clone()));
    (internal_id, label)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, start: f64, end: f64, speaker: &str) -> ElevenLabsWord {
        ElevenLabsWord {
            text: text.to_string(),
            start,
            end,
            speaker_id: Some(speaker.to_string()),
        }
    }

    fn make_seg(index: i64, start_ms: u64, end_ms: u64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: format!("s{}", index),
            transcript_id: "t1".into(),
            index_num: index,
            start_ms,
            end_ms,
            text: text.to_string(),
            confidence: Some(0.9),
        }
    }

    #[test]
    fn test_map_words_to_segments_single_speaker() {
        let words = vec![
            make_word("Hello", 0.0, 0.5, "SPEAKER_00"),
            make_word("world", 0.5, 1.0, "SPEAKER_00"),
        ];
        let segs = vec![make_seg(0, 0, 1000, "Hello world")];
        let result = map_words_to_diarized_segments(&words, &segs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].speaker_id, "speaker_1");
        assert_eq!(result[0].speaker_label, "Speaker 1");
    }

    #[test]
    fn test_map_words_two_speakers() {
        let words = vec![
            make_word("I", 0.0, 0.3, "SPEAKER_00"),
            make_word("say", 0.3, 0.6, "SPEAKER_00"),
            make_word("hello", 1.1, 1.4, "SPEAKER_01"),
            make_word("back", 1.4, 1.8, "SPEAKER_01"),
        ];
        let segs = vec![
            make_seg(0, 0, 800, "I say"),
            make_seg(1, 1000, 2000, "hello back"),
        ];
        let result = map_words_to_diarized_segments(&words, &segs).unwrap();
        assert_eq!(result.len(), 2);
        assert_ne!(result[0].speaker_id, result[1].speaker_id);
    }

    #[test]
    fn test_empty_words_falls_back_gracefully() {
        let words: Vec<ElevenLabsWord> = vec![];
        // With no words, dominant_speaker is called; map_words_to_diarized_segments
        // should still return one segment per input segment (fallback path).
        // We force it via empty words and verify no panic.
        let segs = vec![make_seg(0, 0, 1000, "something")];
        // words is empty so the fallback branch runs; dominant_speaker over
        // empty slice uses the "unknown" key.
        let result = map_words_to_diarized_segments(&words, &segs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "something");
    }

    #[test]
    fn test_resolve_speaker_is_stable() {
        let mut map = HashMap::new();
        let mut counter = 0u32;
        let first = resolve_speaker("SPK_0", &mut map, &mut counter);
        let second = resolve_speaker("SPK_0", &mut map, &mut counter);
        assert_eq!(first, second);
    }

    #[test]
    fn test_resolve_speaker_increments_counter() {
        let mut map = HashMap::new();
        let mut counter = 0u32;
        let a = resolve_speaker("SPK_0", &mut map, &mut counter);
        let b = resolve_speaker("SPK_1", &mut map, &mut counter);
        assert_ne!(a.0, b.0);
        assert_eq!(counter, 2);
    }
}
