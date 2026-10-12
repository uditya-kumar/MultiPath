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
- [x] Capture routes 0.0.0.0/1 + 128.0.0.0/1 (and ::/1 + 8000::/1 when IPv6 capture is on) via `sys::Route`
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
- [ ] Measure the gain with parallel downloads (multi-file / Steam / torrent) against a single link
- [ ] NSIS installer bundling `wintun.dll`
- [ ] README: how it works, and its limits (one TCP connection can't be split without a server)
