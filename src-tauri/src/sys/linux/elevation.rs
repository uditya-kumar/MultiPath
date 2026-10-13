use std::io;
use std::process::Command;

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

/// The capabilities MultiPath runs with instead of root: the adapter and
/// routes, and binding sockets to a device plus raw ICMP.
const CAPABILITIES: &str = "cap_net_admin,cap_net_raw+ep";

/// Gives this executable its capabilities once, through the polkit password
/// prompt, then starts a new copy (capabilities apply from the next start),
/// passing it `args`.
pub fn relaunch_elevated(args: &str) -> io::Result<()> {
    // An AppImage runs from a read-only mount that ignores capabilities.
    if std::env::var_os("APPIMAGE").is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the AppImage can't be given network permissions: install the .deb, or run it with sudo",
        ));
    }
    let exe = std::env::current_exe()?;
    let status = Command::new("pkexec")
        .arg("setcap")
        .arg(CAPABILITIES)
        .arg(&exe)
        .status()
        .map_err(|err| io::Error::new(err.kind(), format!("couldn't run pkexec: {err}")))?;
    match status.code() {
        Some(0) => {}
        // pkexec: the prompt was dismissed, or the user isn't allowed.
        Some(126 | 127) => return Err(io::Error::other("the permission prompt was cancelled")),
        _ => return Err(io::Error::other(format!("setcap failed ({status})"))),
    }
    Command::new(exe).args(args.split_whitespace()).spawn()?;
    Ok(())
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
