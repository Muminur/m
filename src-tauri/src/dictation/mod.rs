pub mod accessibility;
pub mod ai_correct;
pub mod history;
pub mod postprocess;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

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
    pub fn state(&self) -> DictationState {
        *self.state.lock().expect("dictation state lock poisoned")
    }

    /// Transition to a new state. Returns the previous state.
    pub fn transition(&self, new: DictationState) -> DictationState {
        let mut guard = self.state.lock().expect("dictation state lock poisoned");
        let old = *guard;
        *guard = new;
        tracing::info!(from = ?old, to = ?new, "dictation state transition");
        old
    }

    /// Detect double-tap: returns true if two triggers arrive within `threshold_ms`.
    /// Call this each time the hotkey is pressed.
    pub fn detect_double_tap(&self, now_ms: u64, threshold_ms: u64) -> bool {
        let mut guard = self
            .last_trigger_ms
            .lock()
            .expect("last_trigger lock poisoned");
        if let Some(prev) = *guard {
            let delta = now_ms.saturating_sub(prev);
            if delta <= threshold_ms {
                *guard = None; // reset so the next press starts fresh
                return true;
            }
        }
        *guard = Some(now_ms);
        false
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
        assert_eq!(mgr.state(), DictationState::Idle);
    }

    #[test]
    fn test_transition_returns_previous_state() {
        let mgr = DictationManager::new();
        let old = mgr.transition(DictationState::Listening);
        assert_eq!(old, DictationState::Idle);
        assert_eq!(mgr.state(), DictationState::Listening);
    }

    #[test]
    fn test_full_state_machine_cycle() {
        let mgr = DictationManager::new();
        mgr.transition(DictationState::Listening);
        mgr.transition(DictationState::Processing);
        mgr.transition(DictationState::Inserting);
        mgr.transition(DictationState::Idle);
        assert_eq!(mgr.state(), DictationState::Idle);
    }

    #[test]
    fn test_double_tap_within_threshold() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400));
        assert!(mgr.detect_double_tap(1300, 400));
    }

    #[test]
    fn test_double_tap_outside_threshold() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400));
        assert!(!mgr.detect_double_tap(1500, 400));
    }

    #[test]
    fn test_double_tap_resets_after_detection() {
        let mgr = DictationManager::new();
        assert!(!mgr.detect_double_tap(1000, 400));
        assert!(mgr.detect_double_tap(1200, 400));
        // After detection the state resets, so the next single tap should not trigger
        assert!(!mgr.detect_double_tap(1600, 400));
    }

    #[test]
    fn test_dictation_state_serializes() {
        let json = serde_json::to_string(&DictationState::Listening).unwrap();
        assert_eq!(json, "\"listening\"");
        let json = serde_json::to_string(&DictationState::Idle).unwrap();
        assert_eq!(json, "\"idle\"");
    }
}
