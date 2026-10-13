use std::fs;
use std::io;
use std::path::PathBuf;

const CONF: &str = "/proc/sys/net/ipv4/conf";
const STRICT: &str = "1";
const LOOSE: &str = "2";

/// Strict reverse-path filtering drops a reply that comes in on a link
/// whenever the route back to its sender goes elsewhere, and while
/// MultiPath is on that route always goes into the adapter. Loose mode only
/// asks for some route back. Strict settings (per interface, `all`, and
/// `default` for links that appear later) are loosened until this is dropped.
pub struct LooseRpFilter {
    loosened: Vec<PathBuf>,
}

impl LooseRpFilter {
    pub fn apply() -> io::Result<LooseRpFilter> {
        // Built first, so an error halfway restores what was already changed.
        let mut guard = LooseRpFilter {
            loosened: Vec::new(),
        };
        for entry in fs::read_dir(CONF)? {
            let path = entry?.path().join("rp_filter");
            if fs::read_to_string(&path).is_ok_and(|v| v.trim() == STRICT) {
                fs::write(&path, LOOSE)?;
                guard.loosened.push(path);
            }
        }
        if !guard.loosened.is_empty() {
            log::info!(
                "loosened reverse-path filtering on {} entries",
                guard.loosened.len()
            );
        }
        Ok(guard)
    }
}

impl Drop for LooseRpFilter {
    fn drop(&mut self) {
        for path in &self.loosened {
            // A link that has gone since took its setting with it.
            let _ = fs::write(path, STRICT);
        }
    }
}
