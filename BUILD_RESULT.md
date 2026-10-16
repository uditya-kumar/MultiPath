# BUILD_RESULT — MultiPath 1.1.0 on macOS (Apple silicon)

Branch `macos-port`, local commits only (nothing pushed). Machine: MacBook Pro M4, macOS 26.6.2 (arm64).
Toolchain: rustc/cargo 1.99.1 (installed with rustup for this user, no sudo), Node 26.8.1, Xcode CLT clang 21.

## Artifacts

| what | path | notes |
| --- | --- | --- |
| app | `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MultiPath.app` | 12 MB, Mach-O arm64, ad-hoc signed (`codesign --verify --deep --strict` OK), `LSUIElement`, min macOS 12 |
| dmg | `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/MultiPath_1.1.0_aarch64.dmg` | 3.7 MB, `hdiutil verify` VALID, sha256 `a41bd2f5…28403` |
| executable | `…/MultiPath.app/Contents/MacOS/multipath` | sha256 `7d854125…b0c9`; also the root helper (`--tunnel-helper`) and `--headless` |

Not notarised (no Developer ID), so Gatekeeper rejects it (`spctl`: rejected): open it once with right-click → Open.

## How it works on macOS (short)

Turning on shows the standard administrator password dialog (`osascript … with administrator privileges`). It starts
the same executable as a short-lived **root helper** that creates a `utunN` (10.77.0.1, fd77:6d70::1/64), adds the
capture routes `0/1`, `128/1`, `::/1`, `8000::/1` with `route(8)`, writes a supplemental "all domains" resolver in
configd's dynamic store (`State:/Network/Service/dev.multipath.app/DNS`), and hands the utun's file descriptor to the
app over a private Unix socket (`SCM_RIGHTS`). The app (and its web view) never runs as root. Relay sockets leave
through the chosen network with `IP_BOUND_IF`/`IPV6_BOUND_IF`. Turning off sends "stop": routes, then DNS, are
removed. If the app dies, its descriptor closes, so the utun and every route on it vanish, and the helper (seeing EOF)
removes the DNS entry. Every start first removes stale state; all removals are idempotent.

## Commands run and results

| command | result |
| --- | --- |
| `npm install` | OK (+ `@playwright/test`, `@axe-core/playwright` dev deps) |
| `npm run lint` (tsc) | OK |
| `npm test` (Vitest) | 26/26 pass |
| `npm run build` | OK |
| `npm run e2e` (Playwright, Chrome, axe, `demo.html` fixtures) | 20/20 pass |
| `cargo fmt --check` | OK |
| `cargo clippy --all-targets -- -D warnings` | OK (arm64 macOS) |
| `cargo test` | 85 pass, 3 ignored (live probes); repeated 30× green after BUG-005 |
| `cargo test -- --ignored discovery_on_this_machine live_probe live_icmp` | pass: only `en0` Wi-Fi found (Tailscale `utun8`, bridge, Thunderbolt and internal ports excluded); TCP probe via `IP_BOUND_IF` 13 ms; unprivileged ICMP echo via en0 OK |
| Linux, Docker `rust:1-bookworm` (arm64): fmt / clippy / test | OK / OK / 62 pass |
| Windows, `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings` | OK (check only, with a stub `llvm-rc` so `tauri-winres` can run; Windows tests not run) |
| `CI=true npx tauri build --target aarch64-apple-darwin` | `.app` + `.dmg` built (`CI` skips the Finder styling AppleScript, which fails without Automation access) |
| release binary: `--headless` (no root), `--tunnel-helper` (no root / relative socket / `--dns '1.1.1.1,$(reboot)'`) | refused with exit 1 / 1 / 2 / 2 and a clear message |
| release binary: `--autostart` for 6 s, then SIGTERM | stayed up in the menu bar, no capture routes added, empty log, stopped cleanly |

No `sudo`, no utun, no system routes and no DNS changes were made during the build (task safety rules); route/DNS
code was exercised through a command fake and `MULTIPATH_DRY_RUN=1`.

## Commits (`git log main..macos-port`)

- `6ebf268` recon map, feature matrix, architecture plan (`replica/`)
- `31d324b` macOS backend: `sys/macos/`, `engine::privileged`, app glue, docs
- `938a055` helper: unique private folder per listener (BUG-005)
- `18c3a8b` popup: plain-language states, connection spread, macOS look (from `UI_REQUIREMENTS.md`)
- `5d84939` Playwright e2e with axe, contrast fixes, test plan and bug list

