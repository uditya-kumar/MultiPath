use std::io;
use std::net::IpAddr;
use std::process::Command;

use super::adapters::interface_name;

/// Points the adapter at `servers` and makes it the DNS route for every
/// domain (`~.`), so lookups go through MultiPath too. systemd-resolved
/// forgets both once the adapter is gone.
///
/// Only root may: resolved asks anyone else for a password through polkit,
/// capabilities or not. Without it DNS stays with the system resolver, which
/// still works: its queries go out on-link, or bound to a real network.
pub fn set_dns(if_index: u32, servers: &[IpAddr]) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "only root may change systemd-resolved's settings",
        ));
    }
    let name = interface_name(if_index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the adapter is gone"))?;
    let mut dns = vec!["dns".to_owned(), name.clone()];
    dns.extend(servers.iter().map(IpAddr::to_string));
    resolvectl(&dns)?;
    resolvectl(&["domain".to_owned(), name, "~.".to_owned()])
}

fn resolvectl(args: &[String]) -> io::Result<()> {
    let out = Command::new("resolvectl").args(args).output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "resolvectl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}
