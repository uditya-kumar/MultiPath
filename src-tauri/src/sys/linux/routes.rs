//! Routes and addresses on the virtual adapter, over rtnetlink: one request
//! per socket, answered by the kernel with an ack carrying the error code.

use std::io::{self, Read};
use std::net::IpAddr;

use socket2::{Domain, Protocol, Socket, Type};

const HEADER_LEN: usize = 16;

/// An on-link route through `if_index`. Kept so it can be deleted on stop.
pub struct Route {
    if_index: u32,
    prefix: IpAddr,
    prefix_len: u8,
}

impl Route {
    pub fn add(if_index: u32, prefix: IpAddr, prefix_len: u8) -> io::Result<Route> {
        let body = route_body(if_index, prefix, prefix_len, libc::RT_SCOPE_LINK);
        let flags = libc::NLM_F_CREATE | libc::NLM_F_EXCL;
        allow_existing(request(libc::RTM_NEWROUTE, flags as u16, &body))
            .map_err(|err| io::Error::new(err.kind(), format!("adding a route failed: {err}")))?;
        Ok(Route {
            if_index,
            prefix,
            prefix_len,
        })
    }

    /// Deletes a route left behind by a run that didn't stop cleanly.
    /// Returns true if there was one.
    pub fn remove_stale(if_index: u32, prefix: IpAddr, prefix_len: u8) -> bool {
        delete_route(if_index, prefix, prefix_len).is_ok()
    }
}

impl Drop for Route {
    fn drop(&mut self) {
        // Fails harmlessly if the adapter, and with it the route, is already gone.
        let _ = delete_route(self.if_index, self.prefix, self.prefix_len);
    }
}

fn delete_route(if_index: u32, prefix: IpAddr, prefix_len: u8) -> io::Result<()> {
    // RT_SCOPE_NOWHERE matches a route of any scope.
    let body = route_body(if_index, prefix, prefix_len, libc::RT_SCOPE_NOWHERE);
    request(libc::RTM_DELROUTE, 0, &body)
}

pub fn add_unicast_address(if_index: u32, ip: IpAddr, prefix_len: u8) -> io::Result<()> {
    let flags = libc::NLM_F_CREATE | libc::NLM_F_EXCL;
    allow_existing(request(
        libc::RTM_NEWADDR,
        flags as u16,
        &address_body(if_index, ip, prefix_len),
    ))
    .map_err(|err| io::Error::new(err.kind(), format!("adding an address failed: {err}")))
}

/// Like Windows' "object already exists": the route or address is in place.
fn allow_existing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(err) if err.raw_os_error() == Some(libc::EEXIST) => Ok(()),
        other => other,
    }
}

fn family(ip: IpAddr) -> u8 {
    match ip {
        IpAddr::V4(_) => libc::AF_INET as u8,
        IpAddr::V6(_) => libc::AF_INET6 as u8,
    }
}

fn octets(ip: IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(v4) => v4.octets().to_vec(),
        IpAddr::V6(v6) => v6.octets().to_vec(),
    }
}

/// `struct rtmsg` for the main table, then the destination and the adapter.
fn route_body(if_index: u32, prefix: IpAddr, prefix_len: u8, scope: u8) -> Vec<u8> {
    let mut body = vec![
        family(prefix),
        prefix_len,
        0, // source length
        0, // tos
        libc::RT_TABLE_MAIN,
        libc::RTPROT_STATIC,
        scope,
        libc::RTN_UNICAST,
    ];
    body.extend_from_slice(&0u32.to_ne_bytes()); // flags
    push_attr(&mut body, libc::RTA_DST, &octets(prefix));
    push_attr(&mut body, libc::RTA_OIF, &if_index.to_ne_bytes());
    body
}

/// `struct ifaddrmsg`, then the address. Duplicate address detection is
/// skipped: nothing else lives on this link, and until it finishes the
/// address can't be used.
fn address_body(if_index: u32, ip: IpAddr, prefix_len: u8) -> Vec<u8> {
    let mut body = vec![
        family(ip),
        prefix_len,
        libc::IFA_F_NODAD as u8,
        libc::RT_SCOPE_UNIVERSE,
    ];
    body.extend_from_slice(&if_index.to_ne_bytes());
    push_attr(&mut body, libc::IFA_LOCAL, &octets(ip));
    push_attr(&mut body, libc::IFA_ADDRESS, &octets(ip));
    body
}

