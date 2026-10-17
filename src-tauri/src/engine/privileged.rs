//! macOS: creating the utun, its routes and its DNS need root; running the
//! packet stack doesn't. Unless MultiPath already runs as root, turning on
//! starts this same executable as a short-lived root helper, through the
//! administrator password dialog. The helper sets everything up, hands the
//! adapter's descriptor to the app, and undoes its changes when the app says
//! stop or goes away. The app, and its web view, never run as root.

use std::io;
use std::net::IpAddr;
use std::os::fd::{IntoRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tun::AbstractDevice;

use super::{add_capture, clean_up_stale, tun_config, MTU};
use crate::sys::{self, HelperClient, HelperLink, HelperListener, ResolverOverride, Route};

/// Followed by the app's socket: `multipath --tunnel-helper <socket> [--dns <ip,ip>]`.
pub const HELPER_ARG: &str = "--tunnel-helper";
const DNS_ARG: &str = "--dns";
/// From the password being accepted to the adapter arriving.
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(20);
/// The helper removes four routes and a DNS entry.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// What the root side set up, undone when dropped: routes first, then DNS.
pub(super) struct Setup {
    _routes: Vec<Route>,
    _dns: Option<ResolverOverride>,
}

/// Creates the utun with its addresses, routes and DNS. Needs root.
fn set_up(dns: &[IpAddr]) -> io::Result<(tun::Device, u32, Setup)> {
    clean_up_stale();
    let device = tun::create(&tun_config()).map_err(io::Error::other)?;
    let if_index = device.tun_index().map_err(io::Error::other)? as u32;
    let routes = add_capture(if_index)?;
    let dns = if dns.is_empty() {
        None
    } else {
        ResolverOverride::apply(if_index, dns)
            .inspect_err(|err| log::info!("DNS stays with the system resolver: {err}"))
            .ok()
    };
    log::info!("adapter up on interface {if_index}");
    Ok((
        device,
        if_index,
        Setup {
            _routes: routes,
            _dns: dns,
        },
    ))
}

/// Where the routes and DNS are held while MultiPath is on.
pub(super) enum Tunnel {
    /// MultiPath runs as root and set them up itself.
    Own(Setup),
    /// The root helper holds them.
    Helper(HelperLink),
}

impl Tunnel {
    /// Removes the routes and DNS. The adapter itself goes once the packet
    /// stack lets go of it.
    pub async fn close(self) {
        let closed = tokio::task::spawn_blocking(move || match self {
            Tunnel::Own(setup) => drop(setup),
            Tunnel::Helper(link) => {
                if let Err(err) = link.stop(STOP_TIMEOUT) {
                    log::warn!("the helper didn't stop cleanly: {err}");
                }
            }
        })
        .await;
        if let Err(err) = closed {
            log::warn!("couldn't remove the routes: {err}");
        }
    }
}

/// Sets up the adapter, through the helper unless this process runs as root.
pub(super) async fn open(dns: Vec<IpAddr>) -> io::Result<(tun::AsyncDevice, Tunnel)> {
    let (device, tunnel) = tokio::task::spawn_blocking(move || {
        if sys::is_elevated() {
            let (device, _, setup) = set_up(&dns)?;
            return Ok((device, Tunnel::Own(setup)));
        }
        let (fd, link) = ask_helper(&dns)?;
        let mut config = tun::Configuration::default();
        config.raw_fd(fd.into_raw_fd()).mtu(MTU);
        let device = tun::create(&config).map_err(io::Error::other)?;
        Ok::<_, io::Error>((device, Tunnel::Helper(link)))
    })
    .await
    .map_err(io::Error::other)??;
    Ok((tun::AsyncDevice::new(device)?, tunnel))
}

/// Starts the helper as root (the password dialog) and waits for the adapter.
fn ask_helper(dns: &[IpAddr]) -> io::Result<(OwnedFd, HelperLink)> {
    let listener = HelperListener::new()?;
    let exe = std::env::current_exe()?;
    sys::run_with_admin_prompt(&exe, &helper_args(listener.path(), dns))?;
    let (link, fd, if_index) = listener.accept(HANDOFF_TIMEOUT)?;
    log::info!("the helper set up interface {if_index}");
    Ok((fd, link))
}

fn helper_args(socket: &Path, dns: &[IpAddr]) -> Vec<String> {
    let mut args = vec![HELPER_ARG.to_owned(), socket.to_string_lossy().into_owned()];
    if !dns.is_empty() {
        let list: Vec<String> = dns.iter().map(IpAddr::to_string).collect();
        args.extend([DNS_ARG.to_owned(), list.join(",")]);
    }
    args
}

/// What the helper was asked to do.
#[derive(Debug, PartialEq)]
struct Request {
    socket: PathBuf,
    dns: Vec<IpAddr>,
}

/// Reads the helper's arguments. Everything it does as root comes from here,
/// so anything that isn't a socket path or an IP address is refused.
fn parse_args(args: &[String]) -> Result<Request, String> {
    let mut socket = None;
    let mut dns = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            HELPER_ARG => socket = args.next().map(PathBuf::from),
            DNS_ARG => {
                let list = args.next().ok_or(format!("{DNS_ARG} needs servers"))?;
                dns = list
                    .split(',')
                    .map(|s| s.parse().map_err(|_| format!("{s} is not an IP address")))
                    .collect::<Result<_, _>>()?;
            }
            _ => {}
        }
    }
    let socket = socket
        .filter(|s| s.is_absolute())
        .ok_or(format!("{HELPER_ARG} needs the app's socket"))?;
    Ok(Request { socket, dns })
}

