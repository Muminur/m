use crate::error::{AppError, StorageErrorCode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Offline,
    LocalOnly,
    #[default]
    AllowAll,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AccelerationBackend {
    #[default]
    Auto,
    Cpu,
    Metal,
    /// Coming soon: requires .mlmodelc packages not yet available on any CDN.
    CoreMl,
}

impl fmt::Display for AccelerationBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AccelerationBackend::Auto => "auto",
            AccelerationBackend::Cpu => "cpu",
            AccelerationBackend::Metal => "metal",
            AccelerationBackend::CoreMl => "core_ml",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchFolderConfig {
    pub path: String,
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_language")]
    pub language: String,
    pub default_model_id: Option<String>,
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    #[serde(default = "default_true")]
    pub logs_enabled: bool,
    #[serde(default)]
    pub watch_folders: Vec<WatchFolderConfig>,
    #[serde(default = "default_true")]
    pub show_onboarding: bool,
    #[serde(default)]
    pub global_shortcut_transcribe: Option<String>,
    #[serde(default)]
    pub global_shortcut_dictate: Option<String>,
    #[serde(default)]
    pub acceleration_backend: AccelerationBackend,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: "en".to_string(),
            default_model_id: None,
            network_policy: NetworkPolicy::AllowAll,
            logs_enabled: true,
            watch_folders: vec![],
            show_onboarding: true,
            global_shortcut_transcribe: None,
            global_shortcut_dictate: None,
            acceleration_backend: AccelerationBackend::Auto,
        }
    }
}

impl AppSettings {
    fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
        app.path()
            .app_config_dir()
            .map(|dir| dir.join("settings.json"))
            .map_err(|_| AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: "Failed to determine app config directory".into(),
            })
    }

    pub fn load(app: &AppHandle) -> Result<Self, AppError> {
        let path = Self::settings_path(app)?;

        if !path.exists() {
            let settings = AppSettings::default();
            settings.save(app)?;
            return Ok(settings);
        }

        let content = std::fs::read_to_string(&path).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Failed to read settings: {}", e),
        })?;

        serde_json::from_str(&content).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Failed to parse settings: {}", e),
        })
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), AppError> {
        let path = Self::settings_path(app)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::StorageError {
                code: StorageErrorCode::IoError,
                message: format!("Failed to create config dir: {}", e),
            })?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Failed to serialize settings: {}", e),
        })?;

        std::fs::write(&path, content).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("Failed to write settings: {}", e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = AppSettings::default();
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.language, "en");
        assert_eq!(s.network_policy, NetworkPolicy::AllowAll);
        assert!(s.logs_enabled);
        assert!(s.show_onboarding);
    }

    #[test]
    fn test_settings_round_trip() {
        let mut s = AppSettings::default();
        s.theme = Theme::Dark;
        s.network_policy = NetworkPolicy::Offline;
        s.language = "nl".to_string();

        let json = serde_json::to_string(&s).unwrap();
        let s2: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(s2.theme, Theme::Dark);
        assert_eq!(s2.network_policy, NetworkPolicy::Offline);
        assert_eq!(s2.language, "nl");
    }

    #[test]
    fn test_network_policy_serialization() {
        let policy = NetworkPolicy::Offline;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"offline\"");

        let policy2: NetworkPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy2, NetworkPolicy::Offline);
    }

    #[test]
    fn test_acceleration_backend_serialization() {
        let variants = [
            (AccelerationBackend::Auto, "\"auto\""),
            (AccelerationBackend::Cpu, "\"cpu\""),
            (AccelerationBackend::Metal, "\"metal\""),
            (AccelerationBackend::CoreMl, "\"core_ml\""),
        ];
        for (variant, expected) in &variants {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(&json, expected, "wrong serialization for {:?}", variant);
            let back: AccelerationBackend = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, variant);
        }
    }

    #[test]
    fn test_settings_backward_compat_no_acceleration_field() {
        // Old settings JSON without acceleration_backend field must deserialize as Auto
        let old_json = r#"{"theme":"dark","language":"en","network_policy":"allow_all","logs_enabled":true,"watch_folders":[],"show_onboarding":false}"#;
        let s: AppSettings = serde_json::from_str(old_json).unwrap();
        assert_eq!(s.acceleration_backend, AccelerationBackend::Auto);
        assert_eq!(s.theme, Theme::Dark);
    }

    #[test]
    fn test_all_optional_fields_serialize() {
        let mut s = AppSettings::default();
        s.global_shortcut_transcribe = Some("Cmd+Shift+T".to_string());
        s.global_shortcut_dictate = Some("Cmd+Shift+D".to_string());
        s.show_onboarding = false;

        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["global_shortcut_transcribe"], "Cmd+Shift+T");
        assert_eq!(json["global_shortcut_dictate"], "Cmd+Shift+D");
        assert_eq!(json["show_onboarding"], false);
    }
}
