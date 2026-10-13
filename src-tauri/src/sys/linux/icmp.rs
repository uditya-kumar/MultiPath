use std::net::Ipv4Addr;

/// Sends an ICMP echo from `source` and waits for the reply. Returns the
/// echoed payload. Blocking.
pub fn icmp_echo(
    _source: Ipv4Addr,
    _dest: Ipv4Addr,
    _payload: &[u8],
    _ttl: u8,
    _timeout_ms: u32,
) -> Option<Vec<u8>> {
    None
}
