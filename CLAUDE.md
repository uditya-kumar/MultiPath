# MultiPath

A tray app for **Windows and Linux**, inspired by [Plexo](https://github.com/anmolkapil/plexo/), that spreads the
**whole PC's traffic** across several networks at once (Wi-Fi, Ethernet, USB-tethered phone).

## Workflow rules (always follow)
1. **Work feature by feature** through `TaskList.md`, in order. A feature is one task, or a small group of tasks that
   only make sense together.
2. **After each complete feature, lint and test before moving on:**
   - Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (in `src-tauri/`)
   - Rust on the other OS too, since `sys/` is split per OS: the same three in WSL for anything that touches
     `sys/linux/`, `net/` or `engine/`
   - Frontend: `npm run lint` (tsc type check), `npm test` (Vitest) and `npm run build`
   Fix every failure; don't skip or silence a check to get past it.
3. **Commit locally when the feature is done and green:** tick the task(s) in `TaskList.md`, then `git commit` with a
   message describing the feature. **Never push** — no `git push`, no remote, nothing sent to github.com.
4. **Then move to the next feature.** Keep going until every task is ticked.
5. **Stop and ask** only when a decision needs the user (hardware-only testing, product choices, credentials).
   Leave a short note of what's blocked and continue with any work that doesn't depend on the answer.
6. **Design reference is always the Pencil MCP.** Read `design.pen` only through the Pencil MCP tools (`execute` with
   `Get`/`TakeScreenshot`), never with file reads. Check the relevant screen before building or changing any UI, and
   make design changes in Pencil first.

## Hard constraints
- **Everything local.** No remote server, no relay, no separate service process. One Tauri v2 + Rust app.
- **Windows and Linux.** Win32 and Linux-only calls are both fine, but each one lives behind the matching `sys/`
  wrapper, so the layers above it are written once. No macOS.
- **The UI is only a small tray popup** (a Windows 11 flyout, 360 px wide), never a full window. The same popup is
  used on Linux, in the corner of the work area.
- Without a server, one TCP connection can't be split across networks. MultiPath balances **per connection**: a single
  download only gets faster if it opens several connections. Don't promise otherwise in UI or docs.

## How it works
1. An adapter named `MultiPath` (10.77.0.1/24, Wintun on Windows and `/dev/net/tun` on Linux) captures traffic through
   split routes 0.0.0.0/1 and 128.0.0.0/1 (plus ::/1 and 8000::/1 for IPv6), which beat the default route without
   replacing it. IPv6 is captured even when it's switched off or no link has it, otherwise it leaks out of any link
   with an IPv6 route.
2. `ipstack` (a userspace TCP/IP stack) turns the adapter's packets back into TCP streams and UDP flows.
3. For each new flow, the balancer picks the link where it can expect the most bandwidth:
   `weight × capacity / (busy flows + 1)`, optionally sticky by destination. Only busy flows count (moving data, or
   just opened); idle keep-alives don't. Capacity is the link's recent peak, and an idle or unmeasured link counts as
   average so it always gets tried. The relay socket is bound to that link's address and pinned to its adapter
   (`IP_UNICAST_IF` on Windows, `SO_BINDTODEVICE` on Linux), so it leaves through that adapter and never loops back
   into the TUN.
4. Health probes take dead links out of rotation; per-link counters feed the popup once a second. A link that goes
   unusable has its flows closed, so apps reconnect through another one (`engine::close_flows_on_lost_links`).

Privileges are needed for the adapter and routes: administrator on Windows, `cap_net_admin,cap_net_raw` (or root) on
Linux. The app asks for them and relaunches itself (`sys::relaunch_elevated`): UAC on Windows, `pkexec setcap` on
Linux, where capabilities only apply from the next start. The `.deb` runs that `setcap` on install, so an installed
copy never asks.

## Folder structure
```
MultiPath/
├── CLAUDE.md              this file
├── TaskList.md            the plan — tick tasks as they are completed
├── design.pen             UI design (Pencil). Encrypted: open only through the Pencil MCP tools
├── src/                   frontend: React + TypeScript + Vite (the popup)
│   ├── api/               the ONLY place that calls Tauri: invoke wrappers, types, useStatus()
│   ├── components/        presentational pieces: Toggle, NetworkRow, ShareRing, …
│   ├── views/             one file per popup page: Main (on and off), Settings, Connections
│   ├── lib/               pure helpers (formatting, status text) and small hooks
│   └── styles/            tokens.css (from design.pen variables) and global styles
├── scripts/               linux-lab.sh: namespaces with two shaped 10 Mbit/s ISPs, for the lab tests
└── src-tauri/
    ├── linux/             postinst.sh: the .deb's setcap step
    ├── resources/         wintun.dll + licence (bundled on Windows)
    └── src/
        ├── main.rs        calls multipath_lib::run()
        ├── lib.rs         module tree + Tauri builder
        ├── app/           Tauri glue: state, settings, status, commands, ticker, usage, autostart, tray + popup,
        │                  and the headless runner (`--headless`) that drives the same state without Tauri
        ├── engine/        TUN capture and relays: lifecycle, gate, flows, tcp, udp, icmp, sniff
        ├── net/           physical networks: links, health, dial, balancer, router (pick + failover)
        └── sys/           OS wrappers with one API: windows/ (Win32) and linux/ (netlink, /proc, libc)
```

### Boundaries
- `unsafe`, `windows_sys` and `libc` calls live in `sys/` only; everything else uses its safe wrappers.
- `net/` knows nothing about the TUN or Tauri. `engine/` uses `net/` to pick and dial links. `app/` owns the
  engine and is the only layer that touches Tauri.
- The frontend talks to Rust only through `src/api/`. Components get data through props; views compose them.
- Don't add folders for a single file, or layers that only forward calls.

## Commands
```sh
npm install
npm run tauri dev      # Windows: run the terminal as administrator, or use the in-app UAC relaunch
                       # Linux: setcap the dev binary once, or run the in-app pkexec relaunch
npm run tauri build    # Windows: NSIS installer · Linux: .deb and AppImage
cd src-tauri && cargo check && cargo test
sudo ./scripts/linux-lab.sh up     # two shaped ISPs in namespaces; `down` removes them
./multipath --headless --config <dir>   # no Tauri: one JSON status line a second, for tests
```

## Conventions
- Rust: doc comments explain *why*; short `//` comments for non-obvious OS/network details. No commented-out code.
- Both `sys/windows/` and `sys/linux/` export the same names, re-exported from `sys/`. Add to both, or the other OS
  stops building. Wording that differs per OS belongs in the UI, from `Status.platform`, not in two code paths.
- Names match the domain: a **link** is one physical network; a **flow** is one TCP connection or UDP association.
- UI matches `design.pen` exactly (tokens, spacing, colours: Wi-Fi `#4CC2FF`, phone `#FFB454`, Ethernet `#B98CFF`).
  Check the design before UI changes. In the app, use Segoe UI Variable where the design uses Inter.
- After finishing a task, tick it in `TaskList.md` in the same change.

## Gotchas

### Both
- `IP_UNICAST_IF` takes the interface index in **network byte order** for IPv4, but host order for `IPV6_UNICAST_IF`.
- `ipstack` defaults are too small for real traffic: raise the TCP timeout (~30 min), `max_unacked_bytes` and
  `read_buffer_size` (64 KB — capped by the unscaled u16 window), and set the UDP timeout to 60 s.
- In `ipstack`, a TCP stream's `local_addr()` is the app side and `peer_addr()` is the real destination.
- `tun` 0.8: create the device with `tun::create_as_async`; Windows options go in `platform_config` (`device_guid`, `wintun_file`).
- Dropping an `ipstack` TCP stream sends the app nothing, so it waits for data that never comes. `poll_shutdown`
  sends a FIN once inflight data is 0; that's how a lost link's flows are closed (`Flow::lose` → `engine::tcp`).
  There is no reset API.
- `ipstack` completes the TCP handshake before we dial out, so an app never sees a dead IPv6 path and never falls back
  to IPv4. `engine::gate` resets IPv6 SYNs when no link carries IPv6. A link carries IPv6 only after an IPv6 probe
  through it succeeds: phone tethers keep stale (deprecated) IPv6 addresses after their IPv6 is gone.

### Windows
- Routes added with `CreateIpForwardEntry2` outlive the process. Always remove them on stop, and clean up stale ones at startup.
- Win32 error 5010 (object already exists) is treated as success when adding routes and addresses.
- Windows prefers IPv6 even from the adapter's ULA. Since usually only some links have IPv6, the engine raises the
  IPv4 precedence (`::ffff:0:0/96` → 45, active store) while on (`sys::PreferIpv4`); otherwise IPv6 flows pile onto one link.

### Linux
- glibc already prefers IPv4 over the adapter's ULA (RFC 6724 label mismatch), so `PreferIpv4` is Windows-only. `curl`
  still tries IPv6 first by its own design, which the gate answers.
- Routes and addresses go away with the TUN device, even after `kill -9`, so there are no stale ones to clean up.
- RNDIS tethers (Android) never report a link state: `operstate` stays `unknown`. `net::links::is_connected` falls
  back to the up and running flags there. The phone kind comes from the driver name (`rndis_host`, `ipheth`, …).
- Packets from a pinned socket are dropped by strict reverse-path filtering, so `rp_filter` is set to loose (2) on
  each link while on, and restored on stop.
- Only root may set `systemd-resolved`'s DNS: polkit ignores capabilities. With capabilities, DNS stays with the
  system resolver, which still works.
- An AppImage runs from a read-only mount that ignores capabilities, so it can't be given permissions and needs
  `sudo` (`sys::can_elevate` tells the popup to say so).
- `appimagetool` won't follow the redirect that serves its runtime; pass the file through `LDAI_RUNTIME_FILE`.
