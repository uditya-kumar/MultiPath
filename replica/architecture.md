# Architecture: MultiPath for macOS (a port of upstream MultiPath 1.1.0)

The template's web stack (database, auth, payments, hosting) doesn't apply: MultiPath is one local Tauri v2 + Rust
app with no server. The stack is the upstream one; this plan only adds a third `sys/` backend and the glue around it.

## Stack

| layer | choice | why |
| --- | --- | --- |
| shell | Tauri v2 (existing) | already the app; builds a native arm64 `.app` and `.dmg` on macOS |
| UI | React 19 + TS + Vite (existing) | unchanged; only the per-OS wording reads `platform === "macos"` |
| engine | Rust, tokio, `ipstack` (existing) | shared; balancer, health, failover, gate, counters stay one code path |
| adapter | `tun` 0.8 → macOS `utun` | the crate already speaks the utun control socket; names are `utunN` |
| routes | `/sbin/route` + `/sbin/ifconfig` | stable, documented CLIs; argv built by pure, unit-tested functions; `MULTIPATH_DRY_RUN=1` logs instead of running |
| egress pinning | `IP_BOUND_IF` / `IPV6_BOUND_IF` via `socket2::bind_device_by_index_v4/v6` | the Darwin equivalents of `SO_BINDTODEVICE`; the kernel then uses the link's scoped default route |
| DNS | `scutil` dynamic store, `State:/Network/Service/dev.multipath.app/DNS` | a supplemental default resolver (match domain `""`), nothing in persistent prefs; removed on stop and before every start |
| privileges | **root helper mode of the same binary**, started through `osascript … with administrator privileges` | see below; the GUI never runs as root |
| socket owners | `lsof -nP -iTCP -iUDP -FpPn` parsed + `proc_name` | libc has no `socket_fdinfo`; lsof is stable and its `-F` format is meant for parsing |
| login item | `~/Library/LaunchAgents/dev.multipath.app.plist` | per-user, no prompt; `RunAtLoad` with `--autostart` |
| packaging | `tauri build --bundles app,dmg` (arm64) | `.app` + `.dmg`, ad-hoc signed (`signingIdentity: "-"`) |

### Privileges: why a helper and not a root GUI

Windows relaunches the whole app elevated (UAC) and Linux gives the binary capabilities. On macOS neither fits a
local unsigned build: there's no capability system, and running WebKit (the popup) as root is fragile and
needlessly risky. Only three things need root: creating the utun, changing routes, writing the DNS key. Binding
sockets with `IP_BOUND_IF` and ICMP `SOCK_DGRAM` sockets need nothing.

So turning on, from the unprivileged app:

1. The app listens on a Unix socket in its private `$TMPDIR` (mode 0700 folder).
2. It runs `osascript -e 'do shell script "<exe> --tunnel-helper <socket> --dns <servers> &" with administrator
   privileges'`. macOS shows its standard admin password dialog. Cancel → "the administrator prompt was cancelled".
3. The helper (root) cleans stale state (old DNS key; capture routes on a still-alive MultiPath utun), creates the
   utun (10.77.0.1, fd77:6d70::1/64), adds the four capture routes, writes the DNS key, then **sends the utun fd over
   the socket with `SCM_RIGHTS`** and closes its own copy.
4. The app wraps the fd in `tun::AsyncDevice` and runs the shared engine.
5. The helper blocks on the socket. "stop" from the app, or EOF because the app died (even `kill -9`), makes it delete
   the routes and the DNS key, answer "done", and exit. If the app died its fd closed, so the utun and every route on
   it are already gone: the kernel is the fail-safe.

When MultiPath itself runs as root (`sudo multipath --headless`), the same setup runs in-process with the same guard
types, no prompt. Production would replace step 2 with an `SMAppService` launch daemon (signed, notarised, XPC with
code-signing requirement checks) or a Network Extension `NEPacketTunnelProvider`; both need a Developer ID.

## Schema

Tables: 0. No database; `settings.json` and `usage.json` in `~/Library/Application Support/dev.multipath.app/`,
unchanged format. Access rules: per-user files, written by the unprivileged app only (the helper writes nothing).

## API (Tauri commands, unchanged) and new process entry points

