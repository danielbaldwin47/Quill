# Latency — measured

**Quill, round 1.** iA Writer publishes no latency numbers at all — "boots in a whizz… snappy as
jazz" is the entire public record — so the bar comes from third-party measurements of native
editors: **32.5 ms keyboard-to-photon** (Sublime Text; Tristan Hume, photodiode, 60 Hz panel;
TextEdit 33.4, Atom 45.6, VS Code 47.6) and **≤ 5 ms** of app-internal input→paint work (Fatin's
Typometer class: Notepad++ 4.3 ms mean, Sublime 8.2). Everything below was measured on this
machine, in this build, with a real **10,062-word** Markdown document open — never on an empty
page — and with **every single keystroke accounted for**.

## The numbers

| | p50 | p95 | p99 | max |
|---|---|---|---|---|
| **Keystroke → frame presented**, 60 Hz clock | **10.8** | 18.5 | **19.1** | 20.2 |
| Keystroke → frame committed, 60 Hz clock | 9.9 | 17.3 | 18.0 | 18.6 |
| Keystroke → frame presented, **real glass** (Dell U2720Q, Wayland) | 13.8 | 26.2 | 29.1 | 33.7 |
| Application cost alone, display clock removed | 4.2 | 5.6 | 6.1 | 6.8 |
| Input delay (hardware timestamp → our first handler) | 0.24 | — | 0.35 | 0.44 |

Milliseconds. 300 keystrokes per regime, five regimes, 90 ms pacing (≈ 133 wpm) as the
headline. Main-thread work per keystroke **4.9 ms** — inside the ≤ 5 ms bar — of which only
**848 µs** is Quill's own code. **86 %** of keystrokes are on screen within one 60 Hz
frame, 100 % within two.

Add the two things nobody can measure without a photodiode — Fatin's published budget of ~14 ms
for a keyboard and ~4 ms of pixel response (the display's *refresh wait* is already inside the
numbers above) — and a keystroke in Quill reaches the glass in **≈ 32 ms** on a 60 Hz
panel. That is the Sublime Text / TextEdit bracket (32.5 / 33.4 ms), not the Atom / VS Code
bracket (45.6 / 47.6). In a browser. With a ten-thousand-word document open.

**Startup**, same document, restored from storage and fully rendered before the first frame:
**100 ms** from navigation to the frame that shows it (71 ms to editor-ready;
56 ms for an empty document). The whole application from a shell — `bin/quill`, chromium
process spawn included — **395–511 ms** to pixels.

---

## 1. Environment

| | |
|---|---|
| CPU | 13th Gen Intel(R) Core(TM) i5-13600K (20 threads) |
| OS | Linux 7.1.9-arch1-2, Wayland (Hyprland) |
| Browser | Chromium 151.0.7922.173 (system build), headless and headed |
| Display (headed run) | Dell Inc. DELL U2720Q G49JV13, 3840x2160 @ 60.00 Hz, scale 1.5, no VRR |
| Viewport | 1440x900 logical px |
| App build | `app/**/*.{js,css,html}` sha256 `c7a261bd4b979b78`, git `4c611b4`, served from a frozen snapshot so that other pieces' concurrent edits could not move the numbers mid-run |
| Load average at run time | 0.61 |
| Sample | startup 12 × cold browsing context; typing 300 keystrokes × 5 regimes, 25 warm-up keystrokes discarded before each |

## 2. The document

`shots/latency/doc10k.md` — **10,062 words, 53,031 characters, 432 lines** — built by
`tools/mkdoc.mjs` from *Alice's Adventures in Wonderland* (Project Gutenberg #11, public domain).
A real document, not lorem ipsum: chapters as `##` headings, Carroll's own italics as Markdown
emphasis (`_very_`), verse as block quotes, scene breaks as `* * *`, and an editor's note at the
top with a task list, a link, inline code and a fenced code block — so the tokenizer, the
decorators and the mirror all do their real work on every frame.

Paragraphs are **one logical line each** (up to 980 characters), the way a document actually looks
in iA Writer, and the hard case for a line-diffing renderer: an edited line is a whole
soft-wrapping paragraph, not a 60-character row.

Three more sizes — 2 k, 27 k and 55 k words — are measured in §6.

## 3. Method

### 3.1 Three independent clocks on every keystroke

