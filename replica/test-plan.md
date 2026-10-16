# Test plan: MultiPath macOS port

Build: `18c3a8b` + the e2e/contrast commit  Date: 2026-10-17  Env: MacBook Pro M4, macOS 26.6.2, Wi-Fi only (en0),
Tailscale active on utun8. No system-wide routes and no `sudo` during automated testing (task safety rules).

Automation: `cargo test` (85 unit tests on macOS, 62 on Linux in Docker), Vitest (26), Playwright (20, `npm run e2e`,
against `demo.html` fixtures in Chrome with an axe scan and a console-error check on every page), CLI smoke tests on
the release binary. Cases marked **manual** need the admin password and/or a tethered phone: see the checklist in
`BUILD_RESULT.md`.

| case | flow | type | steps | expected | auto | result |
| --- | --- | --- | --- | --- | --- | --- |
| F01-H1 | turn on | happy | Turn on → password → helper | utun up, 4 capture routes, DNS key, popup "Active" | manual | not run (needs admin) |
| F01-H2 | turn on | happy: protocol | helper connects, sends "ok <index>" + fd | app gets a working fd (CLOEXEC), stop → "done" | cargo `hands_over_a_descriptor_and_stops` | pass |
| F01-H3 | turn on | happy: already root | `sudo multipath --headless` | in-process setup, no prompt | manual | not run (needs sudo) |
| F01-E1 | turn on | edge: helper reports an error | helper sends "err …" | message shown as the error, nothing left behind | cargo `passes_on_the_helpers_error` | pass |
| F01-E2 | turn on | edge: helper never starts | no connection within the timeout | "the privileged helper didn't start" | cargo `gives_up_without_a_helper` | pass |
| F01-E3 | turn on | edge: route fails half-way | 3rd `route add` fails | routes already added are deleted | cargo `a_failed_route_undoes_the_ones_before_it` | pass |
| F01-E4 | turn on | edge: route already there | `route add` says "File exists" | treated as success | cargo `adding_twice_is_fine` | pass |
| F01-E5 | turn on | edge: dry run | `MULTIPATH_DRY_RUN=1` | commands logged, prompt not shown | cargo `the_fake_records_and_answers`, code review | pass |
| F01-E6 | turn on | edge: Tailscale utun present | discovery | utun8 never a link, never touched | live `discovery_on_this_machine` | pass |
| F01-N1 | turn on | negative: password dialog cancelled | osascript error -128 | "the administrator prompt was cancelled", hero "Needs attention" | cargo `a_dismissed_prompt_is_a_clear_error`, e2e `F01-N1` | pass |
| F01-N2 | turn on | negative: impostor on the socket | non-root peer connects | refused (PermissionDenied) | cargo `refuses_a_helper_that_isnt_root` | pass |
| F01-N3 | turn on | negative: injected helper args | `--dns '1.1.1.1,$(reboot)'`, relative socket | exit 2, nothing run | cargo `refuses_bad_requests`, CLI smoke | pass |
| F01-N4 | turn on | negative: helper without root | `multipath --tunnel-helper /x` | exit 1 "needs to run as root" | CLI smoke | pass |
| F01-U1 | turn on | UI: off / active screens | demo states | plain hero text, Turn on, privacy line, 3 rows, spread lanes | e2e + vitest | pass |
| F02-H1 | switch a network off | happy | toggle a row | flows on it closed, new ones elsewhere | shared code (`aborting_a_link_closes_only_its_flows`) | pass |
| F02-E1 | switch a network off | edge: all switched off | every row off | "All networks are switched off" + tip | vitest | pass |
| F03-H1 | failover | happy | link loses internet | two probe misses → down, flows closed, message "… moved to Wi-Fi" | shared code + vitest `linkChange` | pass (logic); live manual |
| F03-H2 | failover | happy: probe pinned with IP_BOUND_IF | live probe through en0 | handshake succeeds (13 ms) | live `live_probe` | pass |
| F03-E1 | failover | edge: link comes back | probe succeeds | "… is back" message, used again | vitest | pass |
| F03-E2 | failover | edge: no link works | all down | hero "No internet", header "On · no network is working" | vitest + e2e | pass |
| F04-H1 | tune balancing | happy | weights, adaptive, sticky | balancer follows (shared tests) | cargo `net::balancer`, `net::router` | pass |
| F04-E1 | tune balancing | edge: invalid DNS | `dns.google` | rejected | cargo `dns_servers_are_checked` | pass |
| F04-U1 | settings | plain explanations | settings page | sticky/DNS/privacy explained | vitest + e2e | pass |
| F05-H1 | connections page | happy | flows with owners | owner via lsof | cargo `reads_lsof_output` | pass (parser); live manual |
| F06-H1 | open at login | happy | toggle | LaunchAgent plist written / removed | cargo `agent_runs_the_app_at_login` | pass (plist); live manual |
| F07-H1 | quit / stop | happy | Turn off / Quit | routes then DNS removed, utun closed | cargo `tearing_down_removes_routes_then_dns`, `a_route_is_removed_when_dropped` | pass (commands); live manual |
| F07-E1 | crash cleanup | edge: app killed | `kill -9` while on | helper sees EOF and removes DNS; utun + routes go with the fd | cargo `the_helper_notices_the_app_is_gone` | pass (protocol); live manual |
| F07-E2 | crash cleanup | edge: stale DNS key at start | key present | removed once, second call is a no-op | cargo `stale_entries_are_removed_once` | pass |
| F07-E3 | crash cleanup | edge: stale routes | `route delete` "not in table" | success, idempotent | cargo `cleanup_is_idempotent` | pass |
| X-1 | all | keyboard only | Tab through the popup | main switch, every network switch, Settings, Connections reachable | e2e | pass |
| X-2 | all | screen reader labels | roles and names | `role=status` hero, labelled rows, switches, lanes | e2e + vitest | pass |
| X-3 | all | contrast (WCAG AA) | axe on 6 pages × dark/light | no serious violations | e2e | pass after BUG-001/002 |
| X-4 | all | Reduce Motion | `prefers-reduced-motion` | no pulse animation, numbers don't ease | e2e | pass |
| X-5 | all | long names | "USB 10/100/1000 LAN" | ellipsis, no overflow at 360 px | screenshots | pass after BUG-003 |
| X-6 | all | app starts in the menu bar | release binary `--autostart` | stays up, no routes added, clean log, stops on SIGTERM | smoke | pass |
| X-7 | all | single instance | second launch | first copy shows its popup | cargo `only_one_copy_runs`, `shows_on_request_only` | pass |
| X-8 | build | other OSes | Linux fmt/clippy/test (Docker), Windows clippy (cross, check only) | green | CI-like | pass |
