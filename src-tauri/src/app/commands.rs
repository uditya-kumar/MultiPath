//! Commands the popup can invoke.

use tauri::AppHandle;

use crate::sys;

/// Passed to the elevated copy so it knows to wait for this one to exit.
pub const ELEVATED_ARG: &str = "--elevated";

#[tauri::command]
pub fn is_elevated() -> bool {
    sys::is_elevated()
}

/// Restarts MultiPath as administrator through the UAC prompt. On success this
/// copy exits and the elevated one opens its popup.
#[tauri::command]
pub fn relaunch_as_admin(app: AppHandle) -> Result<(), String> {
    sys::relaunch_elevated(ELEVATED_ARG).map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
