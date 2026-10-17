<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="MultiPath icon" />

# MultiPath

**Use Wi-Fi, Ethernet and a USB-tethered phone at the same time, for the whole PC.**

A small tray app for **Windows, Linux and macOS (Apple silicon)**, built with **Tauri v2 and Rust**. Everything runs
on your computer: no server, no relay, no account.

<a href="https://github.com/uditya-kumar/MultiPath/releases/latest"><img src="https://img.shields.io/badge/Download-Latest%20release-4CC2FF?style=for-the-badge&logoColor=white" alt="Download the latest release" /></a>

![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-deb%20%7C%20AppImage-FCC624?logo=linux&logoColor=black)
![macOS](https://img.shields.io/badge/macOS-Apple%20silicon-000000?logo=apple&logoColor=white)
![Tauri v2](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white)
[![MIT License](https://img.shields.io/badge/License-MIT-22C55E)](LICENSE)

</div>



## Screenshots

<p align="center">
  <img src="public/screenshot_tray.jpg" width="320" alt="The MultiPath tray popup combining Wi-Fi and a USB-tethered phone" />
  <br />
  <em>The tray popup, combining Wi-Fi and a USB-tethered phone</em>
</p>

<p align="center">
  <img src="public/screenshot_downloading.jpeg" width="640" alt="A download manager using several connections, split across Wi-Fi and a phone" />
  <br />
  <em>A download manager with several connections: about half go over Wi-Fi and half over the phone</em>
</p>

## What it does

- Spreads your PC's connections across all your networks, so they all work at once.
- Two 10 Mbps networks give you about **20 Mbps** for anything that opens several connections.
- Lives in the tray as a small popup: turn it on, switch networks on or off, see each network's speed.

## What gets faster

MultiPath balances **connections**. One connection always stays on one network, because splitting it would need a
server on the internet.

| Gets faster ✅ | Doesn't get faster ❌ |
|---|---|
| Speed tests (fast.com, Speedtest) | A single download over one connection |
| YouTube and other streaming sites (video and audio load in chunks) | One video call or game session |
| Download managers, Steam, torrents | |
| Browsers loading many things at once | |
| Several downloads at the same time | |

## Tech stack

| Part | Technology |
|---|---|
| App shell | <img src="https://cdn.simpleicons.org/tauri" width="16" height="16" alt="" /> [Tauri v2](https://tauri.app/) |
| Backend | <img src="https://cdn.simpleicons.org/rust/CE422B" width="16" height="16" alt="" /> Rust |
| Async runtime | <img src="https://cdn.simpleicons.org/tokio/8A8A8A" width="16" height="16" alt="" /> [Tokio](https://tokio.rs/) |
| Traffic capture | <img src="https://cdn.simpleicons.org/wireguard" width="16" height="16" alt="" /> [Wintun](https://www.wintun.net/) adapter on Windows, `/dev/net/tun` on Linux, a `utun` on macOS, via the [`tun`](https://crates.io/crates/tun) crate |
| TCP/IP stack | <img src="https://cdn.simpleicons.org/rust/CE422B" width="16" height="16" alt="" /> [`ipstack`](https://crates.io/crates/ipstack) (userspace) |
| Packets, sockets | <img src="https://cdn.simpleicons.org/rust/CE422B" width="16" height="16" alt="" /> [`etherparse`](https://crates.io/crates/etherparse), [`socket2`](https://crates.io/crates/socket2), [`netdev`](https://crates.io/crates/netdev) |
| System APIs | <img src="https://cdn.simpleicons.org/rust/CE422B" width="16" height="16" alt="" /> [`windows-sys`](https://crates.io/crates/windows-sys), [`libc`](https://crates.io/crates/libc) with netlink on Linux, and on macOS `IP_BOUND_IF` plus `route`/`ifconfig`/`scutil` (routes, sockets, elevation) |
| Frontend | <img src="https://cdn.simpleicons.org/react" width="16" height="16" alt="" /> React 19 |
| Language | <img src="https://cdn.simpleicons.org/typescript" width="16" height="16" alt="" /> TypeScript |
| Build tool | <img src="https://cdn.simpleicons.org/vite" width="16" height="16" alt="" /> Vite |
| Icons | <img src="https://cdn.simpleicons.org/lucide/F56565" width="16" height="16" alt="" /> [Lucide](https://lucide.dev/) |
| Tests | <img src="https://cdn.simpleicons.org/vitest" width="16" height="16" alt="" /> [Vitest](https://vitest.dev/), `cargo test` |
| Packaging | <img src="https://cdn.simpleicons.org/nsis/8A8A8A" width="16" height="16" alt="" /> NSIS, <img src="https://cdn.simpleicons.org/debian/A80030" width="16" height="16" alt="" /> `.deb` and AppImage, <img src="https://cdn.simpleicons.org/apple/8A8A8A" width="16" height="16" alt="" /> `.app` and `.dmg` (through Tauri) |

## How it works

Think of your networks as checkout counters, and each connection as a customer. MultiPath is the manager at the door.

1. **Catch the traffic.** MultiPath adds a virtual network adapter called `MultiPath`. All of the PC's traffic goes
   through it.
2. **Pick a network.** For each new connection, MultiPath works out which network would give it the most speed:

   > **network speed ÷ (connections downloading on it + 1)**

   The network with the biggest number gets the connection. Idle connections don't count.
3. **Send it out.** The connection is made again from that network, pinned to its adapter.
4. **Keep checking.** Every 5 seconds, each network is checked for internet. A network that fails twice in a row gets
   no new connections until it works again.

**Example:** two 10 Mbps networks, and fast.com opens 4 connections:

| Connection | Wi-Fi score | Phone score | Goes to |
|---|---|---|---|
| 1st | 10 ÷ 1 = **10** | 10 ÷ 1 = **10** | Wi-Fi |
| 2nd | 10 ÷ 2 = 5 | 10 ÷ 1 = **10** | Phone |
| 3rd | 10 ÷ 2 = **5** | 10 ÷ 2 = **5** | Wi-Fi |
| 4th | 10 ÷ 3 = 3.3 | 10 ÷ 2 = **5** | Phone |

Both networks end up with half the connections and run flat out: **10 + 10 = 20 Mbps**. A faster network gets
proportionally more.

### Good to know

- **Switching a network off** closes its connections. Apps reconnect through the other networks.
- **Switching it back on** only gets *new* connections. Downloads already running stay where they are until they finish
  or reconnect.
- **IPv6** is only used on networks where it actually works. Otherwise apps are told to use IPv4 straight away.
- **DNS** goes to 8.8.8.8, then 1.1.1.1 (some ISPs block 1.1.1.1). You can change this in Settings.
- **Turning MultiPath off** removes everything it added, so your system goes back to normal.

## Install

Everything is on the [latest release](https://github.com/uditya-kumar/MultiPath/releases/latest) page.

### Windows

1. Run `MultiPath_<version>_x64-setup.exe`.
2. Click the MultiPath icon in the tray and turn it on. Windows asks for administrator permission once.

### Linux

**Debian, Ubuntu, Kali and anything else that uses `.deb`** — the package gives MultiPath the two network
permissions it needs, so turning it on never asks for a password:

```sh
sudo apt install ./MultiPath_<version>_amd64.deb
```

**Anything else** — the AppImage. An AppImage can't hold those permissions, so it needs `sudo`:

```sh
chmod +x MultiPath_<version>_amd64.AppImage
sudo ./MultiPath_<version>_amd64.AppImage
```

### macOS (Apple silicon)

1. Open `MultiPath_<version>_aarch64.dmg` and drag MultiPath to Applications.
2. The build isn't notarised, so the first time, right-click MultiPath → **Open** (or allow it under System Settings →
   Privacy & Security).
3. Click the MultiPath icon in the menu bar and turn it on. macOS asks for an administrator password **each time** you
   turn it on: a small helper runs as root just long enough to add the adapter, routes and DNS, then hands the adapter
   to the app and waits to remove them again. The app itself never runs as root.

The macOS release lives at [timowse/MultiPath-macOS](https://github.com/timowse/MultiPath-macOS/releases/latest).

**Updates (1.2.0 and later):** Settings → **Updates** → **Check for updates**. If there's a newer version, MultiPath
shows what's new and waits for you to click **Install and restart**. It then downloads the update, checks its
signature against the key built into the app, replaces itself and restarts. A download whose signature doesn't check
out is never installed. *Check for updates at start* (off by default) only looks and tells you; it never installs on
its own. Checking asks GitHub for `latest.json`, the only thing MultiPath ever contacts by itself. Coming from 1.1.0,
install the 1.2.0 DMG by hand once.

USB tethering: an iPhone shows up as *iPhone USB* once you trust the Mac and turn on Personal Hotspot. Android phones
need USB tethering over NCM (most phones since Android 11); RNDIS-only phones don't work on Apple silicon.

| Requirement | Why |
|---|---|
| Windows 10 or 11 (64-bit), a 64-bit Linux desktop, or macOS 12+ on Apple silicon | Uses each system's networking APIs |
| Administrator rights (`CAP_NET_ADMIN` on Linux, the admin password on macOS) | Needed to add the adapter and routes |
| Two or more working networks | Otherwise there's nothing to combine |

*Start with Windows* / *Start on login* / *Open at login* (in Settings) puts MultiPath in the tray when you sign in.
On Windows and Linux there's no permission prompt; on macOS the password is asked when it turns on.

**On Linux,** the tray icon needs a panel that shows app indicators — GNOME needs the *AppIndicator* extension. Only
`systemd-resolved` systems get MultiPath's own DNS settings; elsewhere the system resolver is used, which works too.

## Getting started (developers)

**You need:**

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20 or newer
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) — WebView2 and MSVC build tools on Windows;
  `libwebkit2gtk-4.1-dev`, `libgtk-3-dev` and `libayatana-appindicator3-dev` on Linux; the Xcode command line tools
  on macOS

**Run it:**

```sh
git clone https://github.com/uditya-kumar/MultiPath.git
cd MultiPath
npm install
npm run tauri dev       # opens the app; run it as administrator or root, or accept the permission prompt
```

**Build the packages:**

```sh
npm run tauri build     # Windows: bundle/nsis/ · Linux: bundle/deb/ and bundle/appimage/ · macOS: bundle/macos/ and bundle/dmg/
```

On macOS, a build without access to Finder (SSH, an agent) needs `CI=true npm run tauri build`: that skips the
AppleScript that only arranges the `.dmg` window. The macOS build also signs an updater archive
(`createUpdaterArtifacts`), so it needs the updater key. Without it, build with
`--config '{"bundle":{"createUpdaterArtifacts":false}}'`. Releases are built with `scripts/release-macos.sh`, which
signs, lays out the GitHub assets and `latest.json` in `release-assets/`, and uploads nothing. See
`release-assets/RELEASE_INSTRUCTIONS.md`.

**Check before committing:**

```sh
npm run lint && npm test && npm run build
npm run e2e             # the popup's states and the update flow in Chrome (fake update server), with an accessibility scan
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

**Project layout:**

| Folder | What's inside |
|---|---|
| `src/` | The popup (React): `api/` talks to Rust, `components/`, `views/`, `lib/`, `styles/` |
| `src-tauri/src/app/` | Tauri glue: settings, status, commands, tray and popup |
| `src-tauri/src/engine/` | Traffic capture and relaying (TCP, UDP, ICMP) |
| `src-tauri/src/net/` | Networks, health checks, the balancer |
| `src-tauri/src/sys/` | System API wrappers: `windows/`, `linux/` and `macos/`, behind one API |

## Limits

- One connection always uses one network.
- A full-tunnel VPN takes over the routes while it's connected.
- On macOS, Tailscale and other split-tunnel VPNs keep their own routes and DNS; MultiPath only spreads what's left.
- The macOS build is ad-hoc signed (not notarised) and asks for the administrator password at every turn-on. In-app
  updates rely on their own signature, not Apple's, and exist for Apple silicon only. A signed, notarised
  release would use a privileged helper (`SMAppService`) or a Network Extension instead.
- Some sites sign you out if your requests come from different addresses. Turn on *Keep sites on one network* for those.

## License

MultiPath is released under the [MIT License](LICENSE).

Wintun is © WireGuard LLC. Its license is in `src-tauri/resources/wintun-LICENSE.txt`.
