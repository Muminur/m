pub mod deepgram;
pub mod elevenlabs;
pub mod groq_whisper;
pub mod openai_whisper;
pub mod vibevoice;

use crate::error::AppError;
use serde::Serialize;
use std::path::Path;
use std::pin::Pin;

/// Result from a cloud transcription provider.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudTranscriptResult {
    pub text: String,
    pub segments: Vec<CloudSegment>,
    pub language: String,
}

/// A single segment from a cloud transcription.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Option<String>,
}

/// Information about a cloud transcription provider.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProviderInfo {
    pub name: String,
    pub display_name: String,
    pub cost_per_minute_usd: f64,
    pub requires_api_key: bool,
}

/// Cost estimate for cloud transcription.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCostEstimate {
    pub provider: String,
    pub duration_minutes: f64,
    pub estimated_usd: f64,
}

/// Trait for cloud transcription providers.
pub trait CloudTranscriptionProvider: Send + Sync {
    /// Provider name identifier.
    fn name(&self) -> &str;

    /// Cost per minute of audio in USD.
    fn cost_per_minute_usd(&self) -> f64;

    /// Transcribe an audio file.
    fn transcribe(
        &self,
        audio_path: &Path,
        language: Option<&str>,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<CloudTranscriptResult, AppError>> + Send + '_>,
    >;
}

/// List all available cloud transcription providers.
pub fn list_providers() -> Vec<CloudProviderInfo> {
    vec![
        CloudProviderInfo {
            name: "openai_whisper".into(),
            display_name: "OpenAI Whisper".into(),
            cost_per_minute_usd: 0.006,
            requires_api_key: true,
        },
        CloudProviderInfo {
            name: "deepgram".into(),
            display_name: "Deepgram Nova-2".into(),
            cost_per_minute_usd: 0.0043,
            requires_api_key: true,
        },
        CloudProviderInfo {
            name: "groq_whisper".into(),
            display_name: "Groq Whisper".into(),
            cost_per_minute_usd: 0.0011,
            requires_api_key: true,
        },
        CloudProviderInfo {
            name: "elevenlabs".into(),
            display_name: "ElevenLabs".into(),
            cost_per_minute_usd: 0.01,
            requires_api_key: true,
        },
        CloudProviderInfo {
            name: "vibevoice".into(),
            display_name: "VibeVoice".into(),
            cost_per_minute_usd: 0.0,
            requires_api_key: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_providers_not_empty() {
        let providers = list_providers();
        assert!(!providers.is_empty());
    }

    #[test]
    fn test_cloud_provider_info_serializes() {
        let info = CloudProviderInfo {
            name: "test".into(),
            display_name: "Test Provider".into(),
            cost_per_minute_usd: 0.01,
            requires_api_key: true,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["displayName"], "Test Provider");
        assert_eq!(json["costPerMinuteUsd"], 0.01);
        assert_eq!(json["requiresApiKey"], true);
    }

    #[test]
    fn test_cloud_segment_serializes() {
        let seg = CloudSegment {
            start_ms: 0,
            end_ms: 1000,
            text: "Hello".into(),
            speaker: Some("Speaker 1".into()),
        };
        let json = serde_json::to_value(&seg).unwrap();
        assert_eq!(json["startMs"], 0);
        assert_eq!(json["endMs"], 1000);
    }
}
