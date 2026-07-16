//! Floating always-on-top recorder window.
//!
//! Provides a small HUD window with a single record/stop button so the user
//! can start a recording from anywhere without opening the full app. On stop
//! it reuses the SAME pipeline as the tray "Stop and Transcribe" flow: it
//! stops the recording in Rust and emits `tray://record/stopped`, which the
//! main window's tray bridge handles (navigate to the new transcript +
//! auto-start transcription). This keeps a single source of truth for the
//! stop→transcribe→redirect behavior.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::audio::recording::RecordingManager;
use crate::error::{AppError, AudioErrorCode};

const FLOAT_LABEL: &str = "float";
// Sized for a horizontal "pill" widget rendered by the transparent frontend.
const FLOAT_W: f64 = 240.0;
const FLOAT_H: f64 = 76.0;

/// Persisted floating-widget state (remembers whether the user last showed or
/// hid the widget). Stored as `float_widget.json` in the app config dir,
/// alongside `settings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FloatWidgetState {
    visible: bool,
}

/// Path to the persisted float widget state file, if the config dir resolves.
fn float_state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("float_widget.json"))
}

/// Read the persisted "should the widget be visible" flag. Defaults to `true`
/// (visible) on first run or on any read/parse error — the widget is opt-out.
fn read_float_visible(app: &AppHandle) -> bool {
    let Some(path) = float_state_path(app) else {
        return true;
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<FloatWidgetState>(&content) {
            Ok(state) => state.visible,
            Err(e) => {
                tracing::warn!(error = ?e, "float: failed to parse state, defaulting to visible");
                true
            }
        },
        // No file yet → first run → default to visible.
        Err(_) => true,
    }
}

/// Persist the desired visibility. IO errors are logged and ignored (the
/// widget still works, it just won't remember the preference next launch).
fn write_float_visible(app: &AppHandle, visible: bool) {
    let Some(path) = float_state_path(app) else {
        tracing::warn!("float: could not resolve config dir to persist state");
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = ?e, "float: failed to create config dir for state");
            return;
        }
    }
    match serde_json::to_string_pretty(&FloatWidgetState { visible }) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                tracing::warn!(error = ?e, "float: failed to write state file");
            }
        }
        Err(e) => tracing::warn!(error = ?e, "float: failed to serialize state"),
    }
}

/// Convert the freshly-built float `WebviewWindow` into a non-activating,
/// floating NSPanel that appears over fullscreen apps and on every Space.
///
/// This is the load-bearing bit: Tauri's own `set_visible_on_all_workspaces`
/// omits the `FullScreenAuxiliary` collection behavior, so a plain window only
/// shows on the Desktop Space. The non-activating style mask (bit 7) keeps the
/// panel clickable while ensuring it never activates the app / steals focus.
///
/// Must be called exactly once per window (never call `to_panel` twice on the
/// same window). On error we log rather than panic so a failed conversion
/// leaves the app running with a plain (Desktop-only) window.
#[cfg(target_os = "macos")]
fn convert_to_panel(window: &tauri::WebviewWindow) {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::WebviewWindowExt;

    match window.to_panel() {
        Ok(panel) => {
            // NSFloatingWindowLevel == 4.
            panel.set_level(4);
            // NSWindowStyleMaskNonactivatingPanel == 1 << 7 (128): clickable but
            // never activates the owning app, so focus is never stolen.
            panel.set_style_mask(1 << 7);
            // Appear over fullscreen apps AND join every Space.
            panel.set_collection_behaviour(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces,
            );
            tracing::info!("float window converted to floating NSPanel");
        }
        Err(e) => {
            tracing::error!(error = ?e, "float: failed to convert window to NSPanel");
        }
    }
}

/// Show the floating recorder WITHOUT activating the app / stealing focus.
/// On macOS this uses the converted panel handle; elsewhere it falls back to
/// the plain window.
fn show_float(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        match app.get_webview_panel(FLOAT_LABEL) {
            Ok(panel) => {
                // `show()` orders the panel front without activating the app.
                panel.show();
                return;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "float: no panel to show, falling back to window");
            }
        }
    }
    if let Some(win) = app.get_webview_window(FLOAT_LABEL) {
        let _ = win.show();
    }
}

/// Hide the floating recorder. On macOS this orders the panel out; elsewhere
/// it falls back to hiding the plain window.
fn hide_float(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        match app.get_webview_panel(FLOAT_LABEL) {
            Ok(panel) => {
                panel.order_out(None);
                return;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "float: no panel to hide, falling back to window");
            }
        }
    }
    if let Some(win) = app.get_webview_window(FLOAT_LABEL) {
        let _ = win.hide();
    }
}

