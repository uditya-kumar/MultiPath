use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};

const ECHO_REPLY: u8 = 0;
const ECHO_REQUEST: u8 = 8;

static NEXT_SEQ: AtomicU16 = AtomicU16::new(1);

/// Sends an ICMP echo out of `if_index` from `source` and waits for the
/// reply. Returns the echoed payload. Blocking.
///
/// macOS lets any user open an ICMP datagram socket. Unlike Linux it keeps
/// the identifier and hands over whole IPv4 packets, every echo reply
/// included, so the reply is checked by sender, identifier and sequence.
pub fn icmp_echo(
    if_index: u32,
    source: Ipv4Addr,
    dest: Ipv4Addr,
    payload: &[u8],
    ttl: u8,
    timeout_ms: u32,
) -> Option<Vec<u8>> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::ICMPV4)).ok()?;
    // Without the interface the echo would follow the capture routes back
    // into the adapter.
    socket
        .bind_device_by_index_v4(Some(NonZeroU32::new(if_index)?))
        .ok()?;
    socket.bind(&SocketAddrV4::new(source, 0).into()).ok()?;
    socket.set_ttl_v4(u32::from(ttl)).ok()?;

    let id = std::process::id() as u16;
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    socket
        .send_to(
            &echo_request(id, seq, payload),
            &SocketAddrV4::new(dest, 0).into(),
        )
        .ok()?;

    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    let mut buf = vec![0u8; payload.len() + 128];
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        socket
            .set_read_timeout(Some(left.max(Duration::from_millis(1))))
            .ok()?;
        let len = (&socket).read(&mut buf).ok()?;
        let Some(icmp) = strip_ip_header(&buf[..len], dest) else {
            continue;
        };
        if let Some(data) = echo_reply(icmp, id, seq) {
            return Some(data.to_vec());
        }
    }
}

fn echo_request(id: u16, seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut msg = vec![ECHO_REQUEST, 0, 0, 0];
    msg.extend_from_slice(&id.to_be_bytes());
    msg.extend_from_slice(&seq.to_be_bytes());
    msg.extend_from_slice(payload);
    let sum = checksum(&msg);
    msg[2..4].copy_from_slice(&sum.to_be_bytes());
    msg
}

/// The internet checksum: the ones' complement of the ones' complement sum
/// of the 16-bit words.
fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = data
        .chunks(2)
        .map(|w| u32::from(u16::from_be_bytes([w[0], *w.get(1).unwrap_or(&0)])))
        .sum();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Keeps the ICMP part of an IPv4 packet if it came from `from`.
fn strip_ip_header(packet: &[u8], from: Ipv4Addr) -> Option<&[u8]> {
    let first = *packet.first()?;
    if first >> 4 != 4 {
        return None;
    }
    let header_len = usize::from(first & 0x0f) * 4;
    let sender: [u8; 4] = packet.get(12..16)?.try_into().ok()?;
    (Ipv4Addr::from(sender) == from).then_some(())?;
    packet.get(header_len..)
}

/// The data of an echo reply with identifier `id` and sequence number `seq`.
fn echo_reply(icmp: &[u8], id: u16, seq: u16) -> Option<&[u8]> {
    if icmp.len() < 8 || icmp[0] != ECHO_REPLY {
        return None;
    }
    (icmp[4..6] == id.to_be_bytes() && icmp[6..8] == seq.to_be_bytes()).then(|| &icmp[8..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_checksummed_request() {
        let msg = echo_request(0x1234, 1, b"hi");
        assert_eq!(&msg[..2], &[8, 0]);
        assert_eq!(&msg[4..8], &[0x12, 0x34, 0, 1]);
        assert_eq!(&msg[8..], b"hi");
        assert_eq!(checksum(&msg), 0);
    }

    #[test]
    fn matches_replies_from_the_target_only() {
        let mut reply = echo_request(7, 3, b"data");
        reply[0] = ECHO_REPLY;
        let mut packet = vec![
            0x45, 0, 0, 0, 0, 0, 0, 0, 64, 1, 0, 0, 1, 1, 1, 1, 10, 0, 0, 2,
        ];
        packet.extend(&reply);
        let icmp = strip_ip_header(&packet, Ipv4Addr::new(1, 1, 1, 1)).unwrap();
        assert_eq!(echo_reply(icmp, 7, 3), Some(&b"data"[..]));
        assert_eq!(echo_reply(icmp, 8, 3), None);
        assert_eq!(echo_reply(icmp, 7, 4), None);
        assert_eq!(strip_ip_header(&packet, Ipv4Addr::new(8, 8, 8, 8)), None);
        // Not an IPv4 packet.
        assert_eq!(strip_ip_header(&reply, Ipv4Addr::new(1, 1, 1, 1)), None);
    }

    /// Pings 1.1.1.1 through every connected network. Run with
    /// `cargo test live_icmp -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_icmp() {
        for itf in netdev::get_interfaces() {
            let Some(source) = itf.ipv4.first().map(|n| n.addr()) else {
                continue;
            };
            // Only the ports: a VPN's utun has a gateway too.
            if !super::super::adapters::has_connector(itf.index) || itf.gateway.is_none() {
                continue;
            }
            let reply = icmp_echo(
                itf.index,
                source,
                Ipv4Addr::new(1, 1, 1, 1),
                b"mp",
                64,
                2000,
            );
            println!("{} {source}: {reply:?}", itf.name);
            assert_eq!(reply.as_deref(), Some(&b"mp"[..]));
        }
    }
}
