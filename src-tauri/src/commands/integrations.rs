use crate::database::segments;
use crate::database::transcripts;
use crate::database::Database;
use crate::error::{AppError, IntegrationErrorCode};
use crate::integrations;
use crate::network::guard::NetworkGuard;
use std::sync::Arc;
use tauri::State;

/// Push a transcript to a Notion database.
///
/// Retrieves the API key from the system keychain (service: "notion").
/// Returns the URL of the created Notion page.
#[tauri::command]
pub async fn push_to_notion(
    transcript_id: String,
    database_id: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<String, AppError> {
    // Get API key from keychain
    let api_key = tokio::task::spawn_blocking(|| crate::keychain::get("notion", "api_key"))
        .await
        .map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Keychain task failed: {}", e),
        })??
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "Notion API key not found in keychain".into(),
        })?;

    // Load transcript and segments
    let (transcript, segs) = {
        let conn = db.get()?;
        let t = transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| {
            AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: format!("Transcript not found: {}", transcript_id),
            }
        })?;
        let s = segments::get_by_transcript(&conn, &transcript_id)?;
        (t, s)
    };

    let config = integrations::notion::NotionConfig {
        api_key,
        database_id,
    };

    integrations::notion::push_to_notion(
        &config,
        &transcript.title,
        &segs,
        transcript.duration_ms.unwrap_or(0),
        transcript.language.as_deref().unwrap_or("unknown"),
        &guard,
    )
    .await
}

/// Write a transcript to an Obsidian vault as a markdown file.
///
/// Returns the absolute path of the written file.
#[tauri::command]
pub async fn write_to_obsidian(
    transcript_id: String,
    vault_path: String,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    let (transcript, segs) = {
        let conn = db.get()?;
        let t = transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| {
            AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: format!("Transcript not found: {}", transcript_id),
            }
        })?;
        let s = segments::get_by_transcript(&conn, &transcript_id)?;
        (t, s)
    };

    let config = integrations::obsidian::ObsidianConfig { vault_path };

    // Run filesystem write on blocking thread
    let title = transcript.title.clone();
    let duration_ms = transcript.duration_ms.unwrap_or(0);
    let language = transcript.language.clone().unwrap_or_else(|| "unknown".into());
    let created_at = transcript.created_at;

    tokio::task::spawn_blocking(move || {
        integrations::obsidian::write_to_obsidian(
            &config,
            &title,
            &segs,
            duration_ms,
            &language,
            created_at,
        )
    })
    .await
    .map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ApiError,
        message: format!("Obsidian write task failed: {}", e),
    })?
}

/// Validate that a webhook URL is safe (no SSRF to internal/private addresses).
fn validate_webhook_url(url: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url).map_err(|_| AppError::IntegrationError {
        code: IntegrationErrorCode::ConfigurationMissing,
        message: "Invalid webhook URL format".into(),
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: "Webhook URL must use http or https".into(),
            })
        }
    }

    let host = parsed.host_str().unwrap_or("");
    let blocked = [
        "169.254.169.254",
        "localhost",
        "127.0.0.1",
        "::1",
        "0.0.0.0",
    ];
    // Check RFC 1918 172.16.0.0/12 range with proper octet parsing
    let is_rfc1918_172 = if host.starts_with("172.") {
        host.split('.')
            .nth(1)
            .and_then(|octet| octet.parse::<u8>().ok())
            .map(|second| (16..=31).contains(&second))
            .unwrap_or(false)
    } else {
        false
    };

    if blocked.contains(&host)
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || is_rfc1918_172
    {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "Webhook URL cannot target private/internal addresses".into(),
        });
    }
    Ok(())
}

