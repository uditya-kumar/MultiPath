//! Sits between the adapter and the packet stack, and turns away IPv6 flows
//! that no network can carry before the stack accepts them.
//!
//! The stack completes every TCP handshake itself, so an app that tried
//! IPv6 would believe it had connected and wait on a dead connection. A
//! reset to its SYN instead makes it fall back to IPv4 right away (Happy
//! Eyeballs), and every network has IPv4.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use std::time::{Duration, Instant};

use etherparse::{NetSlice, PacketBuilder, SlicedPacket, TransportSlice};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::net::router::Router;

/// How long an answer to "can any network carry IPv6?" is reused, so the
/// router isn't asked for every UDP packet.
const RECHECK: Duration = Duration::from_secs(1);
const HOP_LIMIT: u8 = 64;

pub struct Gate<D> {
    inner: D,
    router: Arc<Router>,
    ipv6: Option<(bool, Instant)>,
    /// A reset that couldn't be written yet.
    reply: Option<Vec<u8>>,
}

impl<D> Gate<D> {
    pub fn new(inner: D, router: Arc<Router>) -> Self {
        Gate {
            inner,
            router,
            ipv6: None,
            reply: None,
        }
    }

    fn ipv6_usable(&mut self) -> bool {
        match self.ipv6 {
            Some((usable, at)) if at.elapsed() < RECHECK => usable,
            _ => {
                let usable = self.router.choose(true, None, &[]).is_some();
                self.ipv6 = Some((usable, Instant::now()));
                usable
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Pass,
    Drop,
    /// Answer with this packet instead.
    Reset(Vec<u8>),
}

/// What to do with one packet from the adapter. `ipv6_usable` is only asked
/// about packets that start or carry an IPv6 flow.
fn verdict(packet: &[u8], ipv6_usable: impl FnOnce() -> bool) -> Verdict {
    let Ok(sliced) = SlicedPacket::from_ip(packet) else {
        return Verdict::Pass;
    };
    let Some(NetSlice::Ipv6(ip)) = &sliced.net else {
        return Verdict::Pass;
    };
    match &sliced.transport {
        Some(TransportSlice::Tcp(tcp)) if tcp.syn() && !tcp.ack() => {
            if ipv6_usable() {
                return Verdict::Pass;
            }
            let header = ip.header();
            // The reset acknowledges the SYN, so the app's stack accepts it.
            let ack = tcp
                .sequence_number()
                .wrapping_add(1)
                .wrapping_add(tcp.payload().len() as u32);
            let builder = PacketBuilder::ipv6(header.destination(), header.source(), HOP_LIMIT)
                .tcp(tcp.destination_port(), tcp.source_port(), 0, 0)
                .rst()
                .ack(ack);
            let mut reply = Vec::with_capacity(builder.size(0));
            match builder.write(&mut reply, &[]) {
                Ok(()) => Verdict::Reset(reply),
                Err(_) => Verdict::Drop,
            }
        }
        Some(TransportSlice::Udp(_)) if !ipv6_usable() => Verdict::Drop,
        _ => Verdict::Pass,
    }
}

impl<D: AsyncRead + AsyncWrite + Unpin> Gate<D> {
    /// Writes a pending reset if the adapter takes it now; otherwise it is
    /// kept for the next read.
    fn send_reply(&mut self, cx: &mut Context<'_>) {
        if let Some(reply) = &self.reply {
            if Pin::new(&mut self.inner).poll_write(cx, reply).is_ready() {
                self.reply = None;
            }
        }
    }
}

impl<D: AsyncRead + AsyncWrite + Unpin> AsyncRead for Gate<D> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            this.send_reply(cx);
            // Each read from the adapter is one packet.
            let start = buf.filled().len();
            ready!(Pin::new(&mut this.inner).poll_read(cx, buf))?;
            let packet = &buf.filled()[start..];
            if packet.is_empty() {
                return Poll::Ready(Ok(()));
            }
            match verdict(packet, || this.ipv6_usable()) {
                Verdict::Pass => return Poll::Ready(Ok(())),
                Verdict::Drop => {}
                Verdict::Reset(reply) => this.reply = Some(reply),
            }
            buf.set_filled(start);
        }
    }
}

impl<D: AsyncWrite + Unpin> AsyncWrite for Gate<D> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: [u8; 16] = [0xfd, 0x77, 0x6d, 0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    const SITE: [u8; 16] = [
        0x26, 0, 0x14, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x24, 0xfe,
    ];

    fn syn() -> Vec<u8> {
        let builder = PacketBuilder::ipv6(APP, SITE, 64)
            .tcp(50000, 443, 1000, 64240)
            .syn();
        let mut packet = Vec::new();
        builder.write(&mut packet, &[]).unwrap();
        packet
    }

    #[test]
    fn a_syn_with_nowhere_to_go_is_reset() {
        let Verdict::Reset(reply) = verdict(&syn(), || false) else {
            panic!("expected a reset");
        };
        let sliced = SlicedPacket::from_ip(&reply).unwrap();
        let Some(NetSlice::Ipv6(ip)) = &sliced.net else {
            panic!("not IPv6");
        };
        assert_eq!(ip.header().source(), SITE);
        assert_eq!(ip.header().destination(), APP);
        let Some(TransportSlice::Tcp(tcp)) = &sliced.transport else {
            panic!("not TCP");
        };
        assert!(tcp.rst() && tcp.ack());
        assert_eq!(tcp.acknowledgment_number(), 1001);
        assert_eq!((tcp.source_port(), tcp.destination_port()), (443, 50000));
    }

    #[test]
    fn a_syn_passes_when_ipv6_works() {
        assert_eq!(verdict(&syn(), || true), Verdict::Pass);
    }

    #[test]
    fn ipv4_is_never_asked_about() {
        let builder = PacketBuilder::ipv4([10, 77, 0, 1], [1, 1, 1, 1], 64)
            .tcp(50000, 443, 1000, 64240)
            .syn();
        let mut packet = Vec::new();
        builder.write(&mut packet, &[]).unwrap();
        assert_eq!(
            verdict(&packet, || panic!("asked about IPv4")),
            Verdict::Pass
        );
    }

    #[test]
    fn ipv6_udp_is_dropped_with_nowhere_to_go() {
        let builder = PacketBuilder::ipv6(APP, SITE, 64).udp(50000, 53);
        let mut packet = Vec::new();
        builder.write(&mut packet, &[0; 12]).unwrap();
        assert_eq!(verdict(&packet, || false), Verdict::Drop);
        assert_eq!(verdict(&packet, || true), Verdict::Pass);
    }
}
