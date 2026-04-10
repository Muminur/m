//! Deep-link intent dispatch for WhisperDesk.
//!
//! Handles `whisperdesk://intent/<id>?params` deep-link URLs by parsing the
//! intent ID and dispatching to the appropriate handler. Used by Apple
//! Shortcuts on macOS and URL protocol handlers on all platforms.

use crate::error::{AppError, ExportErrorCode};

/// Describes an available intent action.
pub struct ShortcutIntent {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
}

/// All intents exposed by WhisperDesk.
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

/// Handle an incoming deep-link intent by ID.
///
/// Each intent emits an event on the app handle so the frontend can react,
/// then returns a JSON acknowledgment.
pub fn handle_intent(
    intent_id: &str,
    params: &serde_json::Value,
    app: &tauri::AppHandle,
) -> Result<serde_json::Value, AppError> {
    use tauri::Emitter;

    tracing::info!(intent = intent_id, "Deep-link intent received");

    match intent_id {
        "transcribe_file" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            tracing::info!(path, "Intent: transcribe_file");
            let _ = app.emit(
                "deep-link-intent",
                serde_json::json!({
                    "intent": "transcribe_file",
                    "path": path,
                }),
            );
            Ok(serde_json::json!({ "status": "dispatched", "intent": intent_id }))
        }
        "get_transcript" => {
            let id = params
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            tracing::info!(transcript_id = id, "Intent: get_transcript");
            let _ = app.emit(
                "deep-link-intent",
                serde_json::json!({
                    "intent": "get_transcript",
                    "id": id,
                }),
            );
            Ok(serde_json::json!({ "status": "dispatched", "intent": intent_id }))
        }
        "start_recording" => {
            tracing::info!("Intent: start_recording");
            let _ = app.emit(
                "deep-link-intent",
                serde_json::json!({
                    "intent": "start_recording",
                }),
            );
            Ok(serde_json::json!({ "status": "dispatched", "intent": intent_id }))
        }
        "stop_recording" => {
            tracing::info!("Intent: stop_recording");
            let _ = app.emit(
                "deep-link-intent",
                serde_json::json!({
                    "intent": "stop_recording",
                }),
            );
            Ok(serde_json::json!({ "status": "dispatched", "intent": intent_id }))
        }
        _ => Err(AppError::ExportError {
            code: ExportErrorCode::FormatError,
            message: format!("Unknown intent: {}", intent_id),
        }),
    }
}

/// Parse a deep-link URL and dispatch the intent.
///
/// URL format: `whisperdesk://intent/<intent_id>?key=value&...`
pub fn dispatch_deep_link(url_str: &str, app: &tauri::AppHandle) {
    tracing::info!(url = url_str, "Deep-link URL received");

    let url = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, url = url_str, "Failed to parse deep-link URL");
            return;
        }
    };

    // Expected: whisperdesk://intent/<intent_id>
    let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();

    if segments.is_empty() {
        tracing::warn!("Deep-link URL has no path segments");
        return;
    }

    let intent_id = if segments[0] == "intent" && segments.len() > 1 {
        segments[1]
    } else {
        segments[0]
    };

    // Collect query parameters into a JSON object
    let params: serde_json::Map<String, serde_json::Value> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
        .collect();
    let params_value = serde_json::Value::Object(params);

    match handle_intent(intent_id, &params_value, app) {
        Ok(result) => {
            tracing::info!(intent = intent_id, result = %result, "Intent dispatched");
        }
        Err(e) => {
            tracing::warn!(intent = intent_id, error = %e, "Intent dispatch failed");
        }
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
    fn test_intents_have_known_ids() {
        let ids: Vec<&str> = INTENTS.iter().map(|i| i.id).collect();
        assert!(ids.contains(&"transcribe_file"));
        assert!(ids.contains(&"get_transcript"));
        assert!(ids.contains(&"start_recording"));
        assert!(ids.contains(&"stop_recording"));
    }
}
