//! Cloud diarization via the Deepgram Nova-2 API with `diarize=true`.
//!
//! The provider sends the combined segment text to Deepgram and maps the
//! returned word-level speaker assignments back onto the original time windows.
//!
//! Network access is gated through [`NetworkGuard`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{AppError, DiarizationErrorCode, NetworkErrorCode};
use crate::network::guard::NetworkGuard;

use super::{DiarizedSegment, DiarizationProvider, TranscriptSegment};

/// Deepgram pre-recorded transcription endpoint.
const DEEPGRAM_URL: &str =
    "https://api.deepgram.com/v1/listen?diarize=true&model=nova-2&punctuate=true";

// ─── API request / response types ────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct DeepgramRequest<'a> {
    /// Raw text submitted for diarization.  Deepgram treats this as the audio
    /// transcript when called in text-input mode.
    text: &'a str,
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    results: Option<DeepgramResults>,
}

#[derive(Debug, Deserialize)]
struct DeepgramResults {
    channels: Option<Vec<DeepgramChannel>>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Option<Vec<DeepgramAlternative>>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    words: Option<Vec<DeepgramWord>>,
}

#[derive(Debug, Deserialize)]
struct DeepgramWord {
    word: String,
    start: f64,
    end: f64,
    speaker: Option<u32>,
    confidence: Option<f64>,
}

// ─── Provider ────────────────────────────────────────────────────────────────

/// Cloud diarization backed by Deepgram Nova-2.
pub struct DeepgramProvider {
    guard: NetworkGuard,
}

impl DeepgramProvider {
    /// Create from an already-initialised [`NetworkGuard`].
    pub fn new(guard: NetworkGuard) -> Self {
        Self { guard }
    }
}

impl DiarizationProvider for DeepgramProvider {
    fn name(&self) -> &str {
        "deepgram"
    }

    fn diarize(
        &self,
        segments: &[TranscriptSegment],
    ) -> Result<Vec<DiarizedSegment>, AppError> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = crate::keychain::get("deepgram", "api_key")?
            .ok_or_else(|| AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: "Deepgram API key not found in keychain".into(),
            })?;

        let full_text: String = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let req = self
            .guard
            .client()
            .post(DEEPGRAM_URL)
            .header("Authorization", format!("Token {}", api_key))
            .header("Content-Type", "application/json")
            .json(&DeepgramRequest { text: &full_text });

        let response = tauri::async_runtime::block_on(self.guard.request(req))?;

        let status = response.status();
        if status == 401 || status == 403 {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: format!(
                    "Deepgram authentication failed (HTTP {})",
                    status.as_u16()
                ),
            });
        }
        if status == 429 {
            return Err(AppError::NetworkError {
                code: NetworkErrorCode::HttpError {
                    status: status.as_u16(),
                },
                message: "Deepgram rate limit exceeded".into(),
            });
        }
        if !status.is_success() {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::ApiError,
                message: format!("Deepgram API error: HTTP {}", status.as_u16()),
            });
        }

        let body: DeepgramResponse =
            tauri::async_runtime::block_on(response.json()).map_err(|e| {
                AppError::DiarizationError {
                    code: DiarizationErrorCode::ApiError,
                    message: format!("Failed to parse Deepgram response: {}", e),
                }
            })?;

        let words = extract_words(body)?;
        if words.is_empty() {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::NoSpeakersDetected,
                message: "Deepgram returned no words".into(),
            });
        }

        map_words_to_diarized_segments(&words, segments)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Flatten the nested Deepgram response into a flat word list.
fn extract_words(body: DeepgramResponse) -> Result<Vec<DeepgramWord>, AppError> {
    let words = body
        .results
        .and_then(|r| r.channels)
        .and_then(|mut ch| ch.pop())
        .and_then(|c| c.alternatives)
        .and_then(|mut alts| alts.pop())
        .and_then(|a| a.words)
        .unwrap_or_default();
    Ok(words)
}

