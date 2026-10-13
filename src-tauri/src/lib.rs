mod app;
mod engine;
mod net;
mod sys;

use std::time::Duration;

use tauri::{Manager, RunEvent, WindowEvent};

use app::state::AppState;
use app::{autostart, commands, headless, popup, ticker, tray};

pub fn run() {
    let has_arg = |arg: &str| std::env::args().any(|a| a == arg);
    if has_arg(headless::HEADLESS_ARG) {
        std::process::exit(headless::run());
    }
    let relaunched = has_arg(commands::ELEVATED_ARG);
    let turn_on = has_arg(commands::TURN_ON_ARG);
    let at_sign_in = has_arg(autostart::AUTOSTART_ARG);
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
            commands::get_status,
            commands::set_enabled,
            commands::set_link_enabled,
            commands::set_link_weight,
            commands::set_option,
            commands::list_connections,
        ])
        .setup(move |app| {
            let elevated = sys::is_elevated();
            if elevated {
                engine::clean_up_stale();
            }
            let state = AppState::new(app.path().app_config_dir()?, autostart::is_enabled());
            let start = elevated && (turn_on || state.settings().on);
            app.manage(state);
            tray::create(app.handle())?;
            ticker::spawn(app.handle().clone());
            if start {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::switch(&handle, true).await;
                });
            }

            if let Some(instance) = &instance {
                let handle = app.handle().clone();
                instance.on_show_requested(move || popup::show(&handle));
            }
            app.manage(instance);
            // At sign-in MultiPath starts quietly in the tray.
            if !at_sign_in {
                popup::show(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != popup::LABEL {
                return;
            }
            // The popup is only ever hidden; quitting goes through the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                popup::hide(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("error while starting MultiPath")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Give the PC its own routing back before going.
                let state = app.state::<AppState>();
                tauri::async_runtime::block_on(state.turn_off(false));
                state.save_usage();
            }
        });
}
