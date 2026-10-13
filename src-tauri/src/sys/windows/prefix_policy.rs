//! Windows' address selection policy table (RFC 6724), which decides whether
//! apps try IPv6 or IPv4 first for a site that has both.

use std::io;
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// IPv4 addresses, as the policy table sees them.
const IPV4_MAPPED: &str = "::ffff:0:0/96";
const IPV4_LABEL: u32 = 4;
/// Above `::/0` (40), so IPv4 sorts first.
const PREFERRED: u32 = 45;
const WINDOWS_DEFAULT: u32 = 35;

/// Makes Windows try IPv4 first while it lives, and restores the old order
/// when dropped. Only the active store changes, so a reboot undoes it too.
///
/// Usually only some networks have IPv6. An app that picks IPv6 can then
/// only be relayed over those, so its traffic can't be spread out.
pub struct PreferIpv4 {
    previous: u32,
}

impl PreferIpv4 {
    pub fn apply() -> io::Result<PreferIpv4> {
        let previous = match current_precedence() {
            // Left over from a run that crashed.
            Some(PREFERRED) | None => WINDOWS_DEFAULT,
            Some(p) => p,
        };
        set_precedence(PREFERRED)?;
        Ok(PreferIpv4 { previous })
    }
}

impl Drop for PreferIpv4 {
    fn drop(&mut self) {
        if let Err(err) = set_precedence(self.previous) {
            log::warn!("couldn't restore the IPv4 precedence: {err}");
        }
    }
}

fn netsh(args: &[&str]) -> io::Result<String> {
    let out = Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(io::Error::other(stdout.trim().to_owned()))
    }
}

fn current_precedence() -> Option<u32> {
    let table = netsh(&[
        "interface",
        "ipv6",
        "show",
        "prefixpolicies",
        "store=active",
    ])
    .ok()?;
    parse_precedence(&table)
}

/// Finds the IPv4 row in `netsh`'s table: precedence, label, prefix.
fn parse_precedence(table: &str) -> Option<u32> {
    table.lines().find_map(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        match cols[..] {
            [precedence, _, IPV4_MAPPED] => precedence.parse().ok(),
            _ => None,
        }
    })
}

fn set_precedence(precedence: u32) -> io::Result<()> {
    netsh(&[
        "interface",
        "ipv6",
        "set",
        "prefixpolicy",
        &format!("prefix={IPV4_MAPPED}"),
        &format!("precedence={precedence}"),
        &format!("label={IPV4_LABEL}"),
        "store=active",
    ])
    .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_ipv4_row() {
        let table = "\
Querying active state...

Precedence  Label  Prefix
----------  -----  --------------------------------
        50      0  ::1/128
        40      1  ::/0
        35      4  ::ffff:0:0/96
         3     13  fc00::/7
";
        assert_eq!(parse_precedence(table), Some(35));
        assert_eq!(parse_precedence("nothing here"), None);
    }
}
