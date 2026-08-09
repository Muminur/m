use crate::error::{AppError, NetworkErrorCode};
use crate::settings::NetworkPolicy;
use reqwest::{Client, RequestBuilder, Response};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};
use url::Host;

/// Maximum response body bytes to capture in error messages (4 KB).
const MAX_ERROR_BODY_BYTES: usize = 4096;

#[derive(Clone)]
pub struct NetworkGuard {
    client: Client,
    // All clones (and restricted children) must observe updates to the same
    // managed authority; do not snapshot this into a new guard.
    policy: Arc<RwLock<NetworkPolicy>>,
    restriction: NetworkPolicy,
}

#[derive(Debug)]
struct RedirectBlocked {
    url: String,
}

impl fmt::Display for RedirectBlocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redirect blocked by network policy: {}", self.url)
    }
}

impl Error for RedirectBlocked {}

impl NetworkGuard {
    /// Only place in codebase allowed to call Client::builder()
    #[allow(clippy::disallowed_methods)]
    pub fn new(policy: NetworkPolicy) -> Result<Self, AppError> {
        Self::with_shared_policy(Arc::new(RwLock::new(policy)), NetworkPolicy::AllowAll)
    }

    /// Make a child which shares policy updates but can never exceed local-only
    /// access. This is used for local services such as Ollama.
    pub fn restricted_to_local(&self) -> Result<Self, AppError> {
        Self::with_shared_policy(Arc::clone(&self.policy), NetworkPolicy::LocalOnly)
    }

    #[allow(clippy::disallowed_methods)]
    fn with_shared_policy(
        policy: Arc<RwLock<NetworkPolicy>>,
        restriction: NetworkPolicy,
    ) -> Result<Self, AppError> {
        let redirect_policy = Arc::clone(&policy);
        let redirect_restriction = restriction.clone();
        let client = Client::builder()
            .user_agent("WhisperDesk/1.0")
            .connect_timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let effective =
                    effective_policy(read_policy(&redirect_policy), redirect_restriction.clone());
                if url_allowed_by_policy(attempt.url(), effective) {
                    reqwest::redirect::Policy::default().redirect(attempt)
                } else {
                    let url = attempt.url().to_string();
                    attempt.error(RedirectBlocked { url })
                }
            }))
            .build()
            .map_err(|e| AppError::NetworkError {
                code: NetworkErrorCode::ConnectionFailed,
                message: format!("Failed to build HTTP client: {}", e),
            })?;
        Ok(Self {
            client,
            policy,
            restriction,
        })
    }

    pub async fn request(&self, req: RequestBuilder) -> Result<Response, AppError> {
        let built = req.build().map_err(network_request_error)?;
        let effective = effective_policy(self.policy(), self.restriction.clone());
        if !url_allowed_by_policy(built.url(), effective.clone()) {
            return Err(policy_blocked_for_url(built.url(), effective));
        }

        self.client
            .execute(built)
            .await
            .map_err(network_request_error)
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
        let bytes = response.bytes().await.unwrap_or_default();
        let truncated = &bytes[..bytes.len().min(MAX_ERROR_BODY_BYTES)];
        String::from_utf8_lossy(truncated).into_owned()
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Return the policy currently applied to new requests.
    pub fn policy(&self) -> NetworkPolicy {
        read_policy(&self.policy)
    }

    /// Apply a policy to requests created after this call.
    pub fn set_policy(&self, policy: NetworkPolicy) {
        *self
            .policy
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
    }
}

