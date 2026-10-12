//! Keeps checking that every network can actually reach the internet, so a
//! Wi-Fi with no backhaul or an unplugged phone drops out of rotation.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::dial;
use super::links::{Link, LinkInfo, Links};

pub const PROBE_INTERVAL: Duration = Duration::from_secs(5);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Anycast HTTPS endpoints. A TCP handshake is enough: it proves the path
/// works without sending anything.
const TARGETS_V4: [&str; 2] = ["1.1.1.1:443", "8.8.8.8:443"];
const TARGETS_V6: [&str; 2] = ["[2606:4700:4700::1111]:443", "[2001:4860:4860::8888]:443"];

/// Round-trip time of a TCP handshake through `link`, or `None` if no target
/// answered. Uses IPv4 when the link has it, as almost every flow does.
pub async fn probe(link: &Link) -> Option<u32> {
    let info = link.info();
    if info.ipv4.is_some() {
        reach(&info, TARGETS_V4).await
    } else {
        reach(&info, TARGETS_V6).await
    }
}

/// Whether IPv6 works through `link`, probed on its own because it can
/// fail while IPv4 is fine.
async fn probe_ipv6(link: &Link, rtt: Option<u32>) -> bool {
    let info = link.info();
    match (info.ipv4, info.ipv6) {
        (_, None) => false,
        (None, Some(_)) => rtt.is_some(),
        (Some(_), Some(_)) => reach(&info, TARGETS_V6).await.is_some(),
    }
}

async fn reach(info: &LinkInfo, targets: [&str; 2]) -> Option<u32> {
    for target in targets {
        let dest: SocketAddr = target.parse().expect("valid probe target");
        let start = Instant::now();
        if dial::tcp(info, dest, PROBE_TIMEOUT).await.is_ok() {
            return Some(start.elapsed().as_millis().min(u32::MAX as u128) as u32);
        }
    }
    None
}

/// Probes every present link, side by side, until `stop` turns true.
pub async fn run(links: Arc<Links>, mut stop: watch::Receiver<bool>) {
    loop {
        let mut probes = JoinSet::new();
        for link in links.present() {
            probes.spawn(async move {
                let rtt = probe(&link).await;
                match rtt {
                    Some(ms) => link.record_success(Some(ms)),
                    None => link.record_failure(),
                }
                link.set_ipv6_ok(probe_ipv6(&link, rtt).await);
            });
        }
        while probes.join_next().await.is_some() {}

        tokio::select! {
            _ = tokio::time::sleep(PROBE_INTERVAL) => {}
            // A dropped sender means the engine is gone.
            changed = stop.changed() => if changed.is_err() { return },
        }
        if *stop.borrow() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Probes every network on this machine through a pinned socket. Run
    /// with `cargo test live_probe -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_probe() {
        let links = Links::default();
        links.refresh();
        for link in links.present() {
            let rtt = probe(&link).await;
            let info = link.info();
            println!(
                "{} (if {}, {:?}): {rtt:?} ms",
                info.name, info.if_index, info.ipv4
            );
            assert!(rtt.is_some(), "{} could not reach the internet", link.id);
        }
    }
}
