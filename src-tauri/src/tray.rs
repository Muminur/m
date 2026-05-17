//! macOS menu bar tray icon for WhisperDesk.
//!
//! Builds the tray icon + menu, emits Tauri events when items are clicked, and
//! exposes [`update_tray_state`] for the frontend to push recording-state changes.

use std::sync::Mutex;

use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Emitter, Manager, Runtime,
};

/// High-level state the tray icon visualizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Recording,
    Paused,
}

/// Stored references to the menu items so [`update_tray_state`] can toggle
/// their enabled flag without rebuilding the whole menu.
pub struct TrayMenuItems<R: Runtime> {
    pub start: MenuItem<R>,
    pub pause: MenuItem<R>,
    pub resume: MenuItem<R>,
    pub stop: MenuItem<R>,
}

/// App-managed handle to mutate tray state after setup.
pub struct TrayHandle<R: Runtime> {
    pub tray_id: tauri::tray::TrayIconId,
    pub items: TrayMenuItems<R>,
    pub current: Mutex<TrayState>,
}

/// Build the tray, register the click handler, and store the handle in app state.
pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let handle = app.handle();

    let start = MenuItem::with_id(handle, "tray.start", "Start Recording", true, None::<&str>)?;
    let pause = MenuItem::with_id(handle, "tray.pause", "Pause", false, None::<&str>)?;
    let resume = MenuItem::with_id(handle, "tray.resume", "Resume", false, None::<&str>)?;
    let stop = MenuItem::with_id(handle, "tray.stop", "Stop & Transcribe", false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(handle)?;
    let show = MenuItem::with_id(handle, "tray.show", "Show WhisperDesk", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "tray.quit", "Quit WhisperDesk", true, Some("Cmd+Q"))?;

    let menu = Menu::with_items(
        handle,
        &[&start, &pause, &resume, &stop, &sep, &show, &quit],
    )?;

    let idle_icon = load_tray_image(handle, TrayState::Idle)?;

    let tray = TrayIconBuilder::with_id("whisperdesk-tray")
        .icon(idle_icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .build(handle)?;

    app.manage(TrayHandle {
        tray_id: tray.id().clone(),
        items: TrayMenuItems {
            start,
            pause,
            resume,
            stop,
        },
        current: Mutex::new(TrayState::Idle),
    });

    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id.as_ref();
    let event_name = match id {
        "tray.start" => "tray://record/start",
        "tray.pause" => "tray://record/pause",
        "tray.resume" => "tray://record/resume",
        "tray.stop" => "tray://record/stop",
        "tray.show" => "tray://window/show",
        "tray.quit" => "tray://app/quit",
        _ => return,
    };
    if let Err(e) = app.emit(event_name, ()) {
        tracing::error!(event = %event_name, error = ?e, "failed to emit tray event");
    }
}

/// Swap the icon and toggle menu item enabled flags based on the new state.
pub fn update_tray_state<R: Runtime>(app: &AppHandle<R>, state: TrayState) {
    let Some(handle) = app.try_state::<TrayHandle<R>>() else {
        tracing::warn!("update_tray_state called before setup_tray");
        return;
    };

    let mut current = handle
        .current
        .lock()
        .expect("tray state mutex poisoned");
    if *current == state {
        return;
    }
    *current = state;
    drop(current);

    if let Ok(image) = load_tray_image(app, state) {
        if let Some(tray) = app.tray_by_id(&handle.tray_id) {
            if let Err(e) = tray.set_icon(Some(image)) {
                tracing::error!(error = ?e, "failed to set tray icon");
            }
        }
    }

    let (start_enabled, pause_enabled, resume_enabled, stop_enabled) = match state {
        TrayState::Idle => (true, false, false, false),
        TrayState::Recording => (false, true, false, true),
        TrayState::Paused => (false, false, true, true),
    };
    let _ = handle.items.start.set_enabled(start_enabled);
    let _ = handle.items.pause.set_enabled(pause_enabled);
    let _ = handle.items.resume.set_enabled(resume_enabled);
    let _ = handle.items.stop.set_enabled(stop_enabled);
}

fn load_tray_image<R: Runtime, M: Manager<R>>(
    manager: &M,
    state: TrayState,
) -> tauri::Result<Image<'static>> {
    let filename = match state {
        TrayState::Idle => "icons/tray-idle.png",
        TrayState::Recording => "icons/tray-recording.png",
        TrayState::Paused => "icons/tray-paused.png",
    };
    let path = manager.path().resolve(filename, tauri::path::BaseDirectory::Resource)?;
    Image::from_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_state_transitions_are_distinct() {
        assert_ne!(TrayState::Idle, TrayState::Recording);
        assert_ne!(TrayState::Recording, TrayState::Paused);
        assert_ne!(TrayState::Paused, TrayState::Idle);
    }
}
