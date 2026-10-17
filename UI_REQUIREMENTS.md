# UI/UX steering — novice-friendly macOS experience

This requirement was added by Timo while the port was in progress. Apply it before calling the build finished.

## Goal

The menu-bar app must look modern and be understandable without networking knowledge. A user should immediately see whether MultiPath is active, which connections are participating, what traffic is doing, and whether intervention is needed.

## Required presentation

- Clear hero state at the top: `Off`, `Starting…`, `Active`, `Limited`, or `Needs attention`, with color, icon, and one plain-language sentence.
- Show a combined live throughput visualization, not just raw numbers. Use a calm animated ring/flow/compact chart when motion is allowed.
- One visual card/row per connection (Wi-Fi, Ethernet, iPhone/USB): recognizable icon, friendly name, enabled toggle, health, current up/down rate, share of active flows, and a subtle per-link color.
- Visually show new connections being distributed across active links. Do not imply that one TCP connection is split.
- Explain status in ordinary language, e.g. `New downloads are using Wi-Fi and iPhone`, not routing jargon.
- When a link fails, show what happened and what MultiPath did: `iPhone disconnected — new connections moved to Wi-Fi.`
- Put technical details (interface name, address, route/DNS details, diagnostic data) behind an `Advanced` disclosure.
- Settings must use short explanations and safe defaults. Explain sticky sites and DNS in plain language.
- Include empty/onboarding states: only one usable connection, permissions needed, tether not detected, no internet, and all links disabled.
- Include explicit privacy reassurance: local only, no relay, no account.
- Keep the compact menu-bar format but allow a slightly larger macOS popover if needed for legibility.
- Follow macOS visual conventions: system typography, sensible spacing, native-feeling controls, light/dark mode, keyboard navigation, visible focus, VoiceOver labels, sufficient contrast, and Reduce Motion support.
- Avoid dense dashboards, unexplained abbreviations, neon/gamer styling, and decorative animations that obscure status.
- Use fresh copy; do not copy proprietary assets. Existing MIT MultiPath assets may be retained under its license.

## Verification

- Add component/view tests for the major states above.
- Capture screenshots of at least Off, Active with 2–3 links, and Needs-attention states.
- Record the UI work and any remaining limitations in `replica/build-log.md` and `BUILD_RESULT.md`.