/// Map Deepgram word-level output into [`DiarizedSegment`]s aligned with the
/// original segment time windows.
fn map_words_to_diarized_segments(
    words: &[DeepgramWord],
    original_segments: &[TranscriptSegment],
) -> Result<Vec<DiarizedSegment>, AppError> {
    let mut speaker_map: HashMap<u32, (String, String)> = HashMap::new();
    let mut speaker_counter: u32 = 0;

    let mut result = Vec::with_capacity(original_segments.len());

    for orig in original_segments {
        let start_s = orig.start_ms as f64 / 1000.0;
        let end_s = orig.end_ms as f64 / 1000.0;

        let seg_words: Vec<&DeepgramWord> = words
            .iter()
            .filter(|w| {
                let mid = (w.start + w.end) / 2.0;
                mid >= start_s && mid < end_s
            })
            .collect();

        if seg_words.is_empty() {
            // Fallback: use the first speaker or unknown.
            let (speaker_id, speaker_label) = speaker_map
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| {
                    speaker_counter += 1;
                    let id = format!("speaker_{}", speaker_counter);
                    let lbl = format!("Speaker {}", speaker_counter);
                    speaker_map.insert(0, (id.clone(), lbl.clone()));
                    (id, lbl)
                });

            result.push(DiarizedSegment {
                text: orig.text.clone(),
                start_ms: orig.start_ms,
                end_ms: orig.end_ms,
                speaker_id,
                speaker_label,
                confidence: orig.confidence.unwrap_or(1.0),
            });
            continue;
        }

        // Dominant speaker in this window.
        let raw_speaker = dominant_speaker_in_slice(&seg_words);
        let (speaker_id, speaker_label) =
            resolve_speaker(raw_speaker, &mut speaker_map, &mut speaker_counter);

        // Average confidence across words in this segment.
        let avg_confidence = {
            let sum: f64 = seg_words
                .iter()
                .map(|w| w.confidence.unwrap_or(1.0))
                .sum();
            (sum / seg_words.len() as f64) as f32
        };

        let text = seg_words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        debug!(
            speaker_id = %speaker_id,
            words = seg_words.len(),
            "deepgram: segment [{}, {}] assigned",
            orig.start_ms,
            orig.end_ms
        );

        result.push(DiarizedSegment {
            text,
            start_ms: orig.start_ms,
            end_ms: orig.end_ms,
            speaker_id,
            speaker_label,
            confidence: avg_confidence,
        });
    }

    Ok(result)
}

/// Return the speaker number that appears most in the slice.
fn dominant_speaker_in_slice(words: &[&DeepgramWord]) -> u32 {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in words {
        let id = w.speaker.unwrap_or(0);
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(id, _)| id)
        .unwrap_or(0)
}

fn resolve_speaker(
    raw_id: u32,
    speaker_map: &mut HashMap<u32, (String, String)>,
    counter: &mut u32,
) -> (String, String) {
    if let Some(pair) = speaker_map.get(&raw_id) {
        return pair.clone();
    }
    *counter += 1;
    let internal_id = format!("speaker_{}", counter);
    let label = format!("Speaker {}", counter);
    speaker_map.insert(raw_id, (internal_id.clone(), label.clone()));
    (internal_id, label)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(word: &str, start: f64, end: f64, speaker: u32) -> DeepgramWord {
        DeepgramWord {
            word: word.to_string(),
            start,
            end,
            speaker: Some(speaker),
            confidence: Some(0.95),
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
    fn test_extract_words_from_valid_response() {
        let body = DeepgramResponse {
            results: Some(DeepgramResults {
                channels: Some(vec![DeepgramChannel {
                    alternatives: Some(vec![DeepgramAlternative {
                        words: Some(vec![make_word("hello", 0.0, 0.5, 0)]),
                    }]),
                }]),
            }),
        };
        let words = extract_words(body).unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].word, "hello");
    }

    #[test]
    fn test_extract_words_from_empty_response() {
        let body = DeepgramResponse { results: None };
        let words = extract_words(body).unwrap();
        assert!(words.is_empty());
    }

    #[test]
    fn test_map_words_single_speaker() {
        let words = vec![
            make_word("hello", 0.0, 0.4, 0),
            make_word("world", 0.5, 0.9, 0),
        ];
        let segs = vec![make_seg(0, 0, 1000, "hello world")];
        let result = map_words_to_diarized_segments(&words, &segs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].speaker_id, "speaker_1");
        assert_eq!(result[0].speaker_label, "Speaker 1");
    }

    #[test]
    fn test_map_words_two_speakers() {
        let words = vec![
            make_word("good", 0.0, 0.3, 0),
            make_word("morning", 0.3, 0.7, 0),
            make_word("indeed", 1.1, 1.5, 1),
        ];
        let segs = vec![
            make_seg(0, 0, 800, "good morning"),
            make_seg(1, 1000, 2000, "indeed"),
        ];
        let result = map_words_to_diarized_segments(&words, &segs).unwrap();
        assert_eq!(result.len(), 2);
        assert_ne!(result[0].speaker_id, result[1].speaker_id);
    }

    #[test]
    fn test_resolve_speaker_stable() {
        let mut map = HashMap::new();
        let mut counter = 0u32;
        let first = resolve_speaker(0, &mut map, &mut counter);
        let second = resolve_speaker(0, &mut map, &mut counter);
        assert_eq!(first, second);
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_dominant_speaker_in_slice_tie_break() {
        let w0 = make_word("a", 0.0, 0.1, 0);
        let w1 = make_word("b", 0.1, 0.2, 1);
        let w2 = make_word("c", 0.2, 0.3, 0);
        let words = vec![&w0, &w1, &w2];
        assert_eq!(dominant_speaker_in_slice(&words), 0);
    }

    #[test]
    fn test_confidence_averaged() {
        let mut w0 = make_word("hi", 0.0, 0.5, 0);
        w0.confidence = Some(0.8);
        let mut w1 = make_word("there", 0.5, 1.0, 0);
        w1.confidence = Some(0.6);
        let segs = vec![make_seg(0, 0, 1000, "hi there")];
        let result = map_words_to_diarized_segments(&[w0, w1], &segs).unwrap();
        assert!((result[0].confidence - 0.7).abs() < 0.01);
    }
}
