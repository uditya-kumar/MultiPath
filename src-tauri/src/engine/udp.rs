//! Relays one captured UDP flow (one app socket talking to one destination)
//! through the network the balancer picks.

use std::io;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use ipstack::IpStackUdpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::flows::Flow;
use crate::net::router::Router;

const DNS_PORT: u16 = 53;
const MAX_DATAGRAM: usize = 64 * 1024;

pub async fn handle(inbound: IpStackUdpStream, router: Arc<Router>, flow: Arc<Flow>) {
    let dst = flow.dst;
    // QUIC hides the site name, so flows stick by destination address.
    let key = dst.ip().to_string();
    let (lease, socket) = match router.bind_udp(dst.is_ipv6(), Some(&key)) {
        Ok(bound) => bound,
        Err(err) => {
            log::debug!("udp {dst}: {err}");
            return;
        }
    };
    if let Err(err) = socket.connect(dst).await {
        log::debug!("udp {dst}: {err}");
        return;
    }
    let _ = flow.link.set(lease.link.id.clone());
    let link = &lease.link;

    let (mut in_r, mut in_w) = tokio::io::split(inbound);
    let mut up = vec![0u8; MAX_DATAGRAM];
    let mut down = vec![0u8; MAX_DATAGRAM];
    loop {
        tokio::select! {
            // The stack ends the flow (with an error) after it has been idle.
            read = in_r.read(&mut up) => {
                let Ok(n @ 1..) = read else { break };
                if socket.send(&up[..n]).await.is_ok() {
                    link.stats.tx.fetch_add(n as u64, Ordering::Relaxed);
                    flow.add_tx(n as u64);
                }
            }
            received = socket.recv(&mut down) => match received {
                Ok(n) => {
                    if in_w.write_all(&down[..n]).await.is_err() {
                        break;
                    }
                    link.stats.rx.fetch_add(n as u64, Ordering::Relaxed);
                    flow.add_rx(n as u64);
                    // A DNS lookup is one question and one answer.
                    if dst.port() == DNS_PORT {
                        break;
                    }
                }
                // An ICMP "port unreachable" for an earlier datagram; the
                // flow itself may carry on.
                Err(err) if err.kind() == io::ErrorKind::ConnectionReset => {}
                Err(_) => break,
            },
        }
    }
}
