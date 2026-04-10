use crate::ai::provider::{AiProvider, CompletionRequest, CompletionResponse, ModelInfo};
use crate::error::{AiErrorCode, AppError};
use crate::network::guard::NetworkGuard;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

const BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

/// Anthropic provider (Claude models).
///
/// Uses the Anthropic Messages API which has a different schema
/// from OpenAI-compatible endpoints.
pub struct AnthropicProvider {
    api_key: String,
    guard: Arc<NetworkGuard>,
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    messages: Vec<ApiMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
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
struct MessagesResponse {
    content: Vec<ContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<StreamDelta>,
}

#[derive(Deserialize)]
struct StreamDelta {
    text: Option<String>,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        Self { api_key, guard }
    }

    fn build_messages(request: &CompletionRequest) -> Vec<ApiMessage> {
        request
            .messages
            .iter()
            .map(|m| ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect()
    }
}

impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-opus-4-6".into(),
                name: "Claude Opus 4.6".into(),
                context_window: 200000,
                cost_per_1k_input: 0.015,
                cost_per_1k_output: 0.075,
            },
            ModelInfo {
                id: "claude-sonnet-4-6".into(),
                name: "Claude Sonnet 4.6".into(),
                context_window: 200000,
                cost_per_1k_input: 0.003,
                cost_per_1k_output: 0.015,
            },
            ModelInfo {
                id: "claude-haiku-4-5".into(),
                name: "Claude Haiku 4.5".into(),
                context_window: 200000,
                cost_per_1k_input: 0.001,
                cost_per_1k_output: 0.005,
            },
        ]
    }

    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, AppError>> + Send + '_>>
    {
        Box::pin(async move {
            let url = format!("{}/messages", BASE_URL);
            let messages = Self::build_messages(&request);

            let body = MessagesRequest {
                model: request.model.clone(),
                messages,
                max_tokens: request.max_tokens.unwrap_or(4096),
                system: request.system.clone(),
                temperature: request.temperature,
                stream: None,
            };

            let req = self
                .guard
                .client()
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", API_VERSION)
                .header("content-type", "application/json")
                .json(&body);

            let response = self.guard.request(req).await?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::AiError {
                    code: AiErrorCode::InvalidApiKey,
                    message: "Invalid Anthropic API key".into(),
                });
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::AiError {
                    code: AiErrorCode::RateLimited,
                    message: "Rate limited by Anthropic".into(),
                });
            }

            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Anthropic API error ({}): {}", status, body_text),
                });
            }

            let resp: MessagesResponse = response.json().await.map_err(|e| AppError::AiError {
                code: AiErrorCode::ApiError,
                message: format!("Failed to parse Anthropic response: {}", e),
            })?;

            let content = resp
                .content
                .into_iter()
                .filter_map(|b| b.text)
                .collect::<Vec<_>>()
                .join("");

            Ok(CompletionResponse {
                content,
                input_tokens: resp.usage.input_tokens,
                output_tokens: resp.usage.output_tokens,
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
            let url = format!("{}/messages", BASE_URL);
            let messages = Self::build_messages(&request);

            let body = MessagesRequest {
                model: request.model.clone(),
                messages,
                max_tokens: request.max_tokens.unwrap_or(4096),
                system: request.system.clone(),
                temperature: request.temperature,
                stream: Some(true),
            };

            let req = self
                .guard
                .client()
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", API_VERSION)
                .header("content-type", "application/json")
                .json(&body);

            let response = self.guard.request(req).await?;
            let status = response.status();

            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Anthropic stream error ({}): {}", status, body_text),
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
                            while let Some(line_end) = buffer.find('\n') {
                                let line = buffer[..line_end].trim().to_string();
                                buffer = buffer[line_end + 1..].to_string();

                                if line.starts_with("data: ") {
                                    let data = &line[6..];
                                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                                        if event.event_type == "content_block_delta" {
                                            if let Some(delta) = event.delta {
                                                if let Some(text) = delta.text {
                                                    if tx.send(Ok(text)).await.is_err() {
                                                        return;
                                                    }
                                                }
                                            }
                                        } else if event.event_type == "message_stop" {
                                            let _ = tx.send(Ok(String::new())).await;
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(Err(AppError::AiError {
                                    code: AiErrorCode::ApiError,
                                    message: format!("Anthropic stream error: {}", e),
                                }))
                                .await;
                            return;
                        }
                    }
                }
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
    fn test_anthropic_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = AnthropicProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_anthropic_provider_models() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = AnthropicProvider::new("test-key".into(), guard);
        let models = provider.models();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "claude-opus-4-6");
        assert_eq!(models[1].id, "claude-sonnet-4-6");
        assert_eq!(models[2].id, "claude-haiku-4-5");
    }
}
