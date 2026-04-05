//! Local speaker diarization using whisper.cpp's `[SPEAKER_TURN]` token.
//!
//! whisper.cpp (and its Rust wrapper `whisper-rs`) can emit the special token
//! `[SPEAKER_TURN]` at points where the speaker is inferred to have changed.
//! This provider scans the raw segment text for that token and assigns
//! sequential speaker labels (Speaker 1, Speaker 2, …).

use std::collections::HashMap;

use tracing::debug;

use crate::error::AppError;

use super::{DiarizationProvider, DiarizedSegment, TranscriptSegment};

/// The literal token whisper.cpp inserts at speaker-change boundaries.
const SPEAKER_TURN_TOKEN: &str = "[SPEAKER_TURN]";

/// Local diarization provider backed by whisper.cpp tinydiarize tokens.
pub struct TinydiarizeProvider;

impl TinydiarizeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TinydiarizeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DiarizationProvider for TinydiarizeProvider {
    fn name(&self) -> &str {
        "tinydiarize"
    }

    fn diarize(&self, segments: &[TranscriptSegment]) -> Result<Vec<DiarizedSegment>, AppError> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        // speaker_counter: how many distinct speakers we have seen so far.
        let mut speaker_counter: u32 = 1;
        // Map internal id → human label for persistence across segments.
        let mut speaker_labels: HashMap<String, String> = HashMap::new();
        // The current speaker id (stable key).
        let mut current_speaker_id = speaker_id_key(speaker_counter);
        speaker_labels.insert(current_speaker_id.clone(), speaker_label(speaker_counter));

        let mut result = Vec::with_capacity(segments.len());

        for seg in segments {
            if seg.text.contains(SPEAKER_TURN_TOKEN) {
                // Emit one DiarizedSegment for the text BEFORE the turn token.
                // Then advance to the next speaker.
                let before = seg
                    .text
                    .splitn(2, SPEAKER_TURN_TOKEN)
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                // Only push a segment if there is actual text before the turn.
                if !before.is_empty() {
                    result.push(DiarizedSegment {
                        text: before,
                        start_ms: seg.start_ms,
                        end_ms: seg.end_ms,
                        speaker_id: current_speaker_id.clone(),
                        speaker_label: speaker_labels[&current_speaker_id].clone(),
                        confidence: seg.confidence.unwrap_or(1.0),
                    });
                }

                // Advance speaker.
                speaker_counter += 1;
                current_speaker_id = speaker_id_key(speaker_counter);
                speaker_labels
                    .entry(current_speaker_id.clone())
                    .or_insert_with(|| speaker_label(speaker_counter));

                // Text after the turn token (same segment, new speaker).
                let after = seg
                    .text
                    .splitn(2, SPEAKER_TURN_TOKEN)
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                // Replace the raw token with clean text in the output segment.
                let clean = if after.is_empty() {
                    // The turn was at the very end; the next segment will carry the
                    // new speaker's text — emit a stub so timestamps are preserved.
                    String::new()
                } else {
                    after
                };

                debug!(
                    speaker_id = %current_speaker_id,
                    "tinydiarize: speaker turn detected in segment {}",
                    seg.index_num
                );

                // Only push if there is non-empty text to avoid vacuous segments.
                if !clean.is_empty() {
                    result.push(DiarizedSegment {
                        text: clean,
                        start_ms: seg.start_ms,
                        end_ms: seg.end_ms,
                        speaker_id: current_speaker_id.clone(),
                        speaker_label: speaker_labels[&current_speaker_id].clone(),
                        confidence: seg.confidence.unwrap_or(1.0),
                    });
                }
            } else {
                // No turn token — entire segment belongs to the current speaker.
                result.push(DiarizedSegment {
                    text: seg.text.trim().to_string(),
                    start_ms: seg.start_ms,
                    end_ms: seg.end_ms,
                    speaker_id: current_speaker_id.clone(),
                    speaker_label: speaker_labels[&current_speaker_id].clone(),
                    confidence: seg.confidence.unwrap_or(1.0),
                });
            }
        }

        Ok(result)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Stable internal key, e.g. `"speaker_1"`.
fn speaker_id_key(n: u32) -> String {
    format!("speaker_{}", n)
}

/// Human-readable label, e.g. `"Speaker 1"`.
fn speaker_label(n: u32) -> String {
    format!("Speaker {}", n)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(index: i64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: format!("seg-{}", index),
            transcript_id: "t1".into(),
            index_num: index,
            start_ms: (index as u64) * 1000,
            end_ms: (index as u64) * 1000 + 500,
            text: text.to_string(),
            confidence: Some(0.9),
        }
    }

