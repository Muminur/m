use crate::database::Database;
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use std::sync::Arc;

/// Hybrid transcription: refine local transcripts using cloud providers.
pub struct HybridTranscriber {
    #[allow(dead_code)]
    guard: Arc<NetworkGuard>,
}

impl HybridTranscriber {
    /// Create a new hybrid transcriber.
    pub fn new(guard: Arc<NetworkGuard>) -> Self {
        Self { guard }
    }

    /// Refine an existing local transcript using a cloud provider.
    ///
    /// Looks up the transcript's audio file, sends it to the cloud provider,
    /// and updates the segments in the database with the cloud results.
    pub async fn refine_with_cloud(
        &self,
        transcript_id: &str,
        provider_name: &str,
        api_key: &str,
        db: &Database,
    ) -> Result<(), AppError> {
        // Look up the transcript to get the audio path
        let audio_path = {
            let conn = db.get()?;
            let path: Option<String> = conn
                .query_row(
                    "SELECT audio_path FROM transcripts WHERE id = ?1",
                    rusqlite::params![transcript_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::CloudTranscriptionError {
                    code: CloudTranscriptionErrorCode::TranscriptionFailed,
                    message: format!("Transcript not found: {}", e),
                })?;
            path.ok_or_else(|| AppError::CloudTranscriptionError {
                code: CloudTranscriptionErrorCode::TranscriptionFailed,
                message: "Transcript has no audio file".into(),
            })?
        };

        let audio = std::path::Path::new(&audio_path);
        if !audio.exists() {
            return Err(AppError::CloudTranscriptionError {
                code: CloudTranscriptionErrorCode::UploadFailed,
                message: format!("Audio file not found: {}", audio_path),
            });
        }

        // Create the appropriate cloud provider
        let provider: Box<dyn crate::cloud_transcription::CloudTranscriptionProvider> =
            match provider_name {
                "openai_whisper" => Box::new(
                    crate::cloud_transcription::openai_whisper::OpenAiWhisperProvider::new(
                        api_key.to_string(),
                        self.guard.clone(),
                    ),
                ),
                "deepgram" => Box::new(
                    crate::cloud_transcription::deepgram::DeepgramProvider::new(
                        api_key.to_string(),
                        self.guard.clone(),
                    ),
                ),
                "groq_whisper" => Box::new(
                    crate::cloud_transcription::groq_whisper::GroqWhisperProvider::new(
                        api_key.to_string(),
                        self.guard.clone(),
                    ),
                ),
                "elevenlabs" => Box::new(
                    crate::cloud_transcription::elevenlabs::ElevenLabsProvider::new(
                        api_key.to_string(),
                        self.guard.clone(),
                    ),
                ),
                other => {
                    return Err(AppError::CloudTranscriptionError {
                        code: CloudTranscriptionErrorCode::ProviderNotFound,
                        message: format!("Unknown cloud provider: {}", other),
                    });
                }
            };

        // Run cloud transcription
        let result = provider.transcribe(audio, None).await?;

        // Update segments in database within a transaction
        let mut conn = db.get()?;
        let tx = conn.transaction().map_err(|e| AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::TranscriptionFailed,
            message: format!("Failed to begin transaction: {}", e),
        })?;

        // Delete existing segments
        tx.execute(
            "DELETE FROM segments WHERE transcript_id = ?1",
            rusqlite::params![transcript_id],
        )?;

        // Insert new segments from cloud result
        for (idx, seg) in result.segments.iter().enumerate() {
            let seg_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO segments (id, transcript_id, index_num, start_ms, end_ms, text, speaker_id, confidence, is_deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1.0, 0)",
                rusqlite::params![
                    seg_id,
                    transcript_id,
                    idx as i64,
                    seg.start_ms as i64,
                    seg.end_ms as i64,
                    seg.text,
                    seg.speaker,
                ],
            )?;
        }

        // Update transcript language if detected
        tx.execute(
            "UPDATE transcripts SET language = ?2, updated_at = strftime('%s', 'now') WHERE id = ?1",
            rusqlite::params![transcript_id, result.language],
        )?;

        tx.commit().map_err(|e| AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::TranscriptionFailed,
            message: format!("Failed to commit refined segments: {}", e),
        })?;

        tracing::info!(
            transcript_id = transcript_id,
            provider = provider_name,
            segments = result.segments.len(),
            "Refined transcript with cloud provider"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_hybrid_transcriber_new() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let _transcriber = HybridTranscriber::new(guard);
    }
}
