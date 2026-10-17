use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::net::balancer::{LinkPolicy, Policy};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LinkPrefs {
    pub enabled: bool,
    /// Relative share of new connections, 1 to 10.
    pub weight: u32,
    pub label: Option<String>,
}

impl Default for LinkPrefs {
    fn default() -> Self {
        LinkPrefs {
            enabled: true,
            weight: 5,
            label: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// MultiPath was on when it last ran, so it turns back on at start.
    pub on: bool,
    /// Keyed by the adapter's GUID, which survives renames and reboots.
    pub links: HashMap<String, LinkPrefs>,
    /// Keep every connection to a site on the network it first used.
    pub sticky: bool,
    /// Scale each network's share by the throughput it has actually delivered.
    pub adaptive: bool,
    pub capture_ipv6: bool,
    pub dns: Vec<IpAddr>,
    /// Look for a new version when MultiPath starts. Off by default: it's the
    /// only thing MultiPath would contact on its own, and it never installs
    /// anything without the user's say-so.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            on: false,
            links: HashMap::new(),
            sticky: false,
            adaptive: true,
            capture_ipv6: true,
            // Some ISPs (Jio among them) drop DNS queries to 1.1.1.1.
            dns: vec!["8.8.8.8".parse().unwrap(), "1.1.1.1".parse().unwrap()],
            check_updates: false,
        }
    }
}

impl Settings {
    pub fn link(&self, id: &str) -> LinkPrefs {
        self.links.get(id).cloned().unwrap_or_default()
    }

    /// What the balancer needs to know.
    pub fn policy(&self) -> Policy {
        Policy {
            links: self
                .links
                .iter()
                .map(|(id, p)| {
                    let policy = LinkPolicy {
                        enabled: p.enabled,
                        weight: p.weight.clamp(1, 10),
                    };
                    (id.clone(), policy)
                })
                .collect(),
            sticky: self.sticky,
            adaptive: self.adaptive,
            block_ipv6: !self.capture_ipv6,
        }
    }

    pub fn load(path: &PathBuf) -> Settings {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            // Write then rename so a crash never leaves half a file.
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_checks_stay_off_unless_chosen() {
        assert!(!Settings::default().check_updates);
        // A settings file from 1.1.0 has no such field.
        let old: Settings = serde_json::from_str(r#"{"on":false,"sticky":true}"#).unwrap();
        assert!(!old.check_updates && old.sticky && !old.on);
        let chosen: Settings = serde_json::from_str(r#"{"checkUpdates":true}"#).unwrap();
        assert!(chosen.check_updates);
    }
}
