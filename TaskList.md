# MultiPath — Task List

Tick a box (`[x]`) as soon as its task is done and verified. Add tasks when new work turns up.
Phases run roughly in order, but a later phase can start once the tasks it depends on are done.

## Phase 0 — Groundwork
- [x] Study Plexo and choose the architecture (Wintun TUN + `ipstack` + per-connection balancing, no remote server)
- [x] Cargo manifest with dependencies (`tauri`, `ipstack`, `tun`, `netdev`, `socket2`, `windows-sys`, …)
- [x] Bundle `wintun.dll` (amd64) and its licence in `src-tauri/resources/`
- [x] `sys/`: Win32 wrappers for elevation, routes, addresses, socket pinning, socket owners, and ICMP
- [x] `net/links.rs`: adapter discovery, link kinds, per-link counters, rates, and history
- [x] `app/settings.rs`: settings model with load and atomic save
- [x] Tray popup design in `design.pen` (Active / Off / Settings / taskbar mockup), with the share ring chosen as the hero
- [x] Folder structure, `CLAUDE.md`, and this task list

## Phase 1 — Tauri shell
- [x] `tauri.conf.json`, capabilities, and app icons
- [x] Vite + React + TypeScript frontend scaffold in `src/`
- [x] `main.rs` → `multipath_lib::run()`; `cargo check` passes with every module wired in
- [x] Tray icon: a left click toggles the popup, a right click opens a menu (Open, Quit)
- [x] Popup window: 360 px wide, frameless, always on top, hidden from the taskbar, anchored above the tray, hides on blur
- [x] Mica background behind the popup (a translucent fill keeps it readable without Mica)
- [x] Single instance (a second launch focuses the existing popup)
- [x] Elevation handoff: `relaunch_as_admin` starts an elevated copy that waits for this one to exit

## Phase 2 — Networks (`net/`)
- [x] Verify `links.rs` discovery on real hardware (Wi-Fi checked; USB tether + Ethernet are in the Phase 6 matrix)
- [x] `health.rs`: a TCP probe to 1.1.1.1:443 per link every 5 s; record latency; mark a link down after 2 failures
- [x] `dial.rs`: open TCP/UDP sockets bound to a link's address and pinned with `IP_UNICAST_IF`, with a connect timeout
- [x] `balancer.rs`: weighted least-connections, an adaptive capacity factor from `peak_rx`, optional sticky sessions
- [x] Failover: when a dial fails, retry once on the next best link and record the failure
- [x] Unit tests for the balancer (weights, disabled links, sticky, all links down)

