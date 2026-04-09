//! Apple Shortcuts intents for WhisperDesk.
//!
//! Exposes Shortcuts actions: "Transcribe file at path",
//! "Get transcript", "Start/stop recording".
//!
//! Full implementation requires macOS Shortcuts API access.
//! This stub provides the interface; full wiring in M10.

use crate::error::AppError;

/// Describes an available Shortcuts intent.
pub struct ShortcutIntent {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
}

/// All intents exposed by WhisperDesk to Apple Shortcuts.
pub const INTENTS: &[ShortcutIntent] = &[
    ShortcutIntent {
        id: "transcribe_file",
        display_name: "Transcribe File",
        description: "Transcribe an audio or video file at a given path",
    },
    ShortcutIntent {
        id: "get_transcript",
        display_name: "Get Transcript",
        description: "Retrieve the transcript text for a given transcript ID",
    },
    ShortcutIntent {
        id: "start_recording",
        display_name: "Start Recording",
        description: "Start microphone or system audio recording",
    },
    ShortcutIntent {
        id: "stop_recording",
        display_name: "Stop Recording",
        description: "Stop the current recording session",
    },
];

/// Handle an incoming Shortcuts intent by ID.
///
/// Full URL scheme / XPC wiring is implemented in M10.
pub fn handle_intent(intent_id: &str, _params: &serde_json::Value) -> Result<serde_json::Value, AppError> {
    tracing::info!("Apple Shortcuts intent received: {}", intent_id);
    match intent_id {
        "transcribe_file" | "get_transcript" | "start_recording" | "stop_recording" => {
            // TODO(M10): implement full Shortcuts URL scheme handler
            Ok(serde_json::json!({ "status": "stub", "intent": intent_id }))
        }
        _ => Err(AppError::ExportError {
            code: crate::error::ExportErrorCode::FormatError,
            message: format!("Unknown Shortcuts intent: {}", intent_id),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intents_not_empty() {
        assert!(!INTENTS.is_empty());
    }

    #[test]
    fn test_handle_known_intent() {
        let result = handle_intent("transcribe_file", &serde_json::json!({}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_unknown_intent() {
        let result = handle_intent("unknown_action", &serde_json::json!({}));
        assert!(result.is_err());
    }
}
