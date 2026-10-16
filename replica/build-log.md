# Build log — MultiPath macOS port

One line per milestone/screen: ID, date, status, what's missing, what was harder than expected.

| ID | date | status | missing | harder than expected |
| --- | --- | --- | --- | --- |
| setup | 2026-10-16 | done | — | Rust wasn't installed: rustup 1.99.1 for this user, no sudo |
| recon/arch | 2026-10-16 | done | Pencil MCP (`design.pen`) isn't available in this session | Tailscale is live on `utun8` with a scoped default and a "match all" resolver: the port must never treat a utun as a link or touch other resolvers |
| sys/macos | 2026-10-17 | done | live admin run | `tun` refuses any utun name but `utunN`, so `is_tun` matches 10.77.0.1; macOS returns EINVAL from `setsockopt` once the peer hung up (found by a test) |
| privileges | 2026-10-17 | done (unverified live) | the real password dialog + helper run need Timo | running WebKit as root is fragile, so a root helper of the same binary hands over the utun fd (`SCM_RIGHTS`) instead of relaunching the whole app as root |
| engine split | 2026-10-17 | done | — | Windows/Linux setup moved into `open_adapter()` unchanged; macOS uses `engine::privileged` |
| discovery | 2026-10-17 | done (Wi-Fi only live) | a USB tether to plug in | netdev reports a gateway on Tailscale's utun; only `enN` ports may be links. `en1-3` (Thunderbolt) and `en4-6` (internal "Ethernet Adapter") would have shown as idle "not connected" rows |
| S01–S04 wording | 2026-10-17 | done | — | per-OS strings moved into `lib/summary.ts` so they're tested |
| S05 tray | 2026-10-17 | done (not clicked live) | — | accessory app needs both `LSUIElement` and `ActivationPolicy::Accessory` (dev runs don't read Info.plist) |
| helper flake | 2026-10-17 | fixed | — | the helper's private folder was named pid + sub-second nanos; macOS clocks tick in microseconds, so two listeners could collide (caught by repeating `cargo test`): now a counter, and retry on AlreadyExists |
| UI (UI_REQUIREMENTS.md) | 2026-10-17 | done | real-window check in the menu bar; VoiceOver pass by hand | see the UI section below |
| packaging | 2026-10-17 | see BUILD_RESULT.md | notarisation (no Developer ID) | `icon.icns` generated from the MIT `icon.png` (512 px, so no 1024 px slot) |

## Live checks done on this Mac (read-only, no root)

- `cargo test -- --ignored discovery_on_this_machine`: one link, `en0` "Wi-Fi", 192.168.178.157, gateway 192.168.178.1,
  global IPv6. Tailscale `utun8`, `bridge0`, `en1-6`, `awdl0`, `llw0`, `ap1`, `anpi*` excluded.
- `live_probe`: TCP handshake to 1.1.1.1:443 through `en0` with `IP_BOUND_IF`: 13 ms.
- `live_icmp`: echo to 1.1.1.1 from 192.168.178.157 via `en0`, unprivileged: payload echoed.

## Not done live (safety rules: no system-wide routes, no sudo during the autonomous build)

- utun creation, capture routes, DNS key, the admin prompt and the helper hand-off on real hardware.
- Two links at once (Wi-Fi + iPhone/Android tether), failover, throughput.

## UI: novice-friendly macOS experience (`UI_REQUIREMENTS.md`)

`UI_REQUIREMENTS.md` appeared in the repo during the port (it says Timo added it). Built as a separate commit so it
can be dropped on its own. The Pencil MCP isn't available in this session, so `design.pen` wasn't updated: the new
parts reuse the existing tokens and components, and the Windows/Linux look stays dark as designed.

| ID | what | status |
| --- | --- | --- |
| S01/S02 hero | `Off`, `Starting…`, `Active`, `Limited` ("Only using Wi-Fi"), `Needs attention` (error, no internet, all switched off, no network) with icon, colour and one sentence (`lib/hero.ts`) | done, tested |
| S02 spread | "Where connections go": a lane per working network, sized by its share of open connections, with a calm pulse (none under Reduce Motion), and the note that one download stays on one network | done |
| S01/S02 rows | per network: icon, name, plain health ("In use · 14 ms", "No internet", "Checking…", "Switched off", "Cable unplugged"), ↓/↑ rates, % of traffic and connection count, colour accent, toggle; interface/address/gateway/RTT/link speed/weight behind **Advanced** | done |
| events | "iPhone USB disconnected — new connections moved to Wi-Fi.", "… lost its internet connection …", "… is back …" for 20 s (`linkChange`, `useLinkEvent`) | done, tested |
| empty states | only one network (Limited / "Connect a second network"), permission note, no phone found tip, no internet, all switched off, no network at all | done, tested |
| privacy | "Runs only on this Mac: no relay server, no account." (off) and a Privacy section in Settings | done |
| S03 copy | plain explanations for adaptive, sticky sites, IPv6 and DNS | done |
| macOS look | system font (`-apple-system`), light and dark following the system on macOS (`data-platform`), status colours as tokens, Reduce Motion (CSS + the eased numbers), VoiceOver labels (`role=status`, labelled rows/rates/toggles), visible focus kept | done; contrast checked by eye only |
| screenshots | `replica/clone-screens/`: off (dark, light), active with 3 links (dark, light), limited, needs attention (no internet; failed start), settings | done (rendered from `demo.html` fixtures in headless Chrome, not the real window) |

Popup width stays 360 px; the extra content makes it taller (the window follows its content).
