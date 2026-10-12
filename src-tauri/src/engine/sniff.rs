//! Reads the site name from the first bytes a client sends: the TLS SNI or
//! the HTTP `Host` header. Used for sticky sessions and the connections list.

#[derive(Debug, PartialEq, Eq)]
pub enum Sniff {
    Host(String),
    /// The protocol carries no name (or isn't one we read).
    NoHost,
    /// The name may be in bytes that haven't arrived yet.
    NeedMore,
}

/// Enough for any ClientHello in practice, including post-quantum key shares.
pub const MAX_SNIFF_BYTES: usize = 16 * 1024;

const HTTP_METHODS: [&[u8]; 9] = [
    b"GET ",
    b"POST ",
    b"PUT ",
    b"HEAD ",
    b"DELETE ",
    b"OPTIONS ",
    b"PATCH ",
    b"CONNECT ",
    b"TRACE ",
];

pub fn sniff(buf: &[u8]) -> Sniff {
    if buf.is_empty() {
        return Sniff::NeedMore;
    }
    if buf[0] == 0x16 {
        return tls_sni(buf);
    }
    if HTTP_METHODS.iter().any(|m| buf.starts_with(m)) {
        return http_host(buf);
    }
    // Too short to tell yet.
    if HTTP_METHODS.iter().any(|m| m.starts_with(buf)) {
        return Sniff::NeedMore;
    }
    Sniff::NoHost
}

/// Bounds-checked reads over a ClientHello that may be cut short.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let bytes = self.buf.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(bytes)
    }

    fn u8(&mut self) -> Option<usize> {
        self.take(1).map(|b| b[0] as usize)
    }

    fn u16(&mut self) -> Option<usize> {
        self.take(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
    }

    fn u24(&mut self) -> Option<usize> {
        self.take(3)
            .map(|b| u32::from_be_bytes([0, b[0], b[1], b[2]]) as usize)
    }
}

fn tls_sni(buf: &[u8]) -> Sniff {
    let mut header = Cursor { buf, pos: 0 };
    let (Some(_), Some(_), Some(record_len)) = (header.u8(), header.u16(), header.u16()) else {
        return Sniff::NeedMore;
    };
    let record_end = 5 + record_len;
    let complete = buf.len() >= record_end;
    let mut c = Cursor {
        buf: &buf[..buf.len().min(record_end)],
        pos: 5,
    };
    match client_hello_sni(&mut c) {
        Some(Some(host)) => Sniff::Host(host),
        Some(None) => Sniff::NoHost,
        // Ran out of bytes: wait for the rest of the record, if there is any.
        None if complete => Sniff::NoHost,
        None => Sniff::NeedMore,
    }
}

/// `None` when the bytes run out; `Some(None)` when there's no SNI.
fn client_hello_sni(c: &mut Cursor) -> Option<Option<String>> {
    const CLIENT_HELLO: usize = 1;
    const SERVER_NAME: usize = 0;
    const HOST_NAME: usize = 0;

    if c.u8()? != CLIENT_HELLO {
        return Some(None);
    }
    c.u24()?; // handshake length
    c.take(2 + 32)?; // version, random
    let session_id = c.u8()?;
    c.take(session_id)?;
    let ciphers = c.u16()?;
    c.take(ciphers)?;
    let compression = c.u8()?;
    c.take(compression)?;
    let extensions_len = c.u16()?;
    let extensions_end = c.pos + extensions_len;
    while c.pos < extensions_end {
        let kind = c.u16()?;
        let len = c.u16()?;
        let data = c.take(len)?;
        if kind != SERVER_NAME {
            continue;
        }
        let mut names = Cursor { buf: data, pos: 2 };
        while let (Some(name_type), Some(name_len)) = (names.u8(), names.u16()) {
            let name = names.take(name_len)?;
            if name_type == HOST_NAME {
                return Some(
                    String::from_utf8(name.to_vec())
                        .ok()
                        .map(|h| h.to_lowercase()),
                );
            }
        }
        return Some(None);
    }
    Some(None)
}