| entry | does | who | input | output | flow |
| --- | --- | --- | --- | --- | --- |
| `get_status` / `set_enabled` / `set_link_enabled` / `set_link_weight` / `set_option` / `list_connections` | as upstream | popup | as upstream | `Status` | F01–F06 |
| `multipath --tunnel-helper <socket> [--dns a,b]` | privileged setup, fd hand-off, teardown | root, via the admin prompt | socket path, DNS IPs (validated) | fd + if index over the socket | F01, F07 |
| `multipath --headless [--config dir]` | engine without UI | root (`sudo`) | config folder | JSON status lines | lab |
| `MULTIPATH_DRY_RUN=1` | every route/ifconfig/scutil call is logged, not run | dev/tests | — | log lines | tests |

Jobs (existing, shared): ticker 1 s, health probes 5 s, link refresh 3 s, close-flows-on-lost-links 1 s.

## Module plan (`src-tauri/src/sys/macos/`)

| file | exports (same names as Linux/Windows where they exist) |
| --- | --- |
| `mod.rs` | re-exports; `clock.rs` and `instance.rs` are the Linux files via `#[path]` (pure POSIX, no copy) |
| `adapters.rs` | `has_connector(if_index)`, `adapter_kind(name, friendly_name)`, `interface_name(if_index)` |
| `command.rs` | runs `route`/`ifconfig`/`scutil`/`lsof`, dry-run switch, a recorder for tests |
| `routes.rs` | `Route::add`, `Route::remove_stale`, `add_unicast_address`; argv builders; "File exists"/"not in table" handling |
| `dns.rs` | `ResolverOverride::apply(if_index, servers)`, `ResolverOverride::remove_stale()`; scutil script builder |
| `sockets.rs` | `pin_socket_to_interface` (IP_BOUND_IF/IPV6_BOUND_IF), `socket_owners` (lsof), `process_name` (proc_name) |
| `icmp.rs` | `icmp_echo` over an ICMP `SOCK_DGRAM` socket pinned with `IP_BOUND_IF` (macOS returns the IP header) |
| `elevation.rs` | `is_elevated` (euid 0), `can_elevate`, `elevates_in_place`, `relaunch_elevated` (unsupported), admin-prompt launcher + AppleScript/shell quoting |
| `helper.rs` | `HelperListener` / `HelperClient`: socket, `SCM_RIGHTS` send/receive, stop/done protocol |

Engine: `engine/mod.rs` splits "set up the adapter" (privileged) from "run the stack" (unprivileged). On macOS
the set-up lives behind `engine/privileged.rs` (helper main + in-process path). `is_tun` also matches the adapter by
its 10.77.0.1 address, since utun names can't be chosen. App: `lib.rs` dispatches `--tunnel-helper`; macOS runs as an
accessory app (menu bar only, no Dock icon); the popup is placed under the menu-bar icon; autostart via LaunchAgent.

## The parts that bite

- **Tailscale / other VPN utuns**: never touch routes or resolvers we didn't add; our utun is recognised by address,
  not by name; capture routes are /1s, so Tailscale's 100.64/10 and its own bound sockets keep working.
- **Stale state**: routes are tied to the utun and die with it; the DNS key is in the dynamic store (gone at reboot)
  and is removed on stop, on helper EOF, and before every start. All removals treat "not there" as success.
- **Idempotency**: `route add` of an existing route ("File exists") is success, `route delete` of a missing one is
  success, removing a missing DNS key is success.
- **Races**: the helper sends the fd only after routes and DNS are in place; the app waits at most 2 min for the
  password and 15 s for the fd; on any error the helper tears down before exiting.
- **Socket pinning**: `IP_BOUND_IF` requires the interface to have a scoped route; macOS installs a scoped default per
  interface with a router, which is exactly what discovery requires (a gateway) before a link is used.
- **Wording**: `Status.platform` becomes `"macos"`; the popup says "macOS will ask for your administrator password
  each time you turn on" and "Start at login".

## Build order

1. Vertical slice: compile on macOS with `sys/macos/` (adapters, sockets, clock, instance), discovery of real links
   (S01 with real rows), `cargo test` green.
2. Must-haves: routes + utun + helper fd hand-off + engine split (F01, F07), elevation prompt, DNS guard, wording,
   tray/popup on macOS, packaging `.app`/`.dmg`.
3. Should-haves: ICMP echo, connections page owners via lsof, LaunchAgent autostart, headless on macOS, docs.
4. Fixes: whatever `replica-test` finds.
