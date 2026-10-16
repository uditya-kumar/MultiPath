//! The notification-area (macOS: menu bar) icon: a left click toggles the
//! popup, a right click opens a small menu. Linux trays (AppIndicator) open the menu on any click
//! and don't report clicks, so there the menu's "Open" shows the popup.

use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use tauri::{AppHandle, Manager, Position, Size, Wry};

use super::{commands, popup};

const TRAY_ID: &str = "main";
const ICON_ON: &[u8] = include_bytes!("../../icons/tray-on.png");
const ICON_OFF: &[u8] = include_bytes!("../../icons/tray-off.png");

/// The menu's on/off item, and what the icon shows now.
struct TrayState {
    toggle: MenuItem<Wry>,
    shown: Mutex<Option<(bool, String)>>,
}

fn physical_xy(position: &Position) -> (f64, f64) {
    match position {
        Position::Physical(p) => (p.x as f64, p.y as f64),
        Position::Logical(p) => (p.x, p.y),
    }
}

fn physical_wh(size: &Size) -> (f64, f64) {
    match size {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open MultiPath", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Turn on", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &toggle, &separator, &quit])?;
    app.manage(TrayState {
        toggle,
        shown: Mutex::new(None),
    });

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON_OFF)?)
        .tooltip("MultiPath — off")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => popup::show(app),
            "toggle" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let on = !app.state::<super::state::AppState>().is_on();
                    if let Err(err) = commands::switch(&app, on).await {
                        log::warn!("{err}");
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let (x, y) = physical_xy(&rect.position);
                let (w, h) = physical_wh(&rect.size);
                popup::set_anchor(tray.app_handle(), x + w / 2.0, y + h / 2.0);
                popup::toggle(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Shows whether MultiPath is combining networks, in the icon, its tooltip
/// and the menu.
pub fn set_active(app: &AppHandle, active: bool, tooltip: &str) -> tauri::Result<()> {
    let state = app.state::<TrayState>();
    let mut shown = state.shown.lock().unwrap();
    let wanted = (active, tooltip.to_owned());
    if shown.as_ref() == Some(&wanted) {
        return Ok(());
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon = if active { ICON_ON } else { ICON_OFF };
        tray.set_icon(Some(Image::from_bytes(icon)?))?;
        tray.set_tooltip(Some(tooltip))?;
    }
    state
        .toggle
        .set_text(if active { "Turn off" } else { "Turn on" })?;
    *shown = Some(wanted);
    Ok(())
}
