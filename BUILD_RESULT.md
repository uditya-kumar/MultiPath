# BUILD_RESULT — MultiPath 1.2.1 on macOS (Apple silicon)

Branch `macos-port`, local commits only (nothing pushed, no release created). Machine: MacBook Pro M4, macOS 26.6.2
(arm64). Toolchain: rustc/cargo 1.99.1 (rustup, per user), Node 26.8.1, Tauri CLI 2.12.3, Xcode CLT clang 21.

## 1.2.1: updater validation release (no behavior changes)

A maintenance release whose only purpose is to give an installed 1.2.0 a real, newer, signed release to update to
(U16). **No product code changed**: the diff from 1.2.0 is the version in `package.json`, `package-lock.json`,
`src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`, plus release docs. The release notes
and `latest.json` say so: *"Updater validation and maintenance release: no behavior changes. It only checks that
in-app updates from 1.2.0 work end to end."*

| what | path | notes |
| --- | --- | --- |
| app | `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MultiPath.app` | Mach-O arm64, ad-hoc signed (`codesign --verify --deep --strict` OK), `CFBundleShortVersionString` 1.2.1 |
| dmg | `release-assets/MultiPath_1.2.1_aarch64.dmg` | 5.1 MiB, `hdiutil verify` VALID, sha256 `19c83618…9b89` |
| updater archive | `release-assets/MultiPath_1.2.1_aarch64.app.tar.gz` | 5.2 MiB, sha256 `6d52b4e9…c6fc`; key `334DFF4341E59A68` (the existing key), trusted comment `version:1.2.1`; the extracted app is 1.2.1 and passes `codesign --verify` |
| signature | `release-assets/MultiPath_1.2.1_aarch64.app.tar.gz.sig` | also inside `latest.json` |
| update feed | `release-assets/latest.json` | version 1.2.1, `darwin-aarch64` → `https://github.com/timowse/MultiPath/releases/download/v1.2.1/MultiPath_1.2.1_aarch64.app.tar.gz` |
| checksums | `release-assets/SHA256SUMS` | `shasum -a 256 -c` OK |
| 1.2.0 files | `release-assets/v1.2.0/` | copied there before the build (the script clears the previous archive, signature and feed); checksums verified |

| command | result |
| --- | --- |
| `npm run lint`, `npm test`, `npm run build` | OK; Vitest 53/53 |
| `npm run e2e` | 31/31 pass |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | 91 pass, 4 ignored (includes `versions_agree`) |
| `scripts/release-macos.sh "<notes>"` | OK, signed with the existing key |
| `cargo test -- --ignored release_assets` | pass: the archive verifies against the configured public key, trusted comment `version:1.2.1` = feed, a flipped byte fails |

**Not done, on purpose:** 1.2.1 was not installed, launched, pushed or published, and `/Applications/MultiPath.app`
(1.2.0) was not touched, so it can test the real updater. No route activation: the built 1.2.1 app was never started
(no helper, no `utun`, no `route`/`scutil` call), and the IPv4 and IPv6 route tables were identical before and after
the build apart from two neighbor-cache expiry counters ticking down. The running 1.2.0 copy and its helper kept
running unchanged.

**To run U16:** publish v1.2.1 as a normal (not pre-) release (`release-assets/RELEASE_INSTRUCTIONS.md`), then in the
installed 1.2.0 open Settings → Updates → Check for updates. It should offer 1.2.1 with the notes above. If MultiPath
is on, installing turns it off, restarts it, and macOS asks for the password again.

## 1.2.0: signed self-updates

Settings → **Updates** → **Check for updates**. On a newer release the popup shows its notes and waits for **Install
and restart**. The official Tauri updater then downloads the archive over HTTPS, verifies its minisign signature
against the public key built into the app (and, with `requireSignedVersion`, that it was signed for the version the
feed announces), replaces the app and restarts it. *Check for updates at start* is off by default and never installs.
The feed is `https://github.com/timowse/MultiPath/releases/latest/download/latest.json`.

### Artifacts

