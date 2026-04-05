//! Live translation for captions.
//!
//! Supports two providers:
//! - **WhisperTranslate** (macOS only): uses whisper-rs translate mode to produce English output.
//! - **DeepL**: calls the DeepL HTTP API (free or pro tier) via [`NetworkGuard`].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppError, IntegrationErrorCode, NetworkErrorCode};
use crate::network::guard::NetworkGuard;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which translation backend to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationProviderKind {
    WhisperTranslate,
    DeepL,
}

/// Persistent configuration stored in settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfig {
    pub enabled: bool,
    pub provider: TranslationProviderKind,
    pub target_language: String,
    pub source_language: Option<String>,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: TranslationProviderKind::DeepL,
            target_language: "EN".to_string(),
            source_language: None,
        }
    }
}

/// A supported language entry returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupportedLanguage {
    pub code: String,
    pub name: String,
}

/// Result of a translation call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Abstraction over translation backends.
#[allow(async_fn_in_trait)]
pub trait TranslationProvider: Send + Sync {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, AppError>;

    fn supported_languages(&self) -> Vec<SupportedLanguage>;
}

// ---------------------------------------------------------------------------
// DeepL provider
// ---------------------------------------------------------------------------

/// DeepL API translation provider.
///
/// Retrieves the API key from the keychain at call time and sends requests
/// through [`NetworkGuard`] to respect the user's network policy.
pub struct DeepLProvider<'a> {
    network: &'a NetworkGuard,
    api_key: String,
    use_free_api: bool,
}

/// JSON shape returned by the DeepL `/v2/translate` endpoint.
#[derive(Debug, Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
    /// Populated by DeepL when source_lang is omitted; retained for future use.
    #[serde(default)]
    #[allow(dead_code)]
    detected_source_language: String,
}

impl<'a> DeepLProvider<'a> {
    /// Create a new provider.  `api_key` must already be resolved (e.g. from keychain).
    pub fn new(network: &'a NetworkGuard, api_key: String, use_free_api: bool) -> Self {
        Self {
            network,
            api_key,
            use_free_api,
        }
    }

    fn base_url(&self) -> &str {
        if self.use_free_api {
            "https://api-free.deepl.com"
        } else {
            "https://api.deepl.com"
        }
    }
}

impl<'a> TranslationProvider for DeepLProvider<'a> {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, AppError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let url = format!("{}/v2/translate", self.base_url());

        let mut form = vec![
            ("text", text.to_string()),
            ("target_lang", target_lang.to_uppercase()),
            ("auth_key", self.api_key.clone()),
        ];

        if !source_lang.is_empty() {
            form.push(("source_lang", source_lang.to_uppercase()));
        }

        let req = self.network.client().post(&url).form(&form);
        let response = self.network.request(req).await?;

        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::AuthenticationFailed,
                message: "DeepL API key is invalid or expired".into(),
            });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::RateLimited,
                message: "DeepL API rate limit exceeded".into(),
            });
        }
        if !status.is_success() {
            return Err(AppError::NetworkError {
                code: NetworkErrorCode::HttpError {
                    status: status.as_u16(),
                },
                message: format!("DeepL API returned HTTP {}", status.as_u16()),
            });
        }

        let body: DeepLResponse =
            response
                .json()
                .await
                .map_err(|e| AppError::IntegrationError {
                    code: IntegrationErrorCode::ApiError,
                    message: format!("Failed to parse DeepL response: {}", e),
                })?;

        body.translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .ok_or_else(|| AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: "DeepL returned empty translations array".into(),
            })
    }

    fn supported_languages(&self) -> Vec<SupportedLanguage> {
        deepl_supported_languages()
    }
}

// ---------------------------------------------------------------------------
// Whisper translate provider (macOS only)
// ---------------------------------------------------------------------------

/// Whisper-rs translate mode provider — translates audio to English during
/// transcription.  This is a no-op text translator because the translation
/// happens at the whisper inference layer, not as a post-processing step.
#[cfg(target_os = "macos")]
pub struct WhisperTranslateProvider;

