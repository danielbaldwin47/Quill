# Latency probes (round 1)

Small, single-purpose scripts kept as evidence for `dev/progress/latency-report.md`. Run from the
repo root with a server on the port you pass.

| file | what it answers |
|---|---|
| `anim.mjs <url> <keys>` | Does an animation running while you type cost latency? (§3.3 of the report: 3.3 ms → 9.4 ms present p50.) |
| `events.mjs <port>` | How many `selection` / `render` / `change` events and layout-reading DOM calls does one keystroke cause? (`Element.getBoundingClientRect` and `Range.getClientRects` are patched to count.) 3.02 → 1.02 and 12.07 → 6.08 across this round's core change. |
| `verify.mjs` | Functional check of the core change: `#input` height tracks `#mirror` after boot, after typing new lines and after a font-size change, in all three faces and both grounds; caret lands on the right line box. |
| `prof.mjs <pace> <keys>` | CPU profile + trace breakdown of the keystroke path (self time per function, per-key totals per trace event). This is what found the forced layout and the word-count scan. |
| `core.before.js` | The round-0 `app/js/core.js`, kept so the A/B in §8 can be re-run: serve a copy of `app/` with this file in place of `js/core.js`. |

## Round 3

| file | what it answers |
|---|---|
| `correctness.mjs` | **Rewritten.** Is anything the reader can *see* ever stale after the deferred render? Types a `~~~` fence into the middle of a 3,285-line document (a ` ``` ` opener is closed by the document's own block 18 lines later; a tilde fence is closed by nothing, so 1,608 lines are deferred), holds the index it typed at instead of searching for it, checks the whole visible range rather than one line, and scrolls 1,200 lines into the pending range *in the same frame*. 17 assertions; exits non-zero on any failure. Round 2's version searched for its fence with `findIndex` and matched the document's own — it never checked a deferred line, and it hid two core.js bugs (report §8.1). |
| `spare.mjs` | How much spare time is there inside one keystroke? Adds 0, 0.5, 1, 2, 4, 8 ms of pure busy-wait to every `input` event and watches keystroke → committed frame. Up to 2 ms changes nothing: the keystroke is scheduler-bound, not work-bound (report §7). |
| `attribution.mjs` | Who spends the keystroke? Splits the main-thread work by *nesting* trace events inside one another (so nothing is double counted): Chrome's textarea insertion / Quill's JS / the layout Quill forces by measuring the caret / the frame's own style-layout-paint / idle callbacks. |
| `ladder.mjs` | The five milestones of one keystroke — JS done, painted, committed, presented — with a histogram. The prototype for what `tools/latency.mjs` now records for every key. |
| `sweep.mjs`, `split.mjs`, `contain52k.mjs` | A/B page-level changes: `#mirror` off, `#caret-layer` off, `.chrome` off, `contain: layout style` on `#mirror .line`, and the `:has()` placeholder rule deleted. This is where the negative results in NOTES.md come from — containment moves PrePaint and Paint and does **not** move end-to-end latency, twice, at 10k and 55k words. |
| `timeline.mjs`, `cnt.mjs` | The main-thread event sequence of individual keystrokes, and how many Layout/Paint passes one keystroke really causes. This is what found the double forced layout and the shape of the frame. |
| `r3-summary.py` | Pulls the round-3 tables out of the raw `r3-*.json` runs. |
