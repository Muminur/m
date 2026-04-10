use crate::cloud_transcription::{CloudTranscriptResult, CloudTranscriptionProvider};
use crate::error::{AppError, CloudTranscriptionErrorCode};
use crate::network::guard::NetworkGuard;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

/// VibeVoice cloud transcription provider (stub).
///
/// The VibeVoice API is not yet documented. This provider is a placeholder
/// that returns a clear error when invoked.
pub struct VibeVoiceProvider {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    guard: Arc<NetworkGuard>,
}

impl VibeVoiceProvider {
    /// Create a new VibeVoice provider (stub).
    pub fn new(api_key: String, guard: Arc<NetworkGuard>) -> Self {
        Self { api_key, guard }
    }
}

impl CloudTranscriptionProvider for VibeVoiceProvider {
    fn name(&self) -> &str {
        "vibevoice"
    }

    fn cost_per_minute_usd(&self) -> f64 {
        0.0
    }

    fn transcribe(
        &self,
        _audio_path: &Path,
        _language: Option<&str>,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<CloudTranscriptResult, AppError>> + Send + '_>,
    > {
        Box::pin(async {
            // VibeVoice API not yet documented
            Err(AppError::CloudTranscriptionError {
                code: CloudTranscriptionErrorCode::ProviderNotFound,
                message: "VibeVoice API is not yet available".into(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::NetworkPolicy;

    #[test]
    fn test_vibevoice_provider_name() {
        let guard = Arc::new(NetworkGuard::new(NetworkPolicy::AllowAll).unwrap());
        let provider = VibeVoiceProvider::new("test-key".into(), guard);
        assert_eq!(provider.name(), "vibevoice");
        assert_eq!(provider.cost_per_minute_usd(), 0.0);
    }
}
