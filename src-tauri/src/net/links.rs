//! The physical networks MultiPath can send traffic through.

use std::collections::VecDeque;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use netdev::interface::state::OperState;
use netdev::interface::types::InterfaceType;

use crate::engine::TUN_NAME;

/// Seconds of throughput history kept for the charts.
pub const HISTORY_LEN: usize = 60;

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub if_index: u32,
    pub name: String,
    pub description: String,
    pub kind: &'static str,
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Addr>,
    pub gateway: Option<String>,
    pub speed_bps: Option<u64>,
    /// Connected, with an address and a default gateway.
    pub present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthState {
    Unknown,
    Up,
    Down,
}

#[derive(Debug)]
pub struct Health {
    pub state: HealthState,
    pub latency_ms: Option<u32>,
    pub consecutive_failures: u32,
}

#[derive(Default)]
pub struct LinkStats {
    pub rx: AtomicU64,
    pub tx: AtomicU64,
    pub tcp_active: AtomicU32,
    pub udp_active: AtomicU32,
    pub opened: AtomicU64,
    pub failed: AtomicU64,
    /// Highest recent download rate in bytes/s, decaying slowly. Drives
    /// adaptive weighting.
    pub peak_rx: AtomicU64,
}

pub struct Rates {
    pub last_rx: u64,
    pub last_tx: u64,
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub history: VecDeque<(u64, u64)>,
}

pub struct Link {
    pub id: String,
    pub info: RwLock<LinkInfo>,
    pub stats: LinkStats,
    pub health: Mutex<Health>,
    pub rates: Mutex<Rates>,
}

impl Link {
    pub fn new(id: String, info: LinkInfo) -> Link {
        Link {
            id,
            info: RwLock::new(info),
            stats: LinkStats::default(),
            health: Mutex::new(Health {
                state: HealthState::Unknown,
                latency_ms: None,
                consecutive_failures: 0,
            }),
            rates: Mutex::new(Rates {
                last_rx: 0,
                last_tx: 0,
                rx_rate: 0,
                tx_rate: 0,
                history: VecDeque::from(vec![(0, 0); HISTORY_LEN]),
            }),
        }
    }

    pub fn info(&self) -> LinkInfo {
        self.info.read().unwrap().clone()
    }

    pub fn usable(&self) -> bool {
        self.info.read().unwrap().present && self.health.lock().unwrap().state != HealthState::Down
    }

    pub fn record_success(&self, latency_ms: Option<u32>) {
        let mut h = self.health.lock().unwrap();
        h.state = HealthState::Up;
        h.consecutive_failures = 0;
        if latency_ms.is_some() {
            h.latency_ms = latency_ms;
        }
    }

    /// Two failures in a row take a network out of rotation until it answers again.
    pub fn record_failure(&self) {
        let mut h = self.health.lock().unwrap();
        h.consecutive_failures += 1;
        if h.consecutive_failures >= 2 {
            h.state = HealthState::Down;
            h.latency_ms = None;
        }
    }
}

fn kind_of(itf: &netdev::Interface) -> &'static str {
    let desc = itf
        .description
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let tether = [
        "remote ndis",
        "rndis",
        "apple mobile device",
        "iphone",
        "android",
        "usb ethernet/rndis",
    ];
    if tether.iter().any(|t| desc.contains(t)) {
        return "phone";
    }
    match itf.if_type {
        InterfaceType::Wireless80211 => "wifi",
        InterfaceType::Wwanpp | InterfaceType::Wwanpp2 | InterfaceType::Wman => "cellular",
        InterfaceType::Ethernet
        | InterfaceType::GigabitEthernet
        | InterfaceType::FastEthernetT
        | InterfaceType::FastEthernetFx => "ethernet",
        _ => "other",
    }
}

fn is_global_v6(ip: &Ipv6Addr) -> bool {
    // 2000::/3 is the global unicast range.
    (ip.segments()[0] & 0xe000) == 0x2000
}

