use crate::cloud_transcription::{self, CloudCostEstimate, CloudProviderInfo};
use crate::database::Database;
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use crate::transcription::hybrid::HybridTranscriber;
use std::sync::Arc;
use tauri::State;

/// List all available cloud transcription providers.
#[tauri::command]
pub async fn list_cloud_providers() -> Result<Vec<CloudProviderInfo>, AppError> {
    Ok(cloud_transcription::list_providers())
}

/// Transcribe an audio file using a cloud provider.
///
/// Returns the transcript ID of the newly created transcript.
#[tauri::command]
pub async fn transcribe_with_cloud(
    file_path: String,
    provider: String,
    language: Option<String>,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<String, AppError> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::UploadFailed,
            message: format!("File not found: {}", file_path),
        });
    }

    // Check file size (limit 25MB for most providers)
    let metadata = tokio::fs::metadata(path).await.map_err(|e| {
        AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::UploadFailed,
            message: format!("Cannot read file metadata: {}", e),
        }
    })?;

    if metadata.len() > 25 * 1024 * 1024 {
        return Err(AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::FileTooLarge,
            message: "File exceeds 25MB limit for cloud transcription".into(),
        });
    }

    // Get API key from keychain
    let api_key = get_provider_api_key(&provider)?;

    let guard_arc = Arc::new(NetworkGuard::new(guard.policy().clone())?);

    // Create the appropriate provider
    let cloud_provider: Box<dyn cloud_transcription::CloudTranscriptionProvider> = match provider.as_str() {
        "openai_whisper" => Box::new(
            cloud_transcription::openai_whisper::OpenAiWhisperProvider::new(api_key, guard_arc),
        ),
        "deepgram" => Box::new(
            cloud_transcription::deepgram::DeepgramProvider::new(api_key, guard_arc),
        ),
        "groq_whisper" => Box::new(
            cloud_transcription::groq_whisper::GroqWhisperProvider::new(api_key, guard_arc),
        ),
        "elevenlabs" => Box::new(
            cloud_transcription::elevenlabs::ElevenLabsProvider::new(api_key, guard_arc),
        ),
        "vibevoice" => Box::new(
            cloud_transcription::vibevoice::VibeVoiceProvider::new(api_key, guard_arc),
        ),
        other => {
            return Err(AppError::CloudTranscriptionError {
                code: CloudTranscriptionErrorCode::ProviderNotFound,
                message: format!("Unknown cloud provider: {}", other),
            });
        }
    };

    // Run transcription
    let result = cloud_provider
        .transcribe(path, language.as_deref())
        .await?;

    // Create transcript in database
    let transcript_id = uuid::Uuid::new_v4().to_string();
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Cloud Transcription".into());

    let mut conn = db.get()?;
    let tx = conn.transaction().map_err(|e| AppError::CloudTranscriptionError {
        code: CloudTranscriptionErrorCode::TranscriptionFailed,
        message: format!("Failed to begin transaction: {}", e),
    })?;

    // Compute word count from full text
    let word_count = result.text.split_whitespace().count() as i64;

    // Compute duration from last segment
    let duration_ms = result
        .segments
        .last()
        .map(|s| s.end_ms as i64)
        .unwrap_or(0);

    tx.execute(
        "INSERT INTO transcripts (id, title, created_at, updated_at, duration_ms, language, source_type, audio_path, is_starred, is_deleted, speaker_count, word_count, metadata)
         VALUES (?1, ?2, strftime('%s','now'), strftime('%s','now'), ?3, ?4, 'file', ?5, 0, 0, 0, ?6, '{}')",
        rusqlite::params![
            transcript_id,
            title,
            duration_ms,
            result.language,
            file_path,
            word_count,
        ],
    )?;

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

    tx.commit().map_err(|e| AppError::CloudTranscriptionError {
        code: CloudTranscriptionErrorCode::TranscriptionFailed,
        message: format!("Failed to commit transcript: {}", e),
    })?;

    tracing::info!(
        transcript_id = transcript_id,
        provider = provider,
        segments = result.segments.len(),
        "Cloud transcription complete"
    );

    Ok(transcript_id)
}

/// Estimate the cost of cloud transcription for a file.
#[tauri::command]
pub async fn estimate_cloud_cost(
    file_path: String,
    provider: String,
) -> Result<CloudCostEstimate, AppError> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::UploadFailed,
            message: format!("File not found: {}", file_path),
        });
    }

    // Estimate duration from file size (rough: 1MB ~ 1 minute for WAV 16-bit mono 16kHz)
    let metadata = std::fs::metadata(path)?;
    let file_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    let estimated_minutes = file_size_mb; // rough heuristic

    let providers = cloud_transcription::list_providers();
    let cost_per_min = providers
        .iter()
        .find(|p| p.name == provider)
        .map(|p| p.cost_per_minute_usd)
        .unwrap_or(0.0);

    Ok(CloudCostEstimate {
        provider,
        duration_minutes: estimated_minutes,
        estimated_usd: estimated_minutes * cost_per_min,
    })
}

/// Refine an existing local transcript with a cloud provider.
#[tauri::command]
pub async fn refine_with_cloud(
    transcript_id: String,
    provider: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<(), AppError> {
    let api_key = get_provider_api_key(&provider)?;
    let guard_arc = Arc::new(NetworkGuard::new(guard.policy().clone())?);
    let hybrid = HybridTranscriber::new(guard_arc);
    hybrid
        .refine_with_cloud(&transcript_id, &provider, &api_key, &db)
        .await
}

/// Helper to get API key from keychain for a provider.
fn get_provider_api_key(provider: &str) -> Result<String, AppError> {
    let service = match provider {
        "openai_whisper" => "openai",
        "groq_whisper" => "groq",
        other => other,
    };

    crate::keychain::get(service, "api_key")?
        .ok_or_else(|| AppError::CloudTranscriptionError {
            code: CloudTranscriptionErrorCode::InvalidApiKey,
            message: format!("No API key configured for '{}'. Add it in Settings.", service),
        })
}