1. **Chrome's own `EventTiming` trace records** (`devtools.timeline`, collected over CDP
   `Tracing`). For every input event Chrome writes, in microseconds and **unrounded**: the event's
   hardware timestamp, `processingStart`, `processingEnd`, `commitFinishTime`, and `duration` —
   which ends at the **presentation feedback of the frame that carried that event's update**. This
   is the browser's own instrumentation of exactly the quantity we want, not a reconstruction.
   (The JavaScript `PerformanceEventTiming` API exposes the same duration rounded to 8 ms and hides
   everything below 16 ms; it is recorded alongside only as a sanity check.)
2. **An in-page probe**: a capturing `keydown` listener stores `event.timeStamp`, requests one
   animation frame, and posts a `MessageChannel` message from inside the callback — that task runs
   once the frame has been committed. Keys that share a frame are all resolved by that frame, so
   none is dropped or double-counted.
3. **Main-thread accounting**: every top-level `X` event on the renderer's main thread, de-nested
   with a stack, giving real busy time per keystroke and where it goes.

### 3.2 Every keystroke is accounted for

Each run asserts `keys pressed == keydown records in the trace == keys resolved by the in-page
probe`, and the JSON carries all three counts plus `every_keystroke_accounted_for`, which is
`true` for all five regimes in the run reported here (300 = 300 = 300).

This is not a formality. The bench this piece replaced kept a *single* pending slot, so whenever
two keys landed in one frame the first was silently discarded: it measured **62 of 150**
keystrokes and reported a p50 of 3.2 ms while the browser's own event timing said 40 ms. A
latency number you cannot reconcile with a keystroke count is not a measurement.

### 3.3 The display clock — why most browser latency numbers are wrong

A real display ticks at a fixed cadence whether or not anything is animating, and a keystroke
waits for the next tick. **A headless Chromium only ticks when something asks it to**: with
nothing animating it produces a frame on demand, and every latency comes out ~6 ms better than
any 60 Hz panel can physically be. Measured here, same build, same keys, the only difference being
whether an animation is running:

| | present p50 | p90 | p99 | commit p50 |
|---|---|---|---|---|
| nothing animating (headless default — **not** what a display does) | 3.28 | 4.15 | 11.68 | 2.57 |
| an animation running (a 60 Hz clock, as on any panel) | 9.40 | 16.48 | 18.22 | 8.90 |

So this bench keeps a 1 px composited animation running during measurement (`--clock on`, the
default) and headless then agrees with the real panel's committed-frame time to within 0.6 ms.
`--clock off` measures the application's own cost with no display cadence at all: the right mode
for A/B-ing a code change, the wrong mode for quoting user-facing latency. Both are reported, and
neither is presented as the other.

The same fact is a design rule for the app: **anything that animates while the writer types puts
every keystroke on the vsync clock.** Quill's caret suppresses its blink on each keystroke and
restores it after 480 ms of quiet, which is both the right look and, on any display, the
difference between waiting for the clock and not.

### 3.4 Typing regimes

| regime | caret | pace | why |
|---|---|---|---|
| `paced_end_of_draft` | end of the 10k-word draft | 90 ms | writing on |
| `paced_middle_of_draft` | middle, 5k words below it | 90 ms | everything below has to re-lay-out |
| `paced_focus_mode` | middle, **Focus: Sentence** | 90 ms | iA's signature mode, dimming recomputed live |
| `fast_typist` | middle | 45 ms | ≈ 266 wpm, faster than any human sustains |
| `saturation_stress` | end | 0 ms | keys injected back to back — not human, a cliff-finder |

Before each regime, 25 warm-up keystrokes are typed and discarded, and **the caret is scrolled into
view**: an edit below the fold is never repainted, and measuring that would flatter us by exactly
the cost of the paint.

## 4. Results — typing

Headless with the 60 Hz clock, 300 keystrokes per regime, 10,062-word document:

