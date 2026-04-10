use crate::ai::provider::{
    AiProvider, CompletionRequest, CompletionResponse, ModelInfo,
};
use crate::error::{AiErrorCode, AppError};
use crate::network::guard::NetworkGuard;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Generic OpenAI-compatible provider.
///
/// Works with any endpoint that follows the OpenAI chat completions API format:
/// DeepSeek, xAI (Grok), OpenRouter, Azure, or custom endpoints.
pub struct OpenAiCompatProvider {
    provider_name: String,
    base_url: String,
    api_key: Option<String>,
    model_list: Vec<ModelInfo>,
    guard: Arc<NetworkGuard>,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: DeltaContent,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}

impl OpenAiCompatProvider {
    /// Create a new OpenAI-compatible provider.
    ///
    /// # Arguments
    /// * `name` - Provider name (e.g. "deepseek", "xai")
    /// * `base_url` - Base URL ending with `/v1` (e.g. `https://api.deepseek.com/v1`)
    /// * `api_key` - API key (None for local providers like Ollama)
    /// * `models` - List of models this provider supports
    /// * `guard` - Network guard for policy enforcement
    pub fn new(
        name: String,
        base_url: String,
        api_key: Option<String>,
        models: Vec<ModelInfo>,
        guard: Arc<NetworkGuard>,
    ) -> Self {
        Self {
            provider_name: name,
            base_url,
            api_key,
            model_list: models,
            guard,
        }
    }

    fn build_request(&self, request: &CompletionRequest) -> Result<reqwest::RequestBuilder, AppError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut messages: Vec<ApiMessage> = Vec::new();

        // Add system message if present
        if let Some(ref system) = request.system {
            messages.push(ApiMessage {
                role: "system".into(),
                content: system.clone(),
            });
        }

        // Add conversation messages
        for msg in &request.messages {
            messages.push(ApiMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        let body = ChatCompletionRequest {
            model: request.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: None,
        };

        let mut req = self.guard.client().post(&url).json(&body);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        Ok(req)
    }
}

impl AiProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.model_list.clone()
    }

    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, AppError>> + Send + '_>>
    {
        Box::pin(async move {
            let req = self.build_request(&request)?;
            let response = self.guard.request(req).await?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::AiError {
                    code: AiErrorCode::InvalidApiKey,
                    message: "Invalid API key".into(),
                });
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::AiError {
                    code: AiErrorCode::RateLimited,
                    message: "Rate limited by provider".into(),
                });
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("API error ({}): {}", status, body),
                });
            }

            let body: ChatCompletionResponse =
                response.json().await.map_err(|e| AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Failed to parse response: {}", e),
                })?;

            let content = body
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default();

            let usage = body.usage.unwrap_or(Usage {
                prompt_tokens: None,
                completion_tokens: None,
            });

            Ok(CompletionResponse {
                content,
                input_tokens: usage.prompt_tokens.unwrap_or(0),
                output_tokens: usage.completion_tokens.unwrap_or(0),
            })
        })
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
        Box::pin(async move {
            let url = format!("{}/chat/completions", self.base_url);

            let mut messages: Vec<ApiMessage> = Vec::new();
            if let Some(ref system) = request.system {
                messages.push(ApiMessage {
                    role: "system".into(),
                    content: system.clone(),
                });
            }
            for msg in &request.messages {
                messages.push(ApiMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                });
            }

            let body = ChatCompletionRequest {
                model: request.model.clone(),
                messages,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                stream: Some(true),
            };

            let mut req = self.guard.client().post(&url).json(&body);
            if let Some(ref key) = self.api_key {
                req = req.header("Authorization", format!("Bearer {}", key));
            }

            let response = self.guard.request(req).await?;
            let status = response.status();

            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Stream API error ({}): {}", status, body_text),
                });
            }

            let (tx, rx) = mpsc::channel(64);

            let mut byte_stream = response.bytes_stream();
            use futures_lite::StreamExt;

            tokio::spawn(async move {
                let mut buffer = String::new();
                while let Some(chunk_result) = byte_stream.next().await {
                    match chunk_result {
                        Ok(bytes) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                            // Process SSE lines
                            while let Some(line_end) = buffer.find('\n') {
                                let line = buffer[..line_end].trim().to_string();
                                buffer = buffer[line_end + 1..].to_string();

                                if line.starts_with("data: ") {
                                    let data = &line[6..];
                                    if data == "[DONE]" {
                                        let _ = tx.send(Ok(String::new())).await;
                                        return;
                                    }
                                    if let Ok(chunk) =
                                        serde_json::from_str::<StreamChunk>(data)
                                    {
                                        if let Some(choice) = chunk.choices.first() {
                                            if let Some(ref content) = choice.delta.content {
                                                if tx.send(Ok(content.clone())).await.is_err() {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(Err(AppError::AiError {
                                    code: AiErrorCode::ApiError,
                                    message: format!("Stream error: {}", e),
                                }))
                                .await;
                            return;
                        }
                    }
                }
                // Stream ended without [DONE]
                let _ = tx.send(Ok(String::new())).await;
            });

            Ok(rx)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_openai_compat_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = OpenAiCompatProvider::new(
            "test-provider".into(),
            "https://api.test.com/v1".into(),
            Some("test-key".into()),
            vec![],
            guard,
        );
        assert_eq!(provider.name(), "test-provider");
    }

    #[test]
    fn test_openai_compat_provider_models() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let models = vec![ModelInfo {
            id: "test-model".into(),
            name: "Test Model".into(),
            context_window: 4096,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.002,
        }];
        let provider = OpenAiCompatProvider::new(
            "test".into(),
            "https://api.test.com/v1".into(),
            None,
            models.clone(),
            guard,
        );
        let listed = provider.models();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "test-model");
    }
}
