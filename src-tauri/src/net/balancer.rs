//! Chooses the network for each new flow. Pure logic, no I/O, so the
//! behaviour can be tested exactly.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// What the user decided for one link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkPolicy {
    pub enabled: bool,
    /// Relative share of new flows, 1 to 10.
    pub weight: u32,
}

impl Default for LinkPolicy {
    fn default() -> Self {
        LinkPolicy {
            enabled: true,
            weight: 5,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Policy {
    /// Keyed by link id. Links not listed use `LinkPolicy::default()`.
    pub links: HashMap<String, LinkPolicy>,
    /// Keep every flow to one site on the link it first used.
    pub sticky: bool,
    /// Scale each link's share by the throughput it has delivered.
    pub adaptive: bool,
    /// Refuse IPv6 flows, so apps fall back to IPv4.
    pub block_ipv6: bool,
}

impl Policy {
    pub fn link(&self, id: &str) -> LinkPolicy {
        self.links.get(id).copied().unwrap_or_default()
    }
}

/// One link that could take the next flow.
#[derive(Clone, Debug)]
pub struct Candidate<'a> {
    pub id: &'a str,
    pub weight: u32,
    /// Flows moving data on the link right now, or just opened. Idle
    /// connections (keep-alives, push channels) don't count: they take no
    /// bandwidth, and counting them would hide where the real load is.
    pub busy: u32,
    /// Recent peak download rate, bytes/s.
    pub peak_rx: u64,
}

/// Below this a link hasn't carried enough traffic for its peak to say
/// anything about its capacity.
const MIN_PEAK_FOR_ADAPTIVE: u64 = 256 * 1024;

/// Each link's estimated capacity, in bytes/s. A link's peak only shows
/// what it carried, not what it could carry, so a link that is idle or
/// barely measured is assumed to be at least as fast as the average.
/// Otherwise a link that starts out with few flows would never get the
/// chance to show its speed.
pub fn capacities(candidates: &[Candidate]) -> Vec<f64> {
    let known: Vec<f64> = candidates
        .iter()
        .filter(|c| c.peak_rx >= MIN_PEAK_FOR_ADAPTIVE)
        .map(|c| c.peak_rx as f64)
        .collect();
    if known.is_empty() {
        return vec![1.0; candidates.len()];
    }
    let mean = known.iter().sum::<f64>() / known.len() as f64;
    candidates
        .iter()
        .map(|c| {
            let peak = c.peak_rx as f64;
            if c.peak_rx < MIN_PEAK_FOR_ADAPTIVE || c.busy == 0 {
                peak.max(mean)
            } else {
                peak
            }
        })
        .collect()
}

/// The link where one more flow can expect the most bandwidth:
/// `weight × capacity / (busy + 1)`. Without adaptive weighting every
/// capacity counts as equal, which makes this weighted least-busy.
///
/// Giving a link more flows lowers its score, so busy flows settle in
/// proportion to `weight × capacity` instead of piling onto one link.
pub fn pick<'a>(candidates: &[Candidate<'a>], adaptive: bool) -> Option<&'a str> {
    let capacity = if adaptive {
        capacities(candidates)
    } else {
        vec![1.0; candidates.len()]
    };
    candidates
        .iter()
        .zip(capacity)
        .map(|(c, capacity)| {
            let share = c.weight.max(1) as f64 * capacity;
            (share / (c.busy as f64 + 1.0), share, c.id)
        })
        // Most room first; on a tie, the bigger share; then a stable order.
        .max_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(b.2.cmp(a.2))
        })
        .map(|(_, _, id)| id)
}

/// How long a site stays on its link after its last flow opened.
const STICKY_TTL: Duration = Duration::from_secs(10 * 60);
const STICKY_MAX_ENTRIES: usize = 4096;

/// Remembers which link each site was sent to, for sticky sessions.
#[derive(Default)]
pub struct Sticky {
    entries: Mutex<HashMap<String, (String, Instant)>>,
}

impl Sticky {
    /// The link `key` used last, if it's still fresh.
    pub fn get(&self, key: &str) -> Option<String> {
        let entries = self.entries.lock().unwrap();
        entries
            .get(key)
            .filter(|(_, at)| at.elapsed() < STICKY_TTL)
            .map(|(id, _)| id.clone())
    }