/// Create the floating recorder window (positioned bottom-right of the primary
/// monitor) and, on macOS, convert it to a floating NSPanel. Built HIDDEN and
/// non-activating: callers decide whether to `show_float`. Never steals focus.
///
/// This performs the one-time window creation + panel conversion. Callers must
/// guard against building twice (see `show_or_toggle_floating_recorder` and
/// `init_float_on_startup`, which check for an existing window first).
fn build_float_window(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(
        app,
        FLOAT_LABEL,
        // Load the plain index.html with NO query string. `WebviewUrl::App`
        // takes a `PathBuf`, and Tauri percent-encodes a `?` to `%3F`, so
        // `index.html?view=float` would load `/index.html%3Fview=float` and
        // `window.location.search` would be empty in the webview. Instead we
        // inject a `window.__WD_FLOAT__` global below (runs before any page JS),
        // which main.tsx uses to render only the FloatingRecorder (no router,
        // no tray bridge, no Layout chrome).
        WebviewUrl::App("index.html".into()),
    )
    .initialization_script("window.__WD_FLOAT__ = true;")
    .title("Recorder")
    .inner_size(FLOAT_W, FLOAT_H)
    .min_inner_size(FLOAT_W, FLOAT_H)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    // Frontend renders a transparent pill; macos-private-api is enabled.
    .transparent(true)
    // Build hidden — panel conversion happens first, then callers show it.
    .visible(false);

    // Bottom-right of the primary monitor (logical coordinates).
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size(); // physical pixels
        let logical_w = size.width as f64 / scale;
        let logical_h = size.height as f64 / scale;
        let x = (logical_w - FLOAT_W - 24.0).max(0.0);
        let y = (logical_h - FLOAT_H - 80.0).max(0.0);
        tracing::info!(x, y, logical_w, logical_h, scale, "float window target position");
        builder = builder.position(x, y);
    } else {
        tracing::warn!("float: primary_monitor() returned None — using default position");
    }

    let win = builder.build()?;
    #[cfg(target_os = "macos")]
    convert_to_panel(&win);
    #[cfg(not(target_os = "macos"))]
    let _ = &win;
    tracing::info!("floating recorder window built");
    Ok(())
}

/// Create + show the floating recorder on app launch, honoring the persisted
/// visibility preference. On first run (or missing/corrupt state) the widget is
/// shown. If the user last hid it, the window/panel is still built + converted
/// (so a later toggle is instant) but kept hidden. Called from `lib.rs` setup.
pub fn init_float_on_startup(app: &AppHandle) {
    if app.get_webview_window(FLOAT_LABEL).is_some() {
        // Already created (defensive — setup runs once, but stay idempotent).
        return;
    }
    let visible = read_float_visible(app);
    match build_float_window(app) {
        Ok(()) => {
            if visible {
                tracing::info!("float: startup — showing (persisted visible)");
                show_float(app);
            } else {
                tracing::info!("float: startup — keeping hidden (persisted hidden)");
                hide_float(app);
            }
        }
        Err(e) => {
            tracing::error!(error = ?e, "float: failed to build window on startup");
        }
    }
}

/// Show the floating recorder if hidden, hide it if visible, or create it if
/// it doesn't exist yet. Never activates the app / steals focus. Persists the
/// new desired visibility. Callable from the tray menu handler (Rust) and the
/// `toggle_floating_recorder` command (frontend).
pub fn show_or_toggle_floating_recorder(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(FLOAT_LABEL) {
        let currently_visible = win.is_visible().unwrap_or(false);
        if currently_visible {
            tracing::info!("floating recorder: visible → hiding");
            hide_float(app);
            write_float_visible(app, false);
        } else {
            tracing::info!("floating recorder: hidden → showing");
            show_float(app);
            write_float_visible(app, true);
        }
    } else {
        tracing::info!("floating recorder: no window, creating + showing");
        match build_float_window(app) {
            Ok(()) => {
                show_float(app);
                write_float_visible(app, true);
            }
            Err(e) => {
                tracing::error!(error = ?e, "failed to build floating recorder window");
            }
        }
    }
}

/// Toggle the floating recorder window (create/show/hide).
#[tauri::command]
pub async fn toggle_floating_recorder(app: AppHandle) -> Result<(), String> {
    show_or_toggle_floating_recorder(&app);
    Ok(())
}

/// Stop the current recording from the floating window and hand off to the
/// existing tray pipeline. Brings the main window forward, stops the
/// recording in Rust, then emits `tray://record/stopped` so the main window's
/// tray bridge navigates to the new transcript and starts transcription.
#[tauri::command]
pub async fn float_stop_recording(app: AppHandle) -> Result<(), AppError> {
    // Bring the main window forward so the user sees the streaming transcript.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }

    let manager = match app.try_state::<Arc<RecordingManager>>() {
        Some(m) => Arc::clone(&m),
        None => {
            return Err(AppError::AudioError {
                code: AudioErrorCode::CaptureFailure,
                message: "RecordingManager not initialized".into(),
            });
        }
    };

    let app_clone = app.clone();
    let result = tokio::task::spawn_blocking(move || manager.stop(&app_clone))
        .await
        .map_err(|e| AppError::AudioError {
            code: AudioErrorCode::CaptureFailure,
            message: format!("Recording stop thread failed: {e}"),
        })??;

    // Same event the tray's "Stop and Transcribe" emits — reused so the
    // navigate + auto-transcribe logic lives in exactly one place (trayBridge).
    if let Err(e) = app.emit("tray://record/stopped", &result) {
        tracing::error!(error = ?e, "failed to emit tray://record/stopped from float");
    }

    Ok(())
}

/// Beacon invoked by the FloatingRecorder React component on mount, so the
/// backend log confirms the float webview actually loaded and rendered the
/// float view (not the main app). Diagnostic + harmless.
#[tauri::command]
pub fn float_ready(is_float: bool) {
    tracing::info!(is_float, "float webview mounted (frontend beacon)");
}