fn read_policy(policy: &RwLock<NetworkPolicy>) -> NetworkPolicy {
    policy
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn effective_policy(global: NetworkPolicy, restriction: NetworkPolicy) -> NetworkPolicy {
    use NetworkPolicy::*;
    match (global, restriction) {
        (Offline, _) | (_, Offline) => Offline,
        (LocalOnly, _) | (_, LocalOnly) => LocalOnly,
        (AllowAll, AllowAll) => AllowAll,
    }
}

fn url_allowed_by_policy(url: &reqwest::Url, policy: NetworkPolicy) -> bool {
    match policy {
        NetworkPolicy::Offline => false,
        NetworkPolicy::LocalOnly => url.host().is_some_and(is_local_host),
        NetworkPolicy::AllowAll => true,
    }
}

fn policy_blocked_for_url(url: &reqwest::Url, policy: NetworkPolicy) -> AppError {
    let message = match policy {
        NetworkPolicy::Offline => "Network is disabled (offline mode)".into(),
        NetworkPolicy::LocalOnly => format!(
            "Network policy 'local_only' blocks external host: {}",
            url.host_str().unwrap_or("")
        ),
        NetworkPolicy::AllowAll => "Network policy blocks this request".into(),
    };
    AppError::NetworkError {
        code: NetworkErrorCode::PolicyBlocked,
        message,
    }
}

fn network_request_error(error: reqwest::Error) -> AppError {
    let code = if redirect_was_blocked(&error) {
        NetworkErrorCode::PolicyBlocked
    } else if error.is_timeout() {
        NetworkErrorCode::Timeout
    } else if error.is_connect() {
        NetworkErrorCode::ConnectionFailed
    } else if let Some(status) = error.status() {
        NetworkErrorCode::HttpError {
            status: status.as_u16(),
            response_body: None,
        }
    } else {
        NetworkErrorCode::ConnectionFailed
    };
    AppError::NetworkError {
        code,
        message: error.to_string(),
    }
}

fn redirect_was_blocked(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(current) = source {
        if current.downcast_ref::<RedirectBlocked>().is_some() {
            return true;
        }
        source = current.source();
    }
    false
}

/// Local-only access permits only the exact `localhost` name and loopback,
/// private, or link-local IP literals. DNS names that merely resemble an IP
/// address (such as `127.attacker.example`) are external and are blocked.
fn is_local_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Host::Ipv6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
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
        assert_eq!(guard.policy(), NetworkPolicy::AllowAll);
    }

    #[test]
    fn test_network_guard_new_offline() {
        let guard = NetworkGuard::new(NetworkPolicy::Offline).unwrap();
        assert_eq!(guard.policy(), NetworkPolicy::Offline);
    }

    #[test]
    fn test_local_only_allows_loopback_hosts() {
        for url in ["http://127.0.0.1/test", "http://127.0.0.2/test"] {
            assert!(url_allowed_by_policy(
                &reqwest::Url::parse(url).unwrap(),
                NetworkPolicy::LocalOnly
            ));
        }
    }

    #[tokio::test]
    async fn test_local_only_blocks_zero_addr() {
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        // An unspecified address is not loopback, private, or link-local.
        let req = guard.client().get("http://0.0.0.0:8080/test");
        let result = guard.request(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            } => {}
            other => panic!("Expected PolicyBlocked for 0.0.0.0, got {:?}", other),
        }
    }

    #[test]
    fn local_only_host_policy_allows_only_local_literals() {
        let allowed = [
            "http://localhost",
            "http://127.0.0.1",
            "http://10.1.2.3",
            "http://172.16.1.1",
            "http://192.168.1.1",
            "http://169.254.1.1",
            "http://[::1]",
            "http://[fc00::1]",
            "http://[fe80::1]",
        ];
        let blocked = [
            "http://localhost.attacker.example",
            "http://127.attacker.example",
            "http://fe80.attacker.example",
            "http://8.8.8.8",
            "http://0.0.0.0",
            "http://[2001:4860:4860::8888]",
        ];

        for url in allowed {
            assert!(
                is_local_host(reqwest::Url::parse(url).unwrap().host().unwrap()),
                "expected {url} to be allowed"
            );
        }
        for url in blocked {
            assert!(
                !is_local_host(reqwest::Url::parse(url).unwrap().host().unwrap()),
                "expected {url} to be blocked"
            );
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

    /// Spawn a one-shot local server that redirects to `location`.
    async fn spawn_redirect_server(location: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    #[tokio::test]
    async fn policy_updates_apply_to_new_requests() {
        let guard = NetworkGuard::new(NetworkPolicy::Offline).unwrap();
        guard.set_policy(NetworkPolicy::LocalOnly);

        let url = spawn_http_server(200, "OK").await;
        assert!(guard.request(guard.client().get(url)).await.is_ok());

        guard.set_policy(NetworkPolicy::Offline);
        let result = guard.request(guard.client().get("http://127.0.0.1")).await;
        assert!(matches!(
            result,
            Err(AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            })
        ));
    }

    #[test]
    fn clones_share_policy_updates() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let clone = guard.clone();
        guard.set_policy(NetworkPolicy::Offline);
        assert_eq!(clone.policy(), NetworkPolicy::Offline);
    }

    #[tokio::test]
    async fn restricted_ollama_child_respects_global_offline() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let ollama_guard = guard.restricted_to_local().unwrap();
        guard.set_policy(NetworkPolicy::Offline);

        let result = ollama_guard
            .request(ollama_guard.client().get("http://127.0.0.1:8080/api/tags"))
            .await;
        assert!(matches!(
            result,
            Err(AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn local_only_blocks_redirect_escape() {
        let url = spawn_redirect_server("http://example.com/escaped").await;
        let guard = NetworkGuard::new(NetworkPolicy::LocalOnly).unwrap();
        let result = guard.request(guard.client().get(url)).await;
        assert!(matches!(
            result,
            Err(AppError::NetworkError {
                code: NetworkErrorCode::PolicyBlocked,
                ..
            })
        ));
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
                NetworkErrorCode::HttpError { response_body, .. } => {
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
