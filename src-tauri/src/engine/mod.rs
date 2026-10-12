//! Captures the PC's traffic on a virtual adapter and relays every flow out
//! through one of the physical networks.
//!
//! The engine owns the adapter, the capture routes and the relays. Keeping
//! the link list fresh and probing health is the caller's job.

mod flows;
mod icmp;
mod sniff;
mod tcp;
mod udp;

use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ipstack::{IpStack, IpStackConfig, IpStackStream, TcpConfig};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tun::AbstractDevice;

pub use flows::{Flow, Flows};

use crate::net::links::Links;
use crate::net::router::{Proto, Router};
use crate::sys::{self, PreferIpv4, Route};
use flows::FlowGuard;

/// Name of the Wintun adapter MultiPath creates.
pub const TUN_NAME: &str = "MultiPath";
pub const TUN_V4: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
/// A unique local address, so IPv6 can be captured too.
pub const TUN_V6: Ipv6Addr = Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1);
const MTU: u16 = 1500;
const TUN_METRIC: u16 = 5;

/// Two halves of the address space: more specific than any default route,
/// so they win without touching the real routes.
const CAPTURE_V4: [(IpAddr, u8); 2] = [
    (IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 1),
    (IpAddr::V4(Ipv4Addr::new(128, 0, 0, 0)), 1),
];
const CAPTURE_V6: [(IpAddr, u8); 2] = [
    (IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 1),
    (IpAddr::V6(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0)), 1),
];

/// Idle connections (chat, push) stay up this long without traffic.
const TCP_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const UDP_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// Larger than the stack's 16 KB default, which caps a flow's throughput.
const TCP_WINDOW: usize = 256 * 1024;

pub struct EngineConfig {
    /// Full path of `wintun.dll`.
    pub wintun: PathBuf,
    /// DNS servers set on the adapter, so lookups go through MultiPath too.
    pub dns: Vec<IpAddr>,
}

pub struct Engine {
    pub flows: Arc<Flows>,
    routes: Vec<Route>,
    prefer_ipv4: Option<PreferIpv4>,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl Engine {
    /// Creates the adapter and starts capturing. Needs administrator rights.
    pub async fn start(
        config: EngineConfig,
        links: Arc<Links>,
        router: Arc<Router>,
    ) -> io::Result<Engine> {
        if !config.wintun.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is missing", config.wintun.display()),
            ));
        }
        clean_up_stale();

        let device = tokio::task::spawn_blocking(move || create_adapter(&config))
            .await
            .map_err(io::Error::other)??;
        let if_index = device.tun_index().map_err(io::Error::other)? as u32;
        let device = tun::AsyncDevice::new(device)?;

        let mut routes = Vec::new();
        // IPv6 is always captured, even when no network has it yet or it's
        // switched off: otherwise it would go straight out of whichever
        // network has an IPv6 route, around MultiPath. Flows the router has
        // no link for are refused.
        sys::add_unicast_address(if_index, IpAddr::V6(TUN_V6), 64)?;
        for (prefix, len) in CAPTURE_V4.into_iter().chain(CAPTURE_V6) {
            routes.push(Route::add(if_index, prefix, len)?);
        }

        // Sites with both families are reached over IPv4, which every
        // network has, so their connections can be spread across all of them.
        let prefer_ipv4 = PreferIpv4::apply()
            .inspect_err(|err| log::warn!("couldn't make IPv4 preferred: {err}"))
            .ok();

        let stack = IpStack::new(stack_config(), device);
        let flows = Arc::new(Flows::default());
        let (stop, stopped) = watch::channel(false);
        let tasks = vec![
            tokio::spawn(accept(stack, router, flows.clone(), stopped.clone())),
            tokio::spawn(close_flows_on_lost_links(links, flows.clone(), stopped)),
        ];
        log::info!("engine started on interface {if_index}");
        Ok(Engine {
            flows,
            routes,
            prefer_ipv4,
            stop,
            tasks,
        })
    }

    /// False once the packet stack has stopped by itself.
    pub fn is_running(&self) -> bool {
        self.tasks.iter().all(|t| !t.is_finished())
    }

    /// Stops capturing and removes the adapter.
    pub async fn stop(self) {
        // Routes first, so traffic goes straight back to the real networks.
        drop(self.routes);
        drop(self.prefer_ipv4);
        let _ = self.stop.send(true);
        for task in self.tasks {
            let _ = task.await;
        }
        log::info!("engine stopped");
    }
}