    pub fn set(&self, key: &str, link_id: &str) {
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= STICKY_MAX_ENTRIES {
            entries.retain(|_, (_, at)| at.elapsed() < STICKY_TTL);
        }
        entries.insert(key.to_owned(), (link_id.to_owned(), Instant::now()));
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn candidate(id: &str, weight: u32) -> Candidate<'_> {
        Candidate {
            id,
            weight,
            busy: 0,
            peak_rx: 0,
        }
    }

    /// Opens `n` busy flows and counts where each one went.
    fn distribute(mut candidates: Vec<Candidate<'_>>, n: usize, adaptive: bool) -> Vec<u32> {
        for _ in 0..n {
            let id = pick(&candidates, adaptive).unwrap();
            candidates.iter_mut().find(|c| c.id == id).unwrap().busy += 1;
        }
        candidates.iter().map(|c| c.busy).collect()
    }

    #[test]
    fn equal_weights_split_evenly() {
        let counts = distribute(vec![candidate("a", 5), candidate("b", 5)], 10, false);
        assert_eq!(counts, vec![5, 5]);
    }

    #[test]
    fn weights_set_the_proportions() {
        let counts = distribute(vec![candidate("wifi", 8), candidate("phone", 4)], 30, false);
        assert_eq!(counts, vec![20, 10]);
    }

    #[test]
    fn the_least_busy_link_takes_the_next_flow() {
        let mut candidates = vec![candidate("a", 5), candidate("b", 5)];
        candidates[0].busy = 3;
        assert_eq!(pick(&candidates, false), Some("b"));
    }

    #[test]
    fn no_candidates_means_no_link() {
        assert_eq!(pick(&[], false), None);
    }

    #[test]
    fn adaptive_splits_by_capacity() {
        let mut fast = candidate("fast", 5);
        fast.peak_rx = 30 * MB;
        let mut slow = candidate("slow", 5);
        slow.peak_rx = 10 * MB;
        // Both already carry a flow, so both peaks are real measurements.
        fast.busy = 1;
        slow.busy = 1;
        let counts = distribute(vec![fast, slow], 38, true);
        assert_eq!(counts, vec![30, 10]);
    }

    #[test]
    fn a_loaded_link_stops_attracting_flows() {
        // The phone carried more so far, but it's already full of busy
        // flows; Wi-Fi's one flow is getting far more per flow.
        let mut wifi = candidate("wifi", 5);
        wifi.peak_rx = 600 * 1024;
        wifi.busy = 1;
        let mut phone = candidate("phone", 5);
        phone.peak_rx = 2 * MB;
        phone.busy = 6;
        assert_eq!(pick(&[wifi, phone], true), Some("wifi"));
    }

    #[test]
    fn an_idle_link_counts_as_average() {
        // Wi-Fi's peak decayed while it had nothing to do; that says
        // nothing about its speed, so it gets the next flow.
        let mut wifi = candidate("wifi", 5);
        wifi.peak_rx = 20 * 1024;
        let mut phone = candidate("phone", 5);
        phone.peak_rx = 2 * MB;
        phone.busy = 2;
        assert_eq!(pick(&[wifi.clone(), phone.clone()], true), Some("wifi"));
        let caps = capacities(&[wifi, phone]);
        assert_eq!(caps[0], caps[1]);
    }

    #[test]
    fn adaptive_without_measurements_is_plain_weighting() {
        let counts = distribute(vec![candidate("a", 6), candidate("b", 3)], 9, true);
        assert_eq!(counts, vec![6, 3]);
    }

    #[test]
    fn sticky_remembers_and_forgets() {
        let sticky = Sticky::default();
        assert_eq!(sticky.get("example.com"), None);
        sticky.set("example.com", "wifi");
        assert_eq!(sticky.get("example.com").as_deref(), Some("wifi"));
        sticky.clear();
        assert_eq!(sticky.get("example.com"), None);
    }

    #[test]
    fn unknown_links_use_the_default_policy() {
        let policy = Policy::default();
        assert_eq!(policy.link("new-adapter"), LinkPolicy::default());
    }
}
