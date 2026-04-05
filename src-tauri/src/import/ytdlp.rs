use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::error::{AppError, ImportErrorCode};

/// Status of the yt-dlp binary on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum YtDlpStatus {
    /// Binary found and meets minimum version requirement.
    Available {
        version: String,
        path: String,
    },
    /// Binary not found on this system.
    NotFound,
    /// Binary found but version is below the minimum required.
    Outdated {
        version: String,
        minimum: String,
    },
}

/// Minimum yt-dlp version required (year.month.day format).
pub const MINIMUM_VERSION: &str = "2023.01.01";

/// Manager responsible for locating and validating the yt-dlp binary.
pub struct YtDlpManager;

impl YtDlpManager {
    /// Check PATH and well-known locations for yt-dlp and return its status.
    pub fn detect() -> Result<YtDlpStatus, AppError> {
        let path = match Self::find_binary() {
            Some(p) => p,
            None => return Ok(YtDlpStatus::NotFound),
        };

        let version = Self::query_version(&path)?;

        if Self::version_meets_minimum(&version, MINIMUM_VERSION) {
            Ok(YtDlpStatus::Available {
                version,
                path: path.to_string_lossy().into_owned(),
            })
        } else {
            Ok(YtDlpStatus::Outdated {
                version,
                minimum: MINIMUM_VERSION.to_string(),
            })
        }
    }

    /// Return the path to a usable yt-dlp binary, or an error if not found.
    pub fn get_binary_path() -> Result<PathBuf, AppError> {
        Self::find_binary().ok_or_else(|| AppError::ImportError {
            code: ImportErrorCode::YtDlpNotFound,
            message: "yt-dlp binary not found. Install it via 'pip install yt-dlp' or from https://github.com/yt-dlp/yt-dlp".into(),
        })
    }

    /// Returns `true` if yt-dlp is available and meets the minimum version.
    pub fn is_available() -> bool {
        matches!(Self::detect(), Ok(YtDlpStatus::Available { .. }))
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Search PATH and common install locations for a yt-dlp executable.
    fn find_binary() -> Option<PathBuf> {
        // 1. PATH lookup via `which` / `where`
        let candidates: &[&str] = if cfg!(target_os = "windows") {
            &["yt-dlp.exe", "yt-dlp"]
        } else {
            &["yt-dlp"]
        };

        for name in candidates {
            if let Ok(output) = Command::new(if cfg!(target_os = "windows") { "where" } else { "which" })
                .arg(name)
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let first_line = stdout.lines().next().unwrap_or("").trim().to_string();
                    if !first_line.is_empty() {
                        return Some(PathBuf::from(first_line));
                    }
                }
            }
        }

        // 2. Homebrew (macOS / Linux)
        let homebrew_path = PathBuf::from("/opt/homebrew/bin/yt-dlp");
        if homebrew_path.exists() {
            return Some(homebrew_path);
        }
        // Intel Homebrew
        let homebrew_intel = PathBuf::from("/usr/local/bin/yt-dlp");
        if homebrew_intel.exists() {
            return Some(homebrew_intel);
        }

        // 3. Local app data (Windows)
        if cfg!(target_os = "windows") {
            if let Some(local_app_data) = dirs::data_local_dir() {
                let win_path = local_app_data.join("Programs").join("yt-dlp").join("yt-dlp.exe");
                if win_path.exists() {
                    return Some(win_path);
                }
            }
        }

        // 4. Common Linux user installs
        if let Some(home) = dirs::home_dir() {
            let user_bin = home.join(".local").join("bin").join("yt-dlp");
            if user_bin.exists() {
                return Some(user_bin);
            }
        }

        None
    }

    /// Run `yt-dlp --version` and return the trimmed version string.
    fn query_version(path: &PathBuf) -> Result<String, AppError> {
        let output = Command::new(path)
            .arg("--version")
            .output()
            .map_err(|e| AppError::ImportError {
                code: ImportErrorCode::YtDlpNotFound,
                message: format!("Failed to run yt-dlp --version: {}", e),
            })?;

        if !output.status.success() {
            return Err(AppError::ImportError {
                code: ImportErrorCode::YtDlpNotFound,
                message: "yt-dlp --version returned non-zero exit code".into(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Compare version strings in `YYYY.MM.DD` format.
    /// Returns `true` if `version >= minimum`.
    pub fn version_meets_minimum(version: &str, minimum: &str) -> bool {
        // Normalise: strip any trailing patch/build suffix after the date portion
        let normalise = |s: &str| -> (u32, u32, u32) {
            let parts: Vec<&str> = s.splitn(4, '.').collect();
            let year = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0u32);
            let month = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0u32);
            let day = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0u32);
            (year, month, day)
        };

        normalise(version) >= normalise(minimum)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yt_dlp_status_available_serializes() {
        let status = YtDlpStatus::Available {
            version: "2024.01.15".into(),
            path: "/usr/local/bin/yt-dlp".into(),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["status"], "available");
        assert_eq!(json["version"], "2024.01.15");
        assert_eq!(json["path"], "/usr/local/bin/yt-dlp");
    }

    #[test]
    fn test_yt_dlp_status_not_found_serializes() {
        let status = YtDlpStatus::NotFound;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["status"], "notFound");
    }

    #[test]
    fn test_yt_dlp_status_outdated_serializes() {
        let status = YtDlpStatus::Outdated {
            version: "2022.05.10".into(),
            minimum: MINIMUM_VERSION.into(),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["status"], "outdated");
        assert_eq!(json["version"], "2022.05.10");
        assert_eq!(json["minimum"], MINIMUM_VERSION);
    }

    #[test]
    fn test_version_meets_minimum_equal() {
        assert!(YtDlpManager::version_meets_minimum("2023.01.01", "2023.01.01"));
    }

    #[test]
    fn test_version_meets_minimum_newer() {
        assert!(YtDlpManager::version_meets_minimum("2024.06.20", "2023.01.01"));
    }

    #[test]
    fn test_version_meets_minimum_older() {
        assert!(!YtDlpManager::version_meets_minimum("2022.12.31", "2023.01.01"));
    }

    #[test]
    fn test_version_month_boundary() {
        assert!(YtDlpManager::version_meets_minimum("2023.02.01", "2023.01.15"));
        assert!(!YtDlpManager::version_meets_minimum("2023.01.01", "2023.02.01"));
    }
}
