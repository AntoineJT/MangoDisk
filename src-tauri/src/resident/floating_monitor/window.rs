use super::{
    placement::Placement, sizing::ItemMeasurement, LayoutMeasurement, Snapshot, EVENT, LABEL,
};
use crate::resident::{preference_schema::WindowsDisplayMode, preferences::ResidentPreferences};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct State {
    snapshot: Mutex<Option<Snapshot>>,
    creation: Mutex<()>,
    position: Mutex<Placement>,
    ready: AtomicBool,
    queued: AtomicBool,
    failed: AtomicBool,
    click_through: AtomicBool,
    measurements: Mutex<Option<LayoutMeasurement>>,
}

pub fn install(app: &tauri::AppHandle) {
    app.manage(State {
        snapshot: Mutex::new(None),
        creation: Mutex::new(()),
        position: Mutex::new(Placement::load(app)),
        ready: AtomicBool::new(false),
        queued: AtomicBool::new(false),
        failed: AtomicBool::new(false),
        click_through: AtomicBool::new(false),
        measurements: Mutex::new(None),
    });
}

pub fn prepare(app: &tauri::AppHandle, preferences: &ResidentPreferences) -> tauri::Result<()> {
    let state = app.state::<State>();
    let _creation = state.creation.lock().unwrap_or_else(|e| e.into_inner());
    if !requested(preferences) || app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    // Document readiness and native placement cannot survive a WebView replacement.
    state.ready.store(false, Ordering::Relaxed);
    *state.measurements.lock().unwrap_or_else(|e| e.into_inner()) = None;
    state
        .position
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .invalidate_window();
    state.click_through.store(false, Ordering::Relaxed);
    let window =
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("floating-monitor.html".into()))
            .title(format!(
                "MangoDisk · {}",
                crate::services::native_labels::NativeLabels::load(app)
                    .text("/floatingMonitor/title")
            ))
            .inner_size(114.0, 26.0)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .shadow(false)
            .transparent(true)
            .always_on_top(preferences.floating.always_on_top)
            .build()?;
    crate::webview_recovery::install(&window);
    super::menu::install(&window);
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
        }
    });
    log::info!("resident_floating_created outcome=hidden_until_ready");
    Ok(())
}

pub(crate) fn recreate_webview(app: &tauri::AppHandle) -> tauri::Result<()> {
    let resident = app.state::<std::sync::Arc<crate::resident::runtime::ResidentState>>();
    let preferences = resident
        .preferences
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    // Current preferences and fullscreen detection own visibility, not the failed handle.
    prepare(app, &preferences)?;
    resident.wake();
    // Recovery runs on a worker. Restore dependent panels only after their anchor
    // has real geometry; fullscreen-hidden monitors still complete placement.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while let Some(window) = app.get_webview_window(LABEL) {
        if app
            .state::<State>()
            .position
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_applied()
        {
            // This UI-thread getter also drains the render that applied placement.
            let _ = window.is_visible()?;
            break;
        }
        if std::time::Instant::now() >= deadline {
            log::warn!(
                "resident_floating_recovery_layout_timeout outcome=restore_remaining_windows"
            );
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Ok(())
}

pub fn prewarm(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let preferences = app
            .state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Err(error) = prepare(&app, &preferences) {
            crate::resident::diagnostics::Failure::record("floating_startup", &error);
            log::warn!("resident_floating_startup_failed recovery=tray_icon");
        }
    });
}

fn requested(preferences: &ResidentPreferences) -> bool {
    preferences.enabled && preferences.windows_display_mode != WindowsDisplayMode::Taskbar
}

