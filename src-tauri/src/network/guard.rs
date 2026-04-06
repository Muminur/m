use crate::error::{AppError, NetworkErrorCode};
use crate::settings::NetworkPolicy;
use reqwest::{Client, RequestBuilder, Response};

pub struct NetworkGuard {
    client: Client,
    policy: NetworkPolicy,
}

impl NetworkGuard {
    /// Only place in codebase allowed to call Client::builder()
    #[allow(clippy::disallowed_methods)]
    pub fn new(policy: NetworkPolicy) -> Result<Self, AppError> {
        let client = Client::builder()
            .user_agent("WhisperDesk/1.0")
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                message: format!("Failed to build HTTP client: {}", e),
            })?;
        Ok(Self { client, policy })
    }

    pub async fn request(&self, req: RequestBuilder) -> Result<Response, AppError> {
        match &self.policy {
            NetworkPolicy::Offline => Err(AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                message: "Network is disabled (offline mode)".into(),
            }),
            NetworkPolicy::LocalOnly => {
                let built = req.build().map_err(|e| AppError::NetworkError {
                    code: NetworkErrorCode::ConnectionFailed,
                    message: e.to_string(),
                })?;
                let host = built.url().host_str().unwrap_or("").to_lowercase();

                let is_local = host == "localhost"
                    || host == "127.0.0.1"
                    || host == "::1";

                if !is_local {
                    return Err(AppError::NetworkError {
                        code: NetworkErrorCode::PolicyBlocked,
                        message: format!(
                            "Network policy 'local_only' blocks external host: {}",
                            host
                        ),
                    });
                }

                self.client
                    .execute(built)
                    .await
                    .map_err(|e| AppError::NetworkError {
                        code: NetworkErrorCode::ConnectionFailed,
                        message: e.to_string(),
                    })
            }
            NetworkPolicy::AllowAll => req.send().await.map_err(|e| {
                let code = if e.is_timeout() {
                    NetworkErrorCode::Timeout
                } else if e.is_connect() {
                    NetworkErrorCode::ConnectionFailed
                } else if let Some(status) = e.status() {
                    NetworkErrorCode::HttpError {
                        status: status.as_u16(),
                    }
                } else {
                    NetworkErrorCode::ConnectionFailed
                };
                AppError::NetworkError {
                    code,
                    message: e.to_string(),
                }
            }),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn policy(&self) -> &NetworkPolicy {
        &self.policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_offline_policy_blocks_requests() {
        let guard = NetworkGuard::new(NetworkPolicy::Offline).unwrap();
        let req = guard.client().get("https://example.com");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            } => {}
            other => panic!("Expected PolicyBlocked, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_local_only_blocks_external_host() {
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        let req = guard.client().get("https://api.example.com/data");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            } => {}
            other => panic!("Expected PolicyBlocked, got {:?}", other),
        }
    }

    #[test]
    fn test_network_guard_new_allow_all() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        assert_eq!(guard.policy(), &NetworkPolicy::AllowAll);
    }

    #[test]
    fn test_network_guard_new_offline() {
        let guard = NetworkGuard::new(NetworkPolicy::Offline).unwrap();
        assert_eq!(guard.policy(), &NetworkPolicy::Offline);
    }
}
