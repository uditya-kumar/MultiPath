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
| packaging | 2026-10-17 | see BUILD_RESULT.md | notarisation (no Developer ID) | `icon.icns` generated from the MIT `icon.png` (512 px, so no 1024 px slot) |

## Live checks done on this Mac (read-only, no root)

- `cargo test -- --ignored discovery_on_this_machine`: one link, `en0` "Wi-Fi", 192.168.178.157, gateway 192.168.178.1,
  global IPv6. Tailscale `utun8`, `bridge0`, `en1-6`, `awdl0`, `llw0`, `ap1`, `anpi*` excluded.
- `live_probe`: TCP handshake to 1.1.1.1:443 through `en0` with `IP_BOUND_IF`: 13 ms.
- `live_icmp`: echo to 1.1.1.1 from 192.168.178.157 via `en0`, unprivileged: payload echoed.

## Not done live (safety rules: no system-wide routes, no sudo during the autonomous build)

- utun creation, capture routes, DNS key, the admin prompt and the helper hand-off on real hardware.
- Two links at once (Wi-Fi + iPhone/Android tether), failover, throughput.