pub fn update(
    app: &tauri::AppHandle,
    preferences: &ResidentPreferences,
    entries: &[super::DisplayEntry],
) {
    let state = app.state::<State>();
    {
        let mut current = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let next = requested(preferences).then(|| Snapshot {
            schema_version: 6,
            entries: entries.to_vec(),
            preferences: preferences.floating.clone(),
            columns: None,
        });
        // Default taskbar mode never creates a WebView or queues floating UI work.
        if current.is_none() && next.is_none() && !state.failed.load(Ordering::Relaxed) {
            return;
        }
        *current = next;
    }
    if state.queued.swap(true, Ordering::Relaxed) {
        return;
    }
    let target = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        target
            .state::<State>()
            .queued
            .store(false, Ordering::Relaxed);
        let state = target.state::<State>();
        match render(&target) {
            Ok(()) if state.failed.swap(false, Ordering::Relaxed) => {
                log::info!("resident_floating_render_recovered")
            }
            Err(error) if !state.failed.swap(true, Ordering::Relaxed) => {
                crate::resident::diagnostics::Failure::record("floating_render", &error);
            }
            _ => {}
        }
    }) {
        state.queued.store(false, Ordering::Relaxed);
        crate::resident::diagnostics::Failure::record("floating_dispatch", &error);
    }
}

pub fn ready(app: &tauri::AppHandle) -> Option<Snapshot> {
    let state = app.state::<State>();
    log::info!("resident_floating_ready stage=measure_layout");
    let snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    app.state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
        .wake();
    snapshot
}

pub fn set_layout(app: &tauri::AppHandle, measurement: LayoutMeasurement) -> bool {
    let state = app.state::<State>();
    let current = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    let Some(snapshot) = current.as_ref() else {
        return false;
    };
    if !measurement.valid() || !measurement.matches(&snapshot.preferences, &snapshot.entries) {
        return false;
    }
    let entries = snapshot.entries.len();
    let mut measured = state.measurements.lock().unwrap_or_else(|e| e.into_inner());
    if state.ready.load(Ordering::Relaxed) && *measured == Some(measurement.clone()) {
        return true;
    }
    *measured = Some(measurement);
    drop(measured);
    state.ready.store(true, Ordering::Relaxed);
    drop(current);
    log::info!("resident_floating_layout_ready entries={entries} outcome=accepted");
    app.state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
        .wake();
    true
}

fn render(app: &tauri::AppHandle) -> tauri::Result<()> {
    let state = app.state::<State>();
    let Some(window) = app.get_webview_window(LABEL) else {
        return Ok(());
    };
    let snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let Some(mut snapshot) = snapshot else {
        if window.is_visible()? {
            set_visible(&window, false)?;
        }
        return Ok(());
    };
    let click_through = snapshot.preferences.click_through;
    if state.click_through.load(Ordering::Relaxed) != click_through {
        if click_through {
            // Never make the window inert before an independent recovery entry exists.
            let tray = app.tray_by_id(crate::resident::TRAY_ID).ok_or_else(|| {
                tauri::Error::Io(std::io::Error::other("floating_recovery_tray_missing"))
            })?;
            tray.set_visible(true)?;
        }
        window.set_ignore_cursor_events(click_through)?;
        state.click_through.store(click_through, Ordering::Relaxed);
        log::info!(
            "resident_floating_click_through enabled={click_through} recovery=tray outcome=applied"
        );
    }
    if !state.ready.load(Ordering::Relaxed) {
        app.emit_to(LABEL, EVENT, &snapshot)?;
        return Ok(());
    }
    let measurements = state
        .measurements
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if measurements
        .as_ref()
        .is_some_and(|measurement| !measurement.matches(&snapshot.preferences, &snapshot.entries))
    {
        app.emit_to(LABEL, EVENT, &snapshot)?;
        return Ok(());
    }
    let widths = measurements
        .and_then(|measurement| measurement.items)
        .map(|items| items.iter().map(ItemMeasurement::width).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![108.0 * snapshot.preferences.font_size.row_height() / 20.0; snapshot.entries.len()]
        });
    window.set_always_on_top(snapshot.preferences.always_on_top)?;
    snapshot.columns = super::placement::apply(&window, &snapshot, &state.position, &widths)?;
    if snapshot.columns.is_none() {
        return Ok(());
    }
    let fullscreen = fullscreen(&window);
    let visible = !fullscreen;
    if visible {
        app.emit_to(LABEL, EVENT, &snapshot)?;
    }
    if visible != window.is_visible()? {
        if visible {
            set_visible(&window, true)?;
        } else {
            set_visible(&window, false)?;
        }
        log::info!("resident_floating_visibility visible={visible} fullscreen={fullscreen}");
    }
    Ok(())
}