#[cfg(target_os = "macos")]
impl TranslationProvider for WhisperTranslateProvider {
    async fn translate(
        &self,
        text: &str,
        _source_lang: &str,
        _target_lang: &str,
    ) -> Result<String, AppError> {
        // Whisper translate is handled at inference time; this pass-through
        // exists so the trait can be used uniformly.
        Ok(text.to_string())
    }

    fn supported_languages(&self) -> Vec<SupportedLanguage> {
        vec![SupportedLanguage {
            code: "EN".to_string(),
            name: "English".to_string(),
        }]
    }
}

// ---------------------------------------------------------------------------
// TranslationManager
// ---------------------------------------------------------------------------

/// Manages provider selection and caches the current configuration.
pub struct TranslationManager {
    config: Mutex<TranslationConfig>,
    /// Cache of language pairs already seen (for UI hints, not a translation cache).
    language_pair_cache: Mutex<HashMap<(String, String), bool>>,
}

impl TranslationManager {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(TranslationConfig::default()),
            language_pair_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> TranslationConfig {
        self.config.lock().expect("config lock poisoned").clone()
    }

    pub fn set_config(&self, config: TranslationConfig) {
        *self.config.lock().expect("config lock poisoned") = config;
    }

    /// Translate text using the currently configured provider.
    pub async fn translate(
        &self,
        text: &str,
        target_lang: &str,
        network: &NetworkGuard,
        api_key: Option<String>,
    ) -> Result<TranslationResult, AppError> {
        let cfg = self.config();

        if !cfg.enabled {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: "Translation is not enabled".into(),
            });
        }

        let source = cfg.source_language.clone().unwrap_or_default();

        let translated = match cfg.provider {
            TranslationProviderKind::DeepL => {
                let key = api_key.ok_or_else(|| AppError::IntegrationError {
                    code: IntegrationErrorCode::ConfigurationMissing,
                    message: "DeepL API key not configured".into(),
                })?;
                let use_free = key.ends_with(":fx");
                let provider = DeepLProvider::new(network, key, use_free);
                provider.translate(text, &source, target_lang).await?
            }
            #[cfg(target_os = "macos")]
            TranslationProviderKind::WhisperTranslate => {
                let provider = WhisperTranslateProvider;
                provider.translate(text, &source, target_lang).await?
            }
            #[cfg(not(target_os = "macos"))]
            TranslationProviderKind::WhisperTranslate => {
                return Err(AppError::IntegrationError {
                    code: IntegrationErrorCode::ConfigurationMissing,
                    message: "WhisperTranslate is only available on macOS".into(),
                });
            }
        };

        // Cache the language pair
        if let Ok(mut cache) = self.language_pair_cache.lock() {
            cache.insert((source.clone(), target_lang.to_string()), true);
        }

        Ok(TranslationResult {
            translated_text: translated,
            source_language: source,
            target_language: target_lang.to_string(),
        })
    }

    /// Return supported languages for the currently selected provider.
    pub fn supported_languages(&self) -> Vec<SupportedLanguage> {
        let cfg = self.config();
        match cfg.provider {
            TranslationProviderKind::DeepL => deepl_supported_languages(),
            #[cfg(target_os = "macos")]
            TranslationProviderKind::WhisperTranslate => {
                vec![SupportedLanguage {
                    code: "EN".to_string(),
                    name: "English".to_string(),
                }]
            }
            #[cfg(not(target_os = "macos"))]
            TranslationProviderKind::WhisperTranslate => vec![],
        }
    }
}

impl Default for TranslationManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DeepL language list
// ---------------------------------------------------------------------------

