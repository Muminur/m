use crate::error::AppError;

/// Trait for inserting dictated text into the focused application.
pub trait TextInserter: Send + Sync {
    /// Insert text at the current cursor position in the focused application.
    fn insert_text(&self, text: &str) -> Result<(), AppError>;

    /// Get the name of the currently focused application, if available.
    fn get_focused_app(&self) -> Result<Option<String>, AppError>;
}

/// Request accessibility permissions from the OS.
/// On macOS this triggers the system prompt; on other platforms it returns true.
#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> Result<bool, AppError> {
    // macOS: would call AXIsProcessTrustedWithOptions via Swift plugin.
    // Stub for now — the Swift bridge is built separately.
    tracing::info!("Requesting macOS accessibility permission");
    Ok(false)
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_permission() -> Result<bool, AppError> {
    // Non-macOS: accessibility is not required for clipboard-based insertion.
    tracing::info!("Accessibility permission not required on this platform");
    Ok(true)
}

/// macOS text inserter that will bridge to the Swift accessibility plugin.
#[cfg(target_os = "macos")]
pub struct MacOSTextInserter;

#[cfg(target_os = "macos")]
impl TextInserter for MacOSTextInserter {
    fn insert_text(&self, text: &str) -> Result<(), AppError> {
        // Will be implemented via Swift plugin bridge
        tracing::warn!(
            len = text.len(),
            "MacOS text insertion not yet bridged to Swift"
        );
        Ok(())
    }

    fn get_focused_app(&self) -> Result<Option<String>, AppError> {
        // Will be implemented via Swift plugin bridge
        Ok(None)
    }
}

/// Stub text inserter for non-macOS platforms and testing.
pub struct StubTextInserter;

impl TextInserter for StubTextInserter {
    fn insert_text(&self, text: &str) -> Result<(), AppError> {
        tracing::debug!(len = text.len(), "StubTextInserter: text insertion (no-op)");
        Ok(())
    }

    fn get_focused_app(&self) -> Result<Option<String>, AppError> {
        Ok(None)
    }
}

/// Create the platform-appropriate text inserter.
pub fn create_text_inserter() -> Box<dyn TextInserter> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSTextInserter)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(StubTextInserter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_inserter_insert_text_succeeds() {
        let inserter = StubTextInserter;
        assert!(inserter.insert_text("hello world").is_ok());
    }

    #[test]
    fn test_stub_inserter_get_focused_app_returns_none() {
        let inserter = StubTextInserter;
        let result = inserter.get_focused_app().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_stub_inserter_empty_text() {
        let inserter = StubTextInserter;
        assert!(inserter.insert_text("").is_ok());
    }

    #[test]
    fn test_request_accessibility_permission_non_macos() {
        // On non-macOS (our test platform), should return Ok(true)
        #[cfg(not(target_os = "macos"))]
        {
            let result = request_accessibility_permission().unwrap();
            assert!(result);
        }
    }

    #[test]
    fn test_create_text_inserter_returns_stub_on_non_macos() {
        #[cfg(not(target_os = "macos"))]
        {
            let inserter = create_text_inserter();
            assert!(inserter.insert_text("test").is_ok());
            assert!(inserter.get_focused_app().unwrap().is_none());
        }
    }

    #[test]
    fn test_stub_inserter_unicode() {
        let inserter = StubTextInserter;
        assert!(inserter.insert_text("Hello! Привет! 你好!").is_ok());
    }
}
