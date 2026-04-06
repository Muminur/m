use crate::ai::provider::{AiProvider, CompletionRequest, CompletionResponse, ModelInfo};
use crate::ai::providers::openai_compat::OpenAiCompatProvider;
use crate::error::AppError;
use crate::network::guard::NetworkGuard;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

const BASE_URL: &str = "https://api.groq.com/openai/v1";

/// Groq provider (LLaMA, Mixtral via Groq LPU inference).
pub struct GroqProvider {
    inner: OpenAiCompatProvider,
}

impl GroqProvider {
    /// Create a new Groq provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        let models = vec![
            ModelInfo {
                id: "llama3-70b-8192".into(),
                name: "LLaMA 3 70B".into(),
                context_window: 8192,
                cost_per_1k_input: 0.00059,
                cost_per_1k_output: 0.00079,
            },
            ModelInfo {
                id: "mixtral-8x7b-32768".into(),
                name: "Mixtral 8x7B".into(),
                context_window: 32768,
                cost_per_1k_input: 0.00024,
                cost_per_1k_output: 0.00024,
            },
        ];
        Self {
            inner: OpenAiCompatProvider::new(
                "groq".into(),
                BASE_URL.into(),
                Some(api_key),
                models,
                guard,
            ),
        }
    }
}

impl AiProvider for GroqProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.inner.models()
    }

    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, AppError>> + Send + '_>>
    {
        self.inner.complete(request)
    }

    fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<mpsc::Receiver<Result<String, AppError>>, AppError>,
                > + Send
                + '_,
        >,
    > {
        self.inner.complete_stream(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_groq_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = GroqProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "groq");
    }

    #[test]
    fn test_groq_provider_models() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = GroqProvider::new("test-key".into(), guard);
        let models = provider.models();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3-70b-8192");
    }
}
