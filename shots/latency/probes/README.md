# Latency probes (round 1)

Small, single-purpose scripts kept as evidence for `progress/latency-report.md`. Run from the
repo root with a server on the port you pass.

| file | what it answers |
|---|---|
| `anim.mjs <url> <keys>` | Does an animation running while you type cost latency? (§3.3 of the report: 3.3 ms → 9.4 ms present p50.) |
| `events.mjs <port>` | How many `selection` / `render` / `change` events and layout-reading DOM calls does one keystroke cause? (`Element.getBoundingClientRect` and `Range.getClientRects` are patched to count.) 3.02 → 1.02 and 12.07 → 6.08 across this round's core change. |
| `verify.mjs` | Functional check of the core change: `#input` height tracks `#mirror` after boot, after typing new lines and after a font-size change, in all three faces and both grounds; caret lands on the right line box. |
| `prof.mjs <pace> <keys>` | CPU profile + trace breakdown of the keystroke path (self time per function, per-key totals per trace event). This is what found the forced layout and the word-count scan. |
| `core.before.js` | The round-0 `app/js/core.js`, kept so the A/B in §8 can be re-run: serve a copy of `app/` with this file in place of `js/core.js`. |
