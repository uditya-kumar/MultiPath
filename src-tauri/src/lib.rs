mod app;
mod engine;
mod net;
mod sys;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{Manager, RunEvent, WindowEvent};

use app::state::AppState;
use app::{autostart, commands, headless, popup, ticker, tray};

pub fn run() {
    let has_arg = |arg: &str| std::env::args().any(|a| a == arg);
    if has_arg(headless::HEADLESS_ARG) {
        std::process::exit(headless::run());
    }
    #[cfg(target_os = "macos")]
    if has_arg(engine::HELPER_ARG) {
        let args: Vec<String> = std::env::args().collect();
        std::process::exit(engine::run_helper(&args));
    }
    // Wayland doesn't let a window place itself, so the popup would land
    // wherever the compositor puts it; through XWayland it goes in the corner.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() && std::env::var_os("DISPLAY").is_some() {
        std::env::set_var("GDK_BACKEND", "x11");
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
        // Signed self-updates: the popup checks, the user confirms, the
        // plugin verifies the signature before installing, then restarts.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            // A menu bar app: no Dock icon, no app menu.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let elevated = sys::is_elevated();
            if elevated {
                engine::clean_up_stale();
            }
            let state = AppState::new(app.path().app_config_dir()?, autostart::is_enabled());
            let can_start = elevated || sys::elevates_in_place();
            let start = can_start && (turn_on || state.settings().on);
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
            app.manage(Mutex::new(instance));
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
                // A restart after an update starts the new copy before this
                // one is gone; it must not find this copy still running.
                if let Some(instance) = app.try_state::<Mutex<Option<sys::Instance>>>() {
                    instance.lock().unwrap().take();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    /// The updater compares against the bundle's version, the popup shows
    /// Cargo's, and npm has its own: a release must bump all three.
    #[test]
    fn versions_agree() {
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(json(include_str!("../tauri.conf.json"))["version"], version);
        assert_eq!(json(include_str!("../../package.json"))["version"], version);
    }

    #[test]
    fn updates_come_signed_from_the_release_repo() {
        let conf = json(include_str!("../tauri.conf.json"));
        let updater = &conf["plugins"]["updater"];
        assert_eq!(
            updater["endpoints"],
            serde_json::json!([
                "https://github.com/timowse/MultiPath/releases/latest/download/latest.json"
            ])
        );
        // A minisign public key (base64 of its text form), never a secret key.
        let pubkey = updater["pubkey"].as_str().unwrap();
        assert!(pubkey.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6"));
        // A signature only for an older release must not pass for a newer one.
        assert_eq!(updater["requireSignedVersion"], true);
        for risky in [
            "dangerousInsecureTransportProtocol",
            "dangerousAcceptInvalidCerts",
            "dangerousAcceptInvalidHostnames",
        ] {
            assert!(updater.get(risky).is_none(), "{risky}");
        }
        // Read the way the plugin reads it at start, so a misspelt key
        // can't quietly leave a check off.
        let parsed: tauri_plugin_updater::Config = serde_json::from_value(updater.clone()).unwrap();
        assert!(parsed.require_signed_version && !parsed.pubkey.is_empty());
        assert!(parsed.endpoints.iter().all(|url| url.scheme() == "https"));
        assert!(!parsed.dangerous_insecure_transport_protocol);
        assert!(
            !parsed.dangerous_accept_invalid_certs && !parsed.dangerous_accept_invalid_hostnames
        );
        let macos = json(include_str!("../tauri.macos.conf.json"));
        assert_eq!(macos["bundle"]["createUpdaterArtifacts"], true);
    }

    /// The popup may check, download-and-install (verified) and restart;
    /// nothing broader such as `updater:default` or `process:allow-exit`.
    #[test]
    fn the_popup_gets_only_the_update_permissions_it_uses() {
        let caps = json(include_str!("../capabilities/default.json"));
        let mut granted: Vec<&str> = caps["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|p| p.starts_with("updater:") || p.starts_with("process:"))
            .collect();
        granted.sort();
        assert_eq!(
            granted,
            [
                "process:allow-restart",
                "updater:allow-check",
                "updater:allow-download-and-install"
            ]
        );
    }

    /// Checks `release-assets/` the way the updater will: the archive's
    /// signature against the public key in tauri.conf.json, and the version
    /// it was signed for against the one latest.json announces. A changed
    /// byte must fail. Run after `scripts/release-macos.sh`:
    /// `cargo test -- --ignored release_assets`.
    #[test]
    #[ignore]
    fn release_assets_verify() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use minisign_verify::{PublicKey, Signature};

        let decode = |b64: &str| String::from_utf8(STANDARD.decode(b64.trim()).unwrap()).unwrap();
        let conf = json(include_str!("../tauri.conf.json"));
        let pubkey = conf["plugins"]["updater"]["pubkey"].as_str().unwrap();
        let key = PublicKey::decode(&decode(pubkey)).unwrap();

        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../release-assets");
        let feed = json(&std::fs::read_to_string(assets.join("latest.json")).unwrap());
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(feed["version"], version);
        let platform = &feed["platforms"]["darwin-aarch64"];
        let archive = format!("MultiPath_{version}_aarch64.app.tar.gz");
        assert_eq!(
            platform["url"],
            format!("https://github.com/timowse/MultiPath/releases/download/v{version}/{archive}")
        );

        let signature =
            Signature::decode(&decode(platform["signature"].as_str().unwrap())).unwrap();
        let mut bytes = std::fs::read(assets.join(&archive)).unwrap();
        key.verify(&bytes, &signature, true).unwrap();
        assert!(signature
            .trusted_comment()
            .split('\t')
            .any(|field| field == format!("version:{version}")));

        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(key.verify(&bytes, &signature, true).is_err());
    }
}
