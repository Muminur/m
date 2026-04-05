//! Diarization subsystem: speaker turn detection and labelling.
//!
//! Three providers are available:
//! - [`tinydiarize::TinydiarizeProvider`] — local, zero-network, token-based
//! - [`elevenlabs::ElevenLabsProvider`] — cloud, requires ElevenLabs API key
//! - [`deepgram::DeepgramProvider`] — cloud, requires Deepgram API key
//!
//! All providers implement [`DiarizationProvider`].

pub mod deepgram;
pub mod elevenlabs;
pub mod tinydiarize;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ─── Core types ───────────────────────────────────────────────────────────────

/// A single transcript segment passed into a diarization provider.
///
/// Mirrors [`crate::database::segments::SegmentRow`] but is kept independent
/// so the diarization module does not take a hard DB dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub transcript_id: String,
    pub index_num: i64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub confidence: Option<f32>,
}

/// A segment annotated with speaker information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiarizedSegment {
    /// Original segment text (may be cleaned of provider-specific tokens).
    pub text: String,
    /// Segment start in milliseconds.
    pub start_ms: u64,
    /// Segment end in milliseconds.
    pub end_ms: u64,
    /// Stable internal speaker identifier (e.g. "speaker_0").
    pub speaker_id: String,
    /// Human-readable label shown in the UI (e.g. "Speaker 1").
    pub speaker_label: String,
    /// Confidence score in [0.0, 1.0].  Providers that do not supply a score
    /// should set this to `1.0`.
    pub confidence: f32,
}

// ─── Trait ───────────────────────────────────────────────────────────────────

/// Common interface for all diarization back-ends.
pub trait DiarizationProvider: Send + Sync {
    /// Assign speaker turns to a slice of transcript segments.
    fn diarize(
        &self,
        segments: &[TranscriptSegment],
    ) -> Result<Vec<DiarizedSegment>, AppError>;

    /// Short identifier used in log messages and API responses.
    fn name(&self) -> &str;
}

// ─── Provider availability ────────────────────────────────────────────────────

/// Metadata about a provider returned to the front-end.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub requires_network: bool,
}

/// Build the list of providers that can be used right now.
///
/// `tinydiarize` is always available.  Cloud providers require an API key in
/// the keychain.
pub fn available_providers() -> Vec<ProviderInfo> {
    let mut providers = vec![ProviderInfo {
        id: "tinydiarize".into(),
        label: "Tinydiarize (local)".into(),
        available: true,
        requires_network: false,
    }];

    // ElevenLabs — available when a key is stored (macOS keychain); on other
    // platforms the keychain always returns an error so the provider is hidden.
    let elevenlabs_available = crate::keychain::get("elevenlabs", "api_key")
        .ok()
        .flatten()
        .is_some();
    providers.push(ProviderInfo {
        id: "elevenlabs".into(),
        label: "ElevenLabs Scribe (cloud)".into(),
        available: elevenlabs_available,
        requires_network: true,
    });

    // Deepgram — same pattern
    let deepgram_available = crate::keychain::get("deepgram", "api_key")
        .ok()
        .flatten()
        .is_some();
    providers.push(ProviderInfo {
        id: "deepgram".into(),
        label: "Deepgram Nova (cloud)".into(),
        available: deepgram_available,
        requires_network: true,
    });

    providers
}