| what | path | notes |
| --- | --- | --- |
| app | `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MultiPath.app` | 15.9 MiB, Mach-O arm64, ad-hoc signed (`codesign --verify --deep --strict` OK), `CFBundleShortVersionString` 1.2.0 |
| dmg | `release-assets/MultiPath_1.2.0_aarch64.dmg` | 5.1 MiB, `hdiutil verify` VALID, sha256 `bb72791a…0104` |
| updater archive | `release-assets/MultiPath_1.2.0_aarch64.app.tar.gz` | 5.2 MiB, sha256 `719f3a91…2ab0`; signed with key `334DFF4341E59A68`, trusted comment `version:1.2.0` |
| signature | `release-assets/MultiPath_1.2.0_aarch64.app.tar.gz.sig` | also inside `latest.json` |
| update feed | `release-assets/latest.json` | version 1.2.0, `darwin-aarch64` → `https://github.com/timowse/MultiPath/releases/download/v1.2.0/MultiPath_1.2.0_aarch64.app.tar.gz` |
| checksums | `release-assets/SHA256SUMS` | |
| how to publish | `release-assets/RELEASE_INSTRUCTIONS.md` | manual: tag `v1.2.0`, upload 4 files, publish as a normal (not pre-) release |

The same files are in `src-tauri/target/aarch64-apple-darwin/release/bundle/{macos,dmg}/` under Tauri's names;
`scripts/release-macos.sh` copies and renames them (the signature covers the bytes, not the name). `release-assets/`
is git-ignored except the instructions.

Signing key: `~/.config/multipath/updater.key` (mode 0600, folder 0700), generated once for this app with an empty
password; the CLI's output went to `/dev/null`, and the build log and assets were checked for key material (none).
Only the public key is in `src-tauri/tauri.conf.json`. **Back the key up**: without it, installed copies can't take
another update.

### Commands run and results (1.2.0)

