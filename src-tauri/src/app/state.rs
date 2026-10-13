//! What the commands, the tray and the ticker share: the networks, the
//! settings, and the engine while it runs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::settings::Settings;
use super::usage::Usage;
use crate::engine::{Engine, EngineConfig, Flows};
use crate::net::links::Links;
use crate::net::router::Router;

/// Long enough for the relays to wind down; the routes go first regardless.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Byte counts of each flow when the connections list was last read, to
/// turn them into rates.
#[derive(Default)]
pub struct FlowCounts {
    pub at: Option<Instant>,
    pub bytes: HashMap<u64, (u64, u64)>,
}

pub struct AppState {
    pub links: Arc<Links>,
    pub router: Arc<Router>,
    pub settings: Mutex<Settings>,
    settings_path: PathBuf,
    pub usage: Mutex<Usage>,
    usage_path: PathBuf,
    engine: tokio::sync::Mutex<Option<Engine>>,
    /// The running engine's flows, readable without waiting on the engine lock.
    pub flows: RwLock<Option<Arc<Flows>>>,
    /// Turning on or off is under way.
    pub busy: AtomicBool,
    /// Why MultiPath last failed to turn on, or stopped by itself.
    pub error: Mutex<Option<String>>,
    pub start_on_login: AtomicBool,
    pub flow_counts: Mutex<FlowCounts>,
    /// Keeps the health probes running for as long as the app does.
    health_stop: watch::Sender<bool>,
}

impl AppState {
    pub fn new(config_dir: PathBuf, start_on_login: bool) -> AppState {
        let settings_path = config_dir.join("settings.json");
        let usage_path = config_dir.join("usage.json");
        let settings = Settings::load(&settings_path);
        let links = Arc::new(Links::default());
        links.refresh();
        let router = Arc::new(Router::new(links.clone(), settings.policy()));
        AppState {
            links,
            router,
            settings: Mutex::new(settings),
            settings_path,
            usage: Mutex::new(Usage::load(&usage_path)),
            usage_path,
            engine: tokio::sync::Mutex::new(None),
            flows: RwLock::new(None),
            busy: AtomicBool::new(false),
            error: Mutex::new(None),
            start_on_login: AtomicBool::new(start_on_login),
            flow_counts: Mutex::new(FlowCounts::default()),
            health_stop: watch::channel(false).0,
        }
    }

    pub fn health_stop(&self) -> watch::Receiver<bool> {
        self.health_stop.subscribe()
    }

    pub fn is_on(&self) -> bool {
        self.flows.read().unwrap().is_some()
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    /// Changes the settings, saves them, and hands the balancer its new policy.
    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings.lock().unwrap();
        change(&mut settings);
        settings.save(&self.settings_path);
        self.router.set_policy(settings.policy());
    }

    pub fn save_usage(&self) {
        self.usage.lock().unwrap().save(&self.usage_path);
    }

    /// Starts capturing. Needs administrator rights.
    pub async fn turn_on(&self) -> Result<(), String> {
        let mut engine = self.engine.lock().await;
        if engine.is_some() {
            return Ok(());
        }
        self.busy.store(true, Ordering::Relaxed);
        let settings = self.settings();
        let config = EngineConfig {
            dns: settings.dns.clone(),
        };
        let started = Engine::start(config, self.links.clone(), self.router.clone()).await;
        self.busy.store(false, Ordering::Relaxed);
        match started {
            Ok(started) => {
                *self.flows.write().unwrap() = Some(started.flows.clone());
                *self.error.lock().unwrap() = None;
                *engine = Some(started);
                drop(engine);
                self.update_settings(|s| s.on = true);
                Ok(())
            }
            Err(err) => {
                log::error!("could not start the engine: {err}");
                let message = format!("Couldn't turn on: {err}");
                *self.error.lock().unwrap() = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Stops capturing. `remember` records that the user turned it off, so
    /// it stays off at the next start.
    pub async fn turn_off(&self, remember: bool) {
        let mut engine = self.engine.lock().await;
        if let Some(running) = engine.take() {
            self.busy.store(true, Ordering::Relaxed);
            *self.flows.write().unwrap() = None;
            if tokio::time::timeout(STOP_TIMEOUT, running.stop())
                .await
                .is_err()
            {
                log::warn!("the engine took too long to stop");
            }
            self.busy.store(false, Ordering::Relaxed);
        }
        drop(engine);
        if remember {
            self.update_settings(|s| s.on = false);
        }
        *self.flow_counts.lock().unwrap() = FlowCounts::default();
    }

    /// Applies settings the engine only reads at start (DNS).
    pub async fn restart(&self) -> Result<(), String> {
        if !self.is_on() {
            return Ok(());
        }
        self.turn_off(false).await;
        self.turn_on().await
    }

    /// Cleans up after the packet stack stopped by itself, which would
    /// otherwise leave the PC without internet. Returns true if it had.
    pub async fn check_engine(&self) -> bool {
        let stalled = match self.engine.try_lock() {
            Ok(engine) => engine.as_ref().is_some_and(|e| !e.is_running()),
            // Turning on or off right now.
            Err(_) => false,
        };
        if stalled {
            self.turn_off(false).await;
            *self.error.lock().unwrap() = Some("MultiPath stopped unexpectedly".into());
        }
        stalled
    }
}
