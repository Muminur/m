use crate::ai::actions::AiActionInput;
use crate::ai::cost::{self, CostEstimate};
use crate::ai::provider::ProviderRegistry;
use crate::ai::providers::ollama::OllamaProvider;
use crate::ai::templates::{self, AiTemplate};
use crate::database::Database;
use crate::error::{AiErrorCode, AppError};
use crate::network::guard::NetworkGuard;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

/// List all registered AI provider names.
#[tauri::command]
pub async fn list_ai_providers(
    registry: State<'_, Arc<Mutex<ProviderRegistry>>>,
) -> Result<Vec<String>, AppError> {
    let reg = registry.lock().await;
    Ok(reg.list())
}

/// Run an AI action on a transcript.
///
/// Looks up the transcript text from the database, then runs the specified action
/// through the selected AI provider. Emits `ai:stream-chunk` events during streaming.
#[tauri::command]
pub async fn run_ai_action(
    transcript_id: String,
    action: AiActionInput,
    db: State<'_, Arc<Database>>,
    registry: State<'_, Arc<Mutex<ProviderRegistry>>>,
    app: tauri::AppHandle,
) -> Result<String, AppError> {
    let ai_action = action.to_action()?;

    // Load transcript text and metadata from database
    let (text, speakers, duration_secs) = {
        let conn = db.get()?;

        // Get full text from segments
        let mut stmt = conn.prepare(
            "SELECT text FROM segments WHERE transcript_id = ?1 AND is_deleted = 0 ORDER BY index_num",
        )?;
        let texts: Vec<String> = stmt
            .query_map(rusqlite::params![transcript_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let full_text = texts.join(" ");

        // Get speaker labels
        let mut speaker_stmt = conn.prepare(
            "SELECT label FROM speakers WHERE transcript_id = ?1 ORDER BY label",
        )?;
        let speaker_list: Vec<String> = speaker_stmt
            .query_map(rusqlite::params![transcript_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        // Get duration
        let duration_ms: Option<i64> = conn
            .query_row(
                "SELECT duration_ms FROM transcripts WHERE id = ?1",
                rusqlite::params![transcript_id],
                |row| row.get(0),
            )
            .ok();

        let duration = duration_ms.unwrap_or(0) as u64 / 1000;

        (full_text, speaker_list, duration)
    };

    if text.is_empty() {
        return Err(AppError::AiError {
            code: AiErrorCode::ApiError,
            message: "Transcript has no text content".into(),
        });
    }

    // Try streaming first, fall back to non-streaming
    let reg = registry.lock().await;
    let provider = reg
        .get(&ai_action.provider)
        .ok_or_else(|| AppError::AiError {
            code: AiErrorCode::ProviderNotFound,
            message: format!("Provider '{}' not registered", ai_action.provider),
        })?;

    // Build the request
    let mut vars = std::collections::HashMap::new();
    vars.insert("transcript".into(), text.clone());
    vars.insert("speaker_list".into(), speakers.join(", "));
    vars.insert("duration".into(), duration_secs.to_string());
    if let Some(ref lang) = ai_action.target_language {
        vars.insert("target_language".into(), lang.clone());
    }

    let prompt_template = match ai_action.action_type {
        crate::ai::actions::AiActionType::Custom => {
            ai_action.custom_prompt.clone().unwrap_or_default()
        }
        ref at => {
            let template = match at {
                crate::ai::actions::AiActionType::Summarize => "Summarize the following transcript in 3-5 sentences:\n\n{{transcript}}",
                crate::ai::actions::AiActionType::ExtractKeyPoints => "Extract the 5 most important key points from this transcript:\n\n{{transcript}}",
                crate::ai::actions::AiActionType::QuestionAnswer => "Answer questions about this transcript. Transcript:\n\n{{transcript}}",
                crate::ai::actions::AiActionType::Translate => "Translate the following transcript to {{target_language}}:\n\n{{transcript}}",
                crate::ai::actions::AiActionType::Rewrite => "Rewrite the following transcript for clarity and readability:\n\n{{transcript}}",
                crate::ai::actions::AiActionType::GenerateChapters => "Generate chapter markers with timestamps for this transcript (duration: {{duration}}s):\n\n{{transcript}}",
                crate::ai::actions::AiActionType::Custom => "",
            };
            template.to_string()
        }
    };

    let prompt = crate::ai::templates::render_template(&prompt_template, &vars);

    let request = crate::ai::provider::CompletionRequest {
        model: ai_action.model.clone(),
        system: Some("You are a helpful assistant that processes transcripts.".into()),
        messages: vec![crate::ai::provider::ChatMessage {
            role: "user".into(),
            content: prompt,
        }],
        max_tokens: Some(4096),
        temperature: Some(0.3),
    };

    // Try streaming
    match provider.complete_stream(request.clone()).await {
        Ok(mut rx) => {
            use tauri::Emitter;
            let mut full_response = String::new();

            while let Some(chunk_result) = rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        if chunk.is_empty() {
                            // Stream done
                            let _ = app.emit("ai:stream-chunk", serde_json::json!({
                                "chunk": "",
                                "done": true
                            }));
                            break;
                        }
                        full_response.push_str(&chunk);
                        let _ = app.emit("ai:stream-chunk", serde_json::json!({
                            "chunk": chunk,
                            "done": false
                        }));
                    }
                    Err(e) => {
                        tracing::error!("Stream error: {}", e);
                        break;
                    }
                }
            }

            Ok(full_response)
        }
        Err(_) => {
            // Fall back to non-streaming
            let response = provider.complete(request).await?;
            Ok(response.content)
        }
    }
}

/// Estimate the cost of an AI operation.
#[tauri::command]
pub async fn estimate_ai_cost(
    provider: String,
    model: String,
    text: String,
) -> Result<CostEstimate, AppError> {
    let input_tokens = cost::estimate_tokens(&text);
    // Estimate output as roughly 25% of input for most operations
    let estimated_output = std::cmp::max(input_tokens / 4, 256);
    Ok(cost::estimate_cost(&provider, &model, input_tokens, estimated_output))
}

/// List all AI prompt templates.
#[tauri::command]
pub async fn list_ai_templates(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AiTemplate>, AppError> {
    templates::list_templates(&db)
}

/// Create a new AI prompt template.
#[tauri::command]
pub async fn create_ai_template(
    name: String,
    description: Option<String>,
    prompt: String,
    action_type: String,
    db: State<'_, Arc<Database>>,
) -> Result<AiTemplate, AppError> {
    templates::create_template(&db, &name, description.as_deref(), &prompt, &action_type)
}

/// Update an existing AI prompt template.
#[tauri::command]
pub async fn update_ai_template(
    id: String,
    name: String,
    description: Option<String>,
    prompt: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    templates::update_template(&db, &id, &name, description.as_deref(), &prompt)
}

/// Delete an AI prompt template.
#[tauri::command]
pub async fn delete_ai_template(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    templates::delete_template(&db, &id)
}

/// List models available on local Ollama instance.
#[tauri::command]
pub async fn list_ollama_models(
    _guard: State<'_, NetworkGuard>,
) -> Result<Vec<String>, AppError> {
    let ollama = OllamaProvider::new(Arc::new(NetworkGuard::new(
        crate::settings::NetworkPolicy::LocalOnly,
    )?));
    ollama.list_local_models().await
}
