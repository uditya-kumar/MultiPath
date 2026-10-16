//! What the popup shows, gathered in one place and sent as JSON.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::Serialize;

use super::state::AppState;
use crate::engine::{TUN_V4, TUN_V6};
use crate::net::links::HealthState;
use crate::net::router::Proto;
use crate::sys;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "windows", "linux" or "macos", for the wording that differs.
    pub platform: &'static str,
    pub on: bool,
    pub busy: bool,
    pub elevated: bool,
    /// Whether turning on can ask for the permissions it needs: not from an
    /// AppImage on Linux.
    pub can_elevate: bool,
    pub error: Option<String>,
    pub ipv6: bool,
    /// Bytes per second across every network.
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub connections: usize,
    pub today_bytes: u64,
    pub links: Vec<LinkStatus>,
    pub settings: SettingsStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    /// "wifi", "ethernet", "phone", "cellular" or "other".
    pub kind: &'static str,
    pub address: Option<String>,
    pub gateway: Option<String>,
    /// Speed the adapter reports, bits per second.
    pub speed_bps: Option<u64>,
    pub latency_ms: Option<u32>,
    /// "up", "down" or "unknown".
    pub health: &'static str,
    /// Connected, with an address and a gateway.
    pub present: bool,
    /// The network the system uses on its own.
    pub is_default: bool,
    pub enabled: bool,
    pub weight: u32,
    pub rx_rate: u64,
    pub tx_rate: u64,
    /// Percent of the current traffic, for the share ring.
    pub share: u8,
    pub connections: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsStatus {
    pub adaptive: bool,
    pub sticky: bool,
    pub capture_ipv6: bool,
    pub dns: Vec<String>,
    pub start_on_login: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: u64,
    /// "tcp" or "udp".
    pub proto: &'static str,
    /// Executable name of the app, such as `chrome.exe`.
    pub app: Option<String>,
    pub host: Option<String>,
    pub destination: String,
    pub link_id: Option<String>,
    pub link_name: Option<String>,
    pub rx: u64,
    pub tx: u64,
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub seconds: u64,
}

/// Below this much download, in bytes/s, the ring shows connections instead.
const IDLE_RATE: u64 = 4 * 1024;

