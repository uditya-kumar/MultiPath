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
    /// Flows currently open on the link.
    pub active: u32,
    /// Recent peak download rate, bytes/s.
    pub peak_rx: u64,
}

/// Below this a link hasn't carried enough traffic for its peak to say
/// anything about its capacity.
const MIN_PEAK_FOR_ADAPTIVE: u64 = 256 * 1024;
/// A link's share is never scaled by more than this, either way, so a slow
/// start doesn't starve a link for good.
const MAX_CAPACITY_SCALE: f64 = 4.0;

/// How much faster or slower each link has proven to be than the average,
/// as a multiplier on its share. Links without a meaningful peak count as
/// average.
pub fn capacity_factors(peaks: &[u64]) -> Vec<f64> {
    let known: Vec<f64> = peaks
        .iter()
        .filter(|&&p| p >= MIN_PEAK_FOR_ADAPTIVE)
        .map(|&p| p as f64)
        .collect();
    if known.len() < 2 {
        return vec![1.0; peaks.len()];
    }
    let mean = known.iter().sum::<f64>() / known.len() as f64;
    peaks
        .iter()
        .map(|&p| {
            if p < MIN_PEAK_FOR_ADAPTIVE {
                1.0
            } else {
                (p as f64 / mean).clamp(1.0 / MAX_CAPACITY_SCALE, MAX_CAPACITY_SCALE)
            }
        })
        .collect()
}

/// Weighted least-connections: the link with the fewest open flows per unit
/// of share takes the next one. Over time each link carries flows in
/// proportion to `weight × capacity`.
pub fn pick<'a>(candidates: &[Candidate<'a>], adaptive: bool) -> Option<&'a str> {
    let factors = if adaptive {
        capacity_factors(&candidates.iter().map(|c| c.peak_rx).collect::<Vec<_>>())
    } else {
        vec![1.0; candidates.len()]
    };
    candidates
        .iter()
        .zip(factors)
        .map(|(c, factor)| {
            let share = c.weight.max(1) as f64 * factor;
            ((c.active as f64 + 1.0) / share, share, c.id)
        })
        // Lowest load first; on a tie, the bigger share; then a stable order.
        .min_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(b.1.total_cmp(&a.1))
                .then(a.2.cmp(b.2))
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

    fn candidate(id: &str, weight: u32) -> Candidate<'_> {
        Candidate {
            id,
            weight,
            active: 0,
            peak_rx: 0,
        }
    }

    /// Opens `n` long-lived flows and counts where each one went.
    fn distribute(mut candidates: Vec<Candidate<'_>>, n: usize, adaptive: bool) -> Vec<u32> {
        for _ in 0..n {
            let id = pick(&candidates, adaptive).unwrap();
            candidates.iter_mut().find(|c| c.id == id).unwrap().active += 1;
        }
        candidates.iter().map(|c| c.active).collect()
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
    fn closed_flows_free_up_a_link() {
        let mut candidates = vec![candidate("a", 5), candidate("b", 5)];
        candidates[0].active = 3;
        assert_eq!(pick(&candidates, false), Some("b"));
    }

    #[test]
    fn no_candidates_means_no_link() {
        assert_eq!(pick(&[], false), None);
    }

    #[test]
    fn adaptive_favours_the_faster_link() {
        let mut fast = candidate("fast", 5);
        fast.peak_rx = 30 * 1024 * 1024;
        let mut slow = candidate("slow", 5);
        slow.peak_rx = 10 * 1024 * 1024;
        let counts = distribute(vec![fast, slow], 40, true);
        assert_eq!(counts, vec![30, 10]);
    }

    #[test]
    fn adaptive_ignores_links_without_real_traffic() {
        assert_eq!(capacity_factors(&[0, 10 * 1024 * 1024]), vec![1.0, 1.0]);
        assert_eq!(capacity_factors(&[1000, 2000]), vec![1.0, 1.0]);
    }

    #[test]
    fn adaptive_scaling_is_bounded() {
        // One link far faster than seven slow ones: both ends hit the clamp.
        let mut peaks = vec![300 * 1024; 7];
        peaks.push(1024 * 1024 * 1024);
        let factors = capacity_factors(&peaks);
        assert_eq!(factors[0], 1.0 / MAX_CAPACITY_SCALE);
        assert_eq!(factors[7], MAX_CAPACITY_SCALE);
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
