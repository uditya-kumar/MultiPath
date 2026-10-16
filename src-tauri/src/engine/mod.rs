//! Captures the PC's traffic on a virtual adapter and relays every flow out
//! through one of the physical networks.
//!
//! The engine owns the adapter, the capture routes and the relays. Keeping
//! the link list fresh and probing health is the caller's job.

mod flows;
mod gate;
mod icmp;
#[cfg(target_os = "macos")]
mod privileged;
mod sniff;
mod tcp;
mod udp;

use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[cfg(windows)]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ipstack::{IpStack, IpStackConfig, IpStackStream, TcpConfig};
use tokio::sync::watch;
use tokio::task::JoinHandle;
#[cfg(not(target_os = "macos"))]
use tun::AbstractDevice;

pub use flows::{Flow, Flows};
#[cfg(target_os = "macos")]
pub use privileged::{run_helper, HELPER_ARG};

use crate::net::links::Links;
use crate::net::router::{Proto, Router};
#[cfg(target_os = "linux")]
use crate::sys;
#[cfg(target_os = "linux")]
use crate::sys::LooseRpFilter;
#[cfg(windows)]
use crate::sys::PreferIpv4;
use crate::sys::Route;
use flows::FlowGuard;

/// Name of the virtual adapter MultiPath creates. macOS names every utun
/// itself (`utunN`), so there the adapter is known by its address.
pub const TUN_NAME: &str = "MultiPath";
pub const TUN_V4: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
/// A unique local address, so IPv6 can be captured too.
pub const TUN_V6: Ipv6Addr = Ipv6Addr::new(0xfd77, 0x6d70, 0, 0, 0, 0, 0, 1);
const MTU: u16 = 1500;
#[cfg(windows)]
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
    /// DNS servers set on the adapter, so lookups go through MultiPath too.
    pub dns: Vec<IpAddr>,
}

pub struct Engine {
    pub flows: Arc<Flows>,
    #[cfg(not(target_os = "macos"))]
    routes: Vec<Route>,
    /// The routes and DNS, held by the privileged helper or by this process.
    #[cfg(target_os = "macos")]
    tunnel: privileged::Tunnel,
    #[cfg(windows)]
    prefer_ipv4: Option<PreferIpv4>,
    #[cfg(target_os = "linux")]
    loose_rp_filter: Option<LooseRpFilter>,
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
        #[cfg(not(target_os = "macos"))]
        let (device, routes) = open_adapter(config.dns).await?;
        #[cfg(target_os = "macos")]
        let (device, tunnel) = privileged::open(config.dns).await?;

        // Sites with both families are reached over IPv4, which every
        // network has, so their connections can be spread across all of them.
        // glibc already does this on Linux, and so does macOS: the adapter's
        // unique local address doesn't share a label with global IPv6
        // (RFC 6724, rule 5).
        #[cfg(windows)]
        let prefer_ipv4 = PreferIpv4::apply()
            .inspect_err(|err| log::warn!("couldn't make IPv4 preferred: {err}"))
            .ok();
        #[cfg(target_os = "linux")]
        let loose_rp_filter = LooseRpFilter::apply()
            .inspect_err(|err| log::warn!("couldn't loosen reverse-path filtering: {err}"))
            .ok();