/// Appends a `struct rtattr`, padded to 4 bytes like every netlink part.
fn push_attr(buf: &mut Vec<u8>, kind: u16, data: &[u8]) {
    let len = 4 + data.len();
    buf.extend_from_slice(&(len as u16).to_ne_bytes());
    buf.extend_from_slice(&kind.to_ne_bytes());
    buf.extend_from_slice(data);
    buf.resize(buf.len().next_multiple_of(4), 0);
}

/// A `struct nlmsghdr` in front of `body`, asking for an ack.
fn message(kind: u16, flags: u16, body: &[u8]) -> Vec<u8> {
    let flags = flags | (libc::NLM_F_REQUEST | libc::NLM_F_ACK) as u16;
    let mut msg = Vec::with_capacity(HEADER_LEN + body.len());
    msg.extend_from_slice(&((HEADER_LEN + body.len()) as u32).to_ne_bytes());
    msg.extend_from_slice(&kind.to_ne_bytes());
    msg.extend_from_slice(&flags.to_ne_bytes());
    msg.extend_from_slice(&1u32.to_ne_bytes()); // sequence number
    msg.extend_from_slice(&0u32.to_ne_bytes()); // port id: the kernel fills it in
    msg.extend_from_slice(body);
    msg
}

fn request(kind: u16, flags: u16, body: &[u8]) -> io::Result<()> {
    let socket = Socket::new(
        Domain::from(libc::AF_NETLINK),
        Type::RAW,
        Some(Protocol::from(libc::NETLINK_ROUTE)),
    )?;
    // With no address the message goes to the kernel.
    socket.send(&message(kind, flags, body))?;
    let mut reply = [0u8; 4096];
    let len = (&socket).read(&mut reply)?;
    ack_result(&reply[..len])
}

/// Reads the error code out of the kernel's `NLMSG_ERROR` ack (0 is success).
fn ack_result(reply: &[u8]) -> io::Result<()> {
    let field = |at: usize, len: usize| reply.get(at..at + len);
    let kind = field(4, 2).map(|b| u16::from_ne_bytes([b[0], b[1]]));
    let code = field(HEADER_LEN, 4).map(|b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]]));
    match (kind, code) {
        (Some(kind), Some(0)) if kind == libc::NLMSG_ERROR as u16 => Ok(()),
        (Some(kind), Some(code)) if kind == libc::NLMSG_ERROR as u16 => {
            Err(io::Error::from_raw_os_error(-code))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected netlink reply",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn builds_a_route_to_the_adapter() {
        let body = route_body(7, IpAddr::V4(Ipv4Addr::new(128, 0, 0, 0)), 1, 253);
        let mut expected = vec![2, 1, 0, 0, 254, 4, 253, 1, 0, 0, 0, 0];
        expected.extend([8, 0, 1, 0, 128, 0, 0, 0]); // RTA_DST
        expected.extend([8, 0, 4, 0, 7, 0, 0, 0]); // RTA_OIF
        assert_eq!(body, expected);
    }

    #[test]
    fn builds_an_ipv6_address_without_dad() {
        let ip = Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1);
        let body = address_body(9, IpAddr::V6(ip), 64);
        assert_eq!(&body[..8], &[10, 64, 2, 0, 9, 0, 0, 0]);
        assert_eq!(&body[8..12], &[20, 0, 2, 0]); // IFA_LOCAL, 16 bytes
        assert_eq!(&body[12..28], &ip.octets());
        assert_eq!(&body[28..32], &[20, 0, 1, 0]); // IFA_ADDRESS
        assert_eq!(body.len(), 48);
    }

    #[test]
    fn pads_attributes_to_four_bytes() {
        let mut buf = Vec::new();
        push_attr(&mut buf, 1, &[0xaa]);
        assert_eq!(buf, [5, 0, 1, 0, 0xaa, 0, 0, 0]);
    }

    #[test]
    fn frames_the_request() {
        let msg = message(libc::RTM_NEWROUTE, 0, &[0; 12]);
        assert_eq!(&msg[..4], &28u32.to_ne_bytes());
        assert_eq!(&msg[4..6], &24u16.to_ne_bytes());
        assert_eq!(&msg[6..8], &5u16.to_ne_bytes()); // NLM_F_REQUEST | NLM_F_ACK
    }

    fn ack(code: i32) -> Vec<u8> {
        let mut reply = message(libc::NLMSG_ERROR as u16, 0, &code.to_ne_bytes());
        reply.extend([0; 16]); // the echoed request header
        reply
    }

    #[test]
    fn reads_the_ack() {
        assert!(ack_result(&ack(0)).is_ok());
        let err = ack_result(&ack(-libc::EEXIST)).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::EEXIST));
        assert!(allow_existing(Err(err)).is_ok());
        assert!(ack_result(&[1, 2, 3]).is_err());
    }
}
