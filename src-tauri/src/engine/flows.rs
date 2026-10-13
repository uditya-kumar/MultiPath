//! Every flow the engine is relaying, for the connections list and for
//! closing flows whose network has gone.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tokio::sync::Notify;
use tokio::task::AbortHandle;

use crate::net::router::Proto;

pub struct Flow {
    pub id: u64,
    pub proto: Proto,
    /// The app's end, on the virtual adapter.
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub started: Instant,
    pub host: OnceLock<String>,
    /// Id of the network carrying the flow, once it's connected.
    pub link: OnceLock<String>,
    pub rx: AtomicU64,
    pub tx: AtomicU64,
    /// Told when a TCP flow's network has gone, so its relay can close the
    /// app's end properly.
    pub lost: Notify,
    losing: AtomicBool,
    abort: Mutex<Option<AbortHandle>>,
}

impl Flow {
    pub fn add_rx(&self, n: u64) {
        self.rx.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_tx(&self, n: u64) {
        self.tx.fetch_add(n, Ordering::Relaxed);
    }

    fn abort(&self) {
        if let Some(handle) = self.abort.lock().unwrap().as_ref() {
            handle.abort();
        }
    }

    /// Ends a flow whose network has gone. Dropping a TCP connection would
    /// leave the app waiting for data that never comes; closing it makes the
    /// app reconnect, through another network. UDP has nothing to close.
    /// False if it was already told, as it stays listed while it closes.
    fn lose(&self) -> bool {
        if self.losing.swap(true, Ordering::Relaxed) {
            return false;
        }
        match self.proto {
            Proto::Tcp => self.lost.notify_one(),
            Proto::Udp => self.abort(),
        }
        true
    }
}

#[derive(Default)]
pub struct Flows {
    next_id: AtomicU64,
    map: Mutex<HashMap<u64, Arc<Flow>>>,
}

impl Flows {
    pub fn open(&self, proto: Proto, src: SocketAddr, dst: SocketAddr) -> Arc<Flow> {
        let flow = Arc::new(Flow {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            proto,
            src,
            dst,
            started: Instant::now(),
            host: OnceLock::new(),
            link: OnceLock::new(),
            rx: AtomicU64::new(0),
            tx: AtomicU64::new(0),
            lost: Notify::new(),
            losing: AtomicBool::new(false),
            abort: Mutex::new(None),
        });
        self.map.lock().unwrap().insert(flow.id, flow.clone());
        flow
    }

    /// Lets the flow be closed from outside. A flow that has already ended
    /// is left alone.
    pub fn set_abort(&self, id: u64, handle: AbortHandle) {
        if let Some(flow) = self.map.lock().unwrap().get(&id) {
            *flow.abort.lock().unwrap() = Some(handle);
        }
    }

    pub fn close(&self, id: u64) {
        self.map.lock().unwrap().remove(&id);
    }

    pub fn snapshot(&self) -> Vec<Arc<Flow>> {
        self.map.lock().unwrap().values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.map.lock().unwrap().len()
    }

    /// Closes every flow on `link_id`. Returns how many were closed.
    pub fn abort_on_link(&self, link_id: &str) -> usize {
        let flows: Vec<Arc<Flow>> = self
            .snapshot()
            .into_iter()
            .filter(|f| f.link.get().map(String::as_str) == Some(link_id))
            .collect();
        flows.iter().filter(|f| f.lose()).count()
    }

    pub fn abort_all(&self) {
        self.snapshot().iter().for_each(|f| f.abort());
    }
}

/// Removes its flow from the table when the task relaying it ends, however
/// it ends.
pub struct FlowGuard {
    pub flows: Arc<Flows>,
    pub flow: Arc<Flow>,
}

impl Drop for FlowGuard {
    fn drop(&mut self) {
        self.flows.close(self.flow.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([10, 77, 0, 1], port))
    }

    #[tokio::test]
    async fn aborting_a_link_closes_only_its_flows() {
        let flows = Arc::new(Flows::default());
        let mut tasks = Vec::new();
        let open = [
            (1, Proto::Tcp, "wifi"),
            (2, Proto::Tcp, "phone"),
            (3, Proto::Udp, "wifi"),
        ];
        for (port, proto, link) in open {
            let flow = flows.open(proto, addr(port), addr(443));
            flow.link.set(link.into()).unwrap();
            let guard = FlowGuard {
                flows: flows.clone(),
                flow: flow.clone(),
            };
            // A TCP relay ends itself when told; a UDP one is stopped.
            let task = tokio::spawn(async move {
                match guard.flow.proto {
                    Proto::Tcp => guard.flow.lost.notified().await,
                    Proto::Udp => std::future::pending::<()>().await,
                }
            });
            flows.set_abort(flow.id, task.abort_handle());
            tasks.push(task);
        }
        assert_eq!(flows.abort_on_link("wifi"), 2);
        // Still closing: not counted again.
        assert_eq!(flows.abort_on_link("wifi"), 0);
        for task in tasks {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(50), task).await;
        }
        let left: Vec<_> = flows.snapshot().iter().map(|f| f.src.port()).collect();
        assert_eq!(left, vec![2]);
    }
}
