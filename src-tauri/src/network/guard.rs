use crate::error::{AppError, NetworkErrorCode};
use crate::settings::NetworkPolicy;
use reqwest::{Client, RequestBuilder, Response};

/// Maximum response body bytes to capture in error messages (4 KB).
const MAX_ERROR_BODY_BYTES: usize = 4096;

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
                    || host.starts_with("127.")
                    || host == "::1"
                    || host == "0.0.0.0"
                    || host.starts_with("fe80");

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
                        response_body: None,
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

    /// Like [`request`](Self::request), but automatically checks the HTTP status
    /// code and returns an error with the (truncated) response body for non-2xx
    /// responses. Use this when you want errors to include the server's message.
    pub async fn request_checked(&self, req: RequestBuilder) -> Result<Response, AppError> {
        let response = self.request(req).await?;
        let status = response.status();
        if !status.is_success() {
            let body = Self::read_error_body(response).await;
            return Err(AppError::NetworkError {
                code: NetworkErrorCode::HttpError {
                    status: status.as_u16(),
                    response_body: Some(body.clone()),
                },
                message: format!("HTTP {} — {}", status.as_u16(), body),
            });
        }
        Ok(response)
    }

    /// Read the response body for error reporting, truncating to [`MAX_ERROR_BODY_BYTES`].
    async fn read_error_body(response: Response) -> String {
        let bytes = response
            .bytes()
            .await
            .unwrap_or_default();
        let truncated = &bytes[..bytes.len().min(MAX_ERROR_BODY_BYTES)];
        String::from_utf8_lossy(truncated).into_owned()
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
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

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

    #[tokio::test]
    async fn test_local_only_allows_localhost() {
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        // 127.0.0.1 is local -- request will be attempted (may fail to connect, but not PolicyBlocked)
        let req = guard.client().get("http://127.0.0.1:99999/test");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                ..
            } => {}
            other => panic!("Expected ConnectionFailed for local host, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_local_only_allows_127_range() {
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        let req = guard.client().get("http://127.0.0.2:99999/test");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                ..
            } => {}
            other => panic!("Expected ConnectionFailed for 127.x, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_local_only_blocks_zero_addr() {
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        // 0.0.0.0 is now treated as local, not blocked as external
        let req = guard.client().get("http://0.0.0.0:99999/test");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                ..
            } => {}
            other => panic!("Expected ConnectionFailed for 0.0.0.0, got {:?}", other),
        }
    }

    /// Spawn a minimal HTTP server that always replies with the given status and body.
    async fn spawn_http_server(status: u16, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let response_bytes = format!(
            "HTTP/1.1 {} ERR\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body,
        );
        tokio::spawn(async move {
            // Accept one connection and send the canned response.
            if let Ok((mut stream, _)) = listener.accept().await {
                // Read the request (discard it).
                let mut buf = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                let _ = stream.write_all(response_bytes.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    #[tokio::test]
    async fn test_request_checked_captures_body_on_error() {
        let error_body = r#"{"error":"invalid_api_key","message":"The API key is not valid"}"#;
        let url = spawn_http_server(401, error_body).await;

        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let req = guard.client().get(&url);
        let result = guard.request_checked(req).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError { code, message } => {
                match &code {
                    NetworkErrorCode::HttpError {
                        status,
                        response_body,
                    } => {
                        assert_eq!(*status, 401);
                        let body = response_body.as_deref().unwrap();
                        assert!(
                            body.contains("invalid_api_key"),
                            "response_body should contain the API error: {}",
                            body
                        );
                    }
                    other => panic!("Expected HttpError, got {:?}", other),
                }
                assert!(
                    message.contains("invalid_api_key"),
                    "message should contain the API error: {}",
                    message
                );
            }
            other => panic!("Expected NetworkError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_request_checked_passes_success_through() {
        let url = spawn_http_server(200, "OK").await;

        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let req = guard.client().get(&url);
        let result = guard.request_checked(req).await;

        assert!(result.is_ok(), "200 response should pass through");
    }

    #[tokio::test]
    async fn test_request_checked_truncates_large_body() {
        // 8 KB body — should be truncated to 4 KB
        let large_body: String = "X".repeat(8192);
        let url = spawn_http_server(500, &large_body).await;

        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let req = guard.client().get(&url);
        let result = guard.request_checked(req).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError { code, .. } => match code {
                NetworkErrorCode::HttpError {
                    response_body, ..
                } => {
                    let body = response_body.unwrap();
                    assert_eq!(
                        body.len(),
                        MAX_ERROR_BODY_BYTES,
                        "Body should be truncated to {} bytes",
                        MAX_ERROR_BODY_BYTES
                    );
                }
                other => panic!("Expected HttpError, got {:?}", other),
            },
            other => panic!("Expected NetworkError, got {:?}", other),
        }
    }
}
