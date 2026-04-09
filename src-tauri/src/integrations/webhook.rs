use crate::error::{AppError, IntegrationErrorCode};
use crate::network::guard::NetworkGuard;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Configuration for outgoing webhooks.
pub struct WebhookConfig {
    pub url: String,
    pub secret: Option<String>,
}

/// Compute HMAC-SHA256 hex digest of the given body using the provided secret.
fn compute_signature(secret: &str, body: &[u8]) -> Result<String, AppError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| {
        AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Invalid HMAC key: {}", e),
        }
    })?;
    mac.update(body);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    Ok(hex::encode(bytes))
}

/// Fire a webhook by POSTing a JSON payload to the configured URL.
///
/// If a secret is configured, the request includes an `X-Webhook-Signature`
/// header containing `sha256={hex_digest}`.
pub async fn fire_webhook(
    config: &WebhookConfig,
    event: &str,
    payload: serde_json::Value,
    guard: &NetworkGuard,
) -> Result<(), AppError> {
    if config.url.is_empty() {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: "Webhook URL cannot be empty".into(),
        });
    }

    let body = serde_json::json!({
        "event": event,
        "payload": payload,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let body_bytes = serde_json::to_vec(&body).map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ApiError,
        message: format!("Failed to serialize webhook body: {}", e),
    })?;

    let mut req = guard
        .client()
        .post(&config.url)
        .header("Content-Type", "application/json")
        .header("X-Webhook-Event", event);

    if let Some(secret) = &config.secret {
        if !secret.is_empty() {
            let sig = compute_signature(secret, &body_bytes)?;
            req = req.header("X-Webhook-Signature", format!("sha256={}", sig));
        }
    }

    let req = req.body(body_bytes);

    let resp = guard.request(req).await?;
    let status = resp.status();

    if !status.is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: format!("Webhook returned error ({}): {}", status, error_text),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_signature() {
        let sig = compute_signature("my_secret", b"hello world").unwrap();
        // Known HMAC-SHA256("my_secret", "hello world") hex
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn test_compute_signature_deterministic() {
        let sig1 = compute_signature("key", b"data").unwrap();
        let sig2 = compute_signature("key", b"data").unwrap();
        assert_eq!(sig1, sig2);
    }
}
