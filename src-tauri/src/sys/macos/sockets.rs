use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::NonZeroU32;

use socket2::Socket;

use super::command::{self, Cmd, LSOF};
use crate::sys::SocketOwners;

/// Forces a socket's traffic out of one interface, whatever the routing table
/// says. This is what keeps MultiPath's own connections from looping back into
/// the virtual adapter that captures everything else.
///
/// `IP_BOUND_IF` / `IPV6_BOUND_IF` (one option per family) make the kernel
/// look the destination up in the interface's scoped routes, so the link's own
/// default gateway is used instead of the capture routes.
pub fn pin_socket_to_interface(socket: &Socket, if_index: u32, v6: bool) -> io::Result<()> {
    let index = NonZeroU32::new(if_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "interface index 0 is no interface",
        )
    })?;
    if v6 {
        socket.bind_device_by_index_v6(Some(index))
    } else {
        socket.bind_device_by_index_v4(Some(index))
    }
}

/// The owning process of every socket that talks through the adapter.
/// Sockets of other users' processes stay unknown without root.
pub fn socket_owners(tun_v4: Ipv4Addr, tun_v6: Ipv6Addr) -> SocketOwners {
    // -F: one field per line, made for parsing. -w: no warnings about
    // file systems it can't read.
    let cmd = Cmd::new(LSOF, &["-nP", "-w", "-iTCP", "-iUDP", "-FpPn"]);
    match command::query(&cmd) {
        Ok(out) => parse_lsof(&out, tun_v4, tun_v6),
        // lsof exits with 1 when it found nothing, too.
        Err(_) => SocketOwners::new(),
    }
}

/// Reads `lsof -F pPn`: `p<pid>` starts a process, `P<TCP|UDP>` and
/// `n<local>[-><remote>]` describe each of its sockets.
fn parse_lsof(out: &str, tun_v4: Ipv4Addr, tun_v6: Ipv6Addr) -> SocketOwners {
    let mut owners = SocketOwners::new();
    let mut pid = None;
    let mut tcp = None;
    for line in out.lines() {
        let Some(field) = line.chars().next() else {
            continue;
        };
        let value = &line[field.len_utf8()..];
        match field {
            'p' => pid = value.parse::<u32>().ok(),
            'P' => tcp = Some(value == "TCP"),
            'n' => {
                let (Some(pid), Some(tcp)) = (pid, tcp) else {
                    continue;
                };
                let local = value.split("->").next().unwrap_or_default();
                let Some((ip, port)) = parse_endpoint(local) else {
                    continue;
                };
                let on_tun = ip == Some(IpAddr::V4(tun_v4)) || ip == Some(IpAddr::V6(tun_v6));
                // UDP sockets are often bound to the wildcard address, so match by port only.
                if on_tun || (!tcp && ip.is_none_or(|ip| ip.is_unspecified())) {
                    owners.entry((tcp, port)).or_insert(pid);
                }
            }
            _ => {}
        }
    }
    owners
}

/// `10.77.0.1:5353`, `[fd77:6d70::1]:443` or `*:5353` (`None` for the wildcard).
fn parse_endpoint(text: &str) -> Option<(Option<IpAddr>, u16)> {
    let (host, port) = text.rsplit_once(':')?;
    let port = port.parse().ok()?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host == "*" {
        return Some((None, port));
    }
    // Scoped IPv6 addresses carry their interface: fe80::1%en0.
    let host = host.split('%').next()?;
    Some((Some(host.parse().ok()?), port))
}

/// The executable name of a process, such as `Safari`.
pub fn process_name(pid: u32) -> Option<String> {
    let mut buf = [0u8; 256];
    let len = unsafe {
        libc::proc_name(
            i32::try_from(pid).ok()?,
            buf.as_mut_ptr().cast(),
            buf.len() as u32,
        )
    };
    let len = usize::try_from(len).ok().filter(|&n| n > 0)?;
    Some(String::from_utf8_lossy(&buf[..len]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use socket2::{Domain, Type};

    const TUN_V4: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
    const TUN_V6: Ipv6Addr = Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1);

    #[test]
    fn pins_both_families() {
        // lo0 is index 1 on every Mac.
        let v4 = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        pin_socket_to_interface(&v4, 1, false).unwrap();
        assert_eq!(v4.device_index_v4().unwrap(), NonZeroU32::new(1));
        let v6 = Socket::new(Domain::IPV6, Type::DGRAM, None).unwrap();
        pin_socket_to_interface(&v6, 1, true).unwrap();
        assert_eq!(v6.device_index_v6().unwrap(), NonZeroU32::new(1));
    }

    #[test]
    fn rejects_bad_interfaces() {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let err = pin_socket_to_interface(&socket, 0, false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(pin_socket_to_interface(&socket, 60_000, false).is_err());
        assert_eq!(socket.device_index_v4().unwrap(), None);
    }

    #[test]
    fn the_option_numbers_are_darwins() {
        // From <netinet/in.h> and <netinet6/in6.h>.
        assert_eq!(libc::IP_BOUND_IF, 25);
        assert_eq!(libc::IPV6_BOUND_IF, 125);
    }

    #[test]
    fn reads_lsof_output() {
        let out = "p412\nf27\nPTCP\nn10.77.0.1:52000->140.82.112.3:443\nf28\nPUDP\nn*:5353\n\
                   p990\nf5\nPTCP\nn192.168.1.20:52001->1.1.1.1:443\nf6\nPTCP\nn[fd77:6d70::1]:52002->[2606:4700::1111]:443\n\
                   f7\nPUDP\nn[fe80::1%en0]:546\n";
        let owners = parse_lsof(out, TUN_V4, TUN_V6);
        assert_eq!(owners.get(&(true, 52000)), Some(&412));
        assert_eq!(owners.get(&(false, 5353)), Some(&412));
        assert_eq!(owners.get(&(true, 52002)), Some(&990));
        // Not on the adapter.
        assert_eq!(owners.get(&(true, 52001)), None);
        assert_eq!(owners.get(&(false, 546)), None);
        assert_eq!(owners.len(), 3);
    }

    #[test]
    fn reads_endpoints() {
        assert_eq!(
            parse_endpoint("10.77.0.1:80"),
            Some((Some(IpAddr::V4(TUN_V4)), 80))
        );
        assert_eq!(parse_endpoint("*:5353"), Some((None, 5353)));
        assert_eq!(
            parse_endpoint("[fd77:6d70::1]:443"),
            Some((Some(IpAddr::V6(TUN_V6)), 443))
        );
        assert_eq!(parse_endpoint("garbage"), None);
    }

    #[test]
    fn names_this_process() {
        let name = process_name(std::process::id()).unwrap();
        assert!(name.starts_with("multipath"), "{name}");
        assert_eq!(process_name(u32::MAX), None);
    }
}
