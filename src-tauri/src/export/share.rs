//! Cross-platform file sharing.
//!
//! On macOS: opens the file with the system app picker via `open`.
//! On Windows: opens with the default app via `cmd /C start`.
//! On Linux: opens with `xdg-open`.

use crate::error::{AppError, ExportErrorCode};

/// Share a file by opening it with the platform default handler.
///
/// On macOS this triggers the OS app picker; on Windows and Linux it opens
/// the file with the registered default application for the file type.
pub fn share_via_sheet(path: &str, _title: &str) -> Result<(), AppError> {
    tracing::info!("Share requested for: {}", path);

    let result = open_with_system(path);

    result.map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("Failed to open file for sharing: {}", e),
    })
}

#[cfg(target_os = "macos")]
fn open_with_system(path: &str) -> Result<(), std::io::Error> {
    std::process::Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_with_system(path: &str) -> Result<(), std::io::Error> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", path])
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_with_system(path: &str) -> Result<(), std::io::Error> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_via_sheet_returns_result() {
        // We cannot actually open a file in tests, but verify the function exists
        // and handles a nonexistent path gracefully (the spawn itself may succeed
        // because the OS command starts asynchronously).
        let _result = share_via_sheet("/tmp/nonexistent_test_file.txt", "Test");
        // On CI / headless, this may succeed or fail depending on platform;
        // the important thing is it doesn't panic.
    }
}
