//! Native resident floating surface; sampling and detail ownership stay in the existing runtime.
#[cfg(windows)]
mod drag;
#[cfg(any(windows, target_os = "macos", test))]
mod geometry;
#[cfg(windows)]
mod interaction;
#[cfg(any(windows, target_os = "macos"))]
mod menu;
#[cfg(any(windows, target_os = "macos", test))]
mod menu_preferences;
#[cfg(windows)]
#[path = "windows.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod native;
#[cfg(any(windows, target_os = "macos"))]
mod placement;
mod sizing;
#[cfg(any(windows, target_os = "macos"))]
mod window;

use super::{preference_schema::FloatingPreferences, tray_display::display_entry::DisplayEntry};
use serde::Serialize;

pub const LABEL: &str = "floating-monitor";
#[cfg(any(windows, target_os = "macos"))]
pub const EVENT: &str = "resident-floating-snapshot";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u32,
    pub interactive: bool,
    pub context_menu: bool,
    pub entries: Vec<DisplayEntry>,
    pub preferences: FloatingPreferences,
    pub columns: Option<Vec<f64>>,
}

pub use sizing::LayoutMeasurement;

#[cfg(windows)]
pub use interaction::{anchor, panel_position, toggle_click_through, toggle_metric};
#[cfg(any(windows, target_os = "macos"))]
pub use window::{install, prepare, prewarm, ready, set_layout, show, update, update_title};

#[cfg(windows)]
pub(crate) use window::recreate_webview;

pub fn show_menu(window: tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(any(windows, target_os = "macos"))]
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
    #[cfg(target_os = "macos")]
    {
        native::start_drag(window)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = window;
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn ready(_app: &tauri::AppHandle) -> Option<Snapshot> {
    None
}

#[cfg(not(any(windows, target_os = "macos")))]
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
