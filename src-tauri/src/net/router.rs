//! Turns "open a connection to X" into a socket on the right network, with
//! one retry on another network if the first one fails.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::net::{TcpStream, UdpSocket};

use super::balancer::{self, Candidate, Policy, Sticky};
use super::dial;
use super::links::{Link, Links, Meter};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The first try plus one failover.
const ATTEMPTS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proto {
    Tcp,
    Udp,
}

/// A flow counted against its link for as long as this is alive.
pub struct Lease {
    pub link: Arc<Link>,
    proto: Proto,
    meter: Arc<Meter>,
}

impl Lease {
    fn new(link: Arc<Link>, proto: Proto) -> Lease {
        counter(&link, proto).fetch_add(1, Ordering::Relaxed);
        link.stats.opened.fetch_add(1, Ordering::Relaxed);
        let meter = link.meter();
        Lease { link, proto, meter }
    }

    pub fn count_rx(&self, n: u64) {
        self.link.stats.rx.fetch_add(n, Ordering::Relaxed);
        self.meter.add_rx(n);
    }

    pub fn count_tx(&self, n: u64) {
        self.link.stats.tx.fetch_add(n, Ordering::Relaxed);
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        counter(&self.link, self.proto).fetch_sub(1, Ordering::Relaxed);
    }
}

fn counter(link: &Link, proto: Proto) -> &AtomicU32 {
    match proto {
        Proto::Tcp => &link.stats.tcp_active,
        Proto::Udp => &link.stats.udp_active,
    }
}

pub struct Router {
    links: Arc<Links>,
    policy: RwLock<Policy>,
    sticky: Sticky,
}

impl Router {
    pub fn new(links: Arc<Links>, policy: Policy) -> Router {
        Router {
            links,
            policy: RwLock::new(policy),
            sticky: Sticky::default(),
        }
    }

    pub fn set_policy(&self, policy: Policy) {
        let was_sticky = self.policy.read().unwrap().sticky;
        if was_sticky && !policy.sticky {
            self.sticky.clear();
        }
        *self.policy.write().unwrap() = policy;
    }

    /// The best link for a new flow, skipping `exclude`. Sticky keys pin a
    /// site to the link it used before, while that link is still eligible.
    pub fn choose(
        &self,
        v6: bool,
        sticky_key: Option<&str>,
        exclude: &[&str],
    ) -> Option<Arc<Link>> {
        let policy = self.policy.read().unwrap();
        if v6 && policy.block_ipv6 {
            return None;
        }
        let eligible: Vec<(Arc<Link>, u32)> = self
            .links
            .all()
            .into_iter()
            .filter(|l| !exclude.contains(&l.id.as_str()))
            .filter(|l| l.usable())
            .filter(|l| {
                let info = l.info.read().unwrap();
                if v6 {
                    info.ipv6.is_some()
                } else {
                    info.ipv4.is_some()
                }
            })
            .filter_map(|l| {
                let p = policy.link(&l.id);
                p.enabled.then_some((l, p.weight))
            })
            .collect();

        let key = sticky_key.filter(|_| policy.sticky);
        if let Some(key) = key {
            if let Some(id) = self.sticky.get(key) {
                if let Some((link, _)) = eligible.iter().find(|(l, _)| l.id == id) {
                    return Some(link.clone());
                }
            }
        }

        let candidates: Vec<Candidate> = eligible
            .iter()
            .map(|(l, weight)| Candidate {
                id: &l.id,
                weight: *weight,
                busy: l.busy(),
                peak_rx: l.stats.peak_rx.load(Ordering::Relaxed),
            })
            .collect();
        let id = balancer::pick(&candidates, policy.adaptive)?;
        let link = eligible.iter().find(|(l, _)| l.id == id)?.0.clone();
        if let Some(key) = key {
            self.sticky.set(key, &link.id);
        }
        Some(link)
    }

    pub async fn connect_tcp(
        &self,
        dest: SocketAddr,
        sticky_key: Option<&str>,
    ) -> io::Result<(Lease, TcpStream)> {
        let mut failed: Vec<String> = Vec::new();
        let mut last_err = no_link();
        for _ in 0..ATTEMPTS {
            let exclude: Vec<&str> = failed.iter().map(String::as_str).collect();
            let Some(link) = self.choose(dest.is_ipv6(), sticky_key, &exclude) else {
                break;
            };
            let lease = Lease::new(link.clone(), Proto::Tcp);
            match dial::tcp(&link.info(), dest, CONNECT_TIMEOUT).await {
                Ok(stream) => return Ok((lease, stream)),
                Err(err) => {
                    link.stats.failed.fetch_add(1, Ordering::Relaxed);
                    log::debug!("{dest} via {}: {err}", link.id);
                    last_err = err;
                    failed.push(link.id.clone());
                }
            }
        }
        Err(last_err)
    }

    pub fn bind_udp(&self, v6: bool, sticky_key: Option<&str>) -> io::Result<(Lease, UdpSocket)> {
        let mut failed: Vec<String> = Vec::new();
        let mut last_err = no_link();
        for _ in 0..ATTEMPTS {
            let exclude: Vec<&str> = failed.iter().map(String::as_str).collect();
            let Some(link) = self.choose(v6, sticky_key, &exclude) else {
                break;
            };
            match dial::udp(&link.info(), v6) {
                Ok(socket) => return Ok((Lease::new(link, Proto::Udp), socket)),
                Err(err) => {
                    link.stats.failed.fetch_add(1, Ordering::Relaxed);
                    last_err = err;
                    failed.push(link.id.clone());
                }
            }
        }
        Err(last_err)
    }
}

