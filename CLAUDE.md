# MultiPath

A Windows tray app, inspired by [Plexo](https://github.com/anmolkapil/plexo/), that spreads the **whole PC's traffic** across
several networks at once (Wi-Fi, Ethernet, USB-tethered phone).

## Workflow rules (always follow)
1. **Work feature by feature** through `TaskList.md`, in order. A feature is one task, or a small group of tasks that
   only make sense together.
2. **After each complete feature, lint and test before moving on:**
   - Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (in `src-tauri/`)
   - Frontend: `npm run lint` (tsc type check) and `npm run build`
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
- **Windows only.** Win32 APIs are fine; nothing needs to be cross-platform.
- **The UI is only a small tray popup** (a Windows 11 flyout, 360 px wide), never a full window.
- Without a server, one TCP connection can't be split across networks. MultiPath balances **per connection**: a single
  download only gets faster if it opens several connections. Don't promise otherwise in UI or docs.

## How it works
1. A Wintun adapter named `MultiPath` (10.77.0.1/24) captures traffic through split routes 0.0.0.0/1 and
   128.0.0.0/1 (plus ::/1 and 8000::/1 for IPv6), which beat the default route without replacing it.
2. `ipstack` (a userspace TCP/IP stack) turns the adapter's packets back into TCP streams and UDP flows.
3. For each new flow, the balancer picks a link (weighted least-connections × adaptive capacity, optionally sticky
   by destination). The relay socket is bound to that link's address and pinned with `IP_UNICAST_IF`, so it leaves
   through that adapter and never loops back into the TUN.
4. Health probes take dead links out of rotation; per-link counters feed the popup once a second.

Admin rights are needed (adapter, routes). The app relaunches itself through UAC (`sys::relaunch_elevated`).

## Folder structure
```
MultiPath/
├── CLAUDE.md              this file
├── TaskList.md            the plan — tick tasks as they are completed
├── design.pen             UI design (Pencil). Encrypted: open only through the Pencil MCP tools
├── src/                   frontend: React + TypeScript + Vite (the popup)
│   ├── api/               the ONLY place that calls Tauri: invoke wrappers, types, useStatus()
│   ├── components/        presentational pieces: Toggle, NetworkRow, ShareRing, …
│   ├── views/             one file per popup screen: Active, Off, Settings, Connections
│   ├── lib/               pure helpers (formatting)
│   └── styles/            tokens.css (from design.pen variables) and global styles
└── src-tauri/
    ├── resources/         wintun.dll + licence (bundled)
    └── src/
        ├── main.rs        calls multipath_lib::run()
        ├── lib.rs         module tree + Tauri builder
        ├── app/           Tauri glue: settings, state, commands, tray + popup window
        ├── engine/        TUN capture and relays: lifecycle, tcp, udp, icmp, sniff
        ├── net/           physical networks: links, health, dial, balancer
        └── sys/           Win32 wrappers: elevation, routes, sockets, icmp
```

### Boundaries
- `unsafe` and `windows_sys` calls live in `sys/` only; everything else uses its safe wrappers.
- `net/` knows nothing about the TUN or Tauri. `engine/` uses `net/` to pick and dial links. `app/` owns the
  engine and is the only layer that touches Tauri.
- The frontend talks to Rust only through `src/api/`. Components get data through props; views compose them.
- Don't add folders for a single file, or layers that only forward calls.

## Commands
```sh
npm install
npm run tauri dev      # run the terminal as administrator, or use the in-app UAC relaunch
npm run tauri build    # NSIS installer
cd src-tauri && cargo check && cargo test
```

## Conventions
- Rust: doc comments explain *why*; short `//` comments for non-obvious Win32/network details. No commented-out code.
- Names match the domain: a **link** is one physical network; a **flow** is one TCP connection or UDP association.
- UI matches `design.pen` exactly (tokens, spacing, colours: Wi-Fi `#4CC2FF`, phone `#FFB454`, Ethernet `#B98CFF`).
  Check the design before UI changes. In the app, use Segoe UI Variable where the design uses Inter.
- After finishing a task, tick it in `TaskList.md` in the same change.

## Gotchas
- `IP_UNICAST_IF` takes the interface index in **network byte order** for IPv4, but host order for `IPV6_UNICAST_IF`.
- `ipstack` defaults are too small for real traffic: raise the TCP timeout (~30 min), `max_unacked_bytes` and
  `read_buffer_size` (64 KB — capped by the unscaled u16 window), and set the UDP timeout to 60 s.
- In `ipstack`, a TCP stream's `local_addr()` is the app side and `peer_addr()` is the real destination.
- `tun` 0.8: create the device with `tun::create_as_async`; Windows options go in `platform_config` (`device_guid`, `wintun_file`).
- Routes added with `CreateIpForwardEntry2` outlive the process. Always remove them on stop, and clean up stale ones at startup.
- Win32 error 5010 (object already exists) is treated as success when adding routes and addresses.
