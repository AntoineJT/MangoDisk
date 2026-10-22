//! Native menus stay outside the compact WebView bounds and remain keyboard accessible.
use super::{window, LABEL};
use crate::resident::{
    preference_schema::{FloatingBackground, FloatingFontSize, FloatingLayout, WindowsDisplayMode},
    preferences,
    runtime::ResidentState,
};
use std::sync::Arc;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    Manager,
};

pub fn install(window: &tauri::WebviewWindow) {
    let app = window.app_handle().clone();
    window.on_menu_event(move |_, event| {
        let action = event.id().as_ref().to_owned();
        if !action.starts_with("floating-") {
            return;
        }
        let app = app.clone();
        // Leave the native menu callback before creating windows or applying preferences.
        tauri::async_runtime::spawn(async move {
            if let Err(error) = act(&app, &action) {
                crate::resident::diagnostics::Failure::record("floating_menu", &error);
                crate::resident::main_window::request(
                    &app,
                    crate::resident::main_window::Destination::DisplaySettings,
                    "floating_menu_failed",
                );
            }
        });
    });
}

pub fn open(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let app = window.app_handle();
    let labels = crate::services::native_labels::NativeLabels::load(app);
    let preferences = app
        .state::<Arc<ResidentState>>()
        .preferences
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if preferences.floating.click_through {
        return Ok(());
    }
    let item = |id, key| MenuItem::with_id(app, id, labels.text(key), true, None::<&str>);
    let check = |id, key, selected| {
        CheckMenuItem::with_id(app, id, labels.text(key), true, selected, None::<&str>)
    };
    let horizontal = check(
        "floating-horizontal",
        "/floatingMonitor/horizontal",
        preferences.floating.layout == FloatingLayout::Horizontal,
    )?;
    let vertical = check(
        "floating-vertical",
        "/floatingMonitor/vertical",
        preferences.floating.layout == FloatingLayout::Vertical,
    )?;
    let top = check(
        "floating-top",
        "/floatingMonitor/alwaysOnTop",
        preferences.floating.always_on_top,
    )?;
    let locked = check(
        "floating-lock",
        "/floatingMonitor/locked",
        preferences.floating.locked,
    )?;
    let snap = check(
        "floating-snap",
        "/floatingMonitor/snapToEdges",
        preferences.floating.snap_to_edges,
    )?;
    let bold = check(
        "floating-bold",
        "/floatingMonitor/bold",
        preferences.floating.bold,
    )?;
    let font_size = Submenu::new(app, labels.text("/floatingMonitor/fontSize"), true)?;
    for (id, value) in [
        ("standard", FloatingFontSize::Standard),
        ("larger", FloatingFontSize::Larger),
        ("large", FloatingFontSize::Large),
        ("extraLarge", FloatingFontSize::ExtraLarge),
    ] {
        font_size.append(&CheckMenuItem::with_id(
            app,
            format!("floating-font-{id}"),
            labels.text(&format!("/floatingMonitor/fontSizes/{id}")),
            true,
            preferences.floating.font_size == value,
            None::<&str>,
        )?)?;
    }
    let background = Submenu::new(app, labels.text("/floatingMonitor/background"), true)?;
    for (id, value) in [
        ("default", FloatingBackground::Default),
        ("darkGray", FloatingBackground::DarkGray),
        ("inkBlue", FloatingBackground::InkBlue),
        ("warmWhite", FloatingBackground::WarmWhite),
        ("lightGray", FloatingBackground::LightGray),
        ("black", FloatingBackground::Black),
        ("white", FloatingBackground::White),
    ] {
        background.append(&CheckMenuItem::with_id(
            app,
            format!("floating-background-{id}"),
            labels.text(&format!("/floatingMonitor/backgrounds/{id}")),
            true,
            preferences.floating.background == value,
            None::<&str>,
        )?)?;
    }
    let settings = item("floating-settings", "/systemStatus/configureAction")?;
    let reset = item("floating-reset", "/floatingMonitor/resetPosition")?;
    let opacity = Submenu::new(app, labels.text("/floatingMonitor/backgroundOpacity"), true)?;
    for value in [100, 90, 80, 70, 60, 50] {
        opacity.append(&CheckMenuItem::with_id(
            app,
            format!("floating-opacity-{value}"),
            format!("{value}%"),
            true,
            preferences.floating.background_opacity == value,
            None::<&str>,
        )?)?;
    }
    let separator = PredefinedMenuItem::separator(app)?;
    let second = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &horizontal,
            &vertical,
            &separator,
            &top,
            &locked,
            &snap,
            &font_size,
            &bold,
            &background,
            &opacity,
            &second,
            &settings,
            &reset,
        ],
    )?;
    window.popup_menu(&menu)
}

fn act(app: &tauri::AppHandle, action: &str) -> tauri::Result<()> {
    match action {
        "floating-reset" => {
            window::reset(app);
            return Ok(());
        }
        "floating-settings" => {
            return crate::resident::main_window::open(
                app,
                crate::resident::main_window::Destination::DisplaySettings,
                LABEL,
            );
        }
        _ => {}
    }
    // A menu can remain open while another window saves. Re-read and retry only
    // a revision conflict; never overwrite the other window's unrelated fields.
    for attempt in 0..2 {
        let mut requested = app
            .state::<Arc<ResidentState>>()
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match action {
            "floating-horizontal" => requested.floating.layout = FloatingLayout::Horizontal,
            "floating-vertical" => requested.floating.layout = FloatingLayout::Vertical,
            "floating-bold" => requested.floating.bold = !requested.floating.bold,
            "floating-font-standard" => requested.floating.font_size = FloatingFontSize::Standard,
            "floating-font-larger" => requested.floating.font_size = FloatingFontSize::Larger,
            "floating-font-large" => requested.floating.font_size = FloatingFontSize::Large,
            "floating-font-extraLarge" => {
                requested.floating.font_size = FloatingFontSize::ExtraLarge
            }
            "floating-background-default" => {
                requested.floating.background = FloatingBackground::Default
            }
            "floating-background-darkGray" => {
                requested.floating.background = FloatingBackground::DarkGray
            }
            "floating-background-inkBlue" => {
                requested.floating.background = FloatingBackground::InkBlue
            }
            "floating-background-warmWhite" => {
                requested.floating.background = FloatingBackground::WarmWhite
            }
            "floating-background-lightGray" => {
                requested.floating.background = FloatingBackground::LightGray
            }
            "floating-background-black" => {
                requested.floating.background = FloatingBackground::Black
            }
            "floating-background-white" => {
                requested.floating.background = FloatingBackground::White
            }
            "floating-top" => requested.floating.always_on_top = !requested.floating.always_on_top,
            "floating-lock" => requested.floating.locked = !requested.floating.locked,
            "floating-snap" => requested.floating.snap_to_edges = !requested.floating.snap_to_edges,
            _ => match action
                .strip_prefix("floating-opacity-")
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|value| [100, 90, 80, 70, 60, 50].contains(value))
            {
                Some(value) => requested.floating.background_opacity = value,
                None => return Ok(()),
            },
        }
        if requested.windows_display_mode == WindowsDisplayMode::Taskbar {
            return Ok(());
        }
        match preferences::apply(app, requested) {
            Ok(_) => {
                log::info!("resident_floating_menu action={action} outcome=committed");
                return Ok(());
            }
            Err(error) if attempt == 0 && error.is_preference_conflict() => continue,
            Err(error) => return Err(tauri::Error::Anyhow(error.into())),
        }
    }
    Ok(())
}
