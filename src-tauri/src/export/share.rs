//! macOS Share Sheet integration via NSSharingService.
//!
//! This module provides a stub on non-macOS platforms.
//! Full Swift plugin implementation is deferred to M10.

use crate::error::AppError;

#[cfg(target_os = "macos")]
pub use macos::share_via_sheet;

#[cfg(not(target_os = "macos"))]
pub fn share_via_sheet(_path: &str, _title: &str) -> Result<(), AppError> {
    Err(AppError::ExportError {
        code: crate::error::ExportErrorCode::FormatError,
        message: "Share Sheet is only available on macOS".into(),
    })
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::error::AppError;

    /// Share a file via the macOS Share Sheet (NSSharingService).
    ///
    /// Full Swift plugin (tauri-plugin-share) is implemented in M10.
    /// This stub logs the intent and returns Ok(()) to allow UI wiring.
    pub fn share_via_sheet(path: &str, _title: &str) -> Result<(), AppError> {
        tracing::info!("Share Sheet requested for: {}", path);
        // TODO(M10): invoke tauri-plugin-share Swift plugin
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_via_sheet_non_macos() {
        #[cfg(not(target_os = "macos"))]
        {
            let result = share_via_sheet("/tmp/test.pdf", "Test");
            assert!(result.is_err());
        }
    }
}