/// The helper: sets up the adapter, hands it to the app, and undoes
/// everything once the app says stop or goes away. Returns the exit code.
pub fn run_helper(args: &[String]) -> i32 {
    let request = match parse_args(args) {
        Ok(request) => request,
        Err(err) => {
            eprintln!("{err}");
            return 2;
        }
    };
    if !sys::is_elevated() {
        eprintln!("the tunnel helper needs to run as root");
        return 1;
    }
    let client = match HelperClient::connect(&request.socket) {
        Ok(client) => client,
        Err(err) => {
            eprintln!("couldn't reach MultiPath: {err}");
            return 1;
        }
    };
    let (device, if_index, setup) = match set_up(&request.dns) {
        Ok(up) => up,
        Err(err) => {
            let _ = client.send_error(&format!("couldn't set up the adapter: {err}"));
            return 1;
        }
    };
    let sent = client.send_adapter(&device, if_index);
    // From here the app holds the only descriptor: when it exits, even
    // killed, the utun goes, and every route through it with it.
    drop(device);
    if let Err(err) = sent {
        eprintln!("couldn't hand over the adapter: {err}");
        return 1;
    }
    client.wait_for_stop();
    drop(setup);
    client.done();
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn passes_the_request_to_the_helper() {
        let dns = [
            "8.8.8.8".parse().unwrap(),
            "2606:4700::1111".parse().unwrap(),
        ];
        let args = helper_args(Path::new("/tmp/mp/helper.sock"), &dns);
        assert_eq!(
            args,
            strings(&[
                "--tunnel-helper",
                "/tmp/mp/helper.sock",
                "--dns",
                "8.8.8.8,2606:4700::1111"
            ])
        );
        let mut argv = strings(&["/Applications/MultiPath.app/Contents/MacOS/multipath"]);
        argv.extend(args);
        assert_eq!(
            parse_args(&argv).unwrap(),
            Request {
                socket: "/tmp/mp/helper.sock".into(),
                dns: dns.to_vec()
            }
        );
        assert_eq!(
            helper_args(Path::new("/s"), &[]),
            strings(&["--tunnel-helper", "/s"])
        );
    }

    #[test]
    fn refuses_bad_requests() {
        assert!(parse_args(&strings(&["--tunnel-helper"])).is_err());
        assert!(parse_args(&strings(&["--tunnel-helper", "relative.sock"])).is_err());
        let err = parse_args(&strings(&[
            "--tunnel-helper",
            "/s",
            "--dns",
            "1.1.1.1,$(reboot)",
        ]))
        .unwrap_err();
        assert_eq!(err, "$(reboot) is not an IP address");
        assert!(parse_args(&strings(&["--tunnel-helper", "/s", "--dns"])).is_err());
    }

    #[test]
    fn the_helper_needs_root() {
        if sys::is_elevated() {
            return;
        }
        assert_eq!(
            run_helper(&strings(&["--tunnel-helper", "/nowhere.sock"])),
            1
        );
        assert_eq!(run_helper(&strings(&["--tunnel-helper"])), 2);
    }

    #[test]
    fn tearing_down_removes_routes_then_dns() {
        let fake = fake::succeed();
        // lo0 stands in for the utun.
        let setup = Setup {
            _routes: add_capture(1).unwrap(),
            _dns: Some(ResolverOverride::apply(1, &["1.1.1.1".parse().unwrap()]).unwrap()),
        };
        let added = fake.lines().len();
        let routes = super::super::capture_routes().count();
        // The IPv6 address, then the routes, then DNS.
        assert_eq!(added, 1 + routes + 1);
        drop(setup);
        let lines = fake.lines();
        let removed = &lines[added..];
        assert_eq!(removed.len(), routes + 1);
        assert!(removed[..routes]
            .iter()
            .all(|l| l.starts_with("/sbin/route -n delete ")));
        assert_eq!(
            [&removed[0], &removed[7], &removed[8], &removed[routes - 1]],
            [
                "/sbin/route -n delete -inet 1.0.0.0/8 -interface lo0",
                "/sbin/route -n delete -inet 128.0.0.0/1 -interface lo0",
                "/sbin/route -n delete -inet6 1::/16 -interface lo0",
                "/sbin/route -n delete -inet6 8000::/1 -interface lo0",
            ]
        );
        assert_eq!(removed[routes], "/usr/sbin/scutil");
    }

    #[test]
    fn a_failed_route_undoes_the_ones_before_it() {
        let mut routes_added = 0;
        let fake = fake::install(move |cmd| {
            if cmd.args.get(1).map(String::as_str) == Some("add") {
                routes_added += 1;
                if routes_added == 3 {
                    return Err(io::Error::other(
                        "route: must be root to alter routing table",
                    ));
                }
            }
            Ok(String::new())
        });
        assert!(add_capture(1).is_err());
        let deletes: Vec<String> = fake
            .lines()
            .into_iter()
            .filter(|l| l.contains(" delete "))
            .collect();
        assert_eq!(
            deletes,
            [
                "/sbin/route -n delete -inet 1.0.0.0/8 -interface lo0",
                "/sbin/route -n delete -inet 2.0.0.0/7 -interface lo0",
            ]
        );
    }
}
