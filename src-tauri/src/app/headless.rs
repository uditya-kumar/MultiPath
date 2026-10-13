//! MultiPath without its popup or tray, for servers and the Linux test lab.
//! It turns on, prints the status as one JSON line a second, and turns off
//! again on Ctrl+C or SIGTERM.

use std::path::PathBuf;
use std::time::Duration;

use super::state::AppState;
use super::status;
use super::ticker::Ticks;
use crate::net::health;
use crate::{engine, sys};

pub const HEADLESS_ARG: &str = "--headless";
/// Followed by the folder for settings and usage.
const CONFIG_ARG: &str = "--config";
/// The app's identifier in `tauri.conf.json`, which names its config folder.
const IDENTIFIER: &str = "dev.multipath.app";

/// Runs until asked to stop, and returns the exit code.
pub fn run() -> i32 {
    let Some(config_dir) = config_dir() else {
        eprintln!("no config folder: pass {CONFIG_ARG} <folder>");
        return 2;
    };
    let _instance = match sys::Instance::acquire(Duration::ZERO) {
        Ok(Some(instance)) => Some(instance),
        Ok(None) => {
            eprintln!("MultiPath is already running");
            return 1;
        }
        Err(err) => {
            eprintln!("single-instance check failed: {err}");
            None
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("could not start: {err}");
            return 1;
        }
    };
    runtime.block_on(serve(config_dir))
}

async fn serve(config_dir: PathBuf) -> i32 {
    engine::clean_up_stale();
    let state = AppState::new(config_dir, false);
    // Turning on is remembered for the app; a headless run shouldn't change it.
    let was_on = state.settings().on;
    tokio::spawn(health::run(state.links.clone(), state.health_stop()));
    if let Err(err) = state.turn_on().await {
        eprintln!("{err}");
        state.update_settings(|s| s.on = was_on);
        return 1;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut ticks = Ticks::new(&state);
    let shutdown = shutdown_requested();
    tokio::pin!(shutdown);
    let code = loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = &mut shutdown => break 0,
        }
        ticks.tick(&state).await;
        let status = status::build(&state);
        match serde_json::to_string(&status) {
            Ok(line) => println!("{line}"),
            Err(err) => eprintln!("could not print the status: {err}"),
        }
        if !state.is_on() {
            eprintln!("{}", status.error.unwrap_or_default());
            break 1;
        }
    };
    state.turn_off(false).await;
    state.update_settings(|s| s.on = was_on);
    state.save_usage();
    code
}

/// The folder after `--config`, or the one the app itself uses.
fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::args().skip_while(|a| a != CONFIG_ARG).nth(1) {
        return Some(dir.into());
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")));
    Some(base?.join(IDENTIFIER))
}

async fn shutdown_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
