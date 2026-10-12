//! Relays one captured TCP connection through the network the balancer picks.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use ipstack::IpStackTcpStream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use super::flows::Flow;
use super::sniff::{self, Sniff, MAX_SNIFF_BYTES};
use crate::net::router::{Lease, Router};

/// Ports where the client speaks first and names the site.
const SNIFF_PORTS: [u16; 4] = [80, 443, 8080, 8443];
/// How long to wait for that first message before connecting without it.
const SNIFF_WAIT: Duration = Duration::from_millis(400);
const COPY_BUFFER: usize = 64 * 1024;

pub async fn handle(mut inbound: IpStackTcpStream, router: Arc<Router>, flow: Arc<Flow>) {
    let dst = flow.dst;
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
    let (mut in_r, mut in_w) = tokio::io::split(inbound);
    let up = pump(&mut in_r, &mut out_w, |n| count_tx(&lease, &flow, n));
    let down = pump(&mut out_r, &mut in_w, |n| count_rx(&lease, &flow, n));
    // Either side failing ends both; a clean close lets the other side finish.
    if let Err(err) = tokio::try_join!(up, down) {
        log::trace!("tcp {dst}: {err}");
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
    let mut buf = vec![0u8; COPY_BUFFER];
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            return w.shutdown().await;
        }
        w.write_all(&buf[..n]).await?;
        count(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