        let stack = IpStack::new(stack_config(), gate::Gate::new(device, router.clone()));
        let flows = Arc::new(Flows::default());
        let (stop, stopped) = watch::channel(false);
        let tasks = vec![
            tokio::spawn(accept(stack, router, flows.clone(), stopped.clone())),
            tokio::spawn(close_flows_on_lost_links(links, flows.clone(), stopped)),
        ];
        log::info!("engine started");
        Ok(Engine {
            flows,
            #[cfg(not(target_os = "macos"))]
            routes,
            #[cfg(target_os = "macos")]
            tunnel,
            #[cfg(windows)]
            prefer_ipv4,
            #[cfg(target_os = "linux")]
            loose_rp_filter,
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
        #[cfg(not(target_os = "macos"))]
        drop(self.routes);
        #[cfg(target_os = "macos")]
        self.tunnel.close().await;
        #[cfg(windows)]
        drop(self.prefer_ipv4);
        #[cfg(target_os = "linux")]
        drop(self.loose_rp_filter);
        let _ = self.stop.send(true);
        for task in self.tasks {
            let _ = task.await;
        }
        log::info!("engine stopped");
    }
}

/// Creates the adapter, points its DNS at `dns` and adds the capture routes.
#[cfg(not(target_os = "macos"))]
async fn open_adapter(dns: Vec<IpAddr>) -> io::Result<(tun::AsyncDevice, Vec<Route>)> {
    #[cfg(windows)]
    let wintun = wintun_path();
    #[cfg(windows)]
    if !wintun.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is missing", wintun.display()),
        ));
    }
    clean_up_stale();

    #[cfg(windows)]
    let setup = move || create_adapter(&dns, wintun);
    #[cfg(target_os = "linux")]
    let setup = create_adapter;
    let device = tokio::task::spawn_blocking(setup)
        .await
        .map_err(io::Error::other)??;
    let if_index = device.tun_index().map_err(io::Error::other)? as u32;
    let device = tun::AsyncDevice::new(device)?;
    #[cfg(target_os = "linux")]
    if let Err(err) = sys::set_dns(if_index, &dns) {
        log::info!("DNS stays with the system resolver: {err}");
    }
    log::info!("adapter up on interface {if_index}");
    Ok((device, add_capture(if_index)?))
}

/// `wintun.dll` is installed in `resources/` next to the executable; in
/// development it's read from the source tree.
#[cfg(windows)]
fn wintun_path() -> PathBuf {
    let installed = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("resources/wintun.dll")));
    match installed {
        Some(path) if path.is_file() => path,
        _ => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/wintun.dll")),
    }
}

/// The routes that send everything into the adapter, IPv6 included: it's
/// always captured, even when no network has it yet or it's switched off,
/// otherwise it would go straight out of whichever network has an IPv6
/// route, around MultiPath. Flows the router has no link for are refused.
/// Needs administrator rights.
fn add_capture(if_index: u32) -> io::Result<Vec<Route>> {
    crate::sys::add_unicast_address(if_index, IpAddr::V6(TUN_V6), 64)?;
    CAPTURE_V4
        .into_iter()
        .chain(CAPTURE_V6)
        .map(|(prefix, len)| Route::add(if_index, prefix, len))
        .collect()
}

fn tun_config() -> tun::Configuration {
    let mut config = tun::Configuration::default();
    #[cfg(not(target_os = "macos"))]
    config.tun_name(TUN_NAME);
    // A utun is point-to-point; naming itself as the other end is how macOS
    // VPNs configure one.
    #[cfg(target_os = "macos")]
    config.destination(TUN_V4);
    config
        .address(TUN_V4)
        .netmask(Ipv4Addr::new(255, 255, 255, 0))
        .mtu(MTU)
        .up();
    config
}

/// Wintun takes the adapter's DNS servers as it creates it.
#[cfg(windows)]
fn create_adapter(dns: &[IpAddr], wintun: PathBuf) -> io::Result<tun::Device> {
    let mut config = tun_config();
    config.metric(TUN_METRIC);
    let dns = dns.to_vec();
    config.platform_config(move |p| {
        p.wintun_file(wintun.into_os_string());
        if !dns.is_empty() {
            p.dns_servers(&dns);
        }
    });
    tun::create(&config).map_err(io::Error::other)
}

#[cfg(target_os = "linux")]
fn create_adapter() -> io::Result<tun::Device> {
    tun::create(&tun_config()).map_err(io::Error::other)
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

/// Windows names the adapter in its friendly name, Linux in the interface
/// name; on macOS (`utunN`) only its address tells it from a VPN's utun.
pub fn is_tun(itf: &netdev::Interface) -> bool {
    itf.name == TUN_NAME
        || itf.friendly_name.as_deref() == Some(TUN_NAME)
        || itf.ipv4.iter().any(|net| net.addr() == TUN_V4)
}

/// Removes capture routes a crashed run left on the adapter, which would
/// otherwise send all traffic into an adapter nobody reads.
pub fn clean_up_stale() {
    #[cfg(target_os = "macos")]
    if crate::sys::ResolverOverride::remove_stale() {
        log::info!("removed stale DNS servers");
    }
    let Some(adapter) = netdev::get_interfaces().into_iter().find(is_tun) else {
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