fn create_adapter(config: &EngineConfig) -> io::Result<tun::Device> {
    let mut tun_config = tun::Configuration::default();
    tun_config
        .tun_name(TUN_NAME)
        .address(TUN_V4)
        .netmask(Ipv4Addr::new(255, 255, 255, 0))
        .mtu(MTU)
        .metric(TUN_METRIC)
        .up();
    let wintun = config.wintun.clone();
    let dns = config.dns.clone();
    tun_config.platform_config(move |p| {
        p.wintun_file(wintun.into_os_string());
        if !dns.is_empty() {
            p.dns_servers(&dns);
        }
    });
    tun::create(&tun_config).map_err(io::Error::other)
}

fn stack_config() -> IpStackConfig {
    let mut tcp = TcpConfig::default();
    tcp.timeout = TCP_IDLE_TIMEOUT;
    tcp.max_unacked_bytes = TCP_WINDOW as u32;
    tcp.read_buffer_size = TCP_WINDOW;
    let mut config = IpStackConfig::default();
    config
        .mtu_unchecked(MTU)
        .udp_timeout(UDP_IDLE_TIMEOUT)
        .with_tcp_config(tcp);
    config
}

/// Removes capture routes a crashed run left on the adapter, which would
/// otherwise send all traffic into an adapter nobody reads.
pub fn clean_up_stale() {
    let Some(adapter) = netdev::get_interfaces()
        .into_iter()
        .find(|i| i.friendly_name.as_deref() == Some(TUN_NAME))
    else {
        return;
    };
    for (prefix, len) in CAPTURE_V4.into_iter().chain(CAPTURE_V6) {
        if Route::remove_stale(adapter.index, prefix, len) {
            log::info!("removed stale route {prefix}/{len}");
        }
    }
}

/// Returns once the engine is told to stop, or once it's gone.
async fn stopped(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow_and_update() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}

async fn accept(
    mut stack: IpStack,
    router: Arc<Router>,
    flows: Arc<Flows>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            stream = stack.accept() => match stream {
                Ok(stream) => dispatch(stream, &router, &flows),
                Err(err) => {
                    log::error!("packet stack stopped: {err}");
                    break;
                }
            },
            _ = stopped(&mut stop) => break,
        }
    }
    flows.abort_all();
}

fn dispatch(stream: IpStackStream, router: &Arc<Router>, flows: &Arc<Flows>) {
    match stream {
        IpStackStream::Tcp(tcp) => {
            let flow = flows.open(Proto::Tcp, tcp.local_addr(), tcp.peer_addr());
            spawn_flow(flows, flow.clone(), tcp::handle(tcp, router.clone(), flow));
        }
        IpStackStream::Udp(udp) => {
            let flow = flows.open(Proto::Udp, udp.local_addr(), udp.peer_addr());
            spawn_flow(flows, flow.clone(), udp::handle(udp, router.clone(), flow));
        }
        IpStackStream::UnknownTransport(packet) => {
            tokio::spawn(icmp::handle(packet, router.clone()));
        }
        IpStackStream::UnknownNetwork(_) => {}
    }
}

fn spawn_flow(
    flows: &Arc<Flows>,
    flow: Arc<Flow>,
    relay: impl Future<Output = ()> + Send + 'static,
) {
    let id = flow.id;
    let guard = FlowGuard {
        flows: flows.clone(),
        flow,
    };
    let task = tokio::spawn(async move {
        let _guard = guard;
        relay.await;
    });
    flows.set_abort(id, task.abort_handle());
}

/// New flows already avoid a network that's gone or failing its probes; this
/// closes the flows still on it, so apps reconnect instead of hanging.
async fn close_flows_on_lost_links(
    links: Arc<Links>,
    flows: Arc<Flows>,
    mut stop: watch::Receiver<bool>,
) {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = stopped(&mut stop) => return,
        }
        for link in links.all() {
            if !link.usable() {
                let closed = flows.abort_on_link(&link.id);
                if closed > 0 {
                    log::info!(
                        "{}: closed {closed} flows after the network was lost",
                        link.id
                    );
                }
            }
        }
    }
}
