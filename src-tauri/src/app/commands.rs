//! Commands the popup can invoke. Each change returns the new status, so
//! the popup updates without waiting for the next tick.

use std::net::IpAddr;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use super::autostart;
use super::settings::LinkPrefs;
use super::state::AppState;
use super::status::{self, Connection, Status};
use super::ticker;
use crate::sys;

/// Passed to the elevated copy so it knows to wait for this one to exit.
pub const ELEVATED_ARG: &str = "--elevated";
/// Passed along with `ELEVATED_ARG` when the user asked to turn on.
pub const TURN_ON_ARG: &str = "--turn-on";

/// Turns MultiPath on or off. Turning on without administrator rights
/// restarts MultiPath elevated, through the UAC prompt, and this copy exits.
pub async fn switch(app: &AppHandle, on: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let result = if !on {
        state.turn_off(true).await;
        Ok(())
    } else if sys::is_elevated() {
        state.turn_on().await
    } else {
        sys::relaunch_elevated(&format!("{ELEVATED_ARG} {TURN_ON_ARG}"))
            .map_err(|e| e.to_string())?;
        app.exit(0);
        Ok(())
    };
    ticker::publish(app);
    result
}

fn current(app: &AppHandle) -> Status {
    ticker::publish(app);
    status::build(&app.state::<AppState>())
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Status {
    status::build(&state)
}

#[tauri::command]
pub async fn set_enabled(app: AppHandle, enabled: bool) -> Result<Status, String> {
    switch(&app, enabled).await?;
    Ok(current(&app))
}

/// Changes the preferences of one network the popup listed.
fn update_link(
    app: &AppHandle,
    id: String,
    change: impl FnOnce(&mut LinkPrefs),
) -> Result<Status, String> {
    let state = app.state::<AppState>();
    if state.links.get(&id).is_none() {
        return Err(format!("no network {id}"));
    }
    state.update_settings(|s| change(s.links.entry(id).or_default()));
    Ok(current(app))
}

#[tauri::command]
pub fn set_link_enabled(app: AppHandle, id: String, enabled: bool) -> Result<Status, String> {
    update_link(&app, id, |p| p.enabled = enabled)
}

#[tauri::command]
pub fn set_link_weight(app: AppHandle, id: String, weight: u32) -> Result<Status, String> {
    update_link(&app, id, |p| p.weight = weight.clamp(1, 10))
}

/// The settings a change can carry; fields left out stay as they are.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OptionChange {
    pub adaptive: Option<bool>,
    pub sticky: Option<bool>,
    pub capture_ipv6: Option<bool>,
    pub dns: Option<Vec<String>>,
    pub start_with_windows: Option<bool>,
}

/// Parses DNS servers typed by the user; empty entries are skipped.
fn parse_dns(servers: &[String]) -> Result<Vec<IpAddr>, String> {
    servers
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().map_err(|_| format!("{s} is not an IP address")))
        .collect()
}

#[tauri::command]
pub async fn set_option(app: AppHandle, change: OptionChange) -> Result<Status, String> {
    let state = app.state::<AppState>();
    let dns = change.dns.as_deref().map(parse_dns).transpose()?;
    if let Some(enabled) = change.start_with_windows {
        tauri::async_runtime::spawn_blocking(move || autostart::set_enabled(enabled))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Couldn't change Start with Windows: {e}"))?;
        state.start_with_windows.store(enabled, Ordering::Relaxed);
    }

    let before = state.settings();
    state.update_settings(|s| {
        if let Some(adaptive) = change.adaptive {
            s.adaptive = adaptive;
        }
        if let Some(sticky) = change.sticky {
            s.sticky = sticky;
        }
        if let Some(capture_ipv6) = change.capture_ipv6 {
            s.capture_ipv6 = capture_ipv6;
        }
        if let Some(dns) = dns {
            s.dns = dns;
        }
    });
    let after = state.settings();
    // The adapter's DNS servers are set when it's created.
    if before.dns != after.dns {
        state.restart().await?;
    }
    Ok(current(&app))
}

#[tauri::command]
pub fn list_connections(state: State<AppState>) -> Vec<Connection> {
    status::connections(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_servers_are_checked() {
        let typed = vec!["1.1.1.1".into(), " 2606:4700::1111 ".into(), "".into()];
        assert_eq!(parse_dns(&typed).unwrap().len(), 2);
        assert!(parse_dns(&["dns.google".into()]).is_err());
    }

    #[test]
    fn option_changes_are_partial() {
        let change: OptionChange = serde_json::from_str(r#"{"captureIpv6":false}"#).unwrap();
        assert_eq!(change.capture_ipv6, Some(false));
        assert!(change.adaptive.is_none() && change.dns.is_none());
    }
}
