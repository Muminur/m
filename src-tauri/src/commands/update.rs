//! Auto-update commands using tauri-plugin-updater.

use crate::error::{AppError, NetworkErrorCode};
use crate::network::guard::NetworkGuard;
use crate::settings::NetworkPolicy;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{command, AppHandle, State};
use tauri_plugin_updater::UpdaterExt;

const UPDATER_ENDPOINT: &str = "https://whisperdesk-updater.whisperdesk.workers.dev";

/// Guard against concurrent update installs.
static INSTALLING: AtomicBool = AtomicBool::new(false);

/// Information about an available update.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

/// Return the current application version from package info.
#[command]
pub async fn get_app_version(app: AppHandle) -> Result<String, AppError> {
    Ok(app.package_info().version.to_string())
}

fn updater_probe_url(version: &str) -> Option<String> {
    let target = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return None;
    };

    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        _ => return None,
    };

    Some(format!("{UPDATER_ENDPOINT}/{target}/{arch}/{version}"))
}

fn updater_status_has_payload(status: reqwest::StatusCode) -> Option<bool> {
    if status == reqwest::StatusCode::NO_CONTENT || status == reqwest::StatusCode::NOT_FOUND {
        Some(false)
    } else if status.is_success() {
        Some(true)
    } else {
        None
    }
}

/// The updater plugin owns its HTTP client, so its requests cannot be routed
/// through `NetworkGuard::request`. Re-check the shared policy immediately
/// before each plugin operation which can access the network.
fn ensure_updater_network_allowed(network: &NetworkGuard) -> Result<(), AppError> {
    match network.policy() {
        NetworkPolicy::AllowAll => Ok(()),
        NetworkPolicy::Offline => Err(AppError::NetworkError {
            code: NetworkErrorCode::PolicyBlocked,
            message: "Network is disabled (offline mode)".into(),
        }),
        NetworkPolicy::LocalOnly => Err(AppError::NetworkError {
            code: NetworkErrorCode::PolicyBlocked,
            message: "Network policy 'local_only' blocks the external update service".into(),
        }),
    }
}

/// Probe the dynamic endpoint before invoking the updater plugin.
///
/// The deployed endpoint historically returned 404 when a release did not
/// contain a compatible signed bundle. That means "no update" rather than an
/// application error. The probe keeps those expected responses out of the
/// updater's error logs while the Worker fix rolls out.
async fn updater_payload_available(
    app: &AppHandle,
    network: &NetworkGuard,
) -> Result<bool, AppError> {
    let version = app.package_info().version.to_string();
    let Some(url) = updater_probe_url(&version) else {
        return Ok(false);
    };

    let request = network.client().get(url).timeout(Duration::from_secs(15));
    let response = network.request(request).await?;

    updater_status_has_payload(response.status()).ok_or_else(|| AppError::NetworkError {
        code: NetworkErrorCode::HttpError {
            status: response.status().as_u16(),
            response_body: None,
        },
        message: format!(
            "Update endpoint returned unexpected status {}",
            response.status()
        ),
    })
}

/// Check whether an update is available.
///
/// Returns `Some(UpdateInfo)` when a newer version exists, `None` when up to date.
#[command]
pub async fn check_for_update(
    app: AppHandle,
    network: State<'_, Arc<NetworkGuard>>,
) -> Result<Option<UpdateInfo>, AppError> {
    if !updater_payload_available(&app, network.inner()).await? {
        return Ok(None);
    }

    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: format!("Failed to build updater: {}", e),
        })?;

    // `updater.check` uses the plugin's client, not NetworkGuard's client.
    ensure_updater_network_allowed(network.inner())?;
    let update = updater.check().await.map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Update check failed: {}", e),
    })?;

    match update {
        Some(u) => Ok(Some(UpdateInfo {
            version: u.version.clone(),
            body: u.body.clone(),
            date: u.date.map(|d| d.to_string()),
        })),
        None => Ok(None),
    }
}

