# MultiPath

Use Wi-Fi, Ethernet and a USB-tethered phone **at the same time**, for the whole PC.
MultiPath lives in the Windows tray. Turn it on and each new connection goes out on whichever network has the most room.

Everything runs on your PC. There's no server, no relay and no account. Inspired by
[Plexo](https://github.com/anmolkapil/plexo/).

## What gets faster, and what doesn't

MultiPath balances **connections**, not packets. Without a server on the other end, one TCP connection can't be split
across two networks. So:

- **Faster:** anything that opens many connections at once. That covers browsers loading pages, Steam and other game
  launchers, torrents, download managers, cloud sync, and several downloads running side by side.
- **Not faster:** a single download over one connection, one video stream, or one speed test connection. Each of these
  runs at the speed of whichever network it was given.

With *Keep sites on one network* turned on, every connection to the same site uses the same network. That fixes banking
and other sites that sign you out when your address changes, but a single site then no longer gets the combined speed.

## How it works

1. MultiPath adds a virtual network adapter ([Wintun](https://www.wintun.net/)) called `MultiPath`. It adds two
   routes, `0.0.0.0/1` and `128.0.0.0/1`, that send all traffic to it. Windows' own default route stays in place.
2. A userspace TCP/IP stack ([ipstack](https://crates.io/crates/ipstack)) turns the adapter's packets back into TCP
   connections and UDP flows.
3. For each new connection, the balancer picks the network where it can expect the most bandwidth. It looks at
   each network's share setting and, with *Adaptive weighting* on, its measured speed, split among the downloads
   already running on it. Idle connections don't count, and a network with nothing to do always gets the next one.
4. The connection is then made again from that network's own address, pinned to that adapter.
5. Health checks take a network out of rotation when it stops reaching the internet, and put it back when it recovers.

While MultiPath is on, Windows looks up names with the DNS servers in Settings (1.1.1.1 and 8.8.8.8 by default). Those
lookups are balanced like any other traffic. IPv6 is always captured, so it can't slip past MultiPath. It is balanced
across the networks that have IPv6 when *Include IPv6* is on. Otherwise IPv6 connections are refused and apps use IPv4.
While MultiPath is on, Windows tries IPv4 first for sites that have both. Usually only some of your networks have IPv6,
so a connection that used IPv6 could only go through those.

Switching a network off in the popup also closes the connections already on it. Apps reconnect through the others, and
download managers resume.

Turning MultiPath off or quitting it removes the adapter's routes, so Windows goes back to normal. If MultiPath ever
crashes and leaves routes behind, it removes them the next time it starts with administrator rights.

## Requirements

- Windows 10 or 11, 64-bit
- Administrator rights. Creating the adapter and routes needs them. The first time you turn MultiPath on, Windows asks
  for permission and MultiPath restarts with it.
- Two or more working network connections

## Install

Run `MultiPath_<version>_x64-setup.exe`. It installs for all users under Program Files, along with `wintun.dll`.

*Start with Windows* (in Settings) registers a scheduled task that starts MultiPath with administrator rights when you
sign in, so no permission prompt appears each time. Turning the setting off, or uninstalling, removes the task.

## Build from source

You need [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) 20+, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows.

```sh
npm install
npm run tauri dev     # run in development
npm run tauri build   # build the installer into src-tauri/target/release/bundle/nsis/
```

Checks:

```sh
npm run lint && npm test                  # frontend
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

## Limits

- One connection always uses one network, as explained above.
- Apps that bind to a specific adapter themselves, and some VPN clients, bypass MultiPath or take over the routes. While
  a full-tunnel VPN is connected, its routes win.
- UDP flows (games, calls, QUIC) are balanced per flow. A call or game session stays on the network it started on.
- Some sites check that all your requests come from one address. Turn on *Keep sites on one network* if they sign you
  out.

## License

Wintun is © WireGuard LLC, and its license is in `src-tauri/resources/wintun-LICENSE.txt`.
