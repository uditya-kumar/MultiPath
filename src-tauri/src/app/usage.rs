//! Data sent and received through MultiPath today, for "3.4 GB today".

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A local date as (year, month, day).
pub type Date = (u16, u16, u16);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub date: Date,
    pub bytes: u64,
}

impl Usage {
    /// Adds `bytes` to today's total, starting over when the date has changed.
    pub fn add(&mut self, today: Date, bytes: u64) {
        if self.date != today {
            *self = Usage {
                date: today,
                bytes: 0,
            };
        }
        self.bytes += bytes;
    }

    /// Today's total; zero when the last traffic was on an earlier day.
    pub fn today(&self, today: Date) -> u64 {
        if self.date == today {
            self.bytes
        } else {
            0
        }
    }

    pub fn load(path: &Path) -> Usage {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec(self) {
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
    fn counts_per_day() {
        let mut usage = Usage::default();
        usage.add((2026, 10, 12), 100);
        usage.add((2026, 10, 12), 50);
        assert_eq!(usage.today((2026, 10, 12)), 150);
        // Midnight passed without traffic.
        assert_eq!(usage.today((2026, 10, 13)), 0);
        usage.add((2026, 10, 13), 7);
        assert_eq!(usage.today((2026, 10, 13)), 7);
    }

    #[test]
    fn survives_a_restart() {
        let path =
            std::env::temp_dir().join(format!("multipath-usage-{}.json", std::process::id()));
        let mut usage = Usage::default();
        usage.add((2026, 10, 12), 42);
        usage.save(&path);
        assert_eq!(Usage::load(&path), usage);
        let _ = std::fs::remove_file(&path);
    }
}