| command | result |
| --- | --- |
| `npm run lint` (tsc) | OK |
| `npm test` (Vitest) | 48/48 pass (26 before; + update flow with a mocked updater, every panel state, Settings section, main-page banner) |
| `npm run build` | OK |
| `npm run e2e` (Playwright, Chrome, axe) | 29/29 pass (20 before; + U01–U09 against the demo page's fake update server, each checking that no request leaves `localhost`) |
| `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | OK / OK (arm64 macOS) |
| `cargo test` | 91 pass, 4 ignored (3 live probes, `release_assets_verify`) |
| `cargo test -- --ignored release_assets` | pass: archive verifies against the configured public key with `minisign-verify` (the updater's library), version matches the feed, a flipped byte fails |
| Linux, Docker `rust:1-bookworm` (arm64): fmt / clippy / test | OK / OK / 67 pass (63 before; + settings, config, capabilities, versions). Windows not re-checked: no `sys/` change, the plugins are cross-platform |
| `scripts/release-macos.sh "…"` | `.app`, `.dmg`, signed updater archive, `release-assets/` + `latest.json` |
| release binary smoke: isolated `HOME` and runtime dir, `--autostart`, 6 s, SIGTERM | stayed up with both plugins loaded, no helper started, route table unchanged, no settings written |

No updater test contacts GitHub: Vitest uses mocks, Playwright a fake server, and Rust only reads local files.
Nothing was published or pushed.

### State of this Mac at the end of the run: MultiPath is **on** (turned on at the Mac, not by this run)

`~/Library/Application Support/dev.multipath.app/settings.json` read `"on": false` when this run started (09:05). At
**09:10:49** the installed **1.1.0** copy in `/Applications` (PID 95183, started 09:02, before this run) was turned on
through the macOS administrator-password dialog. Its root helper (PID 96518) and `utun9` (10.77.0.1) are up, and
settings.json now says `"on": true`. That dialog needs someone at the Mac typing the password. This run didn't launch
MultiPath, show a prompt, or add a route, so it was left alone: stopping or editing a running copy mid-session would
disrupt it, and the app rewrites settings.json anyway. **To get back to Off: click the toggle in the popup (not
Quit).** That removes the routes and DNS and saves `"on": false`. The new 1.2.0 build was only started in isolation
(see the smoke test); it never ran against your settings.

### Limits of the update feature

- **Not published yet.** Until v1.2.0 is a published release with `latest.json`, *Check for updates* says it
  couldn't reach the update server (GitHub answers 404).
- 1.1.0 has no updater: install 1.2.0 from the DMG once.
- A live update (1.2.0 → newer) hasn't been run: it needs a newer published release (`replica/test-plan.md` U16).
- If MultiPath is on, updating turns it off, restarts it, and macOS asks for the password again (the popup says so).
  An app the user can't write to (outside `/Applications` and owned by another user) can't replace itself; the popup
  says to move it to Applications.
- Ad-hoc signed and not notarised (no Developer ID). The updater's minisign check is independent of Apple's.
- Apple silicon only; the Updates section is hidden on Windows and Linux (no feed for them).

## History: the 1.1.0 port

### Live test 1 (2026-10-17): S1 blocker BUG-007, fixed in code, not yet verified live

The first real run (Wi-Fi `en0` only, Tailscale up) set everything up correctly (helper as root, `utun9` with
10.77.0.1 and fd77:6d70::1, 0/1 and 128.0/1 on utun9, the DNS key), but nothing reached the internet: curl to
https://1.1.1.1 and https://example.com timed out, ping lost 100%, the popup said "No internet". Killing the app
restored everything at once.

**Root cause** (details in `replica/bugs.md`, BUG-007): relay and probe sockets are pinned with `IP_BOUND_IF`. For
the *primary* interface macOS has no scoped default route, so XNU falls back to the unscoped default through a
longest-prefix lookup of 0.0.0.0 (`node_lookup_default`), and our own 0.0.0.0/1 won that lookup. Pinned sockets on
en0 therefore had no route, the health probes failed, en0 went "No internet", and the router refused every flow.

**Fix:** on macOS the capture routes now leave out 0.0.0.0/8 and ::/16 (1/8, 2/7 … 128/1 and 1::/16 … 8000::/1;
24 routes). Windows and Linux are unchanged. Regression test `engine::tests::capture_spares_the_default_route_lookup`
failed before the fix and passes now.

After the rebuild, MultiPath was turned on again at the Mac at 08:54:28 (not by Claude): the instance started before
the rebuild (PID 92896) launched the *new* executable as helper (PID 94429). The new route set was in place (`1`,
`2/7` … `128.0/1` on utun9, no `0/1`). Read-only kernel lookups then gave `route -n get 1.1.1.1` → utun9,
`route -n get -ifscope en0 1.1.1.1` → en0 via 192.168.178.1, and the same for IPv6 via en0's router. That's the
lookup that failed before. These are route lookups only, not a traffic test: **live success is still unconfirmed.**
Run checklist step 3 (curl, ping, DNS) to confirm. Since that instance predates the rebuild, quit it and reopen the
app before the full checklist.

Start state: `settings.json` had `"on": false` after the failed run, and the LaunchAgent only passes `--autostart`,
which turns on only when `"on"` is true. Turning on at 08:54 saved `"on": true`. **Turn it off with the toggle (not
by quitting)** if it should stay off at the next start or login.

### How it works on macOS (short)

Turning on shows the standard administrator password dialog (`osascript … with administrator privileges`). It starts
the same executable as a short-lived **root helper** that creates a `utunN` (10.77.0.1, fd77:6d70::1/64), adds the
capture routes with `route(8)` (everything but 0.0.0.0/8 and ::/16: `1/8`, `2/7` … `128/1` and `1::/16` … `8000::/1`,
see BUG-007), writes a supplemental "all domains" resolver in
configd's dynamic store (`State:/Network/Service/dev.multipath.app/DNS`), and hands the utun's file descriptor to the
app over a private Unix socket (`SCM_RIGHTS`). The app (and its web view) never runs as root. Relay sockets leave
through the chosen network with `IP_BOUND_IF`/`IPV6_BOUND_IF`. Turning off sends "stop": routes, then DNS, are
removed. If the app dies, its descriptor closes, so the utun and every route on it vanish, and the helper (seeing EOF)
removes the DNS entry. Every start first removes stale state; all removals are idempotent.

### Commands run and results (1.1.0)

| command | result |
| --- | --- |
| `npm install` | OK (+ `@playwright/test`, `@axe-core/playwright` dev deps) |
| `npm run lint` (tsc) | OK |
| `npm test` (Vitest) | 26/26 pass |
| `npm run build` | OK |
| `npm run e2e` (Playwright, Chrome, axe, `demo.html` fixtures) | 20/20 pass |
| `cargo fmt --check` | OK |
| `cargo clippy --all-targets -- -D warnings` | OK (arm64 macOS) |
| `cargo test` | 87 pass, 3 ignored (live probes); repeated 30× green after BUG-005 (85 before BUG-007's two regression tests) |
| `cargo test -- --ignored discovery_on_this_machine live_probe live_icmp` | pass: only `en0` Wi-Fi found (Tailscale `utun8`, bridge, Thunderbolt and internal ports excluded); TCP probe via `IP_BOUND_IF` 13 ms; unprivileged ICMP echo via en0 OK |
| Linux, Docker `rust:1-bookworm` (arm64): fmt / clippy / test | OK / OK / 63 pass (after BUG-007) |
| Windows, `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings` | OK (check only, with a stub `llvm-rc` so `tauri-winres` can run; Windows tests not run) |
| `CI=true npx tauri build --target aarch64-apple-darwin` | `.app` + `.dmg` built, again after BUG-007 (`CI` skips the Finder styling AppleScript, which fails without Automation access) |
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
- `f1a2958`, `8963f41`, `0c9c51d` BUILD_RESULT, README, UI_REQUIREMENTS
- `9e5936a` macOS capture routes no longer hide the primary network's default route (BUG-007)
- 1.2.1: updater validation release, version bump only (no behavior changes)
- 1.2.0: signed self-updates (updater + process plugins, Settings → Updates, release script and assets; this file's latest update)

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
open release-assets/MultiPath_1.2.0_aarch64.dmg   # drag to Applications
# or run straight from the build:
open src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MultiPath.app
```

1. [ ] Menu bar icon appears, no Dock icon; clicking it drops the popup below the icon. Both networks listed
       (Wi-Fi blue, phone orange), no utun/Thunderbolt rows.
2. [ ] Turn on → macOS password dialog → hero "Active". Cancel once first → "the administrator prompt was cancelled".
3. [ ] While on, in Terminal:
       `ifconfig | grep -B3 10.77.0.1` (one utun, say `utun9`), `netstat -rn -f inet | grep utun9` (`1`, `2/7`, `4/6`,
       `8/5`, `16/4`, `32/3`, `64/2`, `128.0/1`; **no `0/1`**), `netstat -rn -f inet6 | grep utun9` (`1::/16` …
       `8000::/1`, no `::/1`), `route -n get 1.1.1.1` (interface utun9), and the BUG-007 check
       `route -n get -ifscope en0 1.1.1.1` (interface **en0**, not "not in table"), `curl --interface en0 -I
       https://1.1.1.1`, then `curl -I https://example.com`, `scutil --dns | grep -A4 8.8.8.8`, `ping -c3 1.1.1.1`.
4. [ ] fast.com or Speedtest: both rows carry traffic, the ring splits, "Where connections go" shows both;
       `curl -o /dev/null https://speed.cloudflare.com/__down?bytes=50000000` (one connection) stays on one network.
5. [ ] Unplug the phone mid-download → "iPhone USB disconnected — new connections moved to Wi-Fi." within ~5 s; plug
       back → "… is back".
6. [ ] Tailscale still works while on (`tailscale ping <peer>`).
7. [ ] Turn off → `netstat -rn | grep -c utun9` is 0 (`utun9` from step 3), no utun with 10.77.0.1,
       `scutil <<< 'show State:/Network/Service/dev.multipath.app/DNS'` says "No such key".
8. [ ] Turn on, then `pkill -9 -f MultiPath.app/Contents/MacOS/multipath` → internet keeps working, the checks in 7
       pass within a few seconds; start again → it turns on cleanly.
9. [ ] Settings → Open at login → log out/in: MultiPath is in the menu bar (asks for the password if it was on).

If anything ever leaves the Mac without internet: quit MultiPath, or `pkill -9 -f MultiPath.app/Contents/MacOS/multipath`.
The utun closes with the app's descriptor and takes every capture route with it (the BUG-007 live run showed exactly
that). If a DNS entry is left: `sudo scutil <<< 'remove State:/Network/Service/dev.multipath.app/DNS'` (harmless when
already gone).

## Where the rest is

`replica/` — `recon.md`, `features.csv` (clone column filled), `architecture.md`, `build-log.md`, `test-plan.md`,
`bugs.md`, `clone-screens/` (Off, Active with 3 links, Limited, Needs attention, failed start, Settings; dark and light; `updates-*.png` for the update states).
`release-assets/` — the 1.2.1 release files, `latest.json` and `RELEASE_INSTRUCTIONS.md` (the 1.2.0 set is in `v1.2.0/`); `scripts/release-macos.sh` rebuilds them.
