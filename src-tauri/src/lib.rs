// The engine and networking modules are wired into the app in Phase 4 of
// the TaskList; remove this once they are.
#![allow(dead_code, unused_imports)]

mod app;
mod engine;
mod net;
mod sys;

use std::time::Duration;

use tauri::{Manager, WindowEvent};

use app::{commands, popup, tray};

pub fn run() {
    let relaunched = std::env::args().any(|a| a == commands::ELEVATED_ARG);
    // The elevated copy waits for the copy that launched it to exit.
    let wait = if relaunched {
        Duration::from_secs(10)
    } else {
        Duration::ZERO
    };
    let instance = match sys::Instance::acquire(wait) {
        Ok(Some(instance)) => Some(instance),
        Ok(None) => {
            let _ = sys::Instance::signal_running();
            return;
        }
        Err(err) => {
            eprintln!("single-instance check failed: {err}");
            None
        }
    };

    tauri::Builder::default()
        .manage(popup::PopupState::default())
        .invoke_handler(tauri::generate_handler![
            popup::fit_popup,
            popup::hide_popup,
            commands::is_elevated,
            commands::relaunch_as_admin,
        ])
        .setup(move |app| {
            tray::create(app.handle())?;
            if let Some(instance) = &instance {
                let handle = app.handle().clone();
                instance.on_show_requested(move || popup::show(&handle));
            }
            app.manage(instance);
            if relaunched {
                popup::show(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != popup::LABEL {
                return;
            }
            match event {
                WindowEvent::Focused(false) => popup::hide(window.app_handle()),
                // The popup is only ever hidden; quitting goes through the tray menu.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    popup::hide(window.app_handle());
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MultiPath");
}
