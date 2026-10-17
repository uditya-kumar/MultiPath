//! Routes and addresses on the virtual adapter, through `route(8)` and
//! `ifconfig(8)`. Routes through a utun go away with it, even after
//! `kill -9`, so stale ones can only sit on a MultiPath utun that's still
//! open.

use std::io;
use std::net::IpAddr;

use super::adapters::interface_name;
use super::command::{self, Cmd, IFCONFIG, ROUTE};

/// An on-link route through `if_index`. Kept so it can be deleted on stop.
pub struct Route {
    interface: String,
    prefix: IpAddr,
    prefix_len: u8,
}

impl Route {
    pub fn add(if_index: u32, prefix: IpAddr, prefix_len: u8) -> io::Result<Route> {
        let interface = name_of(if_index)?;
        command::change(&route_cmd("add", &interface, prefix, prefix_len))
            .or_else(already_there)
            .map_err(|err| io::Error::new(err.kind(), format!("adding a route failed: {err}")))?;
        Ok(Route {
            interface,
            prefix,
            prefix_len,
        })
    }

    /// Deletes a route left behind by a run that didn't stop cleanly.
    /// Returns true if there was one.
    pub fn remove_stale(if_index: u32, prefix: IpAddr, prefix_len: u8) -> bool {
        interface_name(if_index)
            .is_some_and(|name| delete_route(&name, prefix, prefix_len).unwrap_or(false))
    }
}

impl Drop for Route {
    fn drop(&mut self) {
        if let Err(err) = delete_route(&self.interface, self.prefix, self.prefix_len) {
            log::warn!("{err}");
        }
    }
}

fn name_of(if_index: u32) -> io::Result<String> {
    interface_name(if_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no interface with index {if_index}"),
        )
    })
}

/// Deletes the route; true if it was there. A route that's already gone (or
/// went with its utun) isn't an error, so cleaning up twice is harmless.
fn delete_route(interface: &str, prefix: IpAddr, prefix_len: u8) -> io::Result<bool> {
    match command::change(&route_cmd("delete", interface, prefix, prefix_len)) {
        Ok(_) => Ok(true),
        Err(err) if is_missing(&err) => Ok(false),
        Err(err) => Err(err),
    }
}

/// `route -n add -inet 128.0.0.0/1 -interface utun5`: on-link through the
/// adapter, as wg-quick does on macOS.
fn route_cmd(action: &str, interface: &str, prefix: IpAddr, prefix_len: u8) -> Cmd {
    let family = match prefix {
        IpAddr::V4(_) => "-inet",
        IpAddr::V6(_) => "-inet6",
    };
    Cmd::new(
        ROUTE,
        &[
            "-n",
            action,
            family,
            &format!("{prefix}/{prefix_len}"),
            "-interface",
            interface,
        ],
    )
}

pub fn add_unicast_address(if_index: u32, ip: IpAddr, prefix_len: u8) -> io::Result<()> {
    let interface = name_of(if_index)?;
    command::change(&address_cmd(&interface, ip, prefix_len))
        .or_else(already_there)
        .map(|_| ())
        .map_err(|err| io::Error::new(err.kind(), format!("adding an address failed: {err}")))
}

/// `ifconfig utun5 inet6 fd77:6d70::1 prefixlen 64 alias`. A utun is
/// point-to-point, so an IPv4 address names itself as the other end.
fn address_cmd(interface: &str, ip: IpAddr, prefix_len: u8) -> Cmd {
    let args = match ip {
        IpAddr::V6(_) => [
            interface.to_owned(),
            "inet6".into(),
            ip.to_string(),
            "prefixlen".into(),
            prefix_len.to_string(),
            "alias".into(),
        ]
        .to_vec(),
        IpAddr::V4(_) => [
            interface.to_owned(),
            "inet".into(),
            format!("{ip}/{prefix_len}"),
            ip.to_string(),
            "alias".into(),
        ]
        .to_vec(),
    };
    Cmd::new(IFCONFIG, &args)
}

/// Like Windows' "object already exists": the route or address is in place.
fn already_there(err: io::Error) -> io::Result<String> {
    if err.to_string().contains("File exists") {
        Ok(String::new())
    } else {
        Err(err)
    }
}

/// What `route delete` says about a route that isn't there.
fn is_missing(err: &io::Error) -> bool {
    let text = err.to_string();
    text.contains("not in table") || text.contains("No such process")
}

#[cfg(test)]
mod tests {
    use super::super::command::fake;
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const LO0: u32 = 1;

    fn v4(a: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, 0, 0, 0))
    }

    #[test]
    fn builds_route_commands() {
        assert_eq!(
            route_cmd("add", "utun5", v4(128), 1).line(),
            "/sbin/route -n add -inet 128.0.0.0/1 -interface utun5"
        );
        let v6 = IpAddr::V6(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0));
        assert_eq!(
            route_cmd("delete", "utun5", v6, 1).line(),
            "/sbin/route -n delete -inet6 8000::/1 -interface utun5"
        );
    }

    #[test]
    fn builds_address_commands() {
        let ula = IpAddr::V6(Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1));
        assert_eq!(
            address_cmd("utun5", ula, 64).line(),
            "/sbin/ifconfig utun5 inet6 fd77:6d70::1 prefixlen 64 alias"
        );
        let ip = IpAddr::V4(Ipv4Addr::new(10, 77, 0, 1));
        assert_eq!(
            address_cmd("utun5", ip, 24).line(),
            "/sbin/ifconfig utun5 inet 10.77.0.1/24 10.77.0.1 alias"
        );
    }

    #[test]
    fn a_route_is_removed_when_dropped() {
        let fake = fake::succeed();
        let route = Route::add(LO0, v4(0), 1).unwrap();
        drop(route);
        assert_eq!(
            fake.lines(),
            [
                "/sbin/route -n add -inet 0.0.0.0/1 -interface lo0",
                "/sbin/route -n delete -inet 0.0.0.0/1 -interface lo0",
            ]
        );
    }

    #[test]
    fn adding_twice_is_fine() {
        let _fake = fake::install(|_| {
            Err(io::Error::other(
                "route: writing to routing socket: File exists",
            ))
        });
        let route = Route::add(LO0, v4(0), 1).unwrap();
        std::mem::forget(route);
        assert!(add_unicast_address(LO0, v4(10), 8).is_ok());
    }

    #[test]
    fn cleanup_is_idempotent() {
        let mut present = true;
        let fake = fake::install(move |_| {
            if std::mem::replace(&mut present, false) {
                Ok(String::new())
            } else {
                Err(io::Error::other(
                    "route: writing to routing socket: not in table",
                ))
            }
        });
        assert!(Route::remove_stale(LO0, v4(128), 1));
        assert!(!Route::remove_stale(LO0, v4(128), 1));
        assert_eq!(fake.lines().len(), 2);
        // An interface that's gone has nothing to clean.
        assert!(!Route::remove_stale(u32::MAX, v4(128), 1));
        assert_eq!(fake.lines().len(), 2);
    }

    #[test]
    fn other_failures_are_reported() {
        let _fake = fake::install(|_| {
            Err(io::Error::other(
                "route: must be root to alter routing table",
            ))
        });
        let err = Route::add(LO0, v4(0), 1).err().unwrap();
        assert!(err.to_string().starts_with("adding a route failed"));
        assert!(Route::add(u32::MAX, v4(0), 1).is_err());
    }
}
