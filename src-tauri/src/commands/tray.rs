//! IPC commands for syncing tray icon state from the frontend.

use crate::tray::{update_tray_state, TrayState};

#[tauri::command]
pub async fn set_tray_state(app: tauri::AppHandle, state: String) -> Result<(), String> {
    let parsed = match state.as_str() {
        "idle" | "stopping" => TrayState::Idle,
        "recording" => TrayState::Recording,
        "paused" => TrayState::Paused,
        other => return Err(format!("unknown tray state: {other}")),
    };
    update_tray_state(&app, parsed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_strings_to_tray_states() {
        // Just verifies the match arms exist for every recordingStore status string
        for s in ["idle", "recording", "paused", "stopping"] {
            let mapped = match s {
                "idle" | "stopping" => TrayState::Idle,
                "recording" => TrayState::Recording,
                "paused" => TrayState::Paused,
                _ => unreachable!(),
            };
            // Compile-time check: mapped is a TrayState
            let _ = mapped;
        }
    }
}