/// Each value's share of the total as whole percents that add up to 100
/// (largest remainder), or all zeros when there's nothing to share.
pub fn percents(values: &[u64]) -> Vec<u8> {
    let total: u64 = values.iter().sum();
    if total == 0 {
        return vec![0; values.len()];
    }
    let exact: Vec<f64> = values
        .iter()
        .map(|&v| v as f64 * 100.0 / total as f64)
        .collect();
    let mut out: Vec<u8> = exact.iter().map(|e| e.floor() as u8).collect();
    let mut left = 100 - out.iter().map(|&p| p as u32).sum::<u32>();
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    for i in order {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out
}

pub fn build(state: &AppState) -> Status {
    let settings = state.settings();
    let default_id = state.links.default_link().map(|l| l.id.clone());
    let mut links: Vec<LinkStatus> = state
        .links
        .all()
        .iter()
        .map(|link| {
            let info = link.info();
            let prefs = settings.link(&link.id);
            let (health, latency_ms) = {
                let h = link.health.lock().unwrap();
                (h.state, h.latency_ms)
            };
            let (rx_rate, tx_rate) = {
                let r = link.rates.lock().unwrap();
                (r.rx_rate, r.tx_rate)
            };
            LinkStatus {
                id: link.id.clone(),
                name: prefs.label.clone().unwrap_or(info.name),
                description: info.description,
                kind: info.kind,
                address: info
                    .ipv4
                    .map(|a| a.to_string())
                    .or(info.ipv6.map(|a| a.to_string())),
                gateway: info.gateway,
                speed_bps: info.speed_bps,
                latency_ms,
                health: match health {
                    HealthState::Up => "up",
                    HealthState::Down => "down",
                    HealthState::Unknown => "unknown",
                },
                present: info.present,
                is_default: default_id.as_deref() == Some(link.id.as_str()),
                enabled: prefs.enabled,
                weight: prefs.weight,
                rx_rate,
                tx_rate,
                share: 0,
                connections: link.stats.tcp_active.load(Ordering::Relaxed)
                    + link.stats.udp_active.load(Ordering::Relaxed),
            }
        })
        .collect();

    // The ring shows where the downloads go, matching the rates beside it;
    // with next to nothing flowing, where the connections are.
    let mut basis: Vec<u64> = links.iter().map(|l| l.rx_rate).collect();
    if basis.iter().sum::<u64>() < IDLE_RATE {
        basis = links.iter().map(|l| l.connections as u64).collect();
    }
    for (link, share) in links.iter_mut().zip(percents(&basis)) {
        link.share = share;
    }
    // The default network first, then connected ones, as in the design.
    links.sort_by_key(|l| (!l.is_default, !l.present));

    let flows = state.flows.read().unwrap().clone();
    Status {
        platform: std::env::consts::OS,
        on: flows.is_some(),
        busy: state.busy.load(Ordering::Relaxed),
        elevated: sys::is_elevated(),
        can_elevate: sys::can_elevate(),
        error: state.error.lock().unwrap().clone(),
        ipv6: flows.is_some() && settings.capture_ipv6,
        rx_rate: links.iter().map(|l| l.rx_rate).sum(),
        tx_rate: links.iter().map(|l| l.tx_rate).sum(),
        connections: flows.map_or(0, |f| f.len()),
        today_bytes: state.usage.lock().unwrap().today(sys::local_date()),
        links,
        settings: SettingsStatus {
            adaptive: settings.adaptive,
            sticky: settings.sticky,
            capture_ipv6: settings.capture_ipv6,
            dns: settings.dns.iter().map(|d| d.to_string()).collect(),
            start_on_login: state.start_on_login.load(Ordering::Relaxed),
        },
    }
}

/// The text shown when hovering over the tray icon.
pub fn tooltip(status: &Status) -> String {
    if !status.on {
        return "MultiPath — off".into();
    }
    let combining = status
        .links
        .iter()
        .filter(|l| l.present && l.enabled && l.health != "down")
        .count();
    match combining {
        1 => "MultiPath — on, 1 network".into(),
        n => format!("MultiPath — combining {n} networks"),
    }
}

/// Every flow being relayed, busiest first.
pub fn connections(state: &AppState) -> Vec<Connection> {
    let Some(flows) = state.flows.read().unwrap().clone() else {
        return Vec::new();
    };
    let flows = flows.snapshot();
    let owners = sys::socket_owners(TUN_V4, TUN_V6);
    let mut names: HashMap<u32, Option<String>> = HashMap::new();
    let link_names: HashMap<String, String> = build_link_names(state);

    let now = Instant::now();
    let mut counts = state.flow_counts.lock().unwrap();
    let elapsed = counts
        .at
        .map(|at| now.duration_since(at).as_secs_f64())
        .filter(|&s| s > 0.0);
    let mut seen = HashMap::with_capacity(flows.len());
    let mut list: Vec<Connection> = flows
        .iter()
        .map(|flow| {
            let rx = flow.rx.load(Ordering::Relaxed);
            let tx = flow.tx.load(Ordering::Relaxed);
            seen.insert(flow.id, (rx, tx));
            let rate = |now: u64, before: u64| match elapsed {
                Some(secs) => (now.saturating_sub(before) as f64 / secs) as u64,
                None => 0,
            };
            let (rx_before, tx_before) = counts.bytes.get(&flow.id).copied().unwrap_or((rx, tx));
            let tcp = flow.proto == Proto::Tcp;
            let app = owners.get(&(tcp, flow.src.port())).and_then(|&pid| {
                names
                    .entry(pid)
                    .or_insert_with(|| sys::process_name(pid))
                    .clone()
            });
            let link_id = flow.link.get().cloned();
            Connection {
                id: flow.id,
                proto: if tcp { "tcp" } else { "udp" },
                app,
                host: flow.host.get().cloned(),
                destination: flow.dst.to_string(),
                link_name: link_id.as_ref().and_then(|id| link_names.get(id).cloned()),
                link_id,
                rx,
                tx,
                rx_rate: rate(rx, rx_before),
                tx_rate: rate(tx, tx_before),
                seconds: now.duration_since(flow.started).as_secs(),
            }
        })
        .collect();
    counts.at = Some(now);
    counts.bytes = seen;
    list.sort_by_key(|c| std::cmp::Reverse((c.rx_rate + c.tx_rate, c.rx + c.tx)));
    list
}

fn build_link_names(state: &AppState) -> HashMap<String, String> {
    let settings = state.settings();
    state
        .links
        .all()
        .iter()
        .map(|l| {
            let name = settings.link(&l.id).label.unwrap_or_else(|| l.info().name);
            (l.id.clone(), name)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percents_add_up_to_100() {
        assert_eq!(percents(&[296, 182]), vec![62, 38]);
        assert_eq!(percents(&[1, 1, 1]), vec![34, 33, 33]);
        assert_eq!(percents(&[5, 0]), vec![100, 0]);
        assert_eq!(percents(&[0, 0]), vec![0, 0]);
        assert_eq!(percents(&[]), Vec::<u8>::new());
    }
}
