use crate::ai::provider::{ChatMessage, CompletionRequest, ProviderRegistry};
use crate::error::{AiErrorCode, AppError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Built-in AI action types that can be run on transcripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiActionType {
    Summarize,
    ExtractKeyPoints,
    QuestionAnswer,
    Translate,
    Rewrite,
    GenerateChapters,
    Custom,
}

/// An AI action to run on a transcript.
#[derive(Debug, Clone)]
pub struct AiAction {
    pub action_type: AiActionType,
    pub provider: String,
    pub model: String,
    pub custom_prompt: Option<String>,
    pub target_language: Option<String>,
}

/// Input struct for the Tauri command, received from the frontend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiActionInput {
    pub action_type: String,
    pub provider: String,
    pub model: String,
    pub custom_prompt: Option<String>,
    pub target_language: Option<String>,
}

impl AiActionInput {
    /// Convert the frontend input to a domain AiAction.
    pub fn to_action(&self) -> Result<AiAction, AppError> {
        let action_type = match self.action_type.as_str() {
            "summarize" => AiActionType::Summarize,
            "extractKeyPoints" => AiActionType::ExtractKeyPoints,
            "questionAnswer" => AiActionType::QuestionAnswer,
            "translate" => AiActionType::Translate,
            "rewrite" => AiActionType::Rewrite,
            "generateChapters" => AiActionType::GenerateChapters,
            "custom" => AiActionType::Custom,
            other => {
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Unknown action type: {}", other),
                })
            }
        };

        Ok(AiAction {
            action_type,
            provider: self.provider.clone(),
            model: self.model.clone(),
            custom_prompt: self.custom_prompt.clone(),
            target_language: self.target_language.clone(),
        })
    }
}

/// Get the built-in prompt for an action type.
fn builtin_prompt(action: &AiAction) -> String {
    match action.action_type {
        AiActionType::Summarize => {
            "Summarize the following transcript in 3-5 sentences:\n\n{{transcript}}".into()
        }
        AiActionType::ExtractKeyPoints => {
            "Extract the 5 most important key points from this transcript:\n\n{{transcript}}".into()
        }
        AiActionType::QuestionAnswer => {
            "Answer questions about this transcript. Transcript:\n\n{{transcript}}".into()
        }
        AiActionType::Translate => {
            format!(
                "Translate the following transcript to {{{{target_language}}}}:\n\n{{{{transcript}}}}"
            )
        }
        AiActionType::Rewrite => {
            "Rewrite the following transcript for clarity and readability:\n\n{{transcript}}".into()
        }
        AiActionType::GenerateChapters => {
            "Generate chapter markers with timestamps for this transcript (duration: {{duration}}s):\n\n{{transcript}}".into()
        }
        AiActionType::Custom => action.custom_prompt.clone().unwrap_or_default(),
    }
}

/// Render a prompt template by substituting `{{key}}` with values.
fn render_prompt(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

/// Run an AI action on a transcript.
///
/// Looks up the provider in the registry, builds the prompt, and calls the provider.
pub async fn run_action(
    action: &AiAction,
    transcript_text: &str,
    speaker_list: &[String],
    duration_secs: u64,
    registry: &ProviderRegistry,
) -> Result<String, AppError> {
    let provider = registry
        .get(&action.provider)
        .ok_or_else(|| AppError::AiError {
            code: AiErrorCode::ProviderNotFound,
            message: format!("AI provider '{}' not found", action.provider),
        })?;

    // Build variable map
    let mut vars = HashMap::new();
    vars.insert("transcript".into(), transcript_text.to_string());
    vars.insert("speaker_list".into(), speaker_list.join(", "));
    vars.insert("duration".into(), duration_secs.to_string());
    if let Some(ref lang) = action.target_language {
        vars.insert("target_language".into(), lang.clone());
    }

    let prompt_template = builtin_prompt(action);
    let prompt = render_prompt(&prompt_template, &vars);

    let request = CompletionRequest {
        model: action.model.clone(),
        system: Some("You are a helpful assistant that processes transcripts.".into()),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt,
        }],
        max_tokens: Some(4096),
        temperature: Some(0.3),
    };

    let response = provider.complete(request).await?;
    Ok(response.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prompt_basic() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "World".into());
        let result = render_prompt("Hello {{name}}!", &vars);
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_prompt_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("transcript".into(), "Some text".into());
        vars.insert("duration".into(), "120".into());
        let result = render_prompt("Text: {{transcript}}, Duration: {{duration}}s", &vars);
        assert_eq!(result, "Text: Some text, Duration: 120s");
    }

    #[test]
    fn test_render_prompt_missing_var_unchanged() {
        let vars = HashMap::new();
        let result = render_prompt("Hello {{name}}", &vars);
        assert_eq!(result, "Hello {{name}}");
    }

    #[test]
    fn test_builtin_prompt_summarize() {
        let action = AiAction {
            action_type: AiActionType::Summarize,
            provider: "test".into(),
            model: "test".into(),
            custom_prompt: None,
            target_language: None,
        };
        let prompt = builtin_prompt(&action);
        assert!(prompt.contains("{{transcript}}"));
        assert!(prompt.contains("Summarize"));
    }

    #[test]
    fn test_ai_action_input_to_action() {
        let input = AiActionInput {
            action_type: "summarize".into(),
            provider: "openai".into(),
            model: "gpt-4o".into(),
            custom_prompt: None,
            target_language: None,
        };
        let action = input.to_action().unwrap();
        assert!(matches!(action.action_type, AiActionType::Summarize));
        assert_eq!(action.provider, "openai");
    }

    #[test]
    fn test_ai_action_input_invalid_type() {
        let input = AiActionInput {
            action_type: "invalid".into(),
            provider: "openai".into(),
            model: "gpt-4o".into(),
            custom_prompt: None,
            target_language: None,
        };
        assert!(input.to_action().is_err());
    }
}
