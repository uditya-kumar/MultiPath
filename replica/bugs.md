# Bugs — MultiPath macOS port

Only reproduced bugs. Severity: S1 core blocked / security, S2 feature broken, S3 workaround or visibly wrong, S4 cosmetic.

### BUG-001: muted text below WCAG AA contrast on macOS (dark)

- Severity: S3
- Flow / case: all / X-3
- Screen: S01–S03
- Build: `18c3a8b`  Browser / device: Chrome 1xx (Playwright, `channel: chrome`), 360 px

Steps
1. `npm run e2e` (axe scan on `demo.html?state=active`, dark).

Expected: no serious axe violations.
Actual: `color-contrast` on every `.muted` / `.setting-desc` / Advanced summary: `--text-3` #8e8e8e at 11 px on the
translucent popup is about 3.7:1. Unconnected rows (`.dim`, opacity 0.55) were lower still.
Evidence: e2e `F01 active…` failure listing 20 nodes.
Suspected cause: the design's `--text-3` token. It also applies to Windows/Linux, where it's left as designed (see
"to check").
Status: fixed in the e2e/contrast commit: on macOS `--text-3` #a8a8a8 and only the icon of an unconnected row fades.

### BUG-002: setting descriptions below contrast in light mode

- Severity: S3
- Flow / case: F04 / X-3
- Screen: S03 (light)
- Build: `18c3a8b`

Steps
1. `npm run e2e` → `F04/F06 settings … light mode passes contrast`.

Expected: no serious axe violations.
Actual: `.setting-desc` in the light palette (#6b6b70) below 4.5:1 on the grey row.
Evidence: e2e failure listing the five descriptions.
Status: fixed in the e2e/contrast commit (#57575c).

### BUG-003: long network names clipped without an ellipsis in "Where connections go"

- Severity: S4
- Flow / case: F01 / X-5
- Screen: S02
- Build: before `18c3a8b`

Steps
1. Open `demo.html?state=active` (a link named "USB 10/100/1000 LAN").

Expected: "USB 10/100/1…" with an ellipsis.
Actual: "USB 10/100/100" cut mid-character.
Evidence: first `replica/clone-screens/active-dark.png` capture.
Status: fixed in `18c3a8b` (the label is its own ellipsised span).

### BUG-004: header and hero disagreed when no network had internet

- Severity: S3
- Flow / case: F03 / F03-E2
- Screen: S02
- Build: before `18c3a8b`

Steps
1. Open `demo.html?state=attention` (on; Wi-Fi up but failing probes, phone unplugged).

Expected: the header and the hero tell the same story.
Actual: header "On · using 1 network" (counted enabled links) under a hero saying "No internet".
Evidence: first `replica/clone-screens/attention-dark.png` capture.
Status: fixed in `18c3a8b`: the header counts working links ("On · no network is working"); a regression test is in
`summary.test.ts`.

### BUG-005: the helper's private folder could collide

- Severity: S2 (turning on could fail with "File exists")
- Flow / case: F01 / F01-H2
- Build: `31d324b`

Steps
1. Run `cargo test --lib` repeatedly (1 failure in ~25 runs: `gives_up_without_a_helper`).

Expected: every listener gets its own folder.
Actual: `Os { code: 17, kind: AlreadyExists }`: names were pid + sub-second nanos, and macOS clocks tick in µs.
Evidence: failing test output.
Status: fixed in `938a055` (counter + retry); 30/30 runs green after.

### BUG-006: Vitest ran a tooling example spec outside the app

- Severity: S4
- Build: `6ebf268`
- Steps: `npm test` → `.claude/skills/replica-test/e2e.example.spec.ts` fails to load.
- Status: fixed in `31d324b` (`test.include: src/**`).

### BUG-007: turning on cut the Mac off the internet (pinned sockets had no route)

- Severity: S1 (core blocked: with MultiPath on, nothing reaches the internet)
- Flow / case: F01 (turn on) / live test
- Build: `0c9c51d` (the arm64 `.app` from `BUILD_RESULT.md`), MacBook Pro M4, macOS 26.6.2, Wi-Fi only (`en0`
  192.168.178.157, primary), Tailscale running (`utun8`)

Steps
1. Open the app, turn on, enter the administrator password.
2. `curl https://1.1.1.1`, `curl https://example.com`, `ping 1.1.1.1`.

Expected: all three work through en0; popup "Active" or "Only using Wi-Fi".
Actual: app PID 92635 started root helper PID 92772; `utun9` had 10.77.0.1 and fd77:6d70::1; 0/1 and 128.0/1 pointed at
utun9; the DNS key existed. Both curls timed out, ping lost 100%, the popup said "No internet". Killing the app
removed the routes and the helper, and curl worked again at once.
Evidence: the user's live run (above); `netstat -rn -f inet` shows en0 with only the unscoped default
`default 192.168.178.1 UGScg en0` and no scoped (`UGScIg`) one; XNU `bsd/net/route.c` `rt_lookup_common` /
`node_lookup_default`, `bsd/net/radix.c` `rn_lookup_args`.
Root cause: "No internet" means en0's health probes (TCP to 1.1.1.1:443 / 8.8.8.8:443 through a socket pinned with
`IP_BOUND_IF`) failed, so the router had no usable link and refused every flow (and ping, and DNS). A scoped lookup
for en0 first finds 0/1 → utun9 (wrong interface, discarded), then searches routes scoped to en0: there are none for
the internet, because configd adds scoped defaults only for non-primary interfaces. The last fallback,
`node_lookup_default`, is `rnh_lookup(0.0.0.0, mask NULL)`, i.e. a longest-prefix *match* for 0.0.0.0, which returns
our own 0.0.0.0/1 on utun9, not the default route; its interface isn't en0, so: no route. IPv6 the same with ::/1.
Before turning on, the unscoped lookup lands on en0's default directly, which is why the earlier `live_probe` passed.
Non-primary networks (a phone tether) have scoped defaults and would have worked; the primary one never did.
Fix: on macOS the capture routes cover everything but 0.0.0.0/8 and ::/16 (1.0.0.0/8, 2.0.0.0/7 … 128.0.0.0/1 and
1::/16 … 8000::/1; 24 routes instead of 4), neither of which is ever a destination (NAT64 64:ff9b:: is still
captured). The fallback lookup of 0.0.0.0 / :: then finds the real default on the primary interface. Windows and
Linux keep the four /1 routes. Regression test `engine::tests::capture_spares_the_default_route_lookup` (failed
before the fix with "0.0.0.0/1 hides the default route from pinned sockets"), plus
`capture_covers_every_internet_address_once`.
Status: fixed in code (commit "macOS: capture routes no longer hide the primary network's default route"),
unit-tested. After the rebuild it was turned on again at the Mac (08:54, helper = new executable). Read-only lookups
showed the new routes on utun9 (no 0/1) and `route -n get -ifscope en0 1.1.1.1` → en0 via 192.168.178.1 (IPv6 likewise),
which is the lookup that failed. **Not yet verified with traffic** (curl/ping/DNS): `BUILD_RESULT.md` checklist step 3.

## To check (not reproduced here)

- The same `--text-3` contrast shortfall exists in the Windows/Linux dark flyout (design token); change it in Pencil
  (`design.pen`) first, as CLAUDE.md asks, then in `tokens.css`.
- Whether macOS's resolver actually prefers MultiPath's supplemental "match all" resolver over Tailscale's while both
  are present (needs the live run).
- Whether the popup window itself (not the headless render) looks identical, incl. translucency over the desktop.
