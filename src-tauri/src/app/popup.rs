//! The tray popup window: sized to its content and placed above the tray like a
//! Windows 11 flyout. It stays open when it loses focus, so it can be watched
//! while working elsewhere; it hides through its minimize button, Esc, or
//! another click on the tray icon.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const LABEL: &str = "popup";
const WIDTH: f64 = 360.0;
/// Gap between the popup and the taskbar / screen edge, as in Windows flyouts.
const MARGIN: f64 = 12.0;
const DEFAULT_HEIGHT: f64 = 533.0;

pub struct PopupState {
    /// Logical height of the page, reported by the frontend.
    height: Mutex<f64>,
    /// Centre of the tray icon in physical pixels, once it has been clicked.
    anchor: Mutex<Option<(f64, f64)>>,
}

impl Default for PopupState {
    fn default() -> Self {
        PopupState {
            height: Mutex::new(DEFAULT_HEIGHT),
            anchor: Mutex::new(None),
        }
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Puts the popup in the corner of the work area nearest the taskbar.
fn place(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    let state = app.state::<PopupState>();
    let anchor = *state.anchor.lock().unwrap();
    let monitor = match anchor {
        Some((x, y)) => app.monitor_from_point(x, y)?,
        None => None,
    };
    let Some(monitor) = monitor.or(window.primary_monitor()?) else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);

    let width = WIDTH * scale;
    let height = *state.height.lock().unwrap() * scale;
    let margin = MARGIN * scale;
    let mut x = right - width - margin;
    let mut y = bottom - height - margin;
    if let Some((ax, ay)) = anchor {
        // The tray sits outside the work area, on whichever side the taskbar is.
        if ay < top {
            y = top + margin;
        }
        if ax < left {
            x = left + margin;
        }
    }
    window.set_size(PhysicalSize::new(
        width.round() as u32,
        height.round() as u32,
    ))?;
    window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))?;
    Ok(())
}

pub fn set_anchor(app: &AppHandle, x: f64, y: f64) {
    *app.state::<PopupState>().anchor.lock().unwrap() = Some((x, y));
}

pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    let _ = place(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn is_visible(app: &AppHandle) -> bool {
    window(app).is_some_and(|w| w.is_visible().unwrap_or(false))
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        let _ = window.hide();
    }
}

/// What a left click on the tray icon does.
pub fn toggle(app: &AppHandle) {
    if is_visible(app) {
        hide(app);
    } else {
        show(app);
    }
}

#[tauri::command]
pub fn fit_popup(app: AppHandle, height: f64) -> Result<(), String> {
    *app.state::<PopupState>().height.lock().unwrap() = height.clamp(100.0, 900.0);
    match window(&app) {
        Some(window) => place(&app, &window).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    hide(&app);
}
