use crate::error::{AppError, NetworkErrorCode};
use crate::network::guard::NetworkGuard;
use serde::{Deserialize, Serialize};

/// Configuration for AI-enhanced dictation correction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiCorrectionConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
}

/// Trait for AI-based text correction.
pub trait AiCorrector: Send + Sync {
    /// Correct grammar, spelling, and punctuation in dictated text.
    /// On failure, implementations must return the original text unchanged.
    fn correct(&self, text: &str) -> Result<String, AppError>;
}

/// Default corrector that returns text unchanged.
pub struct DisabledCorrector;

impl AiCorrector for DisabledCorrector {
    fn correct(&self, text: &str) -> Result<String, AppError> {
        Ok(text.to_string())
    }
}

/// AI-powered corrector that calls a configured provider endpoint.
pub struct ConfiguredCorrector {
    config: AiCorrectionConfig,
}

impl ConfiguredCorrector {
    pub fn new(config: AiCorrectionConfig) -> Self {
        Self { config }
    }

    /// Build the request body for the correction API.
    fn build_request_body(&self, text: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a dictation correction assistant. Fix grammar, spelling, and punctuation errors in the following dictated text. Return only the corrected text, nothing else."
                },
                {
                    "role": "user",
                    "content": text
                }
            ],
            "temperature": 0.1,
            "max_tokens": 1024
        })
    }

    /// Send the correction request via NetworkGuard.
    pub async fn correct_async(
        &self,
        text: &str,
        network: &NetworkGuard,
    ) -> Result<String, AppError> {
        if !self.config.enabled || self.config.endpoint.is_empty() {
            return Ok(text.to_string());
        }

        let body = self.build_request_body(text);

        let request = network.client().post(&self.config.endpoint).json(&body);

        let response = match network.request(request).await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!(error = %e, "AI correction request failed, returning original text");
                return Ok(text.to_string());
            }
        };

        let status = response.status();
        if !status.is_success() {
            tracing::warn!(status = %status, "AI correction API returned non-success status");
            return Ok(text.to_string());
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            tracing::warn!(error = %e, "Failed to parse AI correction response");
            AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                message: format!("Failed to parse AI response: {}", e),
            }
        })?;

        // Extract the corrected text from the response (OpenAI-compatible format)
        let corrected = response_body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or(text)
            .to_string();

        if corrected.is_empty() {
            return Ok(text.to_string());
        }

        Ok(corrected)
    }
}

impl AiCorrector for ConfiguredCorrector {
    fn correct(&self, text: &str) -> Result<String, AppError> {
        // Synchronous fallback: returns text unchanged.
        // The async version (correct_async) should be used in practice.
        if !self.config.enabled {
            return Ok(text.to_string());
        }
        tracing::warn!("ConfiguredCorrector::correct called synchronously; use correct_async for actual AI correction");
        Ok(text.to_string())
    }
}

/// Create the appropriate corrector based on configuration.
pub fn create_corrector(config: &AiCorrectionConfig) -> Box<dyn AiCorrector> {
    if config.enabled && !config.endpoint.is_empty() {
        Box::new(ConfiguredCorrector::new(config.clone()))
    } else {
        Box::new(DisabledCorrector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_corrector_returns_text_unchanged() {
        let corrector = DisabledCorrector;
        let result = corrector.correct("hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_disabled_corrector_empty_text() {
        let corrector = DisabledCorrector;
        let result = corrector.correct("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_configured_corrector_disabled_returns_unchanged() {
        let config = AiCorrectionConfig {
            enabled: false,
            endpoint: "http://localhost:8080/v1/chat/completions".into(),
            model: "test".into(),
        };
        let corrector = ConfiguredCorrector::new(config);
        let result = corrector.correct("hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_configured_corrector_enabled_sync_returns_unchanged() {
        let config = AiCorrectionConfig {
            enabled: true,
            endpoint: "http://localhost:8080/v1/chat/completions".into(),
            model: "test".into(),
        };
        let corrector = ConfiguredCorrector::new(config);
        // Synchronous fallback always returns text unchanged
        let result = corrector.correct("hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_create_corrector_disabled() {
        let config = AiCorrectionConfig::default();
        let corrector = create_corrector(&config);
        let result = corrector.correct("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_create_corrector_enabled_with_endpoint() {
        let config = AiCorrectionConfig {
            enabled: true,
            endpoint: "http://localhost:8080/v1/chat/completions".into(),
            model: "gpt-4".into(),
        };
        let corrector = create_corrector(&config);
        // Sync fallback
        let result = corrector.correct("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_create_corrector_enabled_no_endpoint() {
        let config = AiCorrectionConfig {
            enabled: true,
            endpoint: String::new(),
            model: "gpt-4".into(),
        };
        let corrector = create_corrector(&config);
        let result = corrector.correct("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_default_config() {
        let config = AiCorrectionConfig::default();
        assert!(!config.enabled);
        assert!(config.endpoint.is_empty());
        assert!(config.model.is_empty());
    }

    #[test]
    fn test_build_request_body() {
        let config = AiCorrectionConfig {
            enabled: true,
            endpoint: "http://localhost:8080".into(),
            model: "test-model".into(),
        };
        let corrector = ConfiguredCorrector::new(config);
        let body = corrector.build_request_body("hello world");
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["messages"][1]["content"], "hello world");
    }

    #[test]
    fn test_config_serialization() {
        let config = AiCorrectionConfig {
            enabled: true,
            endpoint: "http://example.com".into(),
            model: "gpt-4".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AiCorrectionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.enabled, config.enabled);
        assert_eq!(parsed.endpoint, config.endpoint);
        assert_eq!(parsed.model, config.model);
    }
}
