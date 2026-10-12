//! Every flow the engine is relaying, for the connections list and for
//! closing flows whose network has gone.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

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
        flows.iter().for_each(|f| f.abort());
        flows.len()
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
        for (port, link) in [(1, "wifi"), (2, "phone"), (3, "wifi")] {
            let flow = flows.open(Proto::Tcp, addr(port), addr(443));
            flow.link.set(link.into()).unwrap();
            let guard = FlowGuard {
                flows: flows.clone(),
                flow: flow.clone(),
            };
            let task = tokio::spawn(async move {
                let _guard = guard;
                std::future::pending::<()>().await;
            });
            flows.set_abort(flow.id, task.abort_handle());
            tasks.push(task);
        }
        assert_eq!(flows.abort_on_link("wifi"), 2);
        for task in tasks {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(50), task).await;
        }
        let left: Vec<_> = flows.snapshot().iter().map(|f| f.src.port()).collect();
        assert_eq!(left, vec![2]);
    }
}