/// Interfaces that could reach the internet: up, addressed, with a gateway.
fn discover() -> Vec<(String, LinkInfo)> {
    netdev::get_interfaces()
        .into_iter()
        .filter(|i| !i.is_loopback() && i.friendly_name.as_deref() != Some(TUN_NAME))
        .filter_map(|i| {
            let gw = i.gateway.as_ref()?;
            let gw_v4 = gw.ipv4.iter().find(|g| !g.is_unspecified()).copied();
            let gw_v6 = gw.ipv6.iter().find(|g| !g.is_unspecified()).copied();
            if gw_v4.is_none() && gw_v6.is_none() {
                return None;
            }
            let ipv4 = i
                .ipv4
                .iter()
                .map(|n| n.addr())
                .find(|a| !a.is_link_local() && !a.is_unspecified());
            let ipv6 = i.ipv6.iter().map(|n| n.addr()).find(is_global_v6);
            let up = i.oper_state == OperState::Up;
            let info = LinkInfo {
                if_index: i.index,
                name: i.friendly_name.clone().unwrap_or_else(|| i.name.clone()),
                description: i.description.clone().unwrap_or_default(),
                kind: kind_of(&i),
                ipv4: gw_v4.and(ipv4),
                ipv6: gw_v6.and(ipv6),
                gateway: gw_v4
                    .map(|g| g.to_string())
                    .or(gw_v6.map(|g| g.to_string())),
                speed_bps: i.receive_speed.or(i.transmit_speed),
                present: up && (ipv4.is_some() || ipv6.is_some()),
            };
            Some((i.name, info))
        })
        .filter(|(_, info)| info.present)
        .collect()
}

#[derive(Default)]
pub struct Links {
    list: RwLock<Vec<Arc<Link>>>,
}

impl Links {
    pub fn all(&self) -> Vec<Arc<Link>> {
        self.list.read().unwrap().clone()
    }

    pub fn present(&self) -> Vec<Arc<Link>> {
        self.all()
            .into_iter()
            .filter(|l| l.info.read().unwrap().present)
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<Link>> {
        self.all().into_iter().find(|l| l.id == id)
    }

    /// Re-reads the adapters. Links keep their identity, and their counters,
    /// across address changes, unplugging, and plugging back in.
    /// Returns true when something changed.
    pub fn refresh(&self) -> bool {
        let found = discover();
        let mut list = self.list.write().unwrap();
        let mut changed = false;

        for link in list.iter() {
            let mut info = link.info.write().unwrap();
            match found.iter().find(|(id, _)| *id == link.id) {
                Some((_, fresh)) => {
                    let addr_changed =
                        info.ipv4 != fresh.ipv4 || info.ipv6 != fresh.ipv6 || !info.present;
                    if addr_changed {
                        // A new address deserves a fresh health verdict.
                        *link.health.lock().unwrap() = Health {
                            state: HealthState::Unknown,
                            latency_ms: None,
                            consecutive_failures: 0,
                        };
                        changed = true;
                    }
                    *info = fresh.clone();
                }
                None => {
                    if info.present {
                        info.present = false;
                        changed = true;
                    }
                }
            }
        }

        for (id, info) in found {
            if !list.iter().any(|l| l.id == id) {
                list.push(Arc::new(Link::new(id, info)));
                changed = true;
            }
        }
        changed
    }

    /// Called once a second: turns byte counters into rates.
    pub fn tick(&self, _now: Instant) {
        for link in self.all() {
            let rx = link.stats.rx.load(Ordering::Relaxed);
            let tx = link.stats.tx.load(Ordering::Relaxed);
            let mut r = link.rates.lock().unwrap();
            r.rx_rate = rx.saturating_sub(r.last_rx);
            r.tx_rate = tx.saturating_sub(r.last_tx);
            r.last_rx = rx;
            r.last_tx = tx;
            let sample = (r.rx_rate, r.tx_rate);
            r.history.push_back(sample);
            while r.history.len() > HISTORY_LEN {
                r.history.pop_front();
            }
            // Decays to about half in 25 s, so capacity estimates follow
            // a network that has slowed down.
            let peak = link.stats.peak_rx.load(Ordering::Relaxed);
            let decayed = peak - peak / 36;
            link.stats
                .peak_rx
                .store(decayed.max(r.rx_rate), Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    pub fn from_list(list: Vec<Arc<Link>>) -> Links {
        Links {
            list: RwLock::new(list),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints what discovery sees on this machine. Run with
    /// `cargo test discovery -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn discovery_on_this_machine() {
        for (id, info) in discover() {
            println!("{id}\n  {info:?}");
        }
    }

    #[test]
    fn two_failures_take_a_link_down_and_success_restores_it() {
        let link = Link::new(
            "x".into(),
            LinkInfo {
                if_index: 1,
                name: "x".into(),
                description: String::new(),
                kind: "wifi",
                ipv4: Some(Ipv4Addr::new(192, 168, 1, 2)),
                ipv6: None,
                gateway: None,
                speed_bps: None,
                present: true,
            },
        );
        link.record_failure();
        assert!(link.usable());
        link.record_failure();
        assert!(!link.usable());
        link.record_success(Some(20));
        assert!(link.usable());
    }
}
