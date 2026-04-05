//! Global keyboard shortcuts with collision detection.
//!
//! Uses `tauri-plugin-global-shortcut` for OS-level registration and persists
//! custom bindings to [`AppSettings`].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppError, IntegrationErrorCode};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single shortcut binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    /// Stable identifier, e.g. `"dictation_toggle"`.
    pub id: String,
    /// Accelerator string, e.g. `"CommandOrControl+Shift+D"`.
    pub accelerator: String,
    /// Human-readable description shown in settings UI.
    pub description: String,
    /// Whether this shortcut is currently registered with the OS.
    pub is_active: bool,
}

/// Conflict information returned by collision detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConflict {
    pub conflicting_id: String,
    pub conflicting_accelerator: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Default shortcuts
// ---------------------------------------------------------------------------

/// Returns the built-in default shortcut bindings.
pub fn default_shortcuts() -> Vec<ShortcutBinding> {
    vec![
        ShortcutBinding {
            id: "dictation_toggle".into(),
            accelerator: "CommandOrControl+Shift+D".into(),
            description: "Start / stop dictation".into(),
            is_active: false,
        },
        ShortcutBinding {
            id: "spotlight_bar".into(),
            accelerator: "CommandOrControl+Shift+Space".into(),
            description: "Open spotlight bar".into(),
            is_active: false,
        },
        ShortcutBinding {
            id: "captions_toggle".into(),
            accelerator: "CommandOrControl+Shift+C".into(),
            description: "Toggle captions overlay".into(),
            is_active: false,
        },
    ]
}

// ---------------------------------------------------------------------------
// ShortcutManager
// ---------------------------------------------------------------------------

/// Manages global shortcut registration, conflict detection, and persistence.
///
/// The actual OS-level registration is performed via Tauri's global-shortcut
/// plugin in the command layer.  This struct owns the logical state so that
/// conflict detection and listing work without an `AppHandle`.
pub struct ShortcutManager {
    bindings: Mutex<HashMap<String, ShortcutBinding>>,
}

