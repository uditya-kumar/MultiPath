# Recon map: MultiPath (desktop tray app → macOS Apple Silicon port)

Scope: port the existing MultiPath 1.1.0 app (Windows + Linux) to macOS arm64 as a first-class backend, keeping
every feature of the popup and the engine, without regressing Windows or Linux.
For: Timo, on a MacBook Pro M4 (arm64, macOS 26), for personal use of Wi-Fi + Ethernet + USB-tethered phone at once.
Date: 2026-10-16

This is **not** a clean-room clone. The repository is the MIT-licensed upstream (`uditya-kumar/MultiPath`), so the
recon reads its own source, README and design notes directly. `LICENSE` and attribution stay as they are. The
replica skills' clean-room rules (no source, screenshots never shipped) are adapted accordingly: the source *is*
ours to modify; no third-party proprietary assets are added.

## Sources

| # | source | URL / path | notes |
| --- | --- | --- | --- |
| 1 | upstream source (MIT) | this repo, `src/`, `src-tauri/src/` | the ground truth for every screen, flow and entity below |
| 2 | project notes | `CLAUDE.md`, `TaskList.md` | architecture, gotchas per OS, what was tested on which OS |
| 3 | README | `README.md` | positioning, "what gets faster" table, install steps |
| 4 | reference reel | Instagram `DejoIczR5ye` (narration quoted in `CLAUDE_TASK_MACOS.md`) | the pitch: virtual adapter, per-connection pick, 5 s health checks, failover, "no server, no account" |
| 5 | design | `design.pen` (Pencil, encrypted) | not readable in this session (no Pencil MCP tools); the port changes wording only, not layout |
| 6 | this Mac | `ifconfig -l`, `networksetup -listallhardwareports`, `netstat -rn`, `scutil --dns` | the interface zoo a macOS port must filter (see "macOS findings") |

## Core loop

Turn MultiPath on from the tray popup; every new connection on the machine is sent out through whichever enabled,
healthy network can expect the most bandwidth, and the popup shows the combined speed per network.

## Screens

The UI is one 360 px popup with three pages plus the tray menu. No routes; navigation is in-component state.

| ID | screen | how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S01 | Main (off) | tray icon click / "Open MultiPath" | explain, turn on, list networks | Header + Toggle, callout + primary button, NetworkRow list, Footer | off, not elevated (OS-specific note), cannot elevate (AppImage), turning on, error notice, no networks |
| S02 | Main (on) | S01 → Turn on | live combined speed and share per network | Header, ShareRing, NetworkRow (rates, latency, health), Footer (connections · today) | on, 1 network, combining n, a link down, error after stop |
| S03 | Settings | Footer gear | balancing options, weights, start on login, DNS | SubHeader, SwitchRow ×4, WeightRow per present link, DnsRow | default, error notice, DNS invalid |
| S04 | Connections | Footer list icon | every relayed flow with app, host, rate, link | SubHeader + count chip, connection rows | off (hint), on + empty, on + filled |
| S05 | Tray menu | right click (Windows), any click (Linux), click (macOS menu bar) | Open, Turn on/off, Quit | native menu | on, off |
| S06 | Elevation prompt | Turn on without rights | get admin / capabilities, relaunch | OS dialog (UAC / pkexec / **macOS admin password**) | accepted, cancelled |

## Flows

```
F01 Turn on and combine networks
    S01 -> (S06 if not elevated) -> S02
    happy path clicks: 1 (2 with the prompt)
    edge: prompt cancelled, adapter creation fails, no networks present, already on

F02 Switch one network off / on
    S02 -> NetworkRow toggle
    happy path clicks: 1
    edge: switching off moves its flows (they are closed so apps reconnect)

F03 A network dies mid-download (failover)
    background: health probe (every 5 s) fails twice -> link Down -> its flows closed -> new flows elsewhere
    edge: probe miss once (retried at once), link comes back (Up within ~5 s)

F04 Tune balancing
    S01/S02 -> S03 -> toggle adaptive / sticky / IPv6, drag a weight, edit DNS
    happy path clicks: 2-3
    edge: DNS change restarts the engine, invalid DNS rejected

F05 See where connections go
    S02 -> S04
    edge: off, no flows, unknown app owner

F06 Start on login
    S03 -> "Start on login" (Linux/macOS) / "Start with Windows"
    edge: macOS needs a LaunchAgent; elevation is still asked when turning on

F07 Quit / crash cleanup
    S05 -> Quit -> routes removed, adapter gone, DNS restored
    edge: kill -9 -> next start cleans stale routes and resolver state
```

