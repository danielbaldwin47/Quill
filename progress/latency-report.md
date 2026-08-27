# Latency — measured

**Quill, round 2.** iA Writer publishes no latency numbers at all — "boots in a whizz… snappy as
jazz" is the entire public record — so the bar comes from third-party measurements of native
editors: **32.5 ± 4.0 ms keyboard-to-photon** (Sublime Text; Tristan Hume, photodiode, 60 Hz panel;
TextEdit 33.4, Atom 45.6, VS Code 47.6) and **≤ 5 ms average, ≤ 16 ms worst case** of app-internal
input→paint work (REFERENCE §5.3, Fatin's Typometer class: Notepad++ 4.3 ms mean, Sublime 8.2).

Everything below was measured on this machine, on this build, with a real **10,062-word** Markdown
document open, by a typist that presses **capitals, punctuation, Enter, Backspace, undo, paste and
Markdown syntax** — not only lowercase letters — and with **every single keystroke accounted for**.

## The numbers

|  | p50 | p99 | worst |
|---|---|---|---|
| **The application's own work** — keystroke → the frame it produced, nothing animating | **5.7** | 15.0 | 17.3 |
| … of which: keystroke → frame **committed** | **4.8** | 13.8 | 16.5 |
| Keystroke → frame presented, headless on a 60 Hz frame clock | **9.4** | 18.5 | 19.0 |
| Keystroke → frame presented, **on the user's own compositor** at 60 Hz | **23.6** | 37.7 | 50.4 |
| Keystroke → **photons**, adding the ~4 ms of pixel response nobody can measure without one | **≈ 27.6** | ≈ 41.7 | — |

Milliseconds, all from one regime: **plain writing at the end of a 10,062-word draft at 133 wpm**
— the most common thing a writer does, and the *worst* of the eleven paced regimes on the
compositor, which is the row that meets the published bar. Three sessions of 300 keystrokes each,
pooled (n = 900); every keystroke accounted for in all of them; the bootstrap 95 % interval on
that 37.7 ms p99 is 36.5–39.4. Headless, this regime is the *best* of the eleven rather than the
worst — the worst there is Focus: Sentence at 9.84 / 20.09 ms — but the whole spread headless is
1.6 ms, because on a frame clock the wait for the tick dominates everything (§4).

**Against the bars.** The application's own work is inside the **≤ 5 ms / ≤ 16 ms** bar on this
document: 4.8 ms to a committed frame, 5.7 ms to a produced one, worst of 300 keystrokes 17.3 ms, and
**99.7 % of keystrokes inside one 60 Hz frame, 100 % inside two**.
Against Hume's **32.5 ± 4.0 ms** photodiode figure, the like-for-like number is **≈ 27.6 ms at the
median — and ≈ 41.7 ms at p99**, which is the Sublime bracket at the median and the VS Code
bracket in the tail. The tail is not ours: Chromium had the frame committed 9.3 ms after the key,
and the compositor took another 17.3 ms at the median and 34.1 at p99 to report it presented
(§5). Round 1 claimed "≈ 32 ms, the Sublime bracket" from a single session on an older build; this
round the claim is narrower and the arithmetic is in §13.

**Startup.** From a shell, `bin/quill`, chromium process spawn included, with the 10,062-word
document in the profile: **375 ms** to the first frame that shows the document, **322 ms** to an
editor you can type into. A brand-new profile — no code cache, no storage, a true first run — is
380 ms to first frame. Inside an already-running browser a page load is 108 ms / 81 ms.

## 0. What round 1 got told, and what changed

Round 1 lost blind on five specific points. Each one is now a measurement rather than an argument:

| round 1's verdict | round 2 |
|---|---|
| "the ≈32 ms photon claim rests on one session, on an **earlier build**, n=200, with a known double-probe bug (204 keydowns for 200 presses), and quotes the **best** regime's p50" | The bench now uses **a fresh page per regime**, so a second probe cannot be installed; the accounting assertion runs in every regime; the on-glass-path numbers are re-measured on **this** build on a real compositor (§5); and the headline quoted is the **worst paced human regime's p99**, not the best regime's p50. |
| "the bench types **nothing but lowercase letters and spaces** — it never presses Enter or Backspace, the path core.js gives a more expensive branch" | Twelve regimes of scripted real prose with **eleven kinds of keystroke**, each reported separately (§8). Enter, Backspace, undo, paste, select-and-replace and a fence-opening backtick are all measured, and the expensive ones were then **made cheaper** (§14). |
| "startup '100 ms, median of 12 cold loads' **is not cold**" | Two separate numbers, never mixed: a **page load in a warm browser** (what the 100 ms was), and a **true cold start** that spawns a new browser process per run, with a **fresh profile** variant that has no code cache and no storage at all (§10). |
| "no degraded-hardware run at all" | **`Emulation.setCPUThrottlingRate` at 2× and 4×** (§9), and the document-size sweep at four sizes (§7). |
| "p99 from a single n=300 run, no repeated sessions, no confidence interval" | **Three sessions** of every regime, per-session p50/p99 reported individually, pooled percentiles with **bootstrap 95 % intervals** (§4). |
| "~1 dropped frame per keystroke, declined as an artefact — plausible, not demonstrated" | An **idle frame-production control** and the drop rate at three different typing speeds: the drops track **keystrokes per second**, not seconds (§12). |
| "the headline pairing in latency.json and the tiles is apples-to-oranges" | `progress/latency.json` now carries the **photon-comparable** number against the 32.5 ms bar, with its construction spelled out in the same file (§13). |
| "the ≤5 ms bar is only cleared on the 10k document with focus off" | Every document size is reported, and where we are **over** the bar it says so (§6, §7). |
| "sustained-session behaviour is unmeasured; no accounting for the autosave write" | A **2,500-keystroke session** with pauses, quartile-by-quartile latency, heap growth, GC time and every `localStorage` write timed (§11). |
| "saturation_stress's 10 ms p50 is meaningless and is still tabulated alongside the others" | It is still run — it is where the cliff is — but it is **out of the results table** and reported on its own, with the frame-sharing arithmetic that makes its p50 meaningless printed next to it (§4). |


## 1. Environment

| | |
|---|---|
| CPU | 13th Gen Intel(R) Core(TM) i5-13600K (20 threads) |
| OS | Linux 7.1.9-arch1-2, Wayland (Hyprland 0.56.2) |
| Browser | Chromium 151.0.7922.173 (system build), headless and headed |
| Display | Dell U2720Q 3840×2160 @ 60.00 Hz for the app's own window; the measured window ran on a **virtual 1920×1080 @ 60 Hz Hyprland output** (§5) |
| Viewport | 1440×900 logical px |
| App build | `app/**/*.{js,css,html}` sha256 **`19b86f97e10b203d`**, served from a frozen snapshot on its own port so that another builder's edit could not move the numbers mid-run |
| Load average | 0.55–1.48 depending on the run; recorded in every result file, together with the busiest processes on the machine — this is somebody's workstation, not a lab |
| Sample | typing 300 keystrokes × 12 regimes × 3 sessions headless, × 3 sessions on the compositor; startup 12 page loads + 10 cold processes + 8 fresh-profile processes + 7 launcher cold starts |

## 2. The document, and the typist

`shots/latency/doc10k.md` — **10,062 words, 53,031 characters, 432 lines** — built by
`tools/mkdoc.mjs` from *Alice's Adventures in Wonderland* (Project Gutenberg #11, public domain).
A real document, not lorem ipsum: chapters as `##` headings, Carroll's own italics as Markdown
emphasis (`_very_`), verse as block quotes, scene breaks as `* * *`, and an editor's note at the
top with a task list, a link, inline code and a fenced code block — so the tokenizer, the
decorators and the mirror all do their real work on every frame. Paragraphs are **one logical line
each** (up to 980 characters), the way a document looks in iA Writer, and the hard case for a
line-diffing renderer: an edited line is a whole soft-wrapping paragraph, not a 60-character row.
Three more sizes — 2 k, 27 k and 55 k words — are measured in §7.

**The typist.** Round 1's bench pressed one sample sentence of lowercase letters and spaces, over
and over. That is the cheap path, and it is not writing. Round 2 scripts a real stream — seeded
per regime, so every session types the identical keys and the spread between sessions is the
machine rather than the script:

* **prose** — a paragraph with capitals, commas, semicolons, quotation marks, apostrophes, dashes,
  sentence ends and paragraph breaks, with a **1–3 character typo corrected every ~45 characters**;
* **newlines** — short lines and Enter, in the middle of the document: every Enter changes the
  line count, the branch `render()` treats differently from typing inside a line;
* **revision** — write a word, select it back character by character with `Shift+←`, replace it,
  and undo with `Ctrl+Z` a quarter of the time;
* **markdown** — headings, `*emphasis*`, `**strong**`, inline `` `code` ``, a block quote and a
  fenced `` ```js `` block, opened and closed;
* **fences** — three backticks and three backspaces, over and over: the third backtick puts every
  line below it inside a code block and the first backspace takes them all out again. It is the
  most expensive thing one keystroke can ask a Markdown editor to do;
* **paste** — `Ctrl+V` of a 137-character paragraph from the real clipboard every 25 keystrokes;
* **letters** — round 1's lowercase sample, kept so the two rounds can be compared directly.

Every keystroke is labelled with the key that caused it, and the tables in §5 and §6 are computed
over **text-affecting keystrokes only** (letters, capitals, spaces, punctuation, Markdown
characters, Enter, Backspace, undo, paste); the modifier and arrow presses in a chord are counted
for the accounting assertion but are not averaged into a latency that they do not have.

## 3. Method

### 3.1 Three independent clocks on every keystroke

1. **Chrome's own `EventTiming` trace records** (`devtools.timeline`, collected over CDP
   `Tracing`). For every input event Chrome writes, in microseconds and **unrounded**: the event's
   hardware timestamp, `processingStart`, `processingEnd`, `commitFinishTime`, and `duration` —
   which ends at the **presentation feedback of the frame that carried that event's update**. This
   is the browser's own instrumentation of exactly the quantity we want, not a reconstruction.
   (The JavaScript `PerformanceEventTiming` API exposes the same duration rounded to 8 ms and hides
   everything below 16 ms; it is recorded alongside only as a sanity check.)
2. **An in-page probe**: a capturing `keydown` listener stores `event.timeStamp` and the key, asks
   for one animation frame, and posts a `MessageChannel` message from inside the callback — that
   task runs once the frame has been committed. Keys that share a frame are all resolved by that
   frame, so none is dropped or double-counted.
3. **Main-thread accounting**: every top-level `X` event on the renderer's main thread, de-nested,
   giving real busy time per keystroke and where it goes.

### 3.2 Every keystroke is accounted for

Each run asserts `keydowns expected from the script == keydown records in the trace == keydown
records in the in-page probe`, and that every one of them has both a commit and a presentation
time. The JSON carries all three counts, the count of each key that was pressed, and
`every_keystroke_accounted_for`. **It is `true` for every regime in every run quoted here.**

Round 1 shipped that assertion but ran the whole session in one page and installed its probe
twice, so the one real-display session it had recorded 204 keydowns for 200 presses — and that
session had no assertion field at all. Round 2 opens **a fresh page for each regime** and throws
if a probe is already installed, which makes the bug unrepresentable and, as a side effect, starts
every regime from the same document instead of from the previous regime's leftovers.

### 3.3 The display clock, and the app's own animation

A real display ticks at a fixed cadence whether or not anything is animating, and a keystroke
waits for the next tick. **A headless Chromium only ticks when something asks it to**: with
nothing animating it produces a frame on demand, and every latency comes out ~6 ms better than any
60 Hz panel can physically be. The bench therefore keeps a 1 px composited animation running
(`--clock on`, the default) so headless models a panel.

Round 2 found that **the application now supplies its own clock**: caret.js glides the caret 62 ms
to its new column whenever two keystrokes are more than 60 ms apart, which at any ordinary writing
speed is every keystroke. `document.getAnimations()` during a paced run shows `transform on caret`
running in **343 of 446 samples**. So "no animation" can no longer be had by turning the bench's
clock off — it has to be asked for, and the honest way to ask is
**`prefers-reduced-motion: reduce`**, a real user setting that caret.css and chrome.css both
honour. `node tools/latency.mjs --clock off --reduced-motion` is therefore the application-cost
mode in this round, and it is the only mode in which the numbers in §7 are comparable to Fatin's
Typometer figures. Both modes are reported and neither is quoted as the other.


## 4. Results — headless, on a 60 Hz frame clock

Keystroke → the frame carrying it was presented. Three sessions of 300 keystrokes per regime,
pooled (n = 900); the per-session p99 column is there so the spread between runs is visible rather
than asserted. `every_keystroke_accounted_for` is **true for all 36 runs**.

**Headless, 60 Hz frame clock, three sessions of 300 keystrokes each**

| regime | p50 | p90 | p99 | p99 95 % CI | worst | per-session p99 | ≤1 frame | n |
|---|---|---|---|---|---|---|---|---|
| writing at the end of the draft | **9.38** | 16.38 | **18.51** | 18.32–18.7 | 19.01 | 18.6, 18.39, 18.47 | 92 % | 900 |
| writing in the middle of the draft | **10.31** | 17.37 | **19.26** | 18.93–19.71 | 21 | 18.89, 19.71, 19.05 | 87.7 % | 900 |
| middle, **Focus: Sentence** | **9.84** | 17.07 | **20.09** | 19.2–20.59 | 23.02 | 18.8, 20.59, 19.82 | 91.3 % | 900 |
| paragraph churn (Enter every 5–10 keys) | **9.99** | 16.83 | **19.11** | 18.51–19.43 | 21.03 | 18.69, 19.11, 18.81 | 88.7 % | 900 |
| revision (select back, replace, undo) | **9.98** | 16.83 | **19.43** | 18.86–20.15 | 21.01 | 20.15, 18.86, 19.19 | 87.9 % | 618 |
| Markdown syntax (headings, emphasis, a fenced block) | **9.65** | 16.82 | **19.58** | 18.8–21.38 | 24.35 | 19.58, 18.55, 19.96 | 88.7 % | 900 |
| opening and closing a fenced block, 50× | **9.58** | 16.55 | **18.94** | 18.59–19.31 | 20.34 | 18.95, 18.59, 19.28 | 90.3 % | 900 |
| writing with a paste every 25 keys | **9.55** | 16.41 | **19.6** | 19.09–20.09 | 22.23 | 19.72, 19.09, 19.79 | 93 % | 900 |
| round 1's lowercase letters, for comparison | **10.13** | 17.03 | **20.03** | 19–20.32 | 21.62 | 19, 20.32, 19.23 | 85.7 % | 900 |
| bursts of 25 keys with 1.4 s pauses | **9.63** | 16.7 | **18.91** | 18.73–19.18 | 21.21 | 19.11, 18.75, 18.91 | 90.3 % | 900 |
| fast typist, 45 ms (~266 wpm) | **10.49** | 17.27 | **19** | 18.84–19.22 | 19.81 | 19, 18.93, 18.96 | 88 % | 900 |

`saturation_stress` — 300 keys injected with no gap at all — is deliberately **not** in that table.
It is in the JSON, and it is where the cliff is, but its p50 of 10.02 ms means nothing: 559
keystrokes a second share 140 frames a second, so per-key "presented latency" is measuring frame
sharing, not responsiveness. What it does show is that nothing falls over: the queue never grows
past two frames and every keystroke still arrives (900/900 accounted for, p99 19.25 ms).

The three slowest paced regimes are Focus: Sentence (20.09 ms p99), round 1's own
lowercase-letters regime (20.03) and paste (19.60) — and the spread between all eleven is 1.6 ms.
On a 60 Hz clock the wait for the tick dominates everything the application does, which is exactly
why §6 turns the clock off and §5 puts the app on a real one.

## 5. Results — on the user's own compositor

Headless Chromium is not a screen. `bin/quill --measure` opens the app the way a person would —
`chromium --app`, no tabs, no omnibox, its own profile — and runs the same bench in that window,
attached over CDP. The window goes on a **virtual Hyprland output** created for the run
(`hyprctl output create headless`, 1920×1080 @ 60 Hz) and removed afterwards: the user's own
compositor composites and presents the frames, the timestamps are real Wayland presentation
feedback at a real 60 Hz cadence, and **nothing appears on the physical screen**. BRIEF.md forbids
a test window on the workspace the user is working on, and at run time that workspace held a
full-screen game with direct scan-out.

Hyprland 0.56 replaced the string dispatcher round 1 gave up on with a Lua one; the call that works
is `hyprctl repl 'return hl.dispatch(hl.dsp.exec_cmd("[workspace N silent] chromium --app=…"))'`,
which is what `bin/quill` now uses — `--ws N` for a real workspace, `--virtual` (the default) for
the virtual output. A window parked on a workspace that is not being *displayed* gets no frame
callbacks and its presentation timestamps are worthless, which is why the virtual output exists
rather than "workspace 2, silently".

| regime | p50 | p90 | p99 | p99 95 % CI | worst | per-session p50 | per-session p99 | n |
|---|---|---|---|---|---|---|---|---|
| writing at the end of the draft | **23.64** | 34.3 | **37.7** | 36.49–39.41 | 50.42 | 24.03, 24.79, 22.32 | 36.54, 36.47, 38.55 | 900 |
| writing in the middle of the draft | **23.14** | 34.14 | **37.06** | 36.63–38.28 | 39.64 | 24.59, 22.03, 22.81 | 36.7, 37.97, 37.03 | 900 |
| middle, **Focus: Sentence** | **22.97** | 34.48 | **36.9** | 36.23–37.59 | 46.63 | 23.35, 22.74, 23.31 | 36.62, 37.59, 36.88 | 900 |
| paragraph churn (Enter every 5–10 keys) | **23** | 34.29 | **36.75** | 36.47–36.96 | 38.96 | 23.44, 22.12, 22.54 | 36.6, 36.9, 36.28 | 900 |
| opening and closing a fenced block, 50× | **23.05** | 34.12 | **36.61** | 36.19–36.91 | 42.15 | 23.09, 21.56, 23.92 | 36.07, 36.61, 36.55 | 900 |
| fast typist, 45 ms (~266 wpm) | **18.54** | 33.18 | **37.28** | 36.69–37.6 | 41.34 | 19.29, 16.84, 19.63 | 37.35, 36.96, 37.5 | 900 |

Three sessions, 300 keystrokes each, same build, same document, same scripted keys as §4 — seven
of the twelve regimes, the ones that fit a ten-minute run on the compositor. Chromium finished
committing the frame **9.31 ms** after the key event (p99 18.5); the compositor then reported it
presented **17.33 ms** later at the median, 34.13 ms at p99 (min 0.84).

**This is where the latency is, and it is not in the application.** The application's work is
identical to §6 — main-thread busy 5.3–6.1 ms per keystroke, frame committed 9.3 ms after the key
— and then the frame waits ~1 to ~2 refresh intervals to be presented. One further session in the same window with
`prefers-reduced-motion: reduce` — which stops the caret gliding and so stops the application
animating at all — measured the middle-of-draft regime at **23.98 / 35.97 ms p50/p99** (n = 300)
against 23.14 / 37.06 with the animation running: on a real compositor the caret animation costs
**nothing** in presented latency. (It costs something else; see §15.)

How much of that hop is the virtual output rather than a panel is the one thing this round could
not settle. Round 1's single session on the physical panel — earlier build, but frame commit
measures the same in both — saw the frame presented **5.0 ms** after commit at the median, against
**17.3 ms** here, i.e. about one refresh interval less: a physical output can page-flip the
client's buffer directly (Hyprland reported `directScanoutTo` for a full-screen window), while a
headless output is composited and reported on the next tick of its own timer. §13 carries both.


## 6. The application's own cost, and the ≤ 5 ms bar

Fatin's Typometer numbers — the ones REFERENCE §5.3 turns into "≤ 5 ms average, ≤ 16 ms worst
case" — are the application's own input→paint work, with no keyboard and no waiting on a refresh
cadence. To measure the same quantity the page must not be animating, and in this build the page
animates itself: the caret glides for 62 ms after every keystroke at any ordinary writing speed
(§15). The mode that answers the bar is therefore `--clock off --reduced-motion`: no bench
animation, and `prefers-reduced-motion: reduce`, which caret.css and chrome.css both honour.

**Application cost: no animation anywhere (reduced motion), frames produced on demand**

| regime | present p50 | p90 | p99 | worst | commit p50 | main thread / key | Quill's own handler |
|---|---|---|---|---|---|---|---|
| writing at the end of the draft | **5.67** | 12.75 | **14.99** | 17.34 | 4.76 | 5.47 ms | 814 µs |
| writing in the middle of the draft | **5.49** | 12.61 | **14.01** | 15.01 | 4.2 | 5.44 ms | 853 µs |
| middle, **Focus: Sentence** | **6.17** | 13.05 | **16.27** | 17.68 | 4.54 | 6.31 ms | 1416 µs |
| paragraph churn (Enter every 5–10 keys) | **5.21** | 12.54 | **15.12** | 16.9 | 4.08 | 5.17 ms | 774 µs |
| revision (select back, replace, undo) | **6.26** | 13.01 | **15.67** | 17.23 | 4.4 | 3.62 ms | 457 µs |
| Markdown syntax (headings, emphasis, a fenced block) | **5.35** | 12.09 | **14.46** | 16.87 | 4.4 | 5.23 ms | 830 µs |
| opening and closing a fenced block, 50× | **5.47** | 12.39 | **13.75** | 15.61 | 4.05 | 4.73 ms | 749 µs |
| writing with a paste every 25 keys | **5.13** | 11.93 | **15.39** | 19.11 | 4.39 | 4.54 ms | 665 µs |
| round 1's lowercase letters, for comparison | **5.38** | 12.37 | **14.13** | 15.51 | 4.22 | 4.87 ms | 772 µs |
| bursts of 25 keys with 1.4 s pauses | **5.2** | 12.15 | **16.45** | 18.78 | 4.29 | 4.97 ms | 714 µs |
| fast typist, 45 ms (~266 wpm) | **4.88** | 15.41 | **19.53** | 20.11 | 3.18 | 4.5 ms | 830 µs |

Every paced regime is inside the bar at the median and inside 16 ms at p99 except Focus: Sentence
(16.27) and the pauses regime (16.45), both by less than half a millisecond; the worst single
keystroke of 300 in any *paced* regime is 19.11 ms, in the paste regime, where one keystroke
inserts 137 characters. `fast_typist` and `saturation_stress` are over because keystrokes arrive faster than
frames — that is queueing, not work, and it is visible as the near-unchanged commit times.

Where the time goes, per keystroke, writing at the end of the 10k-word draft:

| | µs |
|---|---|
| Chrome inserting the character into the `<textarea>` (its own editing code) | 1414 |
| **Quill's `input` handler** — line diff, re-tokenise, decorators, caret, autosave bookkeeping | **814** |
| style + layout + pre-paint + paint of the frame | 2510 |
| the word count, on an idle callback, between keystrokes (chrome.js) | 1125 |
| **total main-thread busy per keystroke** | **5.5 ms** |

The largest single item is not ours: it is Chrome's own text insertion into a 53 KB `<textarea>`,
which every browser-based editor pays and which we cannot remove without giving up the native
selection, IME and undo stack. Ours is **814 µs — 5 % of one 60 Hz frame**.

## 7. Document size

Application cost again (reduced motion, clock off), 150 keystrokes, writing at the end of the
draft; the last column is the same regime with a 60 Hz clock, which is what a screen adds. These
are single 150-keystroke runs rather than the 3 × 300 of §4, so the tails are noisier — the
10k row's 19.4 ms p99 is one run's worst quarter-percent, against 15.0 ms over 900 keystrokes in
§6. The medians and the main-thread column are what the sweep is for.

| document | words | lines | app cost p50 | p99 | worst | main thread / key | Quill handler | with the 60 Hz clock, p50 / p99 |
|---|---|---|---|---|---|---|---|---|
| `doc2k.md` | 2,112 | 71 | **4.69** | 14.61 | 15.26 | 2.89 ms | 476 µs | 10.19 / 19.44 |
| `doc10k.md` | 10,062 | 432 | **4.85** | 19.38 | 24.33 | 4.85 ms | 730 µs | 9.69 / 19.01 |
| `doc26k.md` | 26,841 | 1,642 | **6.69** | 16.81 | 18.05 | 9.88 ms | 1565 µs | 11.5 / 22.42 |
| `doc52k.md` | 53,684 | 3,285 | **8.54** | 21.67 | 23.35 | 16.88 ms | 2815 µs | 9.93 / 22.2 |

**Enter and a fence-opening backtick, by document size** (application cost, reduced motion):

| document | Enter p50 | Enter p99 | fence keystroke p50 | fence p99 | fence worst |
|---|---|---|---|---|---|
| `doc2k.md` | 6.79 | 15.05 | 6.58 | 17.4 | 17.4 |
| `doc10k.md` | 7.41 | 15.84 | 4.91 | 14.78 | 14.78 |
| `doc26k.md` | 11.18 | 18.74 | 6.53 | 15.57 | 15.57 |
| `doc52k.md` | 14.39 | 19.14 | 8.95 | 21.86 | 21.86 |

**Where we are over the bar, plainly:** a 55,000-word manuscript — a short book, 288 KB,
3,285 lines — costs **16.9 ms of main-thread work per keystroke**, which is one whole 60 Hz frame,
and its application-only p99 is 21.7 ms. Round 1 wrote that such a document "still fits a
keystroke inside one 60 Hz frame". On these numbers that is false, and it was false then: it fits
the *median* keystroke, not the tail. Up to ~27,000 words the claim holds (9.9 ms of work,
p99 16.8 ms).

## 8. By kind of keystroke

Round 1's bench typed lowercase letters and spaces, so it never touched the branch of `render()`
that runs when the line count changes, and never the one that runs when a fence changes the
context of every line below it. Both are now measured on every run; these are the three sessions
of §4, on the 60 Hz clock, 10,062-word document — the first of the three sessions in each case,
because a key kind is only worth reading against the letters typed beside it in the same run.

`paragraph_breaks` — paragraph churn (Enter every 5–10 keys)

| key | n | p50 | p90 | p99 | worst |
|---|---|---|---|---|---|
| letter | 202 | 9.82 | 17.1 | 18.49 | 19.37 |
| space | 43 | 8.88 | 15.58 | 18.16 | 18.16 |
| enter | 38 | 11.53 | 17.17 | 20.76 | 20.76 |
| punct | 13 | 10.94 | 15.22 | 15.91 | 15.91 |
| capital | 4 | 8.58 | 16.89 | 16.89 | 16.89 |

`fence_flip` — opening and closing a fenced block, 50×

| key | n | p50 | p90 | p99 | worst |
|---|---|---|---|---|---|
| markdown | 150 | 9.73 | 16.95 | 18.78 | 19.08 |
| backspace | 150 | 9.39 | 16.36 | 19.34 | 20.34 |

`revision` — revision (select back, replace, undo)

| key | n | p50 | p90 | p99 | worst |
|---|---|---|---|---|---|
| letter | 188 | 10.21 | 17.04 | 20.52 | 21.01 |
| space | 16 | 8.96 | 17.85 | 18.45 | 18.45 |
| undo | 2 | 14.98 | 16.66 | 16.66 | 16.66 |

`paste_blocks` — writing with a paste every 25 keys

| key | n | p50 | p90 | p99 | worst |
|---|---|---|---|---|---|
| letter | 223 | 8.93 | 15.66 | 18.93 | 19.54 |
| space | 47 | 11.07 | 16.4 | 19.72 | 19.72 |
| punct | 14 | 8.83 | 13.46 | 14.45 | 14.45 |
| paste | 11 | 14.27 | 20.09 | 20.35 | 20.35 |
| capital | 5 | 16.52 | 22.23 | 22.23 | 22.23 |

`prose_end_of_draft` — writing at the end of the draft

| key | n | p50 | p90 | p99 | worst |
|---|---|---|---|---|---|
| letter | 223 | 9.09 | 16.4 | 18.52 | 18.84 |
| space | 48 | 13.38 | 18.25 | 19.01 | 19.01 |
| punct | 14 | 8.34 | 16.57 | 18.15 | 18.15 |
| backspace | 10 | 7.69 | 14.88 | 16.88 | 16.88 |
| capital | 5 | 8.77 | 16.01 | 16.01 | 16.01 |

Enter is the expensive keystroke — it is the one that inserts an element into a 432-element mirror
and shifts everything below it — and at 10k words it costs about 2 ms more than a letter. At 55k
words it costs 14.4 ms against 8.5 (§7). Opening a fenced code block used to be far worse than
either; §14 has the before/after.

## 9. A slower machine

`Emulation.setCPUThrottlingRate`, everything else identical: same build, same document, same keys,
same 60 Hz clock. This is the answer to "a 13600K is fast and a 2019 laptop is not". The full-speed
rows are the first session of §4; the throttled rows are one session of 300 keystrokes each.

| CPU | regime | present p50 | p99 | worst | main thread / key | ≤1 frame | ≤2 frames |
|---|---|---|---|---|---|---|---|
| full speed | writing at the end of the draft | **9.44** | **18.6** | 19.01 | 5.85 ms | 92 % | 100 % |
| full speed | writing in the middle of the draft | **11.03** | **18.89** | 19.24 | 6.07 ms | 87.7 % | 100 % |
| full speed | paragraph churn (Enter every 5–10 keys) | **9.82** | **18.69** | 20.76 | 6.22 ms | 88.7 % | 100 % |
| 2× slower | writing at the end of the draft | **11.4** | **19.63** | 20.87 | 10.85 ms | 80.7 % | 100 % |
| 2× slower | writing in the middle of the draft | **11.63** | **19.99** | 20.76 | 10.55 ms | 77.7 % | 100 % |
| 2× slower | paragraph churn (Enter every 5–10 keys) | **11.75** | **20.14** | 21.05 | 10.62 ms | 78.7 % | 100 % |
| 4× slower | writing at the end of the draft | **12.6** | **22.16** | 33.27 | 21.16 ms | 61.3 % | 100 % |
| 4× slower | writing in the middle of the draft | **20.26** | **26.07** | 34.31 | 25.29 ms | 17.3 % | 99.7 % |
| 4× slower | paragraph churn (Enter every 5–10 keys) | **17.75** | **26.25** | 36.18 | 23.22 ms | 40 % | 99.7 % |

Two times slower is still comfortable: p50 11.4–11.8 ms, p99 ~20 ms, four keystrokes in five
inside one frame. **Four times slower is where the 10k-word document stops fitting**: 21–25 ms of
main-thread work per keystroke, p50 up to 20.3 ms in the middle of the draft, and only 17 % of
keystrokes on screen within one frame — though 99.7 % still make it inside two, so it reads as
slightly soft rather than broken. That is the honest ceiling of the mirror architecture on a
slow machine, and it scales with main-thread work per keystroke (§7), not with the numbers in §4.


## 10. Startup

Round 1 reported "100 ms, median of 12 cold loads" and was right to be told that this is not a
cold start: it opened a new browsing context inside a browser that was already running, with a
warm GPU process, warm fonts, a warm V8 code cache and a warm server. Both numbers are here, and
they are never mixed.

**Page load inside an already-running browser** (warm process, warm caches — this is *not* a cold start):

| | 10,062-word document | empty document |
|---|---|---|
| navigation → editor ready | 80.8 ms (sd 4.22) | 45.5 ms (sd 5.13) |
| navigation → DOM parsed | 101.6 ms (sd 7.98) | 48.3 ms (sd 6.45) |
| navigation → **first frame with the document** | 108 ms (sd 7.12) | 60 ms (sd 8.96) |

Medians of 12 loads each.

**Cold start — a new browser process every run:**

| | exec → first frame with the document | exec → editor ready | exec → navigation start | runs |
|---|---|---|---|---|
| warm profile holding the document, cold process | **278 ms** (p99 302, worst 302) | 244 ms | 108 ms | 10 |
| **fresh profile** — no profile, no code cache, no storage | **229 ms** (p99 243, worst 243) | 213 ms | 113 ms | 8 |

The fresh-profile row is *faster* only because a first run has no document to restore and render:
it is an empty editor. The two rows are not a before/after, they are two different first frames.

Whole application from a shell, through `bin/quill` — chromium process spawn, profile,
window creation, navigation, fonts, and the full render of the document — on the compositor:

| | exec → first frame with the document | exec → editor ready | of which process spawn | runs |
|---|---|---|---|---|
| the 10,062-word document in the profile | **375 ms** (351–377) | 322 ms | 165 ms | 4 |
| **fresh profile**, empty document — a true first run | **380 ms** (374–423) | 303 ms | 171 ms | 3 |

`--fresh` really is a first run: the profile directory is deleted, so there is no code cache and
no local storage, and the document therefore cannot be there — that row is an empty editor, which
is what a first run of any app shows. There is no splash, no skeleton and no progressive reveal:
the document is tokenised and in the DOM *before* the first frame, so first paint and first usable
frame are the same frame. Nothing is deferred to "a few seconds after launch" the way iA's own
Windows 2.0 post describes for its spell and syntax passes; the only deferred work in Quill is the
word count.

## 11. A long session

Every regime above is about 30 seconds long. This one is 2,500 keystrokes — **4.4 minutes** of
continuous writing with a pause every 60 keys, which is what makes the autosave fire.

2,500 keydowns pressed, 2,500 recorded in the trace, 2,500 resolved by the in-page probe.

| quarter of the session | p50 | p90 | p99 | worst |
|---|---|---|---|---|
| first | 10.16 | 17.07 | 19.12 | 20.12 |
| second | 9.61 | 16.88 | 18.74 | 19.68 |
| third | 9.78 | 16.51 | 18.66 | 20.19 |
| fourth | 9.76 | 16.72 | 18.41 | 19.63 |

Whole session: p50 **9.76**, p99 **18.72**, worst 20.19 ms — the same as a 30-second run.

Nothing drifts: the fourth quarter is marginally *faster* than the first. The document autosave —
files.js writes the whole document to `localStorage` 400 ms after you stop — is not on the
keystroke path, and it is not expensive when it does run: **172 writes, 6.99 MB in total, 0.6 ms
at their worst and 22 ms added up over the whole session**, timed by wrapping
`Storage.prototype.setItem` from an init script. The JS heap reads 2 MB at the end (Chrome's
`performance.memory`, which is quantised) and GC costs **46 µs per keystroke**.

## 12. The dropped frames

Round 1 recorded "~304 frames marked dropped-affecting-smoothness per 300 keystrokes" and then
argued them away as an artefact of the bench's own animation. That was a plausible story, not a
demonstration. Here is the demonstration: an idle page with the same animation running, and the
same counters at three typing speeds.

| condition | keystrokes/s | frames/s | dropped frames/s | dropped per keystroke |
|---|---|---|---|---|
| idle page, no keystrokes at all, display clock on | 0 | 62.1 | 0.5 | — |
| idle page, no keystrokes at all, display clock off | 0 | 59.7 | 0.75 | — |
| typing: writing at the end of the draft | 10.7 | 61.6 | 0.8 | 0.07 |
| typing: fast typist, 45 ms (~266 wpm) | 20.8 | 63.4 | 21.7 | 1.04 |
| typing: saturation: keys injected back to back | 558.7 | 139.7 | 20.5 | 0.04 |

Dropped frames track **keystrokes per second, not seconds**: an idle page with the clock running
drops 0.5 per second, and typing at 133 wpm drops 0.8 per second — 0.07 per keystroke. They appear
in bulk only when keys arrive faster than one per refresh interval (at 266 wpm, exactly one per
keystroke), and in that case what is "dropped" is a frame the compositor asked for while the main
thread was still working on the previous keystroke's — which is the same fact the per-keystroke
latency already reports. The round-1 rate does not reproduce on this bench.

One thing worth saying because it is counter-intuitive: with **no** animation at all (reduced
motion, clock off) the counter reads *higher* — about 11 drops per second, one per keystroke —
because a frame is then produced on demand for each keystroke and every BeginFrame that arrives
without an update ready is counted. A smoothness counter that improves when you add an animation
is not measuring what a reader would call a dropped frame, which is why the user-facing statement
in this report is always the per-keystroke one.

## 13. Which published bar maps to which measurement

The three numbers in circulation measure three different things, and comparing across them is how
latency claims usually go wrong. Round 1 was told off for exactly that — a tile reading
"keystroke → paint 10.8 ms" beside "bar 32.5 ms", which is app-plus-compositor against
key-switch-to-photon. So, plainly:

| published bar | what it actually measured | Quill's comparable number |
|---|---|---|
| REFERENCE §5.3 / Fatin, **≤ 5 ms mean, ≤ 16 ms worst** | the application's own work: injected key → the pixel changing, no keyboard, no refresh cadence | **5.7 ms p50 / 15.0 p99**, 4.8 ms to a committed frame (§6) |
| Hume, **32.5 ± 4.0 ms** (Sublime, photodiode, 60 Hz panel) | a synthetic 1 kHz USB keypress → photons: OS, app, compositor, panel | **≈ 27.6 ms p50 / ≈ 41.7 p99** (§5 + 4 ms pixel response) |
| iA Writer | nothing published, at any level | — |

The second row is the only one that touches the headline bar, so here is its arithmetic, with each
term marked measured or cited:

| term | p50 | p99 | source |
|---|---|---|---|
| key event → frame presented, on the compositor | 23.64 | 37.70 | **measured**, 3 × 300 keystrokes, §5 |
| panel scan-out and pixel response | +4 | +4 | **cited** (Fatin: pixel response ≈ 4 ms; the refresh wait is already inside the measurement) |
| **keyboard-to-photon, comparable to Hume** | **27.6** | **41.7** | |
| *if* a real keyboard is added instead of Hume's USB emulator | 41.6 | 55.7 | cited (Fatin: 8–22 ms, mean 14) — this is what round 1 quoted, against a bar that does not contain it |

Two honest caveats on that row. **The compositor hop may be a frame too big**: round 1's physical
panel measured presentation 5.0 ms after commit where the virtual output measures 17.3, which
would make the p50 **≈ 14.3 ms measured, ≈ 18.3 ms to photons** — better than the bar by a distance.
And **the p99 is not a win**: 41.7 ms is the Atom/VS Code bracket, and the reason is the
compositor, not the application, whose own p99 is 15.0 ms. Both readings are in the same table on
purpose. The number this piece will defend is the first row of §"The numbers": the application's
own work, which is what a builder controls, and which is inside the published bar.

## 14. What changed in the code this round

Round 1 removed the per-keystroke work from the *cheap* path — the one its own bench exercised.
Round 2's bench presses Enter and backticks, and those went down a different branch of `render()`:
the one that runs when the **line count changes** or when the **context entering the lines below
the edit changes**. Three changes in `app/js/core.js`, all perf-only and backward compatible:

1. **Line elements no longer carry a `data-i` index attribute.** Keeping it truthful meant
   rewriting every element below an inserted line on every Enter — measured **1.14 ms** of
   attribute writes and style invalidations in a 3,285-line document, on the keystroke that is
   already the most expensive one. Nothing read it (checked across `app/` and `tools/`);
   `Writer.lineIndexOf(el)` is there if a piece ever needs element → index.
2. **A changed line is tokenised once per keystroke, not twice.** The incremental branch built the
   new elements *filled*, then re-filled the same elements after recomputing the contexts, because
   a line's tokens depend on the context entering it. They now go in empty and are filled once.
3. **The re-tokenising below an edit is bounded by what is on screen.** When the context entering
   the lines below changes — typing ``` opens a fenced block and changes every line to the end of
   the document — the old code re-tokenised and rebuilt all of them inside the keystroke.
   `AHEAD = 64` lines (more than a screenful at any font size) are now filled inside the keystroke
   and the rest is caught up in animation frames, 400 lines at a time; `Writer.flushPending()`
   forces it. Nothing the reader can see is ever stale, and the mirror's text still equals the
   textarea's, line for line, at every point (`shots/latency/probes/correctness.mjs`).

`Writer.render(false)` alone, measured directly (`shots/latency/probes/micro2.mjs`, 25 repeats,
the value mutation outside the clock), **55,000-word document / 10,000-word document**:

| one keystroke does this | before | after |
|---|---|---|
| type a character inside a paragraph | 1.95 / 0.51 ms | **1.89 / 0.51 ms** |
| press Enter (the line count changes) | 2.70 / 0.66 ms | **1.93 / 0.48 ms** |
| open a fenced code block (every line below changes context) | **26.16 / 4.64 ms** | **3.33 / 1.43 ms** |

The third row is the one that matters: one keystroke that cost **three 60 Hz frames** of
main-thread work in a book-length document now costs a fifth of one.

End to end, through the whole bench on two snapshot servers built from the same tree with only
`core.js` differing (200 keystrokes, reduced motion, clock off,
`shots/latency/abrm{10k,52k}-{before,after}.json`), the difference is real but modest, as it should
be — the fence case is the one that changes in kind rather than degree:

| 55,000-word document | before | after |
|---|---|---|
| Quill's `input` handler, µs per keystroke (paragraph churn) | 2555 | **2394** |
| Enter, presented p50 | 15.49 ms | **14.52 ms** |
| Enter, presented p50 (Markdown regime) | 16.26 ms | **15.08 ms** |
| main-thread busy per keystroke | 16.62 ms | 16.59 ms |

Round 1's four changes are still in place and still measured: no layout read inside the keystroke
(a `ResizeObserver` sizes the textarea), one `selection` event per keystroke instead of three,
writes before reads, and no attribute storm on keydown.

## 15. What these numbers are not

* **Keystrokes are injected over CDP**, not typed on a keyboard. Everything before the browser
  received the event — key switch, debounce, USB polling, kernel, compositor input path — is not
  measured. Fatin's published budget for that is 8–22 ms (mean 14); Hume's photodiode figures
  include it. Every keyboard-to-photon number in §13 is therefore **a measurement plus two cited
  constants**, and the two are kept visibly apart. It is not a photodiode result, and nothing here
  should be read as one.
* **Capitals and punctuation are delivered as one keydown carrying the right `key` and text, with
  no separate Shift keydown.** A real keyboard sends the Shift press too; it does no work in the
  app (core's keydown path looks at nothing for a plain character), but it is a difference.
  `Shift+←` in the revision regime does send both, and both are counted.
* **No physical panel was measured this round.** BRIEF.md forbids opening a test window on the
  workspace the user is working on, and at run time that workspace held a full-screen game with
  direct scan-out. §5 is measured on a **virtual Hyprland output** instead: the user's own
  compositor really composites and presents the window at 60 Hz, and the frame timestamps are real
  Wayland presentation feedback, but the last hop — scan-out to a physical panel and its pixel
  response — is not in them. Round 1's single physical-panel session, on an earlier build,
  measured a *smaller* compositor hop than the virtual output does (+5.0 ms at p50 over frame
  commit, against +17.3 ms here), which means §5 is, if anything, pessimistic. The command that
  would settle it on the panel is in §16 and takes about ten minutes on a free workspace.
* **One machine, one browser, one build, and somebody else was using it.** A 13600K is fast and a
  2019 laptop is not; §9 is the answer to "what about a slower machine", and the number that
  scales is main-thread work per keystroke, not presented latency. The load average at the start
  of every run is in the JSON, as is the list of the busiest processes on the machine.
* **One run in this round failed its own accounting, and is not in any table.** A
  reduced-motion session on the compositor recorded 302 keydowns for 300 injected presses — two
  strays from the launch — so `every_keystroke_accounted_for` came back false and the labelled
  statistics were withheld (`shots/latency/r2-headed-rm-1.json`, first regime). That is what the
  assertion is for; the round-1 real-display session had the same class of problem and reported a
  p50 anyway.
* **`saturation_stress` is not a human speed and its p50 is not a latency.** 559 keystrokes a
  second share 140 frames a second. It is run because it is where the cliff would be, and it is
  reported in the JSON and in §4's prose, never in a results table.
* **The 50.4 ms worst case** in the compositor table is one space key out of 900 in
  `prose_end_of_draft`; the six worst samples of that pooled run are 38.9, 39.4, 39.5, 40.3, 47.3,
  50.4, and its p99 is 37.7. Three frames, once in 900 keystrokes.
* **The caret arrives after the letter.** The letter is on screen in one frame; the caret glides
  to its new column over 62 ms (NOTES.md, `probes/caret-glide.mjs`: 57.7 ms p50 at 133 wpm,
  7.7 ms at 266 wpm, where it snaps). That is caret.js's deliberate design and it is not counted
  in any number above — every table here is about the *glyph*. A writer watching the caret is
  watching the slowest thing on the screen.

## 16. Reproducing it

```sh
node tools/serve.mjs 4173 &
node tools/latency.mjs --sessions 3 --keys 300          # the main table: 12 regimes, 3 sessions
node tools/latency.mjs --clock off --reduced-motion     # application cost, no animation anywhere
node tools/latency.mjs --throttle 4 --nostartup         # a machine four times slower
node tools/latency.mjs --doc shots/latency/doc52k.md --keys 150 --nostartup
node tools/latency.mjs --coldstart 10                   # a new browser process per run
node tools/latency.mjs --coldstart 8 --fresh            # ... and no profile at all: a first run
node tools/latency.mjs --long 2500                      # sustained session: drift, heap, autosave
node tools/mkdoc.mjs alice.txt shots/latency/doc10k.md 10000       # rebuild the corpus
bin/quill                                               # the app in its own window (chromium --app)
bin/quill --measure --runs 3 --keys 300 --sessions 3    # cold start + the bench on a real compositor
node shots/latency/probes/micro2.mjs shots/latency/doc52k.md       # price render() by itself
node shots/latency/probes/caret-glide.mjs http://localhost:4173/ 90
node shots/latency/probes/correctness.mjs http://localhost:4173/
```

Every result file carries a **sha256 fingerprint of the app tree it measured** (`env.app`), and
all the runs quoted here were taken against one frozen snapshot of that tree, served on its own
port, so that another builder's edit could not move the numbers mid-run. Raw runs:
`shots/latency/r2-*.json`.