fn set_visible(window: &tauri::WebviewWindow, visible: bool) -> tauri::Result<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE};
    // Tao's Show activates a previously hidden window even with focused(false).
    // Own both visibility transitions here: its cached visibility stays hidden,
    // so Tao's Hide would otherwise skip hiding a natively shown monitor.
    unsafe {
        ShowWindow(
            window.hwnd()?.0.cast(),
            if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
        );
    }
    if window.is_visible()? != visible {
        return Err(std::io::Error::other("floating window visibility was not applied").into());
    }
    Ok(())
}

pub fn show(app: &tauri::AppHandle) {
    if app.get_webview_window(LABEL).is_none() {
        let preferences = app
            .state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
            .preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Err(error) = prepare(app, &preferences) {
            crate::resident::diagnostics::Failure::record("floating_reopen", &error);
            return;
        }
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Err(error) = window.unminimize() {
            crate::resident::diagnostics::Failure::record("floating_restore", &error);
        }
    }
    if let Some(state) = app.try_state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
    {
        state.wake();
    }
}

pub(super) fn finish_drag(app: &tauri::AppHandle) {
    // Windows also sends Moved when RDP, DPI, or work-area changes relocate a
    // window. Only an explicit native gesture may replace the saved anchor.
    app.state::<State>()
        .position
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .finish_drag();
}

pub(super) fn reset(app: &tauri::AppHandle) {
    let state = app.state::<State>();
    let mut position = state.position.lock().unwrap_or_else(|e| e.into_inner());
    position.reset();
    drop(position);
    show(app);
}

pub fn anchor(app: &tauri::AppHandle) -> Option<(f64, f64)> {
    let window = app.get_webview_window(LABEL)?;
    let point = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some((
        point.x as f64 + size.width as f64 / 2.0,
        point.y as f64 + size.height as f64 / 2.0,
    ))
}

pub fn panel_position(
    app: &tauri::AppHandle,
    work: (f64, f64, f64, f64),
    size: (f64, f64),
    gap: f64,
) -> Option<(f64, f64)> {
    let window = app.get_webview_window(LABEL)?;
    let origin = window.outer_position().ok()?;
    let bounds = window.outer_size().ok()?;
    Some(super::geometry::detail_position(
        super::geometry::Area {
            x: origin.x,
            y: origin.y,
            width: bounds.width,
            height: bounds.height,
        },
        work,
        size,
        gap,
    ))
}

pub fn toggle_metric(
    app: &tauri::AppHandle,
    metric: mangodisk_core::system_resources::metrics::MetricId,
) -> tauri::Result<()> {
    if app
        .state::<std::sync::Arc<crate::resident::runtime::ResidentState>>()
        .preferences
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .floating
        .click_through
    {
        return Ok(());
    }
    crate::resident::panel::toggle_metric_from(app, LABEL, metric)
}

fn fullscreen(window: &tauri::WebviewWindow) -> bool {
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{
            GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect,
            GetWindowThreadProcessId, GWL_STYLE, WS_CAPTION, WS_MAXIMIZE, WS_THICKFRAME,
        },
    };
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(foreground, &mut pid);
        if pid == std::process::id() {
            return false;
        }
        let mut class = [0u16; 128];
        let length = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(foreground, &mut rect) == 0 {
            return false;
        }
        let Ok(Some(monitor)) = window.current_monitor() else {
            return false;
        };
        let origin = monitor.position();
        let size = monitor.size();
        let style = GetWindowLongW(foreground, GWL_STYLE) as u32;
        rect.left <= origin.x
            && rect.top <= origin.y
            && rect.right >= origin.x + size.width as i32
            && rect.bottom >= origin.y + size.height as i32
            && (style & (WS_CAPTION | WS_THICKFRAME) == 0
                || (style & WS_MAXIMIZE == 0
                    && rect.left == origin.x
                    && rect.top == origin.y
                    && rect.right == origin.x + size.width as i32
                    && rect.bottom == origin.y + size.height as i32))
    }
}
