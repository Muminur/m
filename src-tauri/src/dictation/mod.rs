pub mod accessibility;
pub mod ai_correct;
pub mod history;
pub mod postprocess;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::error::{AppError, DictationErrorCode};

/// Current state of the dictation pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictationState {
    #[default]
    Idle,
    Listening,
    Processing,
    Inserting,
}

/// Manages dictation lifecycle and double-tap detection.
pub struct DictationManager {
    state: Mutex<DictationState>,
    last_trigger_ms: Mutex<Option<u64>>,
}

impl DictationManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DictationState::Idle),
            last_trigger_ms: Mutex::new(None),
        }
    }

    /// Get current dictation state.
    pub fn state(&self) -> Result<DictationState, AppError> {
        Ok(*self.state.lock().map_err(|_| AppError::DictationError {
            code: DictationErrorCode::InvalidState,
            message: "mutex poisoned in DictationManager::state".into(),
        })?)
    }

    /// Transition to a new state. Returns the previous state.
    pub fn transition(&self, new: DictationState) -> Result<DictationState, AppError> {
        let mut guard = self.state.lock().map_err(|_| AppError::DictationError {
            code: DictationErrorCode::InvalidState,
            message: "mutex poisoned in DictationManager::transition".into(),
        })?;
        let old = *guard;
        *guard = new;
        tracing::info!(from = ?old, to = ?new, "dictation state transition");
        Ok(old)
    }

    /// Detect double-tap: returns true if two triggers arrive within `threshold_ms`.
    /// Call this each time the hotkey is pressed.
    pub fn detect_double_tap(&self, now_ms: u64, threshold_ms: u64) -> Result<bool, AppError> {
        let mut guard =
            self.last_trigger_ms
                .lock()
                .map_err(|_| AppError::DictationError {
                    code: DictationErrorCode::InvalidState,
                    message: "mutex poisoned in DictationManager::detect_double_tap".into(),
                })?;
        if let Some(prev) = *guard {
            let delta = now_ms.saturating_sub(prev);
            if delta <= threshold_ms {
                *guard = None; // reset so the next press starts fresh
                return Ok(true);
            }
        }
        *guard = Some(now_ms);
        Ok(false)
    }
}

impl Default for DictationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_idle() {
        let mgr = DictationManager::new();
        assert_eq!(mgr.state().unwrap(), DictationState::Idle);
    }

    #[test]
    fn test_transition_returns_previous_state() {
        let mgr = DictationManager::new();
        let old = mgr.transition(DictationState::Listening).unwrap();
        assert_eq!(old, DictationState::Idle);
        assert_eq!(mgr.state().unwrap(), DictationState::Listening);
    }

    #[test]
    fn test_full_state_machine_cycle() {
        let mgr = DictationManager::new();
        mgr.transition(DictationState::Listening).unwrap();
        mgr.transition(DictationState::Processing).unwrap();
        mgr.transition(DictationState::Inserting).unwrap();
        mgr.transition(DictationState::Idle).unwrap();
        assert_eq!(mgr.state().unwrap(), DictationState::Idle);
    }

    #[test]
    fn test_double_tap_within_threshold() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400).unwrap());
        assert!(mgr.detect_double_tap(1300, 400).unwrap());
    }

    #[test]
    fn test_double_tap_outside_threshold() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400).unwrap());
        assert!(!mgr.detect_double_tap(1500, 400).unwrap());
    }

    #[test]
    fn test_double_tap_resets_after_detection() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400).unwrap());
        assert!(mgr.detect_double_tap(1200, 400).unwrap());
        // After detection the state resets, so the next single tap should not trigger
        assert!(!mgr.detect_double_tap(1600, 400).unwrap());
    }

    #[test]
    fn test_dictation_state_serializes() {
        let json = serde_json::to_string(&DictationState::Listening).unwrap();
        assert_eq!(json, "\"listening\"");
        let json = serde_json::to_string(&DictationState::Idle).unwrap();
        assert_eq!(json, "\"idle\"");
    }
}
