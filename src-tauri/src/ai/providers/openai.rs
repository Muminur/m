use crate::ai::provider::{AiProvider, CompletionRequest, CompletionResponse, ModelInfo};
use crate::ai::providers::openai_compat::OpenAiCompatProvider;
use crate::error::AppError;
use crate::network::guard::NetworkGuard;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

const BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI provider (GPT-4o, GPT-4o-mini).
pub struct OpenAiProvider {
    inner: OpenAiCompatProvider,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        let models = vec![
            ModelInfo {
                id: "gpt-4o".into(),
                name: "GPT-4o".into(),
                context_window: 128000,
                cost_per_1k_input: 0.005,
                cost_per_1k_output: 0.015,
            },
            ModelInfo {
                id: "gpt-4o-mini".into(),
                name: "GPT-4o Mini".into(),
                context_window: 128000,
                cost_per_1k_input: 0.00015,
                cost_per_1k_output: 0.0006,
            },
        ];
        Self {
            inner: OpenAiCompatProvider::new(
                "openai".into(),
                BASE_URL.into(),
                Some(api_key),
                models,
                guard,
            ),
        }
    }
}

impl AiProvider for OpenAiProvider {
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
    fn test_openai_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = OpenAiProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_provider_models() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = OpenAiProvider::new("test-key".into(), guard);
        let models = provider.models();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[1].id, "gpt-4o-mini");
    }
}