## Phase 3 — Engine (`engine/`)
- [x] `mod.rs`: start/stop lifecycle — create the Wintun adapter, assign 10.77.0.1/24 (plus an IPv6 ULA), set DNS and metric
- [x] Capture routes 0.0.0.0/1 + 128.0.0.0/1 (and ::/1 + 8000::/1, always, so IPv6 can't bypass) via `sys::Route`
- [x] On startup, clean up stale routes and the adapter left behind by a crashed run
- [x] `ipstack` config (TCP timeout 30 min, 256 KB unacked/read buffers, UDP timeout 60 s) and the accept loop
- [x] `tcp.rs`: relay each TCP stream through the chosen link and count bytes per link
- [x] `udp.rs`: per-flow UDP relay with idle timeout; DNS goes to the configured servers
- [x] `icmp.rs`: relay pings through `sys::icmp_echo` so `ping` works while MultiPath is on (IPv4)
- [x] `sniff.rs`: read the TLS SNI / HTTP Host for sticky keys and the connections list
- [x] Link loss mid-run: new flows avoid the link, and its flows close quickly instead of hanging
- [ ] Clean stop: routes removed and adapter closed, verified with `route print` (needs an elevated run, so it waits for the user)

## Phase 4 — App glue (`app/`)
- [x] Remove the temporary `allow(dead_code, unused_imports)` in `lib.rs` once the engine is wired in
- [x] `state.rs`: shared `AppState` (engine handle, links, settings and their path)
- [x] Tray menu gets "Turn on / Turn off"; the tray icon shows the on/off state (`tray::set_active`)
- [x] `commands.rs`: `get_status`, `set_enabled`, `set_link_enabled`, `set_link_weight`, `set_option`, `list_connections`
- [x] 1 Hz ticker: `links.tick()`, then emit a `status` event to the popup (only while it is visible)
- [x] Connections snapshot with process names (`sys::socket_owners` + `sys::process_name`)
- [x] Store settings in the app config dir and apply changes live
- [x] Daily data counter ("3.4 GB today"), reset at local midnight
- [x] Start with Windows (a scheduled task with highest privileges, since admin is required; registering it needs the UAC prompt, checked in Phase 6)

## Phase 5 — Frontend (`src/`)
- [x] `styles/tokens.css` from the `design.pen` variables; Segoe UI Variable font
- [x] `api/`: typed `invoke` wrappers, shared types, and a `useStatus()` event hook
- [x] `components/`: Toggle, NetworkRow, ShareRing (SVG donut), IconButton, Header, Footer
- [x] `views/Main.tsx` when on: header, share ring hero, network rows, footer
- [x] `views/Main.tsx` when off: callout, "Turn on" button, admin note
- [x] `views/Settings.tsx`: balancing toggles, weight sliders, app options
- [x] `views/Connections.tsx`: live list of flows (app, destination, link, rate)
- [x] `lib/format.ts`: MB/s, GB, and percentage formatting; smooth number transitions
- [x] Keyboard: Esc goes back a page or closes the popup, Tab order is sensible

## Phase 6 — Test & ship
- [ ] First elevated run: turn on, browse and download, check `route print` shows the /1 routes on MultiPath, turn off and check they're gone
- [ ] "Turn on" from a normal copy → UAC → the elevated copy opens its popup
- [ ] Manual test matrix: discovery of USB tether + Ethernet; Wi-Fi + USB tether, unplug during a download, sleep/resume, alongside a VPN
- [ ] Start with Windows: turning it on registers the task (UAC prompt), and MultiPath starts in the tray after signing in
- [x] Measure the gain with parallel downloads (multi-file / Steam / torrent) against a single link — fast.com, Wi-Fi 10 + tether 10 Mbps → 19–20 Mbps, split 50/50
- [x] NSIS installer bundling `wintun.dll`
- [x] README: how it works, and its limits (one TCP connection can't be split without a server)

## Phase 7 — Linux support
Each OS has its own `sys/windows/` and `sys/linux/` behind one API; elsewhere the few differences sit in the same files,
behind `#[cfg(windows)]` / `#[cfg(target_os = "linux")]`.
The Windows build must stay green after every task.

**Testing without a Linux PC.** Development and tests run in WSL2 (Ubuntu). WSL has only one virtual network, so a
script builds a fake one: network namespaces for "the PC", two "ISPs" and "the internet", joined by veth pairs, each ISP
capped at 10 Mbit/s with `tc`. MultiPath runs headless inside the "PC" namespace and sees two links, like Wi-Fi + phone.
The same script runs in GitHub Actions (`ubuntu-latest`, with sudo). Packages are install-tested in Docker containers.

### Setup
- [x] WSL2 Ubuntu 24.04 dev box: `wsl --install -d Ubuntu-24.04` (needs the user), then Rust, Node and Tauri's Linux deps (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, …)
- [x] Workspace builds on Linux: `cargo check` passes with Windows-only modules and crates behind `cfg` (`windows-sys`, `wintun.dll` resource only on Windows) — `sys/windows/` and `sys/linux/` share one API; bundle settings per OS in `tauri.windows.conf.json` / `tauri.linux.conf.json`
- [x] Headless runner (`multipath --headless [--config <dir>]`): starts the engine and health checks without Tauri and prints the status as a JSON line a second, so the lab can drive it

### System layer (`sys/`, Linux side)
- [x] TUN device via `tun` (no DLL); interface address and MTU via netlink
- [x] Routes: add/remove `0.0.0.0/1`, `128.0.0.0/1`, `::/1`, `8000::/1` via netlink; remove on stop; clean stale ones at startup
- [x] Socket pinning with `SO_BINDTODEVICE` (`socket2::bind_device`); set `rp_filter` to loose (2) on links while on, restore on stop
- [x] DNS while on: `resolvectl dns MultiPath …` + `resolvectl domain MultiPath ~.`; fall back to a warning without systemd-resolved
- [x] Socket owners from `/proc/net/{tcp,tcp6,udp,udp6}` + `/proc/<pid>/fd`; process names from `/proc/<pid>/comm`
- [x] ICMP echo with an unprivileged ping socket (`SOCK_DGRAM` + `IPPROTO_ICMP`), falling back to a raw socket (`CAP_NET_RAW`) where `ping_group_range` leaves it out (WSL); bound to the link's device
- [x] Privileges: run with `cap_net_admin,cap_net_raw` (set once with `pkexec setcap`, then relaunch); clear message when missing. Only root may set systemd-resolved's DNS (polkit ignores capabilities), so with capabilities DNS stays with the system resolver, which still works
- [x] Single instance (a socket in `$XDG_RUNTIME_DIR` is both the lock and the "show" channel; a crashed copy's socket is taken over); local date via `localtime_r`
- [x] Link discovery: phone tethers by driver (`rndis_host`, `ipheth`, `cdc_ncm`, `cdc_ether`) or `usb…`, Wi-Fi by `wireless/` or `wl…`, cellular, `en…`/`eth…` as Ethernet; descriptions from udev's hardware database

### App (`app/`)
- [x] Tray on Linux (AppIndicator, or an XEmbed icon without a StatusNotifier host): menu with Open, Turn on/off, Quit (Linux trays don't report left clicks)
- [x] Popup on Linux: same 360 px window in the corner of the work area (top-right under a top panel, else bottom-right; through XWayland, as Wayland windows can't place themselves), hides on minimize / Esc
- [x] Start on login: `~/.config/autostart/multipath.desktop` (`$XDG_CONFIG_HOME` honoured); the Settings label reads "Start on login" on Linux
- [x] Off-state admin note and status line per OS (UAC on Windows, one-time permission on Linux), from `Status.platform`; the README part is in the docs task

### Testing
- [x] Unit tests for the Linux parsers (`/proc/net/tcp` rows, route and address messages) — run in WSL
- [x] `scripts/linux-lab.sh`: build the namespaces, veths, NAT and 10 Mbit/s `tc` shaping, plus an HTTP/iperf3 server and DNS in "the internet"; tear it all down on exit
- [x] Lab test — spread: 8 parallel connections ≈ 18 Mbit/s split 50/50; a single connection ≈ 9 Mbit/s (each 10 Mbit/s `tbf` uplink carries ~8–9 Mbit/s of TCP); also on Kali with Ethernet + a USB-tethered phone: 6.5 MB/s against 1.2 MB/s on Ethernet alone
- [x] Lab test — failover: unplug ISP 2 mid-download → its flows close within ~1 s (the app gets a FIN, so it reconnects), new ones go to ISP 1; plug it back → it's used again within ~5 s
- [x] Lab test — health: blackhole ISP 2's internet (link stays up) → marked down within ~15 s with no failed requests (dial retries after 5 s), back up ~5 s after restore
- [x] Lab test — IPv6 gate: no working IPv6 → `curl -6` is refused at once, `curl` uses IPv4
- [x] Lab test — clean stop: no MultiPath routes or adapter left after stop or `kill -9` (on Linux the routes go with the adapter), and a restart after `kill -9` starts and stops cleanly
- [ ] GitHub Actions workflow: build, unit tests, and the lab tests on `ubuntu-latest` (the user pushes; nothing is pushed from here)

### Ship
- [x] Bundle `.deb` and AppImage (`npm run tauri build` in WSL); the `.deb` sets capabilities in its post-install script. Checked on Kali: the `.deb` installs with apt, runs as the user with `cap_net_admin,cap_net_raw` and turns on without a prompt (fast.com: 18 Mbit/s over Ethernet + phone, 50/50); the AppImage runs and says it needs sudo
- [ ] Install test in Docker: the `.deb` installs on Ubuntu 24.04 and Debian 12 with its dependencies, and the binary starts
- [x] Popup check in WSLg and on Kali (XFCE, VirtualBox): layout matches `design.pen`; the tray icon registers as a StatusNotifier item; "Turn on" asks through `pkexec` and the relaunched copy turns on
- [ ] Optional real-hardware check: boot an Ubuntu live USB on the Windows PC, run the AppImage with Wi-Fi + USB tether (needs the user)
- [x] Docs: CLAUDE.md (Windows and Linux, gotchas split per OS, folder notes), README (Linux badge and install, tech stack), release notes (the v1.1.0 draft)
