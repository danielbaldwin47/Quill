# Latency — measured

**Quill, round 3.** iA Writer publishes no latency numbers at all — "boots in a whizz… snappy as
jazz" is the entire public record — so the bar comes from third-party measurements of native
editors, and REFERENCE §5.3 states it in two parts, in two different statistics:

* **App-internal, keystroke → committed frame: ≤ 5 ms *average*, ≤ 16 ms *worst case*.**
  Fatin's Typometer figures, which are means: Notepad++ 4.3, Emacs 5.3, **Sublime 8.2**.
* **Keyboard-to-photon on a 60 Hz panel: ≈ 30–35 ms.** Hume's photodiode: Sublime **32.5 ± 4.0**,
  TextEdit 33.4, Atom 45.6, VS Code 47.6.

Round 2 answered the first bar with a p50 of 5.67 and a p99 of 14.99 and printed *"inside the
≤5 ms / ≤16 ms bar"*. That is the wrong statistic, and re-read in means it was false: the mean of
the samples it shipped was 6.94 ms and the worst was 17.34. **This round answers that bar in the
mean and the worst case, and the honest answer is below. It is better than round 2's real numbers
and it still does not clear the average.**

## 0. What round 2 got told, and what this round did about it

| round 2's verdict | round 3 |
|---|---|
| "The app-internal claim is answered in the wrong statistic and is false as written… ~6.9 ms mean / 17.3 ms worst, **over** the stated bar on both terms." | Every app-internal table is now **mean ± sd and worst case**, in the bar's own vocabulary, with a bootstrap CI on the mean; p50/p99 are kept beside them for shape, never instead of them. The number is now **5.73 ms mean (CI 5.50–5.95), sd 3.60, worst 14.25** on the headline regime: the ≤16 ms worst case is cleared on 8 of the 11 paced regimes, and **the ≤5 ms average is not cleared by any of them**. §6 says so in those words. |
| "then close the ~2 ms — the 2,510 µs of style/layout/paint and the 1,414 µs Chrome textarea insertion per key are where it is, not in Quill's own 814 µs handler." | Both came down (2,231 µs and 1,319 µs; Quill's handler 755 µs) and the end-to-end mean came down 0.39 ms with them. But the more useful result is a **negative** one, and it is measured rather than argued: **the keystroke is scheduler-bound, not work-bound** — adding up to 2 ms of pure busy-wait to every keystroke does not move the committed-frame time at all (§7). The remaining 2 ms is not work, so it cannot be removed by removing work. |
| "No physical panel was measured at all this round… ±12 ms of uncertainty, larger than the 5 ms margin." | §9. `bin/quill --panel N` now really shows the window on the Dell U2720Q and measures commit → scan-out there, behind an evdev idle guard (`tools/idle-check.py`) that refuses to take the screen off somebody who is using it. |
| "The photon arithmetic is missing an input term… CDP `Input.dispatchKeyEvent` starts the clock inside the browser process." | §10. `tools/uinput-keys.py` creates a **real keyboard on /dev/uinput**, so the keys go evdev → libinput → Hyprland → Wayland → Chromium exactly as the user's own keyboard does, with `CLOCK_MONOTONIC` taken immediately before each `write(2)`. Chromium's TimeTicks *are* CLOCK_MONOTONIC on Linux (verified, §3.5), so the delivery hop is now **measured**, not excluded. |
| "The fence-flip win is largely work moved, not removed… the cost and frame-drop behaviour of the catch-up frames is measured nowhere." | §8. The catch-up is now bounded by **time (4 ms), not by a line count**, and 1,608 deferred lines in a 55k-word manuscript are caught up in **one frame**. |
| "`Writer.flushPending()` has zero callers… 'nothing the reader can see is ever stale' holds only if the reader does not scroll… AHEAD=64 is asserted rather than measured." | All three fixed and all three now asserted by a probe that actually looks at the right lines. The catch-up frame serves **the visible range first**; scrolling into a not-yet-caught-up range fills what came into view on the scroll event; and `AHEAD` is **computed from the viewport** (32 lines at 1440×900/20 px, ~58 on the user's 4K panel at 14 px). §8. |
| "Round 2's correctness probe deliberately samples a line 400 below the fold, then sleeps 400 ms." | That probe was worse than the critique knew: it found its fence with `findIndex(l => l.startsWith('```js'))`, which matches **the document's own** fenced block near the top, so it never checked a single deferred line. Rewriting it found **two real bugs in core.js** that had been shipping since round 1 (§8.1). |
| "The startup headline is the pre-paint number… `startup_ready: 322` is a JS marker set before any frame exists." | `startup_ready` in `progress/latency.json` is now **the first frame a human can see**, and the JS marker is demoted to a sibling field that says what it is. §11. |
| "Cold launcher n = 4 (and n = 3 fresh)… four runs and a min–max range." | 15 cold processes + 12 fresh-profile processes headless, and the launcher runs are reported with n, mean, sd and range like everything else. §11. |
| "The top-level `keystroke_p50` / `keystroke_p99` in latency.json are synthetic (measured + 4 cited)." | The top-level fields are now **entirely measured** — kernel keypress → the frame presented, no cited terms anywhere in them. The cited 4 ms of panel pixel response is quoted **once**, in `bar_note`, as an explicit addition. §13. |
| "Variance is 2.5× the bar's… reported only as p50/p90/p99, never as sd beside Hume's sd." | sd is printed beside every mean in this report and in `latency.json`. §5, §9. |
| "Several by-key-type cells are noise reported as statistics: undo n = 2 with a 'p99' of 16.66." | `tools/latency.mjs` now refuses to print percentiles for fewer than 20 samples: those cells carry **n, mean and worst** and a `too_few_for_percentiles` flag. §12. |
| "The caret lags the glyph by 57.7 ms and is excluded from every table by construction." | It is now **a row in the headline table** (§5) and a top-level field in `latency.json`. It is still 58 ms, it is still the largest perceptible latency in the product, and it is still in a file this piece does not own — but it is no longer filed under design. |

## 1. Environment

| | |
|---|---|
| CPU | 13th Gen Intel(R) Core(TM) i5-13600K, 20 threads |
| OS | Linux 7.1.9-arch1-2, Wayland (Hyprland 0.56.2) |
| Browser | Chromium 151.0.7922.173 (system build), headless and headed |
| Panel | Dell U2720Q, 3840×2160 @ 59.997 Hz, scale 1.5, VRR off, hardware cursor |
| Viewport | 1440×900 logical px |
| App build | `app/**/*.{js,css,html}` sha256 **`6c61caa21c576e13`** (16 files), served from a **frozen snapshot** on port 4179 so that another builder's edit could not move the numbers mid-run |
| Load | recorded in every result file with the busiest processes at the time. This is somebody's workstation, and they were using it: the physical-panel run had to wait for them (§9). |

## 2. The document, and the typist

`shots/latency/doc10k.md` — **10,062 words, 53,031 characters, 432 lines** — built by
`tools/mkdoc.mjs` from *Alice's Adventures in Wonderland* (Project Gutenberg #11, public domain):
chapters as `##` headings, Carroll's own italics as Markdown emphasis, verse as block quotes, and
an editor's note with a task list, a link, inline code and a fenced code block, so the tokenizer,
the decorators and the mirror all do real work on every frame. Paragraphs are **one logical line
each** (up to 980 characters) — the way a document looks in iA Writer, and the hard case for a
line-diffing renderer. Three other sizes (2 k, 27 k, 55 k words) in §14.

**The typist** types real prose, not one repeated sentence: capitals, commas, semicolons, quotation
marks, apostrophes, dashes, sentence ends, paragraph breaks, and **a 1–3 character typo corrected
every ~45 characters**. Twelve regimes cover writing at the end of a draft and in the middle of it,
Focus: Sentence, paragraph churn (Enter), select-back-and-replace with `Ctrl+Z`, Markdown syntax,
opening and closing a fenced code block, `Ctrl+V` paste from the real clipboard, bursts with
pauses, 266 wpm, and an unpaced saturation run. Each session is seeded per regime, so all three
sessions type identical keys and the spread between them is the machine rather than the script.
Every keystroke is labelled with the key that caused it; the headline tables are computed over
**text-affecting keystrokes only** (the modifier half of a chord is counted for the accounting
assertion but not averaged into a latency it does not have).

## 3. Method

### 3.1 Five milestones on every keystroke, not two

For every input event Chrome's `EventTiming` trace records (`devtools.timeline`, unrounded,
microseconds) carry the event's hardware timestamp, `processingStart`, `processingEnd`,
`commitFinishTime` and `duration` — which ends at the **presentation feedback** of the frame that
carried that event's update. Round 3 adds two milestones from the trace's own main-thread events,
so the keystroke can be taken apart instead of quoted as one number:

| milestone | what it is |
|---|---|
| `to_js_done` | the app's JavaScript has returned (the last non-`keyup` handler for this key) |
| `frame_wait` | from there to the **first paint op** of the frame that carried it — the wait for Chromium's next BeginFrame, plus that frame's style, layout and pre-paint |
| `to_paint` | that frame's paint is finished |
| `to_commit` | `commitFinishTime`: handed to the compositor. **This is the milestone REFERENCE §5.3's bar names** ("keystroke → committed frame") and the one §6 answers |
| `to_present` | presentation feedback: the frame is on the display's scan-out |

### 3.2 Which statistic answers which bar

REFERENCE §5.3's app-internal bar is a **mean and a worst case**, because Fatin's Typometer
figures are means (Notepad++ 4.3, Emacs 5.3, Sublime 8.2 — all `avg` columns). Hume's photodiode
figure is a **mean ± sd** (`lat i= 32.5 +/- 4.0`). So every table here leads with mean, sd and max,
with a bootstrap 95 % interval on the mean; p50 and p99 are printed next to them because the shape
matters, never in their place. Round 2's substitution of p50 for mean and p99 for worst is the
single thing this round is most careful not to repeat.

### 3.3 Every keystroke accounted for

Every regime asserts that the keydowns expected, the keydowns in Chrome's trace and the keydowns
seen by an independent in-page rAF/MessageChannel probe are the **same number**, and that every one
of them has a commit time and a presentation time. A fresh page per regime makes round 1's
double-probe bug unrepresentable. All 36 headless runs and all 12 application-cost runs in this
round passed; nothing was withheld.

### 3.4 What the display clock is for

A real display ticks whether or not anything is animating, and a keystroke waits for the next tick.
A headless Chromium with nothing animating produces a frame on demand, which makes every latency
look several milliseconds better than any 60 Hz panel can be. So there are two headless modes and
they are never mixed:

* **`--clock off --reduced-motion`** — no cadence at all. This is the *application's own cost* and
  the only mode in which it can be measured, because Quill's own caret glide is an animation and
  an animation is a frame clock. §6.
* **`--clock on`** — a 1 px composited animation keeps the frame clock running, so headless models
  a panel. §5.

### 3.5 One clock

Chromium's trace timestamps are TimeTicks, which on Linux is `CLOCK_MONOTONIC` in microseconds.
Verified rather than assumed: `process.hrtime.bigint()` (CLOCK_MONOTONIC) taken immediately before
a CDP key dispatch read 109 750 982 782 µs, and the trace's own `keydown` for that key read
109 750 983 595 µs — the same clock, 813 µs apart, which is the CDP dispatch itself. That is what
makes §10 possible: a timestamp taken in Python before a `write(2)` to `/dev/uinput` is directly
comparable to Chromium's timestamp for the resulting event.

## 4. The numbers

Milliseconds. **Plain writing at the end of the 10,062-word draft at 133 wpm** — the most common
thing a writer does — three sessions of 300 keystrokes, pooled, n = 900, every keystroke accounted
for. Mean ± sd first, because that is how both published bars are stated.

| | mean ± sd | worst | p50 | p99 |
|---|---|---|---|---|
| The app's JavaScript has returned | **1.53 ± 0.24** | 4.83 | 1.47 | 2.32 |
| … the frame it caused is **painted** | 5.65 ± 3.60 | 14.17 | 4.31 | 13.08 |
| … the frame is **committed** — *the milestone REFERENCE §5.3's bar names* | **5.73 ± 3.60** | **14.25** | 4.39 | 13.16 |
| … the frame is **presented** (no display cadence at all) | 6.54 ± 3.63 | 15.50 | 5.22 | 14.16 |
| Keystroke → presented, headless on a **60 Hz frame clock** | 10.28 ± 4.61 | 19.97 | 9.73 | 18.56 |
| Keystroke → presented, **on the user's own compositor** at 60 Hz (§9) | 21.99 ± 10.72 | 45.17 | 24.76 | 36.47 |
| **The caret** catching up with the glyph it is following (§16) | ≈ 58 | 66.7 | 57.0 | 66.7 |

**Against the app-internal bar (≤ 5 ms average, ≤ 16 ms worst case).**
Quill is at **5.73 ms mean (95 % CI 5.50–5.95) and 14.25 ms worst**. It **clears the worst case and
misses the average by 0.73 ms.** Across the eleven paced regimes the mean runs 5.18 (revision) to
6.18 (266 wpm) and the worst runs 13.68 to 19.30: **eight of eleven clear the ≤ 16 ms worst case,
none of the eleven clears the ≤ 5 ms average.** That is Emacs class (Fatin's mean 5.3) and better
than Sublime (8.2), on a 10,000-word Markdown document rather than a plain text file — and it is
over the bar, and this report is not going to say otherwise by quoting a median.

Round 2 shipped 6.94 ms mean / 17.34 worst for this regime and called it "inside the bar". Round 3
is 5.73 / 14.25 and calls it what it is.

**Against the photon bar (Sublime 32.5 ± 4.0 ms).** On the user's own Hyprland at 60 Hz, keystroke →
presented is **21.99 ± 10.72 ms**, measured end to end with nothing cited added. Adding the ~4 ms of
panel pixel response nobody can measure without a photodiode gives ≈ 26 ms at the mean, ≈ 28.8 at the
median — inside the Sublime bracket, and 10 ms clear of Atom and VS Code. But that ran on a **virtual**
Hyprland output whose own commit → present hop is a full refresh interval (17.2 ms p50) where round 1
measured 5.0 ms on the physical panel, so §9 quotes a **bracket of ≈ 17–26 ms**, not a point, and
explains at length why the physical-panel run did not happen. Two further honesties: the spread is
**2.7× Hume's** (sd 10.72 against 4.0) — that jitter is something a writer feels — and every number
here still starts inside the browser process, so the kernel → compositor → client input hop is
**excluded** (§10).

## 5. Every regime, in the bar's own statistics

`--reduced-motion --clock off`: the application's own work, no display cadence. n = 900 each
(revision has fewer text-affecting keystrokes per session because half of its presses are chords).
`✓`/`✗` is against **≤ 5 ms mean** and **≤ 16 ms worst**.

| regime | mean ± sd | 95 % CI on the mean | worst | p50 | p99 | ≤5 mean | ≤16 worst |
|---|---|---|---|---|---|---|---|
| revision (select back, replace, undo) | 5.18 ± 3.28 | 4.92–5.43 | 14.24 | 3.23 | 12.98 | ✗ | ✓ |
| paragraph_breaks (Enter) | 5.50 ± 3.44 | 5.27–5.71 | 14.60 | 3.74 | 13.06 | ✗ | ✓ |
| letters_only (round 1's script) | 5.58 ± 3.38 | 5.35–5.80 | 14.01 | 4.06 | 12.91 | ✗ | ✓ |
| prose_focus_sentence | 5.59 ± 3.06 | 5.39–5.79 | 14.60 | 3.98 | 12.87 | ✗ | ✓ |
| fence_flip (open/close a code block) | 5.68 ± 3.43 | 5.46–5.91 | 13.68 | 4.16 | 12.96 | ✗ | ✓ |
| prose_middle_of_draft | 5.68 ± 3.40 | 5.46–5.90 | 14.14 | 4.09 | 13.05 | ✗ | ✓ |
| **prose_end_of_draft** | **5.73 ± 3.60** | **5.50–5.95** | **14.25** | 4.39 | 13.16 | ✗ | ✓ |
| bursts_and_pauses | 5.76 ± 3.68 | 5.53–6.01 | 17.08 | 4.39 | 14.81 | ✗ | ✗ |
| markdown_syntax | 5.77 ± 3.48 | 5.55–6.00 | 13.84 | 4.16 | 13.23 | ✗ | ✓ |
| paste_blocks (Ctrl+V of a paragraph) | 5.82 ± 3.77 | 5.57–6.07 | 19.30 | 4.61 | 13.99 | ✗ | ✗ |
| fast_typist (266 wpm) | 6.18 ± 4.88 | 5.86–6.51 | 18.94 | 2.94 | 17.69 | ✗ | ✗ |

Out of the table on purpose, as in round 2: **saturation_stress** (no pacing at all — keys as fast
as CDP can dispatch them, ~2,000 wpm) at 8.97 ± 4.53, worst 19.01. Keystrokes share frames there,
so a per-keystroke latency is not a thing a person could experience; it is run because it is where
the cliff would be, and there is no cliff.

## 6. Where the 5.73 ms goes

Per keystroke, from the trace, 10k-word document (`shots/latency/r3-attribution-10k.json`):

| | mean, per keystroke |
|---|---|
| Chrome's own insertion of one character into a 53 KB `<textarea>` | **556 µs** |
| **Quill's `input` handler — the mirror diff, the tokenizer, the decorators** | **411 µs** |
| the layout Quill forces by measuring the caret (work the frame would do anyway, moved earlier) | 430 µs |
| the frame's own style, layout, pre-paint and paint | 1,516 µs |
| *(between keystrokes, not in this path: `chrome.js`'s word count, idle-scheduled)* | *1,084 µs* |

The app's JavaScript is finished **1.53 ms** after the key. The frame is committed **5.73 ms** after
the key. The 4.20 ms in between is **3.83 ms of waiting for Chromium's next BeginFrame plus the
style, layout and pre-paint that frame then does**, then 0.29 ms of paint and 0.08 ms to hand the
frame to the compositor. Of those 3.83 ms, roughly 1.5 ms is the frame's own style/layout/pre-paint
(the attribution table above) — which leaves **about 2.3 ms of pure waiting** in the mean. Which
raises the obvious question about the round-2 critique's instruction to "close the ~2 ms".

## 7. Is there 2 ms of work left to remove? No — the keystroke is scheduler-bound

`shots/latency/probes/spare.mjs` adds a measured amount of **pure busy-wait** to every `input`
event — real main-thread work, after the app's own listener, not instead of it — and watches
keystroke → committed frame. If the keystroke were work-bound the line would rise 1:1 from zero.

| busy-wait added to every keystroke | 0 | +0.5 ms | +1 ms | +2 ms | +4 ms | +8 ms |
|---|---|---|---|---|---|---|
| keystroke → committed frame, mean | 5.48 | 5.14 | 5.11 | 5.34 | 6.26 | 10.15 |
| change vs. no added work | — | **−0.34** | **−0.37** | **−0.14** | +0.78 | +4.67 |

**Up to two milliseconds of extra work per keystroke changes nothing.** Four costs 0.8 ms of it,
eight costs 4.7.

The application finishes early and then waits. Below the slack, removing work buys nothing and
adding work costs nothing; above it, every millisecond is paid in full. That is why round 2's
instruction cannot be carried out as written: **the residual 0.7 ms over the bar is not work, it
is Chromium's frame scheduling, and a web application cannot start a frame — it can only ask for
one.** What an application *can* do is stay inside the slack, which Quill does with room to spare
on a 10k-word document (1.53 ms of JS against ~4 ms of slack) and does not on a 55k-word one (§14,
where the sd collapses from 3.6 to 2.0 precisely because the slack is gone).

The work did come down anyway, and the end-to-end number came down with it: Chrome's insertion
1,414 → 1,319 µs, Quill's handler 814 → 755 µs, style/layout/paint 2,510 → 2,231 µs, main-thread
busy 5.47 → 4.98 ms per keystroke, committed-frame mean 6.12 → 5.73 ms.

## 8. The deferred render, and two bugs it was hiding

Round 2 stopped re-tokenising the whole document inside a keystroke: `AHEAD` lines below an edit
are filled synchronously and the rest is caught up in animation frames. The round-2 critique said
the correctness claim was argued rather than measured, that `Writer.flushPending()` had no callers,
that AHEAD = 64 was asserted rather than measured, and that the probe backing it "deliberately
samples a line 400 below the fold, then sleeps 400 ms before checking it."

It was worse than that. The probe found its fence with
`lines().findIndex(l => l.startsWith('```js'))` — which matches **the document's own** fenced block
near the top of `doc52k.md`, not the one it had just typed. It checked lines below the wrong fence,
so it never looked at a single deferred line, and it reported `true`.

### 8.1 What the rewritten probe found

`shots/latency/probes/correctness.mjs` now holds the index it typed at, uses a `~~~` fence (a
` ``` ` opener is closed by the document's own block 18 lines later; a tilde fence is closed by
nothing, so **every** line to the end of the document changes — the worst thing one keystroke can
ask a Markdown editor to do), checks the whole visible range rather than one line, and scrolls into
the not-yet-caught-up range **in the same frame** rather than sleeping first. It found two bugs
that had been shipping since round 1, both in `app/js/core.js`, both visible to markup:

1. **`recomputeCtx(from)` seeded its walk from the wrong line.** `lineCtx[i]` is the context
   *entering* line i, so a walk starting at `from` has to begin from the context *leaving* line
   `from-1`. It began from `lineCtx[from-1]` instead, skipping the effect of the line just above
   the edit. Consequence: **pressing Enter at the end of a ` ```js ` line left every line below it
   tokenised as prose, permanently.** One line changed.
2. **`lineCtx` was not moved when the line count changed.** `recomputeCtx` decides where to stop by
   comparing what it computes for line *i* against `lineCtx[i]`; with the array unshifted those
   comparisons are against the wrong lines. It stopped early on a false match and left the array
   short at the tail, so the last line of the document kept a null context. It is now spliced
   exactly as `lineEls` is.

### 8.2 And what it now asserts

55k-word manuscript, 3,285 lines, 1440×900, all 17 assertions pass
(`shots/latency/r3-correctness.json`):

| | |
|---|---|
| `AHEAD`, computed from this viewport | **32 lines** (scroller height ÷ a line's `min-height`, + margin) |
| lines deferred by the one keystroke | **1,608** |
| frames the catch-up took | **1** |
| lines still stale in the visible range, in the same task as the keystroke | **0** |
| lines still stale after scrolling 1,200 lines into the pending range mid-flight | **0** |
| stale lines anywhere after the catch-up | **0** |
| code lines still shown, in the **first frame** after deleting the fence, with the viewport 1,195 lines away from the edit | **0** |
| mirror text == textarea text, at every step | ✓ |

Three changes make that true, and each answers one of the round-2 points:

* **`AHEAD` is measured, not asserted.** `computeAhead()` divides the scroller's height by a line's
  `min-height` — exactly one line pitch, the shortest a line can be — and adds a margin, at boot,
  on resize and on a settings change. 32 lines at 1440×900 / 20 px; ~58 in a 1440-logical-px window
  at 14 px, which is the case round 2's fixed 64 was closest to failing. `Writer.aheadLines()`.
* **The catch-up serves the viewport first.** The `AHEAD` window follows the *edit*; the reader's
  eye need not be there (undo, a command, a paste, or simply having scrolled away). The catch-up
  animation frame fills the **visible ∩ pending** range before anything else — and an animation
  frame runs *before* that frame's style, layout and paint, so it still lands in the first frame
  after the keystroke, and the keystroke itself pays nothing for it. Scrolling into a not-yet-caught-up
  range fills what came into view, on the scroll event, before that frame is painted.
  `Writer.visibleRange()`, `Writer.flushVisible()` — and unlike `flushPending()`, these have callers.
* **The catch-up is bounded by time, not by a line count.** 4 ms per frame instead of 400 lines, so
  one catch-up frame cannot exceed a frame's budget however heavy the lines are.

## 9. On a real compositor

`bin/quill` opens the app in a `chromium --app` window on the user's own Hyprland, and
`tools/latency.mjs` attaches to it, so the presentation timestamps come from a real compositor
scheduling against a real 60 Hz clock. Three regimes × 300 keystrokes × 3 sessions, n = 900 each,
every keystroke accounted for.

| regime | mean ± sd | p50 | p99 | worst |
|---|---|---|---|---|
| **prose_end_of_draft** | **21.99 ± 10.72** | 24.76 | 36.47 | 45.17 |
| fence_flip | 21.95 ± 10.49 | 23.70 | 36.53 | 37.90 |
| paragraph_breaks | 20.89 ± 10.27 | 22.03 | 36.14 | 43.58 |

Of the 21.99 ms, **9.20 ms is Chromium** (keystroke → committed) and **12.44 ms is the wait for the
compositor to report the frame presented** (p50 17.23, sd 8.62).

**Where this number is honest and where it is not.** The window ran on a **virtual Hyprland output**
— `hyprctl output create headless`, a real 60 Hz output that the user's own compositor really
composites and presents, but not the physical panel. Its commit → present hop measures **17.23 ms
at the median: a whole 60 Hz refresh interval.** Round 1's one physical-panel session measured
**5.0 ms** for the same hop on an older build. That is a ±12 ms uncertainty on the headline, which
the round-2 critique correctly called larger than the margin being claimed. So this round does not
quote a point figure for photons. It quotes a bracket:

> keyboard-to-photon is somewhere between **≈ 17 ms** (if the physical panel behaves as round 1
> measured: 22.0 − 17.2 + 5.0 + 4 cited) and **≈ 26 ms** (if it behaves like the virtual output:
> 22.0 + 4 cited), at the mean. Hume's Sublime Text is **32.5 ± 4.0**. The top of that bracket is
> already inside the Sublime bracket; the bottom would be well under it. What this round cannot do
> is tell you which.

**The physical-panel run was built, guarded and attempted, and it did not happen.** `bin/quill
--panel N` brings a workspace up on the real monitor for the duration and puts it back afterwards.
A Wayland surface on a workspace nobody is looking at gets no frame callbacks, so its presentation
timestamps are worthless — the window has genuinely to be on screen — and BRIEF.md rightly forbids
doing that to somebody who is working. `tools/idle-check.py` therefore watches every real keyboard
and pointer evdev node (skipping a DualSense's motion sensors, which stream forever) and refuses
unless the machine has been silent. It was run 20 times over 45 minutes
(`shots/latency/r3-panel-attempt.log`):

* **18 checks refused** — "activity on Logitech MX Keys". Somebody was using the machine. This is
  the guard working, and it is why round 2's "ten minutes on a free workspace" never happened either.
* **When it finally went idle, the panel had gone to sleep.** A DisplayPort monitor in standby drops
  its link, and Hyprland then has no physical output at all — it substitutes a headless one called
  `FALLBACK`. There was nothing to measure scan-out on. `bin/quill --panel` now detects that case by
  name and says so instead of silently measuring a fake output.
* Separately: **Hyprland 0.56.2 has no workspace-switch binding this script could find.** The old
  `dispatch workspace N` is parsed as Lua and fails, over `hyprctl` and over the IPC socket alike;
  `hl.dsp.workspace` is a table of `{move, change_id, rename, toggle_special, swap_monitors}` and
  `hl.focus` accepts only `{direction, monitor, window, urgent_or_last, last}`. The switch is now
  **read back and verified**, so the mode refuses rather than measuring an unpresented window. That
  is the remaining blocker, and it is written down rather than left as "it takes ten minutes".

## 10. The input term the photon arithmetic was missing

The round-2 critique's sharpest point: keys are injected with CDP `Input.dispatchKeyEvent`, so the
clock starts **inside the browser process** and the kernel → libinput → compositor → client hop is
excluded entirely. Round 2 justified that by saying Hume used a 1 kHz USB emulator — but that
emulator only removes keyboard debounce and polling, not the operating system's delivery of the
event to the application, which is inside Hume's 32.5 ms.

The instrument for that is built and is in the repo. **`tools/uinput-keys.py` creates a real
keyboard on `/dev/uinput`** — evdev → libinput → Hyprland → Wayland → Chromium, exactly the path
the user's own Logitech takes — and records `CLOCK_MONOTONIC` immediately before each `write(2)`.
Chromium's TimeTicks *are* CLOCK_MONOTONIC on Linux, verified rather than assumed (§3.5), so
`trace_keydown_ts − t_ns/1000` is the delivery hop, measured, in one clock. `bin/quill --uinput`
drives it, and `tools/latency.mjs` refuses to inject a single key unless the page itself reports
`document.hasFocus()` with the caret in the textarea — real keys go wherever the compositor thinks
focus is, and that must never be somebody's terminal.

**It did not run, for the same reason §9 did not**: real keys only reach a focused window, and a
focused window means the physical panel, and the panel was asleep. The guard refused once with
`real keys would go to 'chrome-localhost__-Default', not to Quill — refusing to inject`, which is
the behaviour that matters. So: **every number in this round still excludes the input hop, and this
report says so wherever a photon figure appears** rather than arguing the hop away.

## 11. Startup

`bin/quill` from a shell, chromium process spawn included, 10,062-word document already in the
profile, on a real compositor. n = 12.

| | median | mean ± sd | range |
|---|---|---|---|
| **exec → the first frame a human can see** | **357 ms** | 369.6 ± 62.2 | 331.6–561.9 |
| exec → `window.__quillReady` (a JS marker, **before any frame exists**) | 273 ms | 274.6 ± 8.7 | 263.4–296.9 |
| of which: exec → navigation start (the chromium process) | 160 ms | | |
| brand-new profile: no code cache, no storage, a true first run (n = 8) | 369 ms | 393.1 ± 72.5 | 340–564 |

**`startup_ready` in `progress/latency.json` is now the 357.** Round 2 published 322, which was the
JS marker — the smaller of the two numbers, and the one that is true before anything is on screen.
The marker is still reported, in `detail.startup`, labelled as what it is.

Headless cold processes, for a cleaner distribution: **276.9 ± 10.9 ms** to first frame with the
document (n = 15), **232.9 ± 7.3** with a fresh profile (n = 12). Inside an already-running browser
a page load is **103.3 ± 6.7 ms** to first paint and 76.4 ± 3.2 to the marker (n = 12); an empty
document is 60 ms.

## 12. By kind of keystroke, and what a statistic needs

Round 2 printed a p99 for `undo` from **two** samples and for `capital` from four.
`tools/latency.mjs` now refuses: anything under 20 samples reports **n, mean and worst** and carries
`too_few_for_percentiles: true`. Application cost (keystroke → presented, no cadence), one session
of 300 keystrokes per regime:

| regime | key | n | mean ± sd | worst |
|---|---|---|---|---|
| paragraph_breaks | letter | 202 | 6.10 ± 3.57 | 17.35 |
| | space | 43 | 6.61 ± 3.94 | 13.16 |
| | **Enter** | 38 | **7.33 ± 2.66** | 15.33 |
| | punct | 13 | 5.21 — *too few for percentiles* | 10.71 |
| | capital | 4 | 3.64 — *too few for percentiles* | 5.24 |
| revision | letter | 188 | 6.49 ± 3.25 | 14.48 |
| | modifier (the Shift of a chord) | 96 | 2.15 ± 3.60 | 14.00 |
| | **nav** (Shift+←) | 94 | **9.63 ± 4.91** | 18.95 |
| | space | 16 | 6.92 — *too few* | 13.78 |
| | **undo** | 2 | 9.09 — *two samples; no percentile exists* | 10.33 |
| paste_blocks | letter | 223 | 6.33 ± 3.67 | 14.07 |
| | space | 47 | 5.98 ± 3.66 | 13.37 |
| | **paste** (Ctrl+V of a 137-char paragraph) | 11 | **12.33 — *too few*** | 18.74 |

Enter is the second-most expensive ordinary key and `Shift+←` the most, which is why the revision
regime is the one that presses 94 of them. Paste is the single most expensive thing in the bench
and the reason `paste_blocks` misses the ≤16 ms worst case in §5 — one keystroke that inserts a
paragraph, eleven times in 300.

## 13. Sustained writing

2,500 keystrokes with pauses, 4.4 minutes, headless on the 60 Hz clock. Nothing drifts:

| quartile | 1 | 2 | 3 | 4 |
|---|---|---|---|---|
| mean ms | 10.36 | 10.41 | 10.27 | 10.30 |
| worst ms | 18.88 | 19.98 | 19.59 | 18.76 |

JS heap after: **2,061 KB**. `files.js` flushed the whole document to `localStorage` **172 times**
(400 ms after each pause, never during a burst): **0.12 ms mean, 0.5 ms worst**.

## 14. Bigger documents, and slower machines

Application cost, keystroke → committed frame, `prose_end_of_draft`, n = 300 each:

| document | lines | mean ± sd | worst | main thread per keystroke |
|---|---|---|---|---|
| 2,112 words | 71 | 5.17 ± 3.74 | 14.51 | 2.95 ms |
| 10,062 words | 432 | 5.54 ± 3.45 | 12.95 | 4.82 ms |
| 26,841 words | 1,642 | 6.80 ± 2.97 | 16.74 | 9.14 ms |
| **53,684 words** | 3,285 | **8.55 ± 2.00** | **17.93** | **15.87 ms** |

Read the sd column with §7 in mind: it **falls** as the document grows, from 3.7 to 2.0. That is
the slack disappearing. At 2k–10k words the app finishes early and the variance is the scheduler;
at 55k words there is 15.87 ms of main-thread work per keystroke — a whole 60 Hz frame — the app
is the critical path, and the number is over the ≤16 ms worst-case bar. Where that goes
(`shots/latency/r3-attribution-52k.json`, µs per keystroke): Chrome's own textarea insertion 2,263,
the frame's style/layout/paint 4,833, the layout Quill forces by measuring the caret 1,676,
**Quill's own `input` handler 798**. Roughly half is the mirror and half is Chrome's `<textarea>`
holding 290 KB of text. `contain: layout style` on `#mirror .line` was measured twice and does what
it says to PrePaint and Paint (1,741 → 1,441 and 1,917 → 1,442 µs) without moving the end-to-end
number at all, and was left out.

CPU throttled (`Emulation.setCPUThrottlingRate`), keystroke → presented on the 60 Hz clock:

| | prose_end | prose_middle | paragraph_breaks |
|---|---|---|---|
| 2× slower | 11.28 ± 4.45 | 11.77 ± 4.32 | 11.54 ± 4.40 |
| 4× slower | 12.97 ± 4.10 | 15.17 ± 4.20 | 15.10 ± 4.61 |

## 15. Frames

An idle page with the bench's display clock running produces 62.5 frames/s and drops 0.75/s; with
the clock off, 59.4 and 0.25/s. While typing at 133 wpm the drop rate is **0.013 dropped frames per
keystroke** (23 in 1,728), against round 2's 0.07 — the caret glide, which is what was asking for
those frames, is unchanged, but there is less work landing on top of it. Under the application-cost
regime **100 % of keystrokes are presented within one 60 Hz frame**.

## 16. What would move these numbers next, and what would not

* **Removing more application work would not.** §7 is the measurement, not an opinion: two
  milliseconds of extra busy-wait per keystroke are absorbed by the slack before the next frame.
  The work already fits with room to spare on a 10k-word document; the number that is left is
  Chromium's frame scheduling, and a web application cannot start a frame, only ask for one.
* **The caret would, more than anything else in this table.** 58 ms is 4 whole frames, on the one
  thing a typist's eye is on. `GLIDE_X` in `app/js/caret.js`, one constant. This piece does not
  own that file; it has been reported for two rounds.
* **A 55k-word manuscript still costs a whole frame of main thread per keystroke**, and roughly
  half of that is the mirror rather than Chrome's textarea (§14). The fix is not containment —
  measured twice, no end-to-end effect — it is not rendering line elements the reader cannot see,
  which cannot be done without a way to keep `#mirror`'s height exactly equal to the textarea's,
  and glyph alignment is the one thing this app may never break.
* **`count()` in `app/js/chrome.js`** is a whole-document scan on every keystroke: ~0.9 ms of main
  thread at 10k words, ~4.9 ms at 55k. Idle-scheduled, so it is off the critical path today, and
  it is the second-largest main-thread item in a large document.

## 17. Raw

Every run in this report is a file under `shots/latency/`, and every headline table can be
recomputed from the `samples_to_*_ms` arrays inside them: `r3-appcost.json` (application cost, 12
regimes × 3 sessions), `r3-headless.json` (60 Hz clock, same), `r3-virtual-1.json` (compositor),
`r3-size-*.json`, `r3-throttle*.json`, `r3-long.json`, `r3-cold-*.json`, `r3-coldlaunch-*.json`,
`r3-attribution-*.json`, `r3-spare.json`, `r3-correctness.json`, `r3-caret-glide.txt`, and
`r3-panel-attempt.log` — the 45 minutes of refusals that §9 is about. Probes are in
`shots/latency/probes/`. Both themes and all three faces were re-shot after the core changes:
`r3-check-{light,dark}-{duo,quattro,mono}.png`.