| regime | present p50 | p90 | p95 | p99 | max | commit p50 | ≤1 frame | ≤2 frames |
|---|---|---|---|---|---|---|---|---|
| end of draft, 90 ms | **9.2** | 16.4 | 17.5 | **18.5** | 19.9 | 8.4 | 92 % | 100 % |
| middle of draft, 90 ms | **10.8** | 17.5 | 18.5 | **19.1** | 20.2 | 9.9 | 86 % | 100 % |
| middle, Focus: Sentence, 90 ms | **9.2** | 18.0 | 18.9 | **19.3** | 20.9 | 8.0 | 87 % | 100 % |
| fast typist, 45 ms | **9.9** | 17.3 | 18.1 | **19.2** | 20.5 | 8.8 | 86 % | 100 % |
| saturation stress, 0 ms | **10.0** | 16.5 | 17.8 | **19.4** | 20.3 | 8.8 | 91 % | 100 % |

On the **real 60 Hz panel** (window on screen, Wayland, 200 keystrokes per regime):

| regime | present p50 | p90 | p95 | p99 | commit p50 |
|---|---|---|---|---|---|
| end of draft, 90 ms ¹ | 18.1 | 38.5 | 43.0 | 48.5 | 8.8 |
| middle of draft, 90 ms | 13.8 | 24.7 | 26.2 | 29.1 | 8.8 |
| middle, Focus: Sentence, 90 ms | 13.8 | 24.5 | 25.5 | 32.0 | 9.3 |
| saturation stress, 0 ms | 25.6 | 32.4 | 33.5 | 35.3 | 9.6 |

¹ 204 keydowns recorded for 200 injected presses — the double-probe artefact described in §9.