## Known limits

- **Unverified live (needs you):** utun creation, capture routes, the password dialog and helper hand-off, DNS through
  the supplemental resolver, two networks at once, failover, real throughput, the Connections page's app names, the
  menu bar popup's real look, Open at login. All are unit-tested against fakes, not on the system.
- Asks for the administrator password **every** time it turns on (also at login if it was on). A signed build would use
  an `SMAppService` launch daemon (XPC, code-signing requirement on the client) or a Network Extension
  (`NEPacketTunnelProvider`); both need an Apple Developer ID and notarisation.
- Per-connection balancing only: one TCP connection never gets faster than its network.
- Android tethering needs NCM (most phones since Android 11); RNDIS-only phones don't work on Apple silicon. Phones are
  recognised by their SystemConfiguration name (iPhone/iPad USB, Pixel, Galaxy, …); others show as Ethernet (works,
  different colour).
- Tailscale and other split-tunnel VPNs keep their routes (100.64/10 is more specific than the /1s) and resolvers. If
  Tailscale's "match all" resolver is ordered first, DNS keeps going through Tailscale (still works). A full-tunnel
  VPN takes over the routes, as on the other systems.
- Without root, the Connections page only names the user's own processes (`lsof`).
- `UI_REQUIREMENTS.md` appeared during the port and was implemented as a separate commit (`18c3a8b`); the Pencil MCP
  wasn't available, so `design.pen` wasn't updated. The design's `--text-3` is below AA contrast on Windows/Linux too
  (left as designed; see `replica/bugs.md`).

## Manual live-test checklist

Run at the Mac itself (not over SSH), with Wi-Fi connected and an iPhone (Personal Hotspot on, "Trust") or an Android
phone (USB tethering) plugged in.

```sh
open src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/MultiPath_1.1.0_aarch64.dmg   # drag to Applications
# or run straight from the build:
open src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MultiPath.app
```

1. [ ] Menu bar icon appears, no Dock icon; clicking it drops the popup below the icon. Both networks listed
       (Wi-Fi blue, phone orange), no utun/Thunderbolt rows.
2. [ ] Turn on → macOS password dialog → hero "Active". Cancel once first → "the administrator prompt was cancelled".
3. [ ] While on, in Terminal:
       `ifconfig | grep -B3 10.77.0.1` (one utun), `netstat -rn | grep -E '^(0/1|128.0/1)'` (two routes on that utun),
       `netstat -rn -f inet6 | grep -E '^(::/1|8000::/1)'`, `scutil --dns | grep -A4 8.8.8.8`, `ping -c3 1.1.1.1`.
4. [ ] fast.com or Speedtest: both rows carry traffic, the ring splits, "Where connections go" shows both;
       `curl -o /dev/null https://speed.cloudflare.com/__down?bytes=50000000` (one connection) stays on one network.
5. [ ] Unplug the phone mid-download → "iPhone USB disconnected — new connections moved to Wi-Fi." within ~5 s; plug
       back → "… is back".
6. [ ] Tailscale still works while on (`tailscale ping <peer>`).
7. [ ] Turn off → `netstat -rn | grep -cE '^(0/1|128.0/1)'` is 0, no utun with 10.77.0.1,
       `scutil <<< 'show State:/Network/Service/dev.multipath.app/DNS'` says "No such key".
8. [ ] Turn on, then `pkill -9 -f MultiPath.app/Contents/MacOS/multipath` → internet keeps working, the checks in 7
       pass within a few seconds; start again → it turns on cleanly.
9. [ ] Settings → Open at login → log out/in: MultiPath is in the menu bar (asks for the password if it was on).

If anything ever leaves the Mac without internet: quit MultiPath; if needed,
`sudo route -n delete -inet 0.0.0.0/1; sudo route -n delete -inet 128.0.0.0/1` and
`sudo scutil <<< 'remove State:/Network/Service/dev.multipath.app/DNS'` (all harmless when already gone).

## Where the rest is

`replica/` — `recon.md`, `features.csv` (clone column filled), `architecture.md`, `build-log.md`, `test-plan.md`,
`bugs.md`, `clone-screens/` (Off, Active with 3 links, Limited, Needs attention, failed start, Settings; dark and light).
