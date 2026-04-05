//! Tauri commands for the diarization subsystem.
//!
//! All commands return `Result<T, AppError>` with typed error codes so the
//! front-end can pattern-match on `kind` + `detail.code`.

use std::sync::Arc;

use tauri::{command, State};

use crate::database::{segments as db_segments, transcripts as db_transcripts, Database};
use crate::diarization::{
    available_providers, tinydiarize::TinydiarizeProvider, DiarizedSegment, DiarizationProvider,
    ProviderInfo, TranscriptSegment,
};
use crate::error::{AppError, DiarizationErrorCode};
use crate::network::guard::NetworkGuard;

// ─── Response types ───────────────────────────────────────────────────────────

/// Result returned by `diarize_transcript`.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiarizeTranscriptResult {
    pub transcript_id: String,
    pub segment_count: usize,
    pub speaker_count: usize,
    pub segments: Vec<DiarizedSegment>,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Run diarization on an existing transcript using the specified provider.
///
/// `provider` must be one of: `"tinydiarize"`, `"elevenlabs"`, `"deepgram"`.
#[command]
pub async fn diarize_transcript(
    transcript_id: String,
    provider: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<DiarizeTranscriptResult, AppError> {
    // 1. Verify the transcript exists.
    {
        let conn = db.get()?;
        db_transcripts::get_by_id(&conn, &transcript_id).map_err(|_| {
            AppError::DiarizationError {
                code: DiarizationErrorCode::InvalidTranscript,
                message: format!("Transcript '{}' not found", transcript_id),
            }
        })?;
    }

    // 2. Load segments from the database.
    let raw_segments: Vec<TranscriptSegment> = {
        let conn = db.get()?;
        db_segments::get_by_transcript(&conn, &transcript_id)?
            .into_iter()
            .map(|row| TranscriptSegment {
                id: row.id,
                transcript_id: row.transcript_id,
                index_num: row.index_num,
                start_ms: row.start_ms as u64,
                end_ms: row.end_ms as u64,
                text: row.text,
                confidence: row.confidence.map(|c| c as f32),
            })
            .collect()
    };

    if raw_segments.is_empty() {
        return Err(AppError::DiarizationError {
            code: DiarizationErrorCode::InvalidTranscript,
            message: format!(
                "Transcript '{}' has no segments to diarize",
                transcript_id
            ),
        });
    }

    // 3. Select provider — resolved once, no dynamic dispatch allocation heap
    //    overhead for the local case.
    let diarized: Vec<DiarizedSegment> = match provider.as_str() {
        "tinydiarize" => {
            let p = TinydiarizeProvider::new();
            p.diarize(&raw_segments)?
        }
        "elevenlabs" => {
            // NetworkGuard is shared application state; clone the inner policy
            // to construct a provider-scoped guard instance.
            use crate::diarization::elevenlabs::ElevenLabsProvider;
            use crate::network::guard::NetworkGuard as NG;
            
            let policy = guard.policy().clone();
            let new_guard = NG::new(policy)?;
            let p = ElevenLabsProvider::new(new_guard);
            p.diarize(&raw_segments)?
        }
        "deepgram" => {
            use crate::diarization::deepgram::DeepgramProvider;
            use crate::network::guard::NetworkGuard as NG;
            let policy = guard.policy().clone();
            let new_guard = NG::new(policy)?;
            let p = DeepgramProvider::new(new_guard);
            p.diarize(&raw_segments)?
        }
        unknown => {
            return Err(AppError::DiarizationError {
                code: DiarizationErrorCode::ProviderNotFound,
                message: format!("Unknown diarization provider: '{}'", unknown),
            });
        }
    };

    // 4. Persist speaker_id assignments back onto the segments table.
    {
        let conn = db.get()?;
        for (orig, diarized_seg) in raw_segments.iter().zip(diarized.iter()) {
            conn.execute(
                "UPDATE segments SET speaker_id = ?1 WHERE id = ?2",
                rusqlite::params![diarized_seg.speaker_id, orig.id],
            )
            .map_err(|e| AppError::StorageError {
                code: crate::error::StorageErrorCode::DatabaseError,
                message: format!("Failed to persist speaker_id for segment {}: {}", orig.id, e),
            })?;
        }
    }

    let speaker_count = {
        let mut ids: Vec<&str> = diarized.iter().map(|s| s.speaker_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    };

    Ok(DiarizeTranscriptResult {
        transcript_id,
        segment_count: diarized.len(),
        speaker_count,
        segments: diarized,
    })
}

/// Return the list of providers the application can use right now.
///
/// `tinydiarize` is always present.  Cloud providers are listed only when an
/// API key is stored in the keychain.
#[command]
pub fn get_diarization_providers() -> Vec<ProviderInfo> {
    available_providers()
}

/// Rename a speaker across all segments of a transcript.
///
/// This only updates the `speaker_label` stored in the segments; the stable
/// `speaker_id` is preserved so the mapping survives further edits.
#[command]
pub async fn update_speaker_label(
    transcript_id: String,
    speaker_id: String,
    new_label: String,
    db: State<'_, Arc<Database>>,
) -> Result<usize, AppError> {
    // Validate inputs — speaker_id must be non-empty and transcript must exist.
    if speaker_id.is_empty() {
        return Err(AppError::DiarizationError {
            code: DiarizationErrorCode::ProviderNotFound,
            message: "speaker_id must not be empty".into(),
        });
    }
    if new_label.trim().is_empty() {
        return Err(AppError::DiarizationError {
            code: DiarizationErrorCode::ProviderNotFound,
            message: "new_label must not be blank".into(),
        });
    }

    let conn = db.get()?;

    // Verify transcript exists before touching rows.
    db_transcripts::get_by_id(&conn, &transcript_id).map_err(|_| {
        AppError::DiarizationError {
            code: DiarizationErrorCode::InvalidTranscript,
            message: format!("Transcript '{}' not found", transcript_id),
        }
    })?;

    // speaker_label is stored on each segment individually so a single UPDATE
    // propagates the rename across all segments at once.
    let affected = conn
        .execute(
            "UPDATE segments SET speaker_label = ?1 WHERE transcript_id = ?2 AND speaker_id = ?3",
            rusqlite::params![new_label, transcript_id, speaker_id],
        )
        .map_err(|e| AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Failed to update speaker label: {}", e),
        })?;

    if affected == 0 {
        return Err(AppError::DiarizationError {
            code: DiarizationErrorCode::NoSpeakersDetected,
            message: format!(
                "No segments found for speaker '{}' in transcript '{}'",
                speaker_id, transcript_id
            ),
        });
    }

    Ok(affected)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_diarization_providers_always_includes_tinydiarize() {
        let providers = available_providers();
        let tiny = providers.iter().find(|p| p.id == "tinydiarize");
        assert!(tiny.is_some());
        assert!(tiny.unwrap().available);
    }

    #[test]
    fn test_get_diarization_providers_includes_cloud_entries() {
        let providers = available_providers();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"elevenlabs"));
        assert!(ids.contains(&"deepgram"));
    }

    #[test]
    fn test_provider_info_serializes() {
        let info = ProviderInfo {
            id: "tinydiarize".into(),
            label: "Tinydiarize (local)".into(),
            available: true,
            requires_network: false,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "tinydiarize");
        assert_eq!(json["requiresNetwork"], false);
    }
}
