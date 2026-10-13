use std::io;

/// `CAP_NET_ADMIN`: creating the TUN device and changing routes.
const CAP_NET_ADMIN: u32 = 12;

/// Whether this process may set up the adapter and routes: it runs as root,
/// or was given `CAP_NET_ADMIN`.
pub fn is_elevated() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| effective_caps(&status))
        .is_some_and(|caps| caps & (1 << CAP_NET_ADMIN) != 0)
}

/// The `CapEff` bit set from `/proc/<pid>/status`.
fn effective_caps(status: &str) -> Option<u64> {
    let hex = status.lines().find_map(|l| l.strip_prefix("CapEff:"))?;
    u64::from_str_radix(hex.trim(), 16).ok()
}

pub fn relaunch_elevated(_args: &str) -> io::Result<()> {
    Err(super::not_yet("asking for network permissions"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_effective_capabilities() {
        let status = "Name:\tmultipath\nCapPrm:\t0000000000003000\nCapEff:\t0000000000003000\n";
        let caps = effective_caps(status).unwrap();
        assert_ne!(caps & (1 << CAP_NET_ADMIN), 0);
        assert_eq!(effective_caps("Name:\tx\n"), None);
    }
}
