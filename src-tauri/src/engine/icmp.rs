//! Answers captured pings by sending a real echo out through a physical
//! network, so `ping` keeps working while MultiPath is on. IPv4 only.

use std::net::IpAddr;
use std::sync::Arc;

use ipstack::{IpNumber, IpStackUnknownTransport};

use crate::net::router::Router;
use crate::sys;

const ECHO_REPLY: u8 = 0;
const ECHO_REQUEST: u8 = 8;
const ECHO_TIMEOUT_MS: u32 = 4000;
const TTL: u8 = 64;

pub async fn handle(packet: IpStackUnknownTransport, router: Arc<Router>) {
    if packet.ip_protocol() != IpNumber::ICMP {
        return;
    }
    let IpAddr::V4(dest) = packet.dst_addr() else {
        return;
    };
    let Some((id_seq, data)) = parse_echo_request(packet.payload()) else {
        return;
    };
    let data = data.to_vec();
    let Some((if_index, source)) =
        router
            .choose(false, Some(&dest.to_string()), &[])
            .and_then(|link| {
                let info = link.info();
                Some((info.if_index, info.ipv4?))
            })
    else {
        return;
    };
    let echoed = tokio::task::spawn_blocking(move || {
        sys::icmp_echo(if_index, source, dest, &data, TTL, ECHO_TIMEOUT_MS)
    })
    .await;
    if let Ok(Some(echoed)) = echoed {
        let _ = packet.send(echo_reply(id_seq, &echoed));
    }
}

/// The identifier and sequence number, and the data, of an echo request.
fn parse_echo_request(icmp: &[u8]) -> Option<([u8; 4], &[u8])> {
    if icmp.len() < 8 || icmp[0] != ECHO_REQUEST || icmp[1] != 0 {
        return None;
    }
    Some((icmp[4..8].try_into().ok()?, &icmp[8..]))
}

fn echo_reply(id_seq: [u8; 4], data: &[u8]) -> Vec<u8> {
    let mut reply = vec![ECHO_REPLY, 0, 0, 0];
    reply.extend_from_slice(&id_seq);
    reply.extend_from_slice(data);
    let sum = checksum(&reply);
    reply[2..4].copy_from_slice(&sum.to_be_bytes());
    reply
}

/// The Internet checksum (RFC 1071).
fn checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = bytes
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u32)
        .sum();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_mirrors_the_request() {
        let request = [8, 0, 0xf7, 0xfc, 0x00, 0x01, 0x00, 0x02, b'h', b'i'];
        let (id_seq, data) = parse_echo_request(&request).unwrap();
        assert_eq!(id_seq, [0, 1, 0, 2]);
        let reply = echo_reply(id_seq, data);
        assert_eq!(reply[0], ECHO_REPLY);
        assert_eq!(&reply[4..], &request[4..]);
        // A packet with a correct checksum sums to zero.
        assert_eq!(checksum(&reply), 0);
    }

    #[test]
    fn ignores_everything_but_echo_requests() {
        assert!(parse_echo_request(&[0, 0, 0, 0, 0, 1, 0, 2]).is_none());
        assert!(parse_echo_request(&[8, 0, 0]).is_none());
    }

    #[test]
    fn checksum_handles_odd_lengths() {
        assert_eq!(checksum(&[0x45]), !0x4500);
    }
}