/// Official DeepL supported target languages.
pub fn deepl_supported_languages() -> Vec<SupportedLanguage> {
    vec![
        SupportedLanguage {
            code: "BG".into(),
            name: "Bulgarian".into(),
        },
        SupportedLanguage {
            code: "CS".into(),
            name: "Czech".into(),
        },
        SupportedLanguage {
            code: "DA".into(),
            name: "Danish".into(),
        },
        SupportedLanguage {
            code: "DE".into(),
            name: "German".into(),
        },
        SupportedLanguage {
            code: "EL".into(),
            name: "Greek".into(),
        },
        SupportedLanguage {
            code: "EN".into(),
            name: "English".into(),
        },
        SupportedLanguage {
            code: "ES".into(),
            name: "Spanish".into(),
        },
        SupportedLanguage {
            code: "ET".into(),
            name: "Estonian".into(),
        },
        SupportedLanguage {
            code: "FI".into(),
            name: "Finnish".into(),
        },
        SupportedLanguage {
            code: "FR".into(),
            name: "French".into(),
        },
        SupportedLanguage {
            code: "HU".into(),
            name: "Hungarian".into(),
        },
        SupportedLanguage {
            code: "ID".into(),
            name: "Indonesian".into(),
        },
        SupportedLanguage {
            code: "IT".into(),
            name: "Italian".into(),
        },
        SupportedLanguage {
            code: "JA".into(),
            name: "Japanese".into(),
        },
        SupportedLanguage {
            code: "KO".into(),
            name: "Korean".into(),
        },
        SupportedLanguage {
            code: "LT".into(),
            name: "Lithuanian".into(),
        },
        SupportedLanguage {
            code: "LV".into(),
            name: "Latvian".into(),
        },
        SupportedLanguage {
            code: "NB".into(),
            name: "Norwegian".into(),
        },
        SupportedLanguage {
            code: "NL".into(),
            name: "Dutch".into(),
        },
        SupportedLanguage {
            code: "PL".into(),
            name: "Polish".into(),
        },
        SupportedLanguage {
            code: "PT".into(),
            name: "Portuguese".into(),
        },
        SupportedLanguage {
            code: "RO".into(),
            name: "Romanian".into(),
        },
        SupportedLanguage {
            code: "RU".into(),
            name: "Russian".into(),
        },
        SupportedLanguage {
            code: "SK".into(),
            name: "Slovak".into(),
        },
        SupportedLanguage {
            code: "SL".into(),
            name: "Slovenian".into(),
        },
        SupportedLanguage {
            code: "SV".into(),
            name: "Swedish".into(),
        },
        SupportedLanguage {
            code: "TR".into(),
            name: "Turkish".into(),
        },
        SupportedLanguage {
            code: "UK".into(),
            name: "Ukrainian".into(),
        },
        SupportedLanguage {
            code: "ZH".into(),
            name: "Chinese".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_translation_config_default() {
        let cfg = TranslationConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.provider, TranslationProviderKind::DeepL);
        assert_eq!(cfg.target_language, "EN");
        assert!(cfg.source_language.is_none());
    }

    #[test]
    fn test_translation_config_serialization() {
        let cfg = TranslationConfig {
            enabled: true,
            provider: TranslationProviderKind::DeepL,
            target_language: "DE".into(),
            source_language: Some("EN".into()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: TranslationConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.provider, TranslationProviderKind::DeepL);
        assert_eq!(parsed.target_language, "DE");
        assert_eq!(parsed.source_language.unwrap(), "EN");
    }

    #[test]
    fn test_provider_kind_serialization() {
        let kind = TranslationProviderKind::WhisperTranslate;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, r#""whisper_translate""#);

        let kind2 = TranslationProviderKind::DeepL;
        let json2 = serde_json::to_string(&kind2).unwrap();
        assert_eq!(json2, r#""deep_l""#);
    }

    #[test]
    fn test_deepl_supported_languages_non_empty() {
        let langs = deepl_supported_languages();
        assert!(!langs.is_empty());
        // Must contain common languages
        let codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
        assert!(codes.contains(&"EN"));
        assert!(codes.contains(&"DE"));
        assert!(codes.contains(&"FR"));
        assert!(codes.contains(&"ES"));
        assert!(codes.contains(&"JA"));
        assert!(codes.contains(&"ZH"));
        assert!(codes.contains(&"KO"));
    }

    #[test]
    fn test_translation_manager_config_roundtrip() {
        let manager = TranslationManager::new();
        let cfg = TranslationConfig {
            enabled: true,
            provider: TranslationProviderKind::DeepL,
            target_language: "FR".into(),
            source_language: Some("EN".into()),
        };
        manager.set_config(cfg.clone());
        let loaded = manager.config();
        assert!(loaded.enabled);
        assert_eq!(loaded.target_language, "FR");
    }

    #[test]
    fn test_translation_manager_supported_languages_deepl() {
        let manager = TranslationManager::new();
        let mut cfg = TranslationConfig::default();
        cfg.provider = TranslationProviderKind::DeepL;
        manager.set_config(cfg);
        let langs = manager.supported_languages();
        assert!(!langs.is_empty());
    }

    #[tokio::test]
    async fn test_translate_disabled_returns_error() {
        let manager = TranslationManager::new();
        // Default config has enabled=false
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let result = manager.translate("hello", "EN", &guard, None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                ..
            } => {}
            other => panic!("Expected ConfigurationMissing, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_translate_deepl_missing_key_returns_error() {
        let manager = TranslationManager::new();
        manager.set_config(TranslationConfig {
            enabled: true,
            provider: TranslationProviderKind::DeepL,
            target_language: "DE".into(),
            source_language: None,
        });
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let result = manager.translate("hello", "DE", &guard, None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                ..
            } => {}
            other => panic!("Expected ConfigurationMissing, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_translate_offline_policy_blocks() {
        let manager = TranslationManager::new();
        manager.set_config(TranslationConfig {
            enabled: true,
            provider: TranslationProviderKind::DeepL,
            target_language: "DE".into(),
            source_language: None,
        });
        let guard = NetworkGuard::new(NetworkPolicy::Offline).unwrap();
        let result = manager
            .translate("hello", "DE", &guard, Some("fake-key:fx".into()))
            .await;
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
    fn test_deepl_provider_base_url_free() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let provider = DeepLProvider::new(&guard, "key:fx".into(), true);
        assert_eq!(provider.base_url(), "https://api-free.deepl.com");
    }

    #[test]
    fn test_deepl_provider_base_url_pro() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let provider = DeepLProvider::new(&guard, "key".into(), false);
        assert_eq!(provider.base_url(), "https://api.deepl.com");
    }

    #[tokio::test]
    async fn test_deepl_empty_text_returns_empty() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let provider = DeepLProvider::new(&guard, "fake-key".into(), true);
        let result = provider.translate("", "EN", "DE").await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_deepl_whitespace_only_returns_empty() {
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let provider = DeepLProvider::new(&guard, "fake-key".into(), true);
        let result = provider.translate("   ", "EN", "DE").await.unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_translation_result_serialization() {
        let result = TranslationResult {
            translated_text: "Hallo".into(),
            source_language: "EN".into(),
            target_language: "DE".into(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["translatedText"], "Hallo");
        assert_eq!(json["sourceLanguage"], "EN");
        assert_eq!(json["targetLanguage"], "DE");
    }

    #[test]
    fn test_supported_language_serialization() {
        let lang = SupportedLanguage {
            code: "DE".into(),
            name: "German".into(),
        };
        let json = serde_json::to_value(&lang).unwrap();
        assert_eq!(json["code"], "DE");
        assert_eq!(json["name"], "German");
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn test_whisper_translate_unavailable_on_non_macos() {
        let manager = TranslationManager::new();
        manager.set_config(TranslationConfig {
            enabled: true,
            provider: TranslationProviderKind::WhisperTranslate,
            target_language: "EN".into(),
            source_language: None,
        });
        let guard = NetworkGuard::new(NetworkPolicy::AllowAll).unwrap();
        let result = manager.translate("hello", "EN", &guard, None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_deepl_free_key_detection() {
        // Keys ending with :fx are free-tier
        assert!("some-key:fx".ends_with(":fx"));
        assert!(!"some-key".ends_with(":fx"));
    }
}
