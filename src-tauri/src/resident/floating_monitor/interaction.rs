//! Click-through is window-wide; the independent tray remains the recovery surface.
use crate::resident::{preferences, runtime::ResidentState};
use std::sync::Arc;
use tauri::Manager;

pub fn toggle(app: &tauri::AppHandle) -> tauri::Result<()> {
    for attempt in 0..2 {
        let mut requested = app
            .state::<Arc<ResidentState>>()
            .preferences
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if !requested.enabled
            || requested.windows_display_mode
                == crate::resident::preference_schema::WindowsDisplayMode::Taskbar
        {
            return Ok(());
        }
        requested.floating.click_through = !requested.floating.click_through;
        match preferences::apply(app, requested) {
            Ok(_) => return Ok(()),
            Err(error) if attempt == 0 && error.is_preference_conflict() => continue,
            Err(error) => return Err(tauri::Error::Anyhow(error.into())),
        }
    }
    Ok(())
}