## Components

| component | variants | states | used on |
| --- | --- | --- | --- |
| Header | main, sub (back) | on, off, switching, warn dot | S01–S04 |
| Toggle | — | on, off, disabled | S01–S03 |
| NetworkRow | wifi / ethernet / phone / cellular / other colours | present, not connected, disabled, down, default | S01, S02 |
| ShareRing | — | rate-based, connection-based (idle) | S02 |
| SwitchRow / WeightRow / DnsRow | — | default, editing, error | S03 |
| Footer | — | on (connections · today), off (n networks ready) | S01, S02 |
| Notice | error | shown, hidden | S01–S03 |

## Inferred data model

The app has no database; everything below is in-memory state plus two JSON files in the app config folder.

```
Link        id (OS interface name), if_index, name, description, kind, ipv4, ipv6, gateway, speed_bps, present,
            health {state, latency_ms, consecutive_failures, ipv6_ok}, stats {rx, tx, busy, peak_rx, …}
            evidence: net/links.rs   confidence: high
Flow        id, proto (tcp|udp), src, dst, host (SNI/HTTP sniff), link id, rx, tx, started
            evidence: engine/flows.rs   confidence: high
Settings    on, links{id -> {enabled, weight 1-10, label}}, sticky, adaptive, capture_ipv6, dns[]
            evidence: app/settings.rs (settings.json)   confidence: high
Usage       date (y, m, d), bytes
            evidence: app/usage.rs (usage.json)   confidence: high
Status      platform, on, busy, elevated, can_elevate, error, rates, connections, today_bytes, links[], settings
            evidence: app/status.rs, src/api/types.ts   confidence: high
```

Relationships: Link 1-n Flow (a flow is pinned to one link for life), Settings 1-n LinkPrefs keyed by Link id.

## macOS findings (this Mac, 2026-10-16)

- `ifconfig -l`: `lo0 gif0 stf0 anpi0-3 en0-en6 bridge0 utun0-8 ap1 awdl0 llw0`. Only `en0` (Wi-Fi) is a real,
  connected network. `en1-en3` are Thunderbolt ports bridged into `bridge0`; `en4-en6` are internal
  "Ethernet Adapter (enN)" ports tied to `anpi*`; `awdl0`/`llw0` are AirDrop; `ap1` is the hotspot; `utun*` are
  system/VPN tunnels.
- **Tailscale is active** on `utun8` (100.64/10, a scoped default, a supplemental resolver at 100.100.100.100).
  A port must not touch its routes or resolver, and its own utun must never be mistaken for MultiPath's.
- utun names can't be chosen (`utunN`), so MultiPath's adapter is recognised by its address 10.77.0.1, not its name.
- netdev on macOS already gives SystemConfiguration names ("Wi-Fi", "iPhone USB", "Thunderbolt 1") and per-interface
  default gateways from the routing table.

## Feature matrix

See `features.csv`. Must: 23, should: 7, could: 4 (3 of the could rows are skipped).

## Out of scope (cannot or should not be done here)

- Splitting one TCP connection across networks (needs a server; upstream's own constraint).
- A notarised, Developer-ID-signed build and a production privileged helper (needs Timo's Apple Developer account).
- A Network Extension (`NEPacketTunnelProvider`) build: needs the Network Extension entitlement and signing.
- Live hardware validation with Wi-Fi + tethered phone + admin approval in this autonomous session (safety rules
  forbid system-wide capture routes during development); it goes to a manual checklist.

## Size

Screens 6, flows 7, entities 5. Hard parts: utun + routes on Darwin without breaking Tailscale; binding sockets with
`IP_BOUND_IF`/`IPV6_BOUND_IF`; elevation for an unsigned app; DNS via the dynamic store without stale state.
Size: M (the engine, balancer and UI are shared; the work is a new `sys/macos/` backend plus app glue).