impl ShortcutManager {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for binding in default_shortcuts() {
            map.insert(binding.id.clone(), binding);
        }
        Self {
            bindings: Mutex::new(map),
        }
    }

    /// Validate an accelerator string format.
    /// Must contain at least one modifier and a key.
    pub fn validate_accelerator(accelerator: &str) -> Result<(), AppError> {
        if accelerator.trim().is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: "Accelerator string is empty".into(),
            });
        }

        let parts: Vec<&str> = accelerator.split('+').collect();
        if parts.len() < 2 {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: format!(
                    "Accelerator '{}' must have at least one modifier and a key",
                    accelerator
                ),
            });
        }

        let valid_modifiers = [
            "CommandOrControl",
            "CmdOrCtrl",
            "Command",
            "Cmd",
            "Control",
            "Ctrl",
            "Shift",
            "Alt",
            "Option",
            "Super",
        ];

        // All parts except the last must be valid modifiers
        for part in &parts[..parts.len() - 1] {
            let trimmed = part.trim();
            if !valid_modifiers
                .iter()
                .any(|m| m.eq_ignore_ascii_case(trimmed))
            {
                return Err(AppError::IntegrationError {
                    code: IntegrationErrorCode::ConfigurationMissing,
                    message: format!("Unknown modifier '{}' in accelerator", trimmed),
                });
            }
        }

        // Last part must be a non-empty key
        let key = parts.last().unwrap().trim();
        if key.is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: "Accelerator ends with '+' but has no key".into(),
            });
        }

        Ok(())
    }

    /// Register a shortcut binding (logical state only).
    /// The caller is responsible for OS-level registration via the Tauri plugin.
    pub fn register(&self, id: &str, accelerator: &str, description: &str) -> Result<(), AppError> {
        Self::validate_accelerator(accelerator)?;

        let conflicts = self.detect_conflicts_internal(accelerator, Some(id));
        if !conflicts.is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: format!(
                    "Accelerator '{}' conflicts with '{}'",
                    accelerator, conflicts[0].conflicting_id
                ),
            });
        }

        let binding = ShortcutBinding {
            id: id.to_string(),
            accelerator: accelerator.to_string(),
            description: description.to_string(),
            is_active: true,
        };

        self.bindings
            .lock()
            .expect("bindings lock poisoned")
            .insert(id.to_string(), binding);

        tracing::info!(id, accelerator, "Shortcut registered");
        Ok(())
    }

    /// Unregister a shortcut (logical state only).
    pub fn unregister(&self, id: &str) -> Result<(), AppError> {
        let mut bindings = self.bindings.lock().expect("bindings lock poisoned");
        match bindings.get_mut(id) {
            Some(binding) => {
                binding.is_active = false;
                tracing::info!(id, "Shortcut unregistered");
                Ok(())
            }
            None => Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: format!("No shortcut with id '{}'", id),
            }),
        }
    }

    /// List all registered shortcut bindings.
    pub fn list_registered(&self) -> Vec<ShortcutBinding> {
        let bindings = self.bindings.lock().expect("bindings lock poisoned");
        let mut list: Vec<ShortcutBinding> = bindings.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    /// Detect conflicts with an accelerator string against all registered shortcuts.
    pub fn detect_conflicts(&self, accelerator: &str) -> Vec<ShortcutConflict> {
        self.detect_conflicts_internal(accelerator, None)
    }

    /// Internal conflict detection; `exclude_id` allows skipping self when updating.
    fn detect_conflicts_internal(
        &self,
        accelerator: &str,
        exclude_id: Option<&str>,
    ) -> Vec<ShortcutConflict> {
        let normalized = normalize_accelerator(accelerator);
        let bindings = self.bindings.lock().expect("bindings lock poisoned");
        let mut conflicts = Vec::new();

        for binding in bindings.values() {
            if let Some(exclude) = exclude_id {
                if binding.id == exclude {
                    continue;
                }
            }
            if binding.is_active && normalize_accelerator(&binding.accelerator) == normalized {
                conflicts.push(ShortcutConflict {
                    conflicting_id: binding.id.clone(),
                    conflicting_accelerator: binding.accelerator.clone(),
                    description: binding.description.clone(),
                });
            }
        }

        conflicts
    }

    /// Update an existing binding's accelerator.
    pub fn update_binding(&self, id: &str, new_accelerator: &str) -> Result<(), AppError> {
        Self::validate_accelerator(new_accelerator)?;

        let conflicts = self.detect_conflicts_internal(new_accelerator, Some(id));
        if !conflicts.is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: format!(
                    "Accelerator '{}' conflicts with '{}'",
                    new_accelerator, conflicts[0].conflicting_id
                ),
            });
        }

        let mut bindings = self.bindings.lock().expect("bindings lock poisoned");
        match bindings.get_mut(id) {
            Some(binding) => {
                let old = binding.accelerator.clone();
                binding.accelerator = new_accelerator.to_string();
                tracing::info!(id, old_accel = %old, new_accel = %new_accelerator, "Shortcut updated");
                Ok(())
            }
            None => Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ConfigurationMissing,
                message: format!("No shortcut with id '{}'", id),
            }),
        }
    }

    /// Get a single binding by id.
    pub fn get(&self, id: &str) -> Option<ShortcutBinding> {
        self.bindings
            .lock()
            .expect("bindings lock poisoned")
            .get(id)
            .cloned()
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalize an accelerator for comparison: lowercase, sorted modifiers.
fn normalize_accelerator(accel: &str) -> String {
    let parts: Vec<&str> = accel.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return String::new();
    }

    let key = parts.last().unwrap().to_lowercase();
    let mut modifiers: Vec<String> = parts[..parts.len() - 1]
        .iter()
        .map(|m| {
            let lower = m.to_lowercase();
            // Normalize aliases
            match lower.as_str() {
                "commandorcontrol" | "cmdorctrl" | "command" | "cmd" | "control" | "ctrl" => {
                    "cmdorctrl".to_string()
                }
                "option" => "alt".to_string(),
                other => other.to_string(),
            }
        })
        .collect();
    modifiers.sort();
    modifiers.dedup();

    format!("{}+{}", modifiers.join("+"), key)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shortcuts_count() {
        let defaults = default_shortcuts();
        assert_eq!(defaults.len(), 3);
    }

    #[test]
    fn test_default_shortcut_ids() {
        let defaults = default_shortcuts();
        let ids: Vec<&str> = defaults.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"dictation_toggle"));
        assert!(ids.contains(&"spotlight_bar"));
        assert!(ids.contains(&"captions_toggle"));
    }

    #[test]
    fn test_default_shortcuts_inactive_initially() {
        for s in default_shortcuts() {
            assert!(
                !s.is_active,
                "Default shortcut '{}' should start inactive",
                s.id
            );
        }
    }

    #[test]
    fn test_shortcut_binding_serialization() {
        let binding = ShortcutBinding {
            id: "test".into(),
            accelerator: "Ctrl+Shift+T".into(),
            description: "Test shortcut".into(),
            is_active: true,
        };
        let json = serde_json::to_value(&binding).unwrap();
        assert_eq!(json["id"], "test");
        assert_eq!(json["accelerator"], "Ctrl+Shift+T");
        assert_eq!(json["isActive"], true);
    }

    #[test]
    fn test_validate_accelerator_valid() {
        assert!(ShortcutManager::validate_accelerator("CommandOrControl+Shift+D").is_ok());
        assert!(ShortcutManager::validate_accelerator("Ctrl+A").is_ok());
        assert!(ShortcutManager::validate_accelerator("Alt+Shift+Space").is_ok());
    }

    #[test]
    fn test_validate_accelerator_empty() {
        assert!(ShortcutManager::validate_accelerator("").is_err());
    }

    #[test]
    fn test_validate_accelerator_no_modifier() {
        assert!(ShortcutManager::validate_accelerator("A").is_err());
    }

    #[test]
    fn test_validate_accelerator_unknown_modifier() {
        assert!(ShortcutManager::validate_accelerator("Foo+A").is_err());
    }

    #[test]
    fn test_validate_accelerator_trailing_plus() {
        assert!(ShortcutManager::validate_accelerator("Ctrl+").is_err());
    }

    #[test]
    fn test_manager_register_and_list() {
        let mgr = ShortcutManager::new();
        mgr.register("test_action", "Alt+Shift+T", "Test action")
            .unwrap();

        let list = mgr.list_registered();
        let found = list.iter().find(|b| b.id == "test_action");
        assert!(found.is_some());
        let binding = found.unwrap();
        assert!(binding.is_active);
        assert_eq!(binding.accelerator, "Alt+Shift+T");
    }

    #[test]
    fn test_manager_unregister() {
        let mgr = ShortcutManager::new();
        mgr.register("test_unreg", "Alt+Shift+U", "Test unregister")
            .unwrap();
        assert!(mgr.get("test_unreg").unwrap().is_active);

        mgr.unregister("test_unreg").unwrap();
        assert!(!mgr.get("test_unreg").unwrap().is_active);
    }

    #[test]
    fn test_unregister_unknown_id_errors() {
        let mgr = ShortcutManager::new();
        assert!(mgr.unregister("nonexistent").is_err());
    }

    #[test]
    fn test_conflict_detection_same_accelerator() {
        let mgr = ShortcutManager::new();
        mgr.register("action_a", "Alt+Shift+X", "Action A").unwrap();

        let conflicts = mgr.detect_conflicts("Alt+Shift+X");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflicting_id, "action_a");
    }

    #[test]
    fn test_conflict_detection_normalized_aliases() {
        let mgr = ShortcutManager::new();
        mgr.register("action_b", "CommandOrControl+Shift+K", "Action B")
            .unwrap();

        // CmdOrCtrl is an alias for CommandOrControl
        let conflicts = mgr.detect_conflicts("CmdOrCtrl+Shift+K");
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_no_conflict_different_keys() {
        let mgr = ShortcutManager::new();
        mgr.register("action_c", "Alt+Shift+A", "Action C").unwrap();

        let conflicts = mgr.detect_conflicts("Alt+Shift+B");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_detection_skips_inactive() {
        let mgr = ShortcutManager::new();
        mgr.register("action_d", "Alt+Shift+Z", "Action D").unwrap();
        mgr.unregister("action_d").unwrap();

        let conflicts = mgr.detect_conflicts("Alt+Shift+Z");
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_update_binding() {
        let mgr = ShortcutManager::new();
        mgr.register("action_e", "Alt+Shift+E", "Action E").unwrap();

        mgr.update_binding("action_e", "Alt+Shift+F").unwrap();
        let binding = mgr.get("action_e").unwrap();
        assert_eq!(binding.accelerator, "Alt+Shift+F");
    }

    #[test]
    fn test_update_binding_conflict_blocked() {
        let mgr = ShortcutManager::new();
        mgr.register("action_f", "Alt+Shift+G", "Action F").unwrap();
        mgr.register("action_g", "Alt+Shift+H", "Action G").unwrap();

        let result = mgr.update_binding("action_f", "Alt+Shift+H");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_binding_same_accelerator_ok() {
        let mgr = ShortcutManager::new();
        mgr.register("action_h", "Alt+Shift+I", "Action H").unwrap();

        // Updating to same accelerator should succeed (excluded from conflict check)
        mgr.update_binding("action_h", "Alt+Shift+I").unwrap();
    }

    #[test]
    fn test_register_conflict_blocked() {
        let mgr = ShortcutManager::new();
        mgr.register("first", "Alt+Shift+J", "First").unwrap();

        let result = mgr.register("second", "Alt+Shift+J", "Second");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_accelerator_case_insensitive() {
        assert_eq!(
            normalize_accelerator("Ctrl+Shift+A"),
            normalize_accelerator("ctrl+shift+a")
        );
    }

    #[test]
    fn test_normalize_accelerator_alias_equivalence() {
        assert_eq!(
            normalize_accelerator("CommandOrControl+A"),
            normalize_accelerator("CmdOrCtrl+A")
        );
        assert_eq!(
            normalize_accelerator("Option+B"),
            normalize_accelerator("Alt+B")
        );
    }

    #[test]
    fn test_normalize_accelerator_modifier_order_independent() {
        assert_eq!(
            normalize_accelerator("Shift+Ctrl+A"),
            normalize_accelerator("Ctrl+Shift+A")
        );
    }

    #[test]
    fn test_shortcut_conflict_serialization() {
        let conflict = ShortcutConflict {
            conflicting_id: "test".into(),
            conflicting_accelerator: "Ctrl+A".into(),
            description: "Test".into(),
        };
        let json = serde_json::to_value(&conflict).unwrap();
        assert_eq!(json["conflictingId"], "test");
    }

    #[test]
    fn test_list_registered_sorted() {
        let mgr = ShortcutManager::new();
        let list = mgr.list_registered();
        let ids: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = ShortcutManager::new();
        assert!(mgr.get("nonexistent").is_none());
    }
}