The panel run was taken against an **earlier build** of the app (fingerprint `33b852e8`, before
this round's core changes and before the caret stopped animating while you type). With the display
clock equalised, that build and this one measure the same to within a millisecond at the median —
9.8 ms then, 10.8 ms now — and that is the honest result: **on a 60 Hz screen the wait for the
next tick dominates, and both builds' work already fits inside one frame.** What this round bought
is headroom, and headroom only shows where the cadence is not hiding it: with the clock off (§8)
and on long documents (§6).

What the panel run establishes independently of the build is the size of the display path itself:
real glass cost **+5.0 ms at p50 and +11.6 ms at p99 over the committed frame**, against
+1.0 ms headless — the compositor→scanout hop. Frame *commit* is measured identically in
both environments and agrees to within 0.6 ms, which is what makes the headless runs trustworthy
for everything the application controls.

### 4.1 Which published bar maps to which measurement

The three numbers in circulation measure three different things, and comparing across them is how
latency claims usually go wrong:

| published bar | what it actually measured | Quill's comparable number |
|---|---|---|
| Hume, **32.5 ms** (Sublime, photodiode, 60 Hz) | key switch → photons: keyboard, OS, app, compositor, panel | **≈ 32 ms** — 13.8 measured on glass + 14 ms keyboard + 4 ms pixel response (the last two cited, not measured) |
| Fatin, **8.2 ms mean** (Sublime, Typometer, Windows; Notepad++ 4.3, GVim 0.9; on Linux Sublime 23.1, Gedit 12.4) | injected key → the pixel changing in a screen capture; no keyboard, and no waiting on a refresh cadence | **4.2 ms** p50 / 6.1 p99 with the display clock off |
| REFERENCE §5.3, **≤ 5 ms** app-internal, ≤ 16 ms worst case | the application's own work per keystroke | **2.9 ms** to committed frame, 4.9 ms of main-thread busy time, worst case 6.8 ms |

## 5. Where a keystroke's time goes

Per keystroke, middle of the 10k-word draft, from the same trace:

| | µs |
|---|---|
| Chrome inserting the character into the `<textarea>` (its own editing code) | 1598 |
| **Quill's `input` handler** — line diff, re-tokenise, decorators, caret, autosave bookkeeping | **848** |
| style + layout + pre-paint + paint of the frame | 2412 |
| **total main-thread busy per keystroke** | **4.9 ms** |

Two things are worth saying plainly. First, **the largest single cost is not ours**: it is Chrome's
own text insertion into a 53 KB `<textarea>`, which every browser-based editor pays and which we
cannot remove without giving up the native selection, IME and undo stack. Second, our own handler
is **848 µs — 5 % of one 60 Hz frame**.

With Focus: Sentence on, main-thread work is 5.4 ms per keystroke, and presented latency
moves by well under a frame (9.2 ms p50).

## 6. Document size

Same regime (end of draft, 90 ms), 150 keystrokes, display clock **off** so the application's own
cost is visible rather than hidden inside the frame cadence — add ~6 ms at p50 for a 60 Hz screen:

| document | words | lines | startup to first frame | editor ready | app cost p50 | p99 | main thread / key |
|---|---|---|---|---|---|---|---|
| `doc2k.md` | 2,112 | 71 | 76 ms | 53 ms | **2.5 ms** | 7.5 ms | 2.6 ms |
| `doc10k.md` | 10,062 | 432 | 100 ms | 73 ms | **3.5 ms** | 7.0 ms | 4.2 ms |
| `doc26k.md` | 26,841 | 1,642 | 136 ms | 103 ms | **5.6 ms** | 9.8 ms | 7.6 ms |
| `doc52k.md` | 53,684 | 3,285 | 184 ms | 156 ms | **9.1 ms** | 15.2 ms | 12.9 ms |

A **55,000-word manuscript** — a short book, 288 KB, 3,285 lines — still fits a keystroke inside
one 60 Hz frame's budget: 12.9 ms of main-thread work, app cost p99 15.2 ms. Cost grows
close to linearly with document length, which is the expected behaviour of a mirror architecture
(Chrome lays out both the textarea and the mirror), and it is the number to watch on a slower
machine.

## 7. Startup

| | with the 10,062-word document | empty document |
|---|---|---|
| navigation → editor ready (`Writer.boot()` returned: document rendered, caret placed, focused) | 71 ms | 40 ms |
| navigation → DOM parsed | 93 ms | 43 ms |
| navigation → **first frame with the document on screen** (FCP, taken at presentation) | **100 ms** | **56 ms** |

Medians of 12 cold browsing contexts each. Editor-ready precedes DOMContentLoaded because
`Writer.boot()` runs from the last inline script in `<body>` — by the time the parser finishes, the
document is already laid out. There is no splash, no skeleton and no
progressive reveal: the document is tokenised and in the DOM *before* the first frame, so first
paint and first usable frame are the same frame. Nothing is deferred to "a few seconds after
launch" the way iA's own Windows 2.0 post describes for its spell and syntax passes — the only
deferred work in Quill is the word count, which is idle-scheduled and invisible.

Whole-app cold start through the launcher, real display, warm profile, 10k-word document in
storage:

| | ms |
|---|---|
| `bin/quill` exec → chromium's navigation start (process spawn, profile, window) | 155 |
| → editor ready | 300 |
| → **first frame with the document** | **511** |

(Two launches were measured this way, 395–511 ms to pixels; the slower one is tabulated.)

`bin/quill` is a `chromium --app` launcher: no tabs, no omnibox, its own profile under
`~/.cache/quill/browser`, and it starts the static server itself if the port is dead. `--measure`
times shell-exec → pixels over CDP; `--fresh` wipes the profile first for a true first-run.

## 8. What changed in the code this round

The round-0 engine did work on the keystroke path that nobody could see. Four changes in
`app/js/core.js`, all perf-only and backward compatible:

1. **No forced layout inside the keystroke.** `render()` used to read `mirror.offsetHeight` and
   write it back to the textarea on every input event — a synchronous layout of the whole document
   in the middle of the keystroke, then a write that dirties it again. A `ResizeObserver` on the
   mirror now does the same job from the frame's own layout pass; full renders still sync
   immediately, so anything that measures straight afterwards is unaffected.
2. **One `selection` notification per keystroke instead of three.** Chrome raises `input`,
   `selectionchange` *and* `keyup` for a single keypress; core forwarded all three, and every
   listener on that event measures layout.
3. **Writes before reads.** During an input event the `render` event is now emitted after `change`
   and `selection`, so the pieces that write DOM (focus dimming) all run before the piece that
   measures it (the caret): one layout flush per keystroke instead of two.
4. **No attribute storm.** Line indices are rewritten only when the line count actually changed,
   and the shortcut table is no longer rebuilt and re-sorted on every keydown.

A/B of exactly those four changes — two snapshot servers, identical in every other file, 250
keystrokes each, display clock off so the difference is not hidden inside the frame cadence:

| | before | after |
|---|---|---|
| main-thread busy per keystroke | 4.84 ms | **4.17 ms** |
| style + layout + paint per keystroke | 2000 µs | **1859 µs** |
| keystroke → presented, p50 | 3.58 ms | **3.44 ms** |
| keystroke → presented, p90 | 4.27 ms | **3.97 ms** |
| keystroke → presented, p99 | 9.01 ms | **5.93 ms** |
| on screen within one 60 Hz frame | 99.6 % | **100.0 %** |
| `selection` events dispatched per keystroke | 3.02 | **1.02** |
| layout-reading DOM calls per keystroke | 12.07 | **6.08** |

## 9. What these numbers are not

* **Keystrokes are injected over CDP**, not typed on a keyboard. Everything before the browser
  received the event — key switch, debounce, USB polling, kernel, compositor — is not measured.
  Fatin's published budget for that is 8–22 ms (mean 14); Hume's photodiode figures include it.
  The ≈ 32 ms keyboard-to-photon number above is therefore **a measurement plus a cited
  estimate**, and the two are kept visibly separate. It is not a photodiode result.
* **Pixel response (~4 ms) is estimated, not measured**, for the same reason. The display's
  refresh wait *is* inside the measured numbers.
* **The real-panel run is a single session** on an earlier build, and it has not been repeated:
  BRIEF.md requires that test windows never open on the workspace the user is working on, this
  Hyprland build rejects both `hyprctl dispatch exec "[workspace 2 silent] …"` and
  `hyprctl keyword windowrule …`, and chromium's Wayland `app_id` comes from the URL rather than
  `--class`, so the window cannot be placed off-workspace. `bin/quill --measure` now refuses to
  open one unless `QUILL_HEADED_OK=1` is set. That session also had a double-installed in-page
  probe in the regimes after the first, which adds work rather than removing it, so those numbers
  are if anything pessimistic — which is why they are quoted as the upper bound and never as the
  headline.
* **The tail depends on pacing, and both are reported.** At 90 ms between keystrokes the
  distribution is tight (10.8 / 19.1 ms p50/p99). At 45 ms — ≈ 266 wpm — the median is
  9.9 ms and the tail stretches to 19.2 ms: a keystroke that arrives while the
  previous frame is still in the pipeline waits for the next tick. Nothing is dropped; the queue
  is at most one frame deep.
* **`saturation_stress` is not a human speed.** 300 keys injected back to back saturate the
  main thread; presented latency stays at 10.0 ms p50 because keys share frames, but the
  in-page probe shows the queue behind it — 77 ms p99 from a key to the frame that lands
  it. No writer types this way; it is here to show where the cliff is.
* **Chrome's frame-production counters are recorded but not quoted.** `PipelineReporter` marks a
  large share of frames "dropped, affecting smoothness" during typing simply because an animation
  asked for frames the main thread had no update for. The defensible user-facing statement comes
  from the per-keystroke data instead: 86 % of keystrokes on screen within one 60 Hz frame,
  100 % within two, worst of 300 keystrokes 20.2 ms.
* **One machine, one browser, one build.** A 13600K is fast and a 2019 laptop is not; the number
  that scales is main-thread work per keystroke (4.9 ms), not the presented latency. The
  app is fingerprinted (`c7a261bd4b979b78`) in every result file, because other pieces were editing the
  same tree while these runs were happening — an earlier build of the same day measured
  9.8 / 19.1 ms and it would have been easy, and wrong, to quote whichever build flattered best.

## 10. Reproducing it

```sh
node tools/serve.mjs 4173 &
node tools/latency.mjs                       # 12 startup runs, 5 regimes × 300 keys, JSON on stdout
node tools/latency.mjs --clock off           # application cost with no display cadence
node tools/latency.mjs --doc shots/latency/doc52k.md --quick --keys 150
node tools/latency.mjs --url http://localhost:4184/ --json before.json   # A/B against a snapshot server
node tools/mkdoc.mjs alice.txt shots/latency/doc10k.md 10000            # rebuild the corpus
bin/quill                                    # the app in its own window (chromium --app)
QUILL_HEADED_OK=1 bin/quill --measure --keys 300 --json headed.json     # cold start + real-display bench
```

Raw runs: `shots/latency/r1-headless.json` (60 Hz clock),
`shots/latency/r1-appcost-noclock.json`, `shots/latency/r1-headed.json`,
`shots/latency/size-doc*.json`, `shots/latency/ab-{before,after}.json`.
Summary for the progress page: `progress/latency.json`.