/// Fire a webhook with transcript data.
///
/// Retrieves the webhook secret from the keychain if configured (service: "webhook").
#[tauri::command]
pub async fn fire_webhook(
    url: String,
    transcript_id: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<(), AppError> {
    validate_webhook_url(&url)?;

    // Load transcript metadata for the webhook payload
    let (transcript, seg_count) = {
        let conn = db.get()?;
        let t = transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| {
            AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: format!("Transcript not found: {}", transcript_id),
            }
        })?;
        let segs = segments::get_by_transcript(&conn, &transcript_id)?;
        (t, segs.len())
    };

    // Optionally get webhook secret from keychain (non-fatal if missing)
    let secret = tokio::task::spawn_blocking(|| crate::keychain::get("webhook", "api_key"))
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();

    let config = integrations::webhook::WebhookConfig { url, secret };

    let payload = serde_json::json!({
        "transcriptId": transcript.id,
        "title": transcript.title,
        "language": transcript.language,
        "durationMs": transcript.duration_ms,
        "segmentCount": seg_count,
        "createdAt": transcript.created_at,
    });

    integrations::webhook::fire_webhook(&config, "transcript.completed", payload, &guard).await
}

/// Translate text using the DeepL API.
///
/// Retrieves the API key from the system keychain (service: "deepl").
#[tauri::command]
pub async fn translate_with_deepl(
    text: String,
    target_lang: String,
    guard: State<'_, NetworkGuard>,
) -> Result<String, AppError> {
    let api_key = tokio::task::spawn_blocking(|| crate::keychain::get("deepl", "api_key"))
        .await
        .map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Keychain task failed: {}", e),
        })??
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "DeepL API key not found in keychain".into(),
        })?;

    let config = integrations::deepl::DeepLConfig {
        api_key,
        target_lang,
    };

    integrations::deepl::translate_text(&config, &text, &guard).await
}

/// Translate transcript segments individually using DeepL.
///
/// Returns a Vec of translated text strings aligned with the original segments.
#[tauri::command]
pub async fn translate_segments_deepl(
    transcript_id: String,
    target_lang: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<Vec<String>, AppError> {
    let api_key = tokio::task::spawn_blocking(|| crate::keychain::get("deepl", "api_key"))
        .await
        .map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Keychain task failed: {}", e),
        })??
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "DeepL API key not found in keychain".into(),
        })?;

    let segs = {
        let conn = db.get()?;
        segments::get_by_transcript(&conn, &transcript_id)?
    };

    let texts: Vec<String> = segs.iter().map(|s| s.text.clone()).collect();

    let config = integrations::deepl::DeepLConfig {
        api_key,
        target_lang,
    };

    integrations::deepl::translate_segments(&config, &texts, &guard).await
}

/// Translate SRT content using the DeepL API, preserving SRT structure.
///
/// Retrieves the API key from the system keychain (service: "deepl").
/// Returns the translated SRT string.
#[tauri::command]
pub async fn translate_srt_deepl(
    srt_content: String,
    target_lang: String,
    guard: State<'_, NetworkGuard>,
) -> Result<String, AppError> {
    let api_key = tokio::task::spawn_blocking(|| crate::keychain::get("deepl", "api_key"))
        .await
        .map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Keychain task failed: {}", e),
        })??
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "DeepL API key not found in keychain".into(),
        })?;

    let config = integrations::deepl::DeepLConfig {
        api_key,
        target_lang,
    };

    integrations::deepl::translate_srt(&config, &srt_content, &guard).await
}

/// Translate all segments of a transcript using the DeepL API.
///
/// Retrieves the API key from the system keychain (service: "deepl").
/// Returns the concatenated translated text.
#[tauri::command]
pub async fn translate_transcript_deepl(
    transcript_id: String,
    target_lang: String,
    db: State<'_, Arc<Database>>,
    guard: State<'_, NetworkGuard>,
) -> Result<String, AppError> {
    let api_key = tokio::task::spawn_blocking(|| crate::keychain::get("deepl", "api_key"))
        .await
        .map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Keychain task failed: {}", e),
        })??
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "DeepL API key not found in keychain".into(),
        })?;

    // Load all segment text
    let text = {
        let conn = db.get()?;
        let segs = segments::get_by_transcript(&conn, &transcript_id)?;
        if segs.is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: "Transcript has no segments".into(),
            });
        }
        segs.iter()
            .map(|s| s.text.trim())
            .collect::<Vec<_>>()
            .join(" ")
    };

    let config = integrations::deepl::DeepLConfig {
        api_key,
        target_lang,
    };

    integrations::deepl::translate_text(&config, &text, &guard).await
}
