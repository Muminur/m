use crate::error::AppError;
use serde::Serialize;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Information about a model offered by an AI provider.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Request sent to an AI provider.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Response received from an AI provider.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Trait for AI provider implementations.
///
/// Each provider (OpenAI, Anthropic, etc.) implements this trait to provide
/// model listing, completion, and streaming capabilities.
pub trait AiProvider: Send + Sync {
    /// Returns the provider name (e.g., "openai", "anthropic").
    fn name(&self) -> &str;

    /// Returns the list of models available from this provider.
    fn models(&self) -> Vec<ModelInfo>;

    /// Perform a non-streaming completion request.
    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, AppError>> + Send + '_>>;

    /// Perform a streaming completion request.
    /// Sends chunks through the returned receiver. The final empty string signals completion.
    fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<mpsc::Receiver<Result<String, AppError>>, AppError>>
                + Send
                + '_,
        >,
    >;
}

/// Registry of AI providers.
///
/// Stores boxed providers indexed by name. Thread-safe via Arc wrapping.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
}

impl ProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider. Overwrites any existing provider with the same name.
    pub fn register(&mut self, provider: Arc<dyn AiProvider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// Look up a provider by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_is_empty() {
        let registry = ProviderRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_registry_get_missing_returns_none() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_model_info_serializes() {
        let info = ModelInfo {
            id: "gpt-4o".into(),
            name: "GPT-4o".into(),
            context_window: 128000,
            cost_per_1k_input: 0.005,
            cost_per_1k_output: 0.015,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "gpt-4o");
        assert_eq!(json["contextWindow"], 128000);
    }

    #[test]
    fn test_completion_response_serializes() {
        let resp = CompletionResponse {
            content: "Hello".into(),
            input_tokens: 10,
            output_tokens: 5,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["inputTokens"], 10);
        assert_eq!(json["outputTokens"], 5);
    }
}
