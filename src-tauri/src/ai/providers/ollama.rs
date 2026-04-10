use crate::ai::provider::{AiProvider, CompletionRequest, CompletionResponse, ModelInfo};
use crate::error::{AiErrorCode, AppError};
use crate::network::guard::NetworkGuard;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Ollama provider for local LLM inference.
///
/// Uses `NetworkPolicy::LocalOnly` — all requests go to localhost.
/// No API key required.
pub struct OllamaProvider {
    base_url: String,
    guard: Arc<NetworkGuard>,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Option<Vec<OllamaModel>>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaResponseMessage>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaResponseMessage>,
    done: Option<bool>,
}

impl OllamaProvider {
    /// Create a new Ollama provider pointing at localhost.
    pub fn new(guard: Arc<NetworkGuard>) -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            guard,
        }
    }

    /// List model names available on the local Ollama instance.
    pub async fn list_local_models(&self) -> Result<Vec<String>, AppError> {
        let url = format!("{}/api/tags", self.base_url);
        let req = self.guard.client().get(&url);
        let response = self.guard.request(req).await?;

        if !response.status().is_success() {
            return Err(AppError::AiError {
                code: AiErrorCode::ApiError,
                message: "Failed to list Ollama models — is Ollama running?".into(),
            });
        }

        let tags: TagsResponse = response.json().await.map_err(|e| AppError::AiError {
            code: AiErrorCode::ApiError,
            message: format!("Failed to parse Ollama tags: {}", e),
        })?;

        Ok(tags
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.name)
            .collect())
    }
}

impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn models(&self) -> Vec<ModelInfo> {
        // Ollama models are dynamic; return empty list.
        // Use list_local_models() to discover at runtime.
        vec![]
    }

    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, AppError>> + Send + '_>>
    {
        Box::pin(async move {
            let url = format!("{}/api/chat", self.base_url);

            let mut messages = Vec::new();
            if let Some(ref system) = request.system {
                messages.push(OllamaChatMessage {
                    role: "system".into(),
                    content: system.clone(),
                });
            }
            for msg in &request.messages {
                messages.push(OllamaChatMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                });
            }

            let body = OllamaChatRequest {
                model: request.model.clone(),
                messages,
                stream: false,
            };

            let req = self.guard.client().post(&url).json(&body);
            let response = self.guard.request(req).await?;
            let status = response.status();

            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Ollama error ({}): {}", status, body_text),
                });
            }

            let resp: OllamaChatResponse =
                response.json().await.map_err(|e| AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Failed to parse Ollama response: {}", e),
                })?;

            let content = resp.message.and_then(|m| m.content).unwrap_or_default();

            Ok(CompletionResponse {
                content,
                input_tokens: 0,
                output_tokens: 0,
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
            let url = format!("{}/api/chat", self.base_url);

            let mut messages = Vec::new();
            if let Some(ref system) = request.system {
                messages.push(OllamaChatMessage {
                    role: "system".into(),
                    content: system.clone(),
                });
            }
            for msg in &request.messages {
                messages.push(OllamaChatMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                });
            }

            let body = OllamaChatRequest {
                model: request.model.clone(),
                messages,
                stream: true,
            };

            let req = self.guard.client().post(&url).json(&body);
            let response = self.guard.request(req).await?;
            let status = response.status();

            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::AiError {
                    code: AiErrorCode::ApiError,
                    message: format!("Ollama stream error ({}): {}", status, body_text),
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

                                if line.is_empty() {
                                    continue;
                                }

                                if let Ok(chunk) = serde_json::from_str::<OllamaStreamChunk>(&line)
                                {
                                    if let Some(msg) = chunk.message {
                                        if let Some(text) = msg.content {
                                            if tx.send(Ok(text)).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                    if chunk.done == Some(true) {
                                        let _ = tx.send(Ok(String::new())).await;
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(Err(AppError::AiError {
                                    code: AiErrorCode::ApiError,
                                    message: format!("Ollama stream error: {}", e),
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
    fn test_ollama_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap());
        let provider = OllamaProvider::new(guard);
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_ollama_provider_models_empty() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap());
        let provider = OllamaProvider::new(guard);
        assert!(provider.models().is_empty());
    }
}