fn no_link() -> io::Error {
    io::Error::new(io::ErrorKind::NetworkUnreachable, "no network is available")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::balancer::LinkPolicy;
    use crate::net::links::LinkInfo;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn link(id: &str, v4: bool, v6: bool) -> Arc<Link> {
        Arc::new(Link::new(
            id.into(),
            LinkInfo {
                if_index: 1,
                name: id.into(),
                description: String::new(),
                kind: "wifi",
                ipv4: v4.then_some(Ipv4Addr::new(192, 168, 1, 2)),
                ipv6: v6.then_some(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                gateway: None,
                speed_bps: None,
                present: true,
            },
        ))
    }

    fn router(links: Vec<Arc<Link>>, policy: Policy) -> Router {
        Router::new(Arc::new(Links::from_list(links)), policy)
    }

    fn chosen(router: &Router, key: Option<&str>) -> Option<String> {
        router.choose(false, key, &[]).map(|l| l.id.clone())
    }

    #[test]
    fn spreads_flows_by_weight() {
        let mut policy = Policy::default();
        policy.links.insert(
            "a".into(),
            LinkPolicy {
                enabled: true,
                weight: 6,
            },
        );
        policy.links.insert(
            "b".into(),
            LinkPolicy {
                enabled: true,
                weight: 3,
            },
        );
        let r = router(vec![link("a", true, false), link("b", true, false)], policy);
        let mut leases = Vec::new();
        for _ in 0..9 {
            leases.push(Lease::new(r.choose(false, None, &[]).unwrap(), Proto::Tcp));
        }
        let on_a = leases.iter().filter(|l| l.link.id == "a").count();
        assert_eq!(on_a, 6);
    }

    #[test]
    fn leases_release_their_slot() {
        let a = link("a", true, false);
        let lease = Lease::new(a.clone(), Proto::Udp);
        assert_eq!(a.stats.udp_active.load(Ordering::Relaxed), 1);
        drop(lease);
        assert_eq!(a.stats.udp_active.load(Ordering::Relaxed), 0);
        assert_eq!(a.stats.opened.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn skips_disabled_and_down_links() {
        let mut policy = Policy::default();
        policy.links.insert(
            "off".into(),
            LinkPolicy {
                enabled: false,
                weight: 10,
            },
        );
        let down = link("down", true, false);
        down.record_failure();
        down.record_failure();
        let r = router(
            vec![link("off", true, false), down, link("ok", true, false)],
            policy,
        );
        for _ in 0..5 {
            assert_eq!(chosen(&r, None).as_deref(), Some("ok"));
        }
    }

    #[test]
    fn nothing_usable_means_no_link() {
        let down = link("down", true, false);
        down.record_failure();
        down.record_failure();
        let r = router(vec![down], Policy::default());
        assert_eq!(chosen(&r, None), None);
    }

    #[test]
    fn needs_an_address_in_the_destination_family() {
        let r = router(
            vec![link("v4", true, false), link("v6", false, true)],
            Policy::default(),
        );
        assert_eq!(r.choose(true, None, &[]).unwrap().id, "v6");
        assert_eq!(r.choose(false, None, &[]).unwrap().id, "v4");
    }

    #[test]
    fn blocked_ipv6_has_no_link() {
        let policy = Policy {
            block_ipv6: true,
            ..Policy::default()
        };
        let r = router(vec![link("dual", true, true)], policy);
        assert!(r.choose(true, None, &[]).is_none());
        assert_eq!(r.choose(false, None, &[]).unwrap().id, "dual");
    }

    #[test]
    fn exclude_drives_failover() {
        let r = router(
            vec![link("a", true, false), link("b", true, false)],
            Policy::default(),
        );
        assert_eq!(r.choose(false, None, &["a"]).unwrap().id, "b");
        assert!(r.choose(false, None, &["a", "b"]).is_none());
    }

    #[test]
    fn sticky_keeps_a_site_on_one_link() {
        let policy = Policy {
            sticky: true,
            ..Policy::default()
        };
        let r = router(vec![link("a", true, false), link("b", true, false)], policy);
        let first = chosen(&r, Some("bank.example")).unwrap();
        let _busy = Lease::new(r.links.get(&first).unwrap(), Proto::Tcp);
        for _ in 0..5 {
            assert_eq!(
                chosen(&r, Some("bank.example")).as_deref(),
                Some(first.as_str())
            );
        }
        // Without a key the busier link is avoided as usual.
        assert_ne!(chosen(&r, None).as_deref(), Some(first.as_str()));
    }

    #[test]
    fn sticky_moves_when_its_link_goes_away() {
        let policy = Policy {
            sticky: true,
            ..Policy::default()
        };
        let r = router(vec![link("a", true, false), link("b", true, false)], policy);
        let first = chosen(&r, Some("site")).unwrap();
        let gone = r.links.get(&first).unwrap();
        gone.record_failure();
        gone.record_failure();
        let second = chosen(&r, Some("site")).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn sticky_off_ignores_keys() {
        let r = router(
            vec![link("a", true, false), link("b", true, false)],
            Policy::default(),
        );
        let first = r.choose(false, Some("site"), &[]).unwrap();
        let _busy = Lease::new(first.clone(), Proto::Tcp);
        assert_ne!(r.choose(false, Some("site"), &[]).unwrap().id, first.id);
    }
}
