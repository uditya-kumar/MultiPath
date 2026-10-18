//! Relays one captured TCP connection through the network the balancer picks.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use ipstack::IpStackTcpStream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Notify;
use tokio::time::Instant;

use super::flows::Flow;
use super::sniff::{self, Sniff, MAX_SNIFF_BYTES};
use crate::net::router::{Lease, Router};

/// Ports where the client speaks first and names the site.
const SNIFF_PORTS: [u16; 4] = [80, 443, 8080, 8443];
/// How long to wait for that first message before connecting without it.
const SNIFF_WAIT: Duration = Duration::from_millis(400);
/// Each direction's copy buffer starts small and doubles while reads fill it,
/// so the thousands of idle flows a busy PC keeps open hold little memory and
/// a download still gets the big buffer after a few reads.
const COPY_BUFFER_MIN: usize = 4 * 1024;
const COPY_BUFFER_MAX: usize = 64 * 1024;
/// How long the close may wait for the app to take what was already sent.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

pub async fn handle(mut inbound: IpStackTcpStream, router: Arc<Router>, flow: Arc<Flow>) {
    let dst = flow.dst;
    // No network can carry it (IPv6 switched off, or on no network): close it
    // at once instead of first waiting for the site name.
    if router.choose(dst.is_ipv6(), None, &[]).is_none() {
        return;
    }
    let first = if SNIFF_PORTS.contains(&dst.port()) {
        match read_first(&mut inbound, &flow).await {
            Some(first) => first,
            None => return,
        }
    } else {
        Vec::new()
    };

    let key = flow
        .host
        .get()
        .map(|h| sniff::site_key(h).to_owned())
        .unwrap_or_else(|| dst.ip().to_string());
    let (lease, outbound) = match router.connect_tcp(dst, Some(&key)).await {
        Ok(connected) => connected,
        Err(err) => {
            log::debug!("tcp {dst} ({key}): {err}");
            return;
        }
    };
    let _ = flow.link.set(lease.link.id.clone());

    let (mut out_r, mut out_w) = outbound.into_split();
    if !first.is_empty() {
        if out_w.write_all(&first).await.is_err() {
            return;
        }
        count_tx(&lease, &flow, first.len());
    }
    let tx = |n| count_tx(&lease, &flow, n);
    let rx = |n| count_rx(&lease, &flow, n);
    if let Err(err) = relay(inbound, &mut out_r, &mut out_w, &flow.lost, tx, rx).await {
        log::trace!("tcp {dst}: {err}");
    }
}

/// Copies both ways until the app's end closes, either side fails, or `lost`
/// says the network has gone.
///
/// The stack has no half-close: it only reports the app's end once the whole
/// connection is gone, so nothing more can reach the app. Waiting for the site
/// to close as well kept flows open for as long as a quiet site didn't notice.
async fn relay<A, R, W>(
    app: A,
    site_r: &mut R,
    site_w: &mut W,
    lost: &Notify,
    tx: impl Fn(usize),
    rx: impl Fn(usize),
) -> io::Result<()>
where
    A: AsyncRead + AsyncWrite,
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let (mut app_r, mut app_w) = tokio::io::split(app);
    let up = pump(&mut app_r, site_w, tx);
    let down = async {
        pump(site_r, &mut app_w, rx).await?;
        // The site is done sending; the app may still be.
        std::future::pending().await
    };
    let relayed = tokio::select! {
        relayed = up => Some(relayed),
        relayed = down => Some(relayed),
        _ = lost.notified() => None,
    };
    match relayed {
        Some(relayed) => relayed,
        None => {
            // The network under it has gone. A close tells the app to reconnect.
            let _ = tokio::time::timeout(CLOSE_WAIT, app_w.shutdown()).await;
            Ok(())
        }
    }
}