    #[test]
    fn test_empty_segments_returns_empty() {
        let provider = TinydiarizeProvider::new();
        let result = provider.diarize(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_speaker_no_turn_tokens() {
        let provider = TinydiarizeProvider::new();
        let segments = vec![seg(0, "Hello world."), seg(1, "How are you?")];
        let result = provider.diarize(&segments).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].speaker_id, "speaker_1");
        assert_eq!(result[0].speaker_label, "Speaker 1");
        assert_eq!(result[1].speaker_id, "speaker_1");
    }

    #[test]
    fn test_two_speakers_with_turn_token() {
        let provider = TinydiarizeProvider::new();
        let segments = vec![
            seg(0, "Hello, I am Alice."),
            seg(1, "Nice to meet you. [SPEAKER_TURN] Hi, I am Bob."),
            seg(2, "Great to meet you too."),
        ];
        let result = provider.diarize(&segments).unwrap();
        // Seg 0 → Speaker 1; seg 1 splits into before (Speaker 1) + after (Speaker 2); seg 2 → Speaker 2
        assert!(result.len() >= 3);
        let speaker_ids: Vec<&str> = result.iter().map(|s| s.speaker_id.as_str()).collect();
        assert!(speaker_ids.contains(&"speaker_1"));
        assert!(speaker_ids.contains(&"speaker_2"));
        // Text after turn should not contain the raw token
        for seg in &result {
            assert!(!seg.text.contains(SPEAKER_TURN_TOKEN));
        }
    }

    #[test]
    fn test_no_turn_tokens_in_any_segment() {
        let provider = TinydiarizeProvider::new();
        let segments = vec![
            seg(0, "First sentence."),
            seg(1, "Second sentence."),
            seg(2, "Third sentence."),
        ];
        let result = provider.diarize(&segments).unwrap();
        assert_eq!(result.len(), 3);
        // All must be assigned to the same speaker
        for r in &result {
            assert_eq!(r.speaker_id, "speaker_1");
            assert_eq!(r.speaker_label, "Speaker 1");
        }
    }

    #[test]
    fn test_multiple_consecutive_turns() {
        let provider = TinydiarizeProvider::new();
        // Each segment has a turn at the very end (no text after token)
        let segments = vec![
            seg(0, "Alpha [SPEAKER_TURN]"),
            seg(1, "Beta [SPEAKER_TURN]"),
            seg(2, "Gamma"),
        ];
        let result = provider.diarize(&segments).unwrap();
        // "Alpha" → speaker_1; "Beta" → speaker_2; "Gamma" → speaker_3
        let texts: Vec<&str> = result.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Alpha"));
        assert!(texts.contains(&"Beta"));
        assert!(texts.contains(&"Gamma"));
        let gamma = result.iter().find(|s| s.text == "Gamma").unwrap();
        assert_eq!(gamma.speaker_id, "speaker_3");
    }

    #[test]
    fn test_turn_token_at_start_of_segment() {
        let provider = TinydiarizeProvider::new();
        // Turn token at the very start — no "before" text, so only "after" emitted
        let segments = vec![
            seg(0, "I start first."),
            seg(1, "[SPEAKER_TURN] Then I respond."),
        ];
        let result = provider.diarize(&segments).unwrap();
        assert!(result.len() >= 2);
        let respond_seg = result
            .iter()
            .find(|s| s.text.contains("Then I respond"))
            .unwrap();
        assert_eq!(respond_seg.speaker_id, "speaker_2");
    }

    #[test]
    fn test_speaker_label_persistence() {
        let provider = TinydiarizeProvider::new();
        let segments = vec![seg(0, "A [SPEAKER_TURN] B"), seg(1, "C")];
        let result = provider.diarize(&segments).unwrap();
        // "C" in seg 1 should be speaker_2, same as "B"
        let b_seg = result.iter().find(|s| s.text == "B").unwrap();
        let c_seg = result.iter().find(|s| s.text == "C").unwrap();
        assert_eq!(b_seg.speaker_id, c_seg.speaker_id);
    }

    #[test]
    fn test_confidence_propagated() {
        let provider = TinydiarizeProvider::new();
        let mut s = seg(0, "Hello");
        s.confidence = Some(0.75);
        let result = provider.diarize(&[s]).unwrap();
        assert!((result[0].confidence - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_missing_confidence_defaults_to_one() {
        let provider = TinydiarizeProvider::new();
        let mut s = seg(0, "Hello");
        s.confidence = None;
        let result = provider.diarize(&[s]).unwrap();
        assert!((result[0].confidence - 1.0).abs() < f32::EPSILON);
    }
}
