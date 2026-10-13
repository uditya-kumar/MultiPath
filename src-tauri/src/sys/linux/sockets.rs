use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use socket2::Socket;

use super::adapters::interface_name;
use crate::sys::SocketOwners;

/// Forces a socket's traffic out of one interface, whatever the routing table
/// says. This is what keeps MultiPath's own connections from looping back into
/// the virtual adapter that captures everything else.
pub fn pin_socket_to_interface(socket: &Socket, if_index: u32, _v6: bool) -> io::Result<()> {
    let name = interface_name(if_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no interface with index {if_index}"),
        )
    })?;
    // SO_BINDTODEVICE, which covers both families.
    socket.bind_device(Some(name.as_bytes()))
}

/// The owning process of every socket that talks through the adapter.
/// Sockets of other users' processes stay unknown without root.
pub fn socket_owners(tun_v4: Ipv4Addr, tun_v6: Ipv6Addr) -> SocketOwners {
    // Socket inode → (is_tcp, local port), for the sockets worth a lookup.
    let mut wanted = HashMap::new();
    for (table, tcp) in [
        ("tcp", true),
        ("tcp6", true),
        ("udp", false),
        ("udp6", false),
    ] {
        let Ok(text) = fs::read_to_string(format!("/proc/net/{table}")) else {
            continue;
        };
        for row in parse_table(&text) {
            let on_tun = row.ip == IpAddr::V4(tun_v4) || row.ip == IpAddr::V6(tun_v6);
            // UDP sockets are often bound to the wildcard address, so match by port only.
            if on_tun || (!tcp && row.ip.is_unspecified()) {
                wanted.insert(row.inode, (tcp, row.port));
            }
        }
    }

    let mut owners = SocketOwners::new();
    if wanted.is_empty() {
        return owners;
    }
    let Ok(procs) = fs::read_dir("/proc") else {
        return owners;
    };
    for entry in procs.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse().ok()) else {
            continue;
        };
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Some(inode) = fs::read_link(fd.path()).ok().and_then(|t| socket_inode(&t)) else {
                continue;
            };
            if let Some(&key) = wanted.get(&inode) {
                owners.entry(key).or_insert(pid);
            }
        }
    }
    owners
}

struct SocketRow {
    ip: IpAddr,
    port: u16,
    inode: u64,
}

/// Rows of `/proc/net/{tcp,tcp6,udp,udp6}`: `sl local rem st tx:rx tr:when
/// retrnsmt uid timeout inode …`, with the local address as `ADDR:PORT` in hex.
fn parse_table(text: &str) -> Vec<SocketRow> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (ip, port) = fields.get(1)?.split_once(':')?;
            Some(SocketRow {
                ip: parse_ip(ip)?,
                port: u16::from_str_radix(port, 16).ok()?,
                inode: fields.get(9)?.parse().ok()?,
            })
        })
        .collect()
}

/// The kernel prints the address as 32-bit words in host byte order.
fn parse_ip(hex: &str) -> Option<IpAddr> {
    if hex.len() != 8 && hex.len() != 32 {
        return None;
    }
    let mut bytes = Vec::with_capacity(16);
    for word in hex.as_bytes().chunks(8) {
        let word = u32::from_str_radix(std::str::from_utf8(word).ok()?, 16).ok()?;
        bytes.extend_from_slice(&word.to_ne_bytes());
    }
    match <[u8; 4]>::try_from(bytes.as_slice()) {
        Ok(v4) => Some(IpAddr::from(v4)),
        Err(_) => Some(IpAddr::from(<[u8; 16]>::try_from(bytes).ok()?)),
    }
}

/// `socket:[12345]`, the link target of a socket's file descriptor.
fn socket_inode(target: &std::path::Path) -> Option<u64> {
    target
        .to_str()?
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// The executable name of a process, such as `firefox`.
pub fn process_name(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reads_proc_net_rows() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 01004D0A:A3C2 0A0B0CCB:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 48213 1 0000000000000000 20 4 30 10 -1
   1: garbage
";
        let rows = parse_table(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ip, IpAddr::V4(Ipv4Addr::new(10, 77, 0, 1)));
        assert_eq!(rows[0].port, 0xa3c2);
        assert_eq!(rows[0].inode, 48213);
    }

    #[test]
    fn reads_ipv6_addresses() {
        assert_eq!(
            parse_ip("706D77FD000000000000000001000000"),
            Some(IpAddr::V6(Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1)))
        );
        assert_eq!(
            parse_ip("0000000000000000FFFF00000100007F"),
            Some(IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped()))
        );
        assert_eq!(parse_ip("0A00"), None);
    }

    #[test]
    fn finds_its_own_sockets() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let port = client.local_addr().unwrap().port();
        let owners = socket_owners(Ipv4Addr::LOCALHOST, Ipv6Addr::LOCALHOST);
        assert_eq!(owners.get(&(true, port)), Some(&std::process::id()));
    }

    #[test]
    fn reads_socket_links() {
        assert_eq!(socket_inode(Path::new("socket:[48213]")), Some(48213));
        assert_eq!(socket_inode(Path::new("/dev/null")), None);
    }
}
