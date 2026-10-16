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

## To check (not reproduced here)

- The same `--text-3` contrast shortfall exists in the Windows/Linux dark flyout (design token); change it in Pencil
  (`design.pen`) first, as CLAUDE.md asks, then in `tokens.css`.
- Whether macOS's resolver actually prefers MultiPath's supplemental "match all" resolver over Tailscale's while both
  are present (needs the live run).
- Whether the popup window itself (not the headless render) looks identical, incl. translucency over the desktop.