/// Download and install the pending update, then restart the application.
///
/// Guards against concurrent invocations with an atomic flag — a second call
/// while an install is in progress returns an error immediately.
#[command]
pub async fn download_and_install_update(
    app: AppHandle,
    network: State<'_, Arc<NetworkGuard>>,
) -> Result<(), AppError> {
    if INSTALLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: "An update install is already in progress".into(),
        });
    }

    let result = do_install(&app, network.inner()).await;
    // Reset flag on error; on success app.restart() diverges so this is unreachable.
    if result.is_err() {
        INSTALLING.store(false, Ordering::SeqCst);
    }
    result
}

async fn do_install(app: &AppHandle, network: &NetworkGuard) -> Result<(), AppError> {
    if !updater_payload_available(app, network).await? {
        return Err(AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: "No update available to install".into(),
        });
    }

    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: format!("Failed to build updater: {}", e),
        })?;

    // The policy may have changed while the endpoint probe was in flight.
    ensure_updater_network_allowed(network)?;
    let update = updater.check().await.map_err(|e| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: format!("Update check failed: {}", e),
    })?;

    let update = update.ok_or_else(|| AppError::NetworkError {
        code: NetworkErrorCode::ConnectionFailed,
        message: "No update available to install".into(),
    })?;

    // Check again after discovery and immediately before the plugin begins
    // downloading/installing the release payload.
    ensure_updater_network_allowed(network)?;
    update
        .download_and_install(|_chunk_length, _content_length| {}, || {})
        .await
        .map_err(|e| AppError::NetworkError {
            code: NetworkErrorCode::ConnectionFailed,
            message: format!("Download/install failed: {}", e),
        })?;

    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_info_serialization() {
        let info = UpdateInfo {
            version: "1.2.0".into(),
            body: Some("Bug fixes".into()),
            date: Some("2025-01-15".into()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["version"], "1.2.0");
        assert_eq!(json["body"], "Bug fixes");
        assert_eq!(json["date"], "2025-01-15");
    }

    #[test]
    fn test_update_info_serialization_none_fields() {
        let info = UpdateInfo {
            version: "1.0.0".into(),
            body: None,
            date: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["version"], "1.0.0");
        assert!(json["body"].is_null());
        assert!(json["date"].is_null());
    }

    #[test]
    fn updater_probe_accepts_no_update_responses() {
        assert_eq!(
            updater_status_has_payload(reqwest::StatusCode::NO_CONTENT),
            Some(false)
        );
        assert_eq!(
            updater_status_has_payload(reqwest::StatusCode::NOT_FOUND),
            Some(false)
        );
    }

    #[test]
    fn updater_probe_requires_a_successful_payload() {
        assert_eq!(
            updater_status_has_payload(reqwest::StatusCode::OK),
            Some(true)
        );
        assert_eq!(
            updater_status_has_payload(reqwest::StatusCode::BAD_GATEWAY),
            None
        );
    }

    #[test]
    fn updater_plugin_operations_require_allow_all_policy() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        assert!(ensure_updater_network_allowed(&guard).is_ok());

        for policy in [NetworkPolicy::Offline, NetworkPolicy::LocalOnly] {
            guard.set_policy(policy);
            assert!(matches!(
                ensure_updater_network_allowed(&guard),
                Err(AppError::NetworkError {
                    code: NetworkErrorCode::PolicyBlocked,
                    ..
                })
            ));
        }
    }

    #[test]
    fn updater_policy_recheck_observes_runtime_changes() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        assert!(ensure_updater_network_allowed(&guard).is_ok());
        guard.set_policy(NetworkPolicy::Offline);
        assert!(ensure_updater_network_allowed(&guard).is_err());
    }

    #[test]
    fn updater_probe_url_matches_supported_build_target() {
        let url = updater_probe_url("1.2.3");

        if cfg!(any(target_os = "macos", target_os = "windows"))
            && matches!(std::env::consts::ARCH, "aarch64" | "x86_64")
        {
            let url = url.expect("supported desktop target should have an updater URL");
            assert!(url.ends_with("/1.2.3"));
            assert!(url.contains(std::env::consts::ARCH));
        } else {
            assert!(url.is_none());
        }
    }
}
