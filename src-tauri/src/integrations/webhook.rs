use crate::error::{AppError, IntegrationErrorCode};
use crate::network::guard::NetworkGuard;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use url::Url;

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

/// Validate that a webhook URL is safe (no SSRF to internal networks).
fn validate_webhook_url(raw: &str) -> Result<(), AppError> {
    let parsed = Url::parse(raw).map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ConfigurationMissing,
        message: format!("Invalid webhook URL: {}", e),
    })?;

    // Only HTTPS allowed
    if parsed.scheme() != "https" {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Webhook URL must use https scheme, got '{}'", parsed.scheme()),
        });
    }

    let host = parsed.host_str().unwrap_or("").to_lowercase();

    // Block loopback and internal hosts
    if host == "localhost"
        || host == "127.0.0.1"
        || host.starts_with("127.")
        || host == "::1"
        || host == "0.0.0.0"
        || host == "169.254.169.254"
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("fe80")
    {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ConfigurationMissing,
            message: format!("Webhook URL targets a blocked internal host: {}", host),
        });
    }

    // Block 172.16.0.0/12 range (172.16.* through 172.31.*)
    if host.starts_with("172.") {
        if let Some(second_octet) = host.split('.').nth(1) {
            if let Ok(octet) = second_octet.parse::<u8>() {
                if (16..=31).contains(&octet) {
                    return Err(AppError::IntegrationError {
                        code: IntegrationErrorCode::ConfigurationMissing,
                        message: format!("Webhook URL targets a blocked internal host: {}", host),
                    });
                }
            }
        }
    }

    Ok(())
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

    validate_webhook_url(&config.url)?;

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

    #[test]
    fn test_validate_webhook_url_https_allowed() {
        assert!(validate_webhook_url("https://hooks.example.com/webhook").is_ok());
    }

    #[test]
    fn test_validate_webhook_url_http_rejected() {
        assert!(validate_webhook_url("http://hooks.example.com/webhook").is_err());
    }

    #[test]
    fn test_validate_webhook_url_localhost_rejected() {
        assert!(validate_webhook_url("https://localhost/webhook").is_err());
        assert!(validate_webhook_url("https://127.0.0.1/webhook").is_err());
        assert!(validate_webhook_url("https://127.0.0.2/webhook").is_err());
        assert!(validate_webhook_url("https://[::1]/webhook").is_err());
        assert!(validate_webhook_url("https://0.0.0.0/webhook").is_err());
    }

    #[test]
    fn test_validate_webhook_url_internal_networks_rejected() {
        assert!(validate_webhook_url("https://10.0.0.1/webhook").is_err());
        assert!(validate_webhook_url("https://192.168.1.1/webhook").is_err());
        assert!(validate_webhook_url("https://172.16.0.1/webhook").is_err());
        assert!(validate_webhook_url("https://172.31.255.255/webhook").is_err());
        assert!(validate_webhook_url("https://169.254.169.254/latest/meta-data").is_err());
    }

    #[test]
    fn test_validate_webhook_url_172_outside_range_allowed() {
        // 172.15.x.x and 172.32.x.x are not RFC-1918
        assert!(validate_webhook_url("https://172.15.0.1/webhook").is_ok());
        assert!(validate_webhook_url("https://172.32.0.1/webhook").is_ok());
    }

    #[test]
    fn test_validate_webhook_url_link_local_ipv6_rejected() {
        assert!(validate_webhook_url("https://fe80::1/webhook").is_err());
    }
}