fn http_host(buf: &[u8]) -> Sniff {
    let head_end = buf.windows(4).position(|w| w == b"\r\n\r\n");
    let head = &buf[..head_end.unwrap_or(buf.len())];
    let text = String::from_utf8_lossy(head);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    if !request_line.contains(" HTTP/") && head_end.is_some() {
        return Sniff::NoHost;
    }
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("host") {
                let host = value.trim();
                // Strip a port, but not the colons of a bracketed IPv6 address.
                let host = match host.rsplit_once(':') {
                    Some((h, port))
                        if !h.ends_with(':') && port.bytes().all(|b| b.is_ascii_digit()) =>
                    {
                        h
                    }
                    _ => host,
                };
                return Sniff::Host(host.trim_matches(['[', ']']).to_lowercase());
            }
        }
    }
    if head_end.is_some() || buf.len() >= MAX_SNIFF_BYTES {
        Sniff::NoHost
    } else {
        Sniff::NeedMore
    }
}

/// Groups subdomains of one site, so `www.bank.com` and `api.bank.com` stick
/// together. A heuristic without the public suffix list: two labels, or three
/// under country second-levels like `co.uk`.
pub fn site_key(host: &str) -> &str {
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    if labels.len() <= 2 || host.parse::<std::net::IpAddr>().is_ok() {
        return host;
    }
    let n = labels.len();
    let second = labels[n - 2];
    let country_second_level =
        labels[n - 1].len() == 2 && (second.len() <= 3 || matches!(second, "gouv" | "kommune"));
    let keep = if country_second_level { 3 } else { 2 };
    let skip: usize = labels[..n - keep].iter().map(|l| l.len() + 1).sum();
    &host[skip..]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ClientHello with the given extensions before and after SNI.
    fn client_hello(host: &str, padding_before: usize) -> Vec<u8> {
        let mut sni = Vec::new();
        let name = host.as_bytes();
        sni.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
        sni.push(0);
        sni.extend_from_slice(&(name.len() as u16).to_be_bytes());
        sni.extend_from_slice(name);

        let mut extensions = Vec::new();
        // A big key_share-like extension first, as Chrome may send.
        extensions.extend_from_slice(&51u16.to_be_bytes());
        extensions.extend_from_slice(&(padding_before as u16).to_be_bytes());
        extensions.extend(std::iter::repeat_n(0xAA, padding_before));
        extensions.extend_from_slice(&0u16.to_be_bytes());
        extensions.extend_from_slice(&(sni.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&sni);

        let mut body = vec![0x03, 0x03];
        body.extend([7u8; 32]);
        body.push(0); // session id
        body.extend_from_slice(&[0, 2, 0x13, 0x01]); // one cipher suite
        body.extend_from_slice(&[1, 0]); // null compression
        body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        body.extend(extensions);

        let mut handshake = vec![1];
        handshake.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        handshake.extend(body);

        let mut record = vec![0x16, 0x03, 0x01];
        record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        record.extend(handshake);
        record
    }

    #[test]
    fn reads_sni() {
        let hello = client_hello("Example.COM", 10);
        assert_eq!(sniff(&hello), Sniff::Host("example.com".into()));
    }

    #[test]
    fn waits_for_sni_split_across_segments() {
        let hello = client_hello("video.example.com", 1500);
        assert_eq!(sniff(&hello[..1200]), Sniff::NeedMore);
        assert_eq!(sniff(&hello), Sniff::Host("video.example.com".into()));
    }

    #[test]
    fn reads_http_host() {
        let req = b"GET / HTTP/1.1\r\nUser-Agent: x\r\nhost: www.Example.com:8080\r\n\r\n";
        assert_eq!(sniff(req), Sniff::Host("www.example.com".into()));
        assert_eq!(sniff(b"GET / HTTP/1.1\r\nUser-Ag"), Sniff::NeedMore);
        assert_eq!(sniff(b"GET / HTTP/1.1\r\n\r\n"), Sniff::NoHost);
        let v6 = b"GET / HTTP/1.1\r\nHost: [2001:db8::1]:80\r\n\r\n";
        assert_eq!(sniff(v6), Sniff::Host("2001:db8::1".into()));
    }

    #[test]
    fn other_protocols_have_no_host() {
        assert_eq!(sniff(b"SSH-2.0-OpenSSH"), Sniff::NoHost);
        assert_eq!(sniff(b"GE"), Sniff::NeedMore);
        assert_eq!(sniff(&[0x00, 0x01]), Sniff::NoHost);
    }

    #[test]
    fn site_keys_group_subdomains() {
        assert_eq!(site_key("www.bank.com"), "bank.com");
        assert_eq!(site_key("api.eu.bank.com"), "bank.com");
        assert_eq!(site_key("bank.com"), "bank.com");
        assert_eq!(site_key("shop.bbc.co.uk"), "bbc.co.uk");
        assert_eq!(site_key("1.2.3.4"), "1.2.3.4");
    }
}