/// Reads what the client sends first, until the site name is known or it's
/// clear there isn't one. `None` if the client went away.
async fn read_first(inbound: &mut IpStackTcpStream, flow: &Flow) -> Option<Vec<u8>> {
    let deadline = Instant::now() + SNIFF_WAIT;
    let mut first = Vec::new();
    let mut buf = vec![0u8; 4096];
    while first.len() < MAX_SNIFF_BYTES {
        match tokio::time::timeout_at(deadline, inbound.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => return None,
            Ok(Ok(n)) => first.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        match sniff::sniff(&first) {
            Sniff::Host(host) => {
                let _ = flow.host.set(host);
                break;
            }
            Sniff::NoHost => break,
            Sniff::NeedMore => {}
        }
    }
    Some(first)
}

fn count_tx(lease: &Lease, flow: &Flow, n: usize) {
    lease.count_tx(n as u64);
    flow.add_tx(n as u64);
}

fn count_rx(lease: &Lease, flow: &Flow, n: usize) {
    lease.count_rx(n as u64);
    flow.add_rx(n as u64);
}

/// Copies one direction until EOF, then passes the close on.
async fn pump<R, W>(r: &mut R, w: &mut W, count: impl Fn(usize)) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; COPY_BUFFER_MIN];
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            return w.shutdown().await;
        }
        w.write_all(&buf[..n]).await?;
        count(n);
        if n == buf.len() && buf.len() < COPY_BUFFER_MAX {
            buf.resize(buf.len() * 2, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::task::{Context, Poll};
    use tokio::io::ReadBuf;

    /// Fills every buffer it's given, a fixed number of times, and remembers
    /// how big each one was.
    struct Firehose<'a> {
        reads_left: usize,
        sizes: &'a Mutex<Vec<usize>>,
    }

    impl AsyncRead for Firehose<'_> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if self.reads_left > 0 {
                self.reads_left -= 1;
                self.sizes.lock().unwrap().push(buf.remaining());
                buf.put_slice(&vec![7; buf.remaining()]);
            }
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn pump_buffer_starts_small_and_grows_while_reads_fill_it() {
        let sizes = Mutex::new(Vec::new());
        let mut src = Firehose {
            reads_left: 7,
            sizes: &sizes,
        };
        pump(&mut src, &mut tokio::io::sink(), |_| {})
            .await
            .unwrap();
        let k = 1024;
        assert_eq!(
            *sizes.lock().unwrap(),
            [4 * k, 8 * k, 16 * k, 32 * k, 64 * k, 64 * k, 64 * k]
        );
    }

    #[tokio::test]
    async fn pump_buffer_stays_small_for_small_reads() {
        let (mut src, mut src_far) = tokio::io::duplex(1024 * 1024);
        let writer = async {
            for _ in 0..50 {
                src_far.write_all(&[1; 100]).await.unwrap();
                tokio::task::yield_now().await;
            }
            src_far.shutdown().await.unwrap();
        };
        let largest = AtomicUsize::new(0);
        let copier = async {
            let mut buf = Vec::new();
            pump(&mut src, &mut buf, |n| {
                largest.fetch_max(n, Ordering::Relaxed);
            })
            .await
            .unwrap();
            buf.len()
        };
        let (_, copied) = tokio::join!(writer, copier);
        assert_eq!(copied, 5000);
        assert!(largest.load(Ordering::Relaxed) < COPY_BUFFER_MIN);
    }

    /// An app end and a site end for `relay`, with the far side of each.
    fn pipes() -> (
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
    ) {
        let (app, app_far) = tokio::io::duplex(64 * 1024);
        let (site, site_far) = tokio::io::duplex(64 * 1024);
        (app, app_far, site, site_far)
    }

    #[tokio::test]
    async fn relay_ends_when_the_app_closes_even_if_the_site_stays_quiet() {
        let (app, mut app_far, site, mut site_far) = pipes();
        let (mut site_r, mut site_w) = tokio::io::split(site);
        let lost = Notify::new();
        app_far.write_all(b"bye").await.unwrap();
        app_far.shutdown().await.unwrap();
        let relayed = relay(app, &mut site_r, &mut site_w, &lost, |_| {}, |_| {});
        tokio::time::timeout(Duration::from_secs(5), relayed)
            .await
            .expect("relay waited for the site")
            .unwrap();
        // What the app sent, then its close, reached the site.
        let mut got = Vec::new();
        site_far.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, b"bye");
    }

    #[tokio::test]
    async fn relay_carries_on_after_the_site_closes() {
        let (app, mut app_far, site, mut site_far) = pipes();
        let (mut site_r, mut site_w) = tokio::io::split(site);
        let lost = Notify::new();
        let relayed = relay(app, &mut site_r, &mut site_w, &lost, |_| {}, |_| {});
        let ends = async {
            site_far.write_all(b"answer").await.unwrap();
            site_far.shutdown().await.unwrap();
            let mut got = Vec::new();
            app_far.read_to_end(&mut got).await.unwrap();
            assert_eq!(got, b"answer");
            // The app can still send after the site's close.
            app_far.write_all(b"more").await.unwrap();
            let mut more = [0u8; 4];
            site_far.read_exact(&mut more).await.unwrap();
            assert_eq!(&more, b"more");
            app_far.shutdown().await.unwrap();
        };
        let (relayed, _) = tokio::join!(relayed, ends);
        relayed.unwrap();
    }

    #[tokio::test]
    async fn relay_closes_the_app_end_when_the_network_is_lost() {
        let (app, mut app_far, site, _site_far) = pipes();
        let (mut site_r, mut site_w) = tokio::io::split(site);
        let lost = Notify::new();
        lost.notify_one();
        relay(app, &mut site_r, &mut site_w, &lost, |_| {}, |_| {})
            .await
            .unwrap();
        let mut got = Vec::new();
        app_far.read_to_end(&mut got).await.unwrap();
        assert!(got.is_empty());
    }

    #[tokio::test]
    async fn pump_copies_counts_and_closes() {
        let (mut src, mut src_far) = tokio::io::duplex(1024);
        let (mut dst, mut dst_far) = tokio::io::duplex(1024 * 1024);
        let total = AtomicUsize::new(0);

        let data: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        let writer = async {
            src_far.write_all(&data).await.unwrap();
            src_far.shutdown().await.unwrap();
        };
        let copier = pump(&mut src, &mut dst, |n| {
            total.fetch_add(n, Ordering::Relaxed);
        });
        let (_, copied) = tokio::join!(writer, copier);
        copied.unwrap();

        let mut received = Vec::new();
        dst_far.read_to_end(&mut received).await.unwrap();
        assert_eq!(received, data);
        assert_eq!(total.load(Ordering::Relaxed), data.len());
    }
}
