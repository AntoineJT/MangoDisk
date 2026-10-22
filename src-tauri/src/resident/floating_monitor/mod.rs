//! Windows-only resident surface; sampling and detail ownership stay in the existing runtime.
#[cfg(windows)]
mod drag;
#[cfg(any(windows, test))]
mod geometry;
#[cfg(windows)]
mod interaction;
#[cfg(windows)]
mod menu;
#[cfg(windows)]
mod placement;
mod sizing;
#[cfg(windows)]
mod window;

use super::{preference_schema::FloatingPreferences, tray_display::display_entry::DisplayEntry};
use serde::Serialize;

pub const LABEL: &str = "floating-monitor";
#[cfg(windows)]
pub const EVENT: &str = "resident-floating-snapshot";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u32,
    pub entries: Vec<DisplayEntry>,
    pub preferences: FloatingPreferences,
    pub columns: Option<Vec<f64>>,
}

pub use sizing::LayoutMeasurement;

#[cfg(windows)]
pub use interaction::toggle as toggle_click_through;

#[cfg(windows)]
pub use window::{
    anchor, install, panel_position, prepare, prewarm, ready, set_layout, show, toggle_metric,
    update,
};

#[cfg(windows)]
pub(crate) use window::recreate_webview;

pub fn show_menu(window: tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(windows)]
    if window.label() == LABEL {
        return menu::open(&window);
    }
    let _ = window;
    Ok(())
}

pub fn start_drag(window: tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(windows)]
    {
        drag::start(window)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn ready(_app: &tauri::AppHandle) -> Option<Snapshot> {
    None
}

#[cfg(not(windows))]
pub fn set_layout(_app: &tauri::AppHandle, _measurement: LayoutMeasurement) -> bool {
    false
}

#[cfg(not(windows))]
pub fn toggle_metric(
    _app: &tauri::AppHandle,
    _metric: mangodisk_core::system_resources::metrics::MetricId,
) -> tauri::Result<()> {
    Ok(())
}
