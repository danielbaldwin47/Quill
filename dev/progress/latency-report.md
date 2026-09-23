# Latency — measured

**Quill, round 4.** iA Writer publishes no latency numbers at all — "boots in a whizz… snappy as
jazz" is the entire public record — so the bar is third-party, and REFERENCE §5.2/§5.3 state it in
two parts and two different statistics:

* **App-internal, keystroke → committed frame: ≤ 5 ms *average*, ≤ 16 ms *worst case*.**
  Fatin's Typometer figures are means: Notepad++ 4.3, Emacs 5.3, **Sublime 8.2** (max 35.2, sd 2.0).
* **Keyboard-to-photon on a 60 Hz panel ≈ 30–35 ms.** Hume's photodiode: Sublime **32.5 ± 4.0**,
  TextEdit 33.4, Atom 45.6, VS Code 47.6.

Round 3 answered the first bar honestly and **failed it**: 5.73 ms mean, 0 of 12 regimes clearing
the average. It also could not put a number on the second one at all, only a 17–29 ms bracket.

Round 4 is different, and the reason is not a measurement trick. It is **one file this piece does
not own**, which the round-3 critic told it to go and fix.

| | round 3 | round 4 |
|---|---|---|
| keystroke → the caret has stopped moving, at 133 wpm | **57.0 ms** | **8.1 ms** |
| keystroke → committed frame, application cost, mean (bar: ≤ 5) | 5.73 ± 3.60 | **2.43 ± 0.66** |
| … worst (bar: ≤ 16) | 14.25 | **15.61** |
| paced regimes clearing the ≤ 5 ms average | **0 of 11** | **10 of 11** |
| keystroke → presented, on the user's own compositor | 21.99 ± 10.72 | **11.40 ± 4.85** |
| **kernel keypress** → presented, real keys through /dev/uinput | *not measured* | **12.17 ± 5.01** |
| cold `bin/quill` launch → the first frame a human could see | 357 ms | 370 ms |

## 0. What round 3 was told, and what this round did about it

| round 3's verdict | round 4 |
|---|---|
| **"The caret. `GLIDE_X = 62`… every keystroke glides… 57 ms p50 while the glyph is on the glass in ~6 ms… a self-inflicted 10× on the one moving thing a typist's eye actually tracks. It is one constant. Fix it before spending another round chasing a ±12 ms panel measurement — and once fixed, report caret-settled latency as a headline milestone."** | Fixed, in `app/js/caret.js`, and written up in NOTES.md under this piece because that file belongs to the caret piece. §4. Caret-settled is now a headline row in §5 and a top-level field in `latency.json`. **57.0 → 8.1 ms**, and the caret is now in the glyph's own frame on **97.5 %** of keystrokes instead of 2.5 %. And it was not only the caret: the glide is an animation, an animation is a frame clock, and removing it took **2.7 ms off every keystroke's scheduling** — which is what turned the ≤ 5 ms average from a fail into a pass. §4.2. |
| "Keyboard-to-photon is unresolved… the input delivery hop is excluded from every number in the file. `tools/uinput-keys.py` exists and never ran." | **It runs now, and the hop is measured: 0.36 ms mean (p99 0.63, n = 4 505).** Every headline compositor figure in §8 starts at a `write(2)` to `/dev/uinput` — evdev → libinput → Hyprland → Wayland → Chromium — not inside the browser. It never ran before because **it could not have**: `struct.pack('=llHHi')` is 16 bytes and `struct input_event` is 24 on this kernel, so every key press was silently swallowed as a bare `SYN_REPORT`. The device enumerated perfectly and delivered nothing. One character. §3.6. |
| "the compositor runs used a virtual Hyprland headless output whose commit→present hop is 17.23 ms p50 (a whole 60 Hz interval)…" | That 17.2 ms **was the caret glide**, not the output. With nothing animating, the same virtual output's commit → present hop is **1.98 ms** (p50 1.85). §8. The ±12 ms uncertainty round 3 could not resolve has collapsed to a term this report states as arithmetic instead of guessing: §9. |
| "The physical-panel run… the Dell U2720Q was asleep with its DisplayPort link down." | **This machine has no display attached at all.** Every DRM connector on all seven cards reads `disconnected` (`dev/shots/latency/r4-no-display.txt`), and Hyprland is running on a `FALLBACK` headless output. There is no panel to measure scan-out on, and this round does not pretend there is: §9 says exactly which term is measured, which is arithmetic and which is cited. |
| "the app-internal bar fails and the report says so: 5.73 ms mean against ≤ 5 ms, with 0 of 12 regimes clearing the mean." | **2.43 ms mean (95 % CI 2.39–2.48), 15.61 ms worst**, n = 900. Ten of the eleven paced regimes clear the ≤ 5 ms average; eight clear the ≤ 16 ms worst case. §6. |
| "Distribution shape is far worse than the bar's: compositor sd 10.72 vs Hume's 4.0, p99 36.47." | Compositor **sd 4.85** against Hume's 4.0, p99 19.84. §8. |
| "the deferred renderer leaves 1,608 of 3,285 lines unrendered after a keystroke… idle callbacks of 1.08 ms/key at 10k and 4.96 ms/key at 55k." | Re-asserted (§13) and the idle cost re-measured: 0.91 ms/key at 10k. It is still `chrome.js`'s whole-document word count and it is still somebody else's file; NOTES.md says so with the number. |
| "Only 3 of 12 regimes ran on a compositor, and they are the cheap ones." | **All twelve** ran on the compositor, three sessions each, n = 900 apiece — including `fast_typist` and `saturation_stress`. §8. |
| "One machine, one Chromium build, one 60 Hz refresh… CPU throttling is a proxy for a slow machine." | Still true, still said (§14). |

## 1. Environment

| | |
|---|---|
| CPU | 13th Gen Intel(R) Core(TM) i5-13600K, 20 threads |
| OS | Linux 7.1.9-arch1-2, Wayland (Hyprland 0.56.2) |
| Browser | Chromium 151.0.7922.173 (system build), headless and headed |
| **Display** | **none. Every DRM connector on this machine reads `disconnected`; Hyprland runs on a `FALLBACK` headless output.** Dump: `dev/shots/latency/r4-no-display.txt` |
| Compositor output used | `hyprctl output create headless`, 1920×1080 @ 60 Hz, scale 2 — a real output the user's own Hyprland really composites and presents, created and removed by `bin/quill` |
| Viewport | 1440×900 logical px (at scale 2 that is 2 880 × 1 800 device px of paint per frame — more work than a 1× panel, not less) |
| App build | served from a **frozen snapshot** of `app/` on port 4191, sha256 `e0eb82b9c5be152c` (16 js/css/html files), so that another builder's edit could not move the numbers mid-run. `env.app.sha256` inside the raw files fingerprints the *working tree* at the moment each run started, which is why it moves; the bytes served did not. The snapshot differs from the `app/` committed with this round by one CSS **comment** in `caret.css` and nothing else. |
| Load | recorded in every result file, with the busiest processes at the time. This is somebody's workstation and things were running on it (an `omarchy-agent`, a `ddcutil` spinning at 85 % on an absent monitor). That makes these numbers pessimistic, not optimistic. |

## 2. The document, and the typist

`dev/shots/latency/doc10k.md` — **10,062 words, 53,031 characters, 432 lines** — built by
`tools/mkdoc.mjs` from *Alice's Adventures in Wonderland* (Project Gutenberg #11, public domain):
chapters as `##` headings, Carroll's own italics as Markdown emphasis, verse as block quotes, and
an editor's note with a task list, a link, inline code and a fenced code block, so the tokenizer,
the decorators and the mirror all do real work on every frame. Paragraphs are **one logical line
each** (up to 980 characters) — the way a document looks in iA Writer, and the hard case for a
line-diffing renderer. Three other sizes (2 k, 27 k, 55 k words) in §11.

**The typist** types real prose, not one repeated sentence: capitals, commas, semicolons, quotation
marks, apostrophes, dashes, sentence ends, paragraph breaks, and **a 1–3 character typo corrected
every ~45 characters**. Twelve regimes cover writing at the end of a draft and in the middle of it,
Focus: Sentence, paragraph churn (Enter), select-back-and-replace with `Ctrl+Z`, Markdown syntax,
opening and closing a fenced code block, `Ctrl+V` paste from the real clipboard, bursts with
pauses, 266 wpm, and an unpaced saturation run. Each session is seeded per regime, so all three
sessions type identical keys and the spread between them is the machine rather than the script.
Every keystroke is labelled by the key that caused it; headline tables are computed over
**text-affecting keystrokes only**.

## 3. Method

### 3.1 Seven milestones on every keystroke

Chrome's `EventTiming` trace records (`devtools.timeline`, unrounded, microseconds) carry each
event's hardware timestamp, `processingStart`, `processingEnd`, `commitFinishTime` and `duration`
(which ends at the **presentation feedback** of the frame carrying that event's update). Round 4
adds two more milestones, taken from the top-level main-thread task that ran the frame:

| milestone | what it is |
|---|---|
| `to_js_done` | the app's JavaScript has returned (last non-`keyup` handler for this key) |
| `scheduler_wait` | from there to the **start of the top-level task that ran the frame**. Nothing is running in this interval. It is Chromium deciding when to begin a frame, and no web application can shorten it — it can only ask for one |
| `frame_work` | that task's own style, layout, pre-paint, paint and commit |
| `to_paint` | the frame's paint is finished |
| `to_commit` | `commitFinishTime`. **The milestone REFERENCE §5.3's bar names** |
| `to_present` | presentation feedback: the frame is on the display's scan-out |
| **`app_cost`** | `to_js_done + frame_work` — **the keystroke with the frame *cadence* removed and nothing else removed.** This is what Typometer measures, and §7 uses it |

`app_cost` is not a smaller number invented to pass a bar; both it and `to_commit` are printed
side by side everywhere, and §6's pass/fail column is scored on `to_commit`, the stricter one.

### 3.2 Which statistic answers which bar

REFERENCE §5.3's app-internal bar is a **mean and a worst case** because Fatin's figures are means.
Hume's is a **mean ± sd**. So every table leads with mean, sd and max, with a bootstrap 95 %
interval on the mean; p50 and p99 sit beside them for shape, never in their place. Round 2's
substitution of p50 for mean is the one thing this bench will not repeat.

### 3.3 Every keystroke accounted for

Every regime asserts that the keydowns expected, the keydowns in Chrome's trace and the keydowns
seen by an independent in-page rAF/MessageChannel probe are the **same number**, and that every one
of them carries a commit time and a presentation time. **All 36 application-cost runs, all 36
headless runs, all 36 compositor runs and all 15 real-keyboard runs passed. Nothing was withheld.**
A fresh page per regime makes round 1's double-probe bug unrepresentable.

### 3.4 Is there a display cadence in this run, or not? — measured, not assumed

Round 3 argued about which runs had a frame clock. This round tests it. Every run reports
`display_cadence_ms`: the gaps between the presentation timestamps of consecutive keystroke-carrying
frames, the share of those gaps that are whole multiples of 16.67 ms, and the **circular
concentration** of those timestamps against a 16.67 ms grid (1.0 = every frame on the same phase of
a 60 Hz clock, 0.0 = no clock at all).

| run | gaps that are a multiple of 16.67 ms | phase concentration |
|---|---|---|
| application cost (`--clock off --reduced-motion`) | **0.3 %** | **0.00** |
| headless on a 60 Hz frame clock (`--clock on`) | **98.7 %** | **0.98** |
| the user's own compositor, virtual output | 0.3 % | 0.00 |

(The application-cost row is measured in `r4-appcost-cadence.json`, a separate 300-key run of the
same regime in the same mode — the cadence check was added to the bench after the twelve-regime
application-cost run had already started, so that file does not carry it. Its `to_commit` is
2.41 ± 0.56 against the big run's 2.43 ± 0.66: the same population.)

So §6's application-cost numbers really are free of display cadence, §5's headless-60 Hz numbers
really are on one, and — the thing round 3 got wrong — **Hyprland's headless output does not gate
presentation on a vblank.** §9 is about what that means and what it does not.

### 3.5 One clock

Chromium's trace timestamps are TimeTicks, which on Linux is `CLOCK_MONOTONIC` in microseconds.
Verified rather than assumed: `process.hrtime.bigint()` taken immediately before a CDP key dispatch
read 109 750 982 782 µs and the trace's own `keydown` for that key read 109 750 983 595 µs — the
same clock, 813 µs apart, which is the CDP dispatch itself. That is what makes §3.6 possible.

### 3.6 Real keys, through the kernel

`tools/uinput-keys.py` creates a **real keyboard on `/dev/uinput`** and records `CLOCK_MONOTONIC`
immediately before each `write(2)`. The keys travel evdev → libinput → Hyprland → Wayland →
Chromium exactly as the user's own keyboard does. `trace_keydown_ts − t_ns/1000` is therefore the
delivery hop, measured, in one clock.

Two things had to be fixed before it worked, and both are worth stating plainly:

1. **It could never have worked before.** `struct.pack('=llHHi', …)` is 16 bytes; `struct
   input_event` on 64-bit Linux is 24 (a `timeval` is two 8-byte longs). The kernel therefore read
   `type` out of what was really the next event's timestamp, saw `0`, and turned every key press
   into a bare `SYN_REPORT`. The device enumerated correctly — `hyprctl devices` listed it as a
   keyboard with the right layout — and delivered nothing. `'@llHHi'` with a size assertion.
2. **Real keys go wherever the compositor thinks focus is, and this machine has terminals on it.**
   The keys are now typed **in chunks of 25**, and between every chunk the page itself is asked
   whether it still has keyboard focus with the caret in the textarea; if it does not, the injector
   is told to stop and the run is discarded. `bin/quill` refuses to start at all unless the
   compositor's active window is the window it just opened. Two independent guards, and the second
   is the authority: a Wayland client reports focus only if the compositor sent it keyboard enter.

## 4. The caret — the biggest number in the product, and what it was hiding

### 4.1 What it was

`app/js/caret.js` glided the caret to its new column over `GLIDE_X = 62 ms` whenever two keystrokes
were more than `SNAP_MS = 60 ms` apart. 133 wpm is 90 ms between keys, so **at any ordinary writing
speed every keystroke glided.** The glyph was on the glass in ~3 ms; the caret arrived 57 ms later.

Measured black box — nothing but the caret's own client rect, read every animation frame, with the
final position taken from the last frame before the next keystroke
(`dev/shots/latency/probes/caret-settle.mjs`, `r4-caret-before.json` / `r4-caret-after.json`):

| ms between keys → | 45 (266 wpm) | 90 (133 wpm) | 150 (80 wpm) | 250 (48 wpm) |
|---|---|---|---|---|
| keydown → caret stationary, **before** | 8.5 | **57.3** | 57.0 | 57.0 |
| keydown → caret stationary, **after** | 8.6 | **8.1** | 8.3 | 8.5 |
| in the glyph's own frame, before | 97 % | **1 %** | 1 % | 3 % |
| in the glyph's own frame, after | 97 % | **97.5 %** | 97.5 % | 97.5 % |
| how far behind the letter, in cells, before | 0 | **1.0** | 1.0 | 1.0 |
| … after | 0 | **0** | 0 | 0 |

(The 45 ms column is the one speed at which the old code already snapped — which is why round 2's
bench, which typed fast, never saw this.)

The fix is `EDIT_SNAP_MS = 150`: **a caret move less than 150 ms after a text change never glides.**
Typing, Enter, Backspace, paste and undo snap, always, at every speed. Navigation — a click, a word
jump, an arrow between lines — keeps a glide, shortened to 34 ms along a line and 46 ms between
them, and only for hops of three cells or more. Four constants and eight lines. NOTES.md has the
whole diff and the reasoning, because that file belongs to the caret piece.

**The bench now asserts it on every keystroke of every run.** A listener on `window` in the bubble
phase (so it runs after the app's own) reads three inline-style *strings* off the caret — no layout,
no cost — and records whether its final position was written in the keystroke's own task with
nothing animating it. Across every run in this round — application cost, headless, compositor and real-keyboard —
**39,129 of 39,129 input events, `all_static: true`, in every regime.** Before the fix the same probe reads 142 of 145.

### 4.2 And what it was hiding

An animation is a frame clock. While one runs, the update a keystroke causes waits for the next tick
instead of producing a frame on demand. `document.getAnimations()` while typing at 133 wpm:

| | before | after |
|---|---|---|
| samples with `transform on caret` running | **343 of 446** | **0** |
| samples with anything running | 343 | 37 (the chrome bars' fade, at the edges of a burst) |

Same document, same script, same machine, `--reduced-motion --clock off`, 120 keys:

| | before | after |
|---|---|---|
| keystroke → committed frame, mean | 5.29 ms | **2.55 ms** |
| of which **`scheduler_wait`** — nothing running, waiting to be scheduled | 3.67 ms | **0.64 ms** |
| of which **`app_cost`** — the app's JS and its frame's work | 1.62 ms | 1.91 ms |

**The application's own work did not change. The 2.7 ms was the caret.** Note that this was true
even under `prefers-reduced-motion: reduce`, where `caret.css` cancels the transition: `moveTo()`
still wrote a `transform` and a `transition-duration` on every keystroke and scheduled a `settle()`
timer 86 ms later, so there was a second style change and a second frame per key regardless of the
media query. Round 3's headline finding — *"the keystroke is scheduler-bound; adding 2 ms of
busy-wait changes nothing"* — was measuring the caret's own frame clock without knowing it. It was
true, and the cause was in the product, not in Chromium.

## 5. The numbers

Milliseconds. **Plain writing at the end of the 10,062-word draft at 133 wpm** — the most common
thing a writer does — three sessions of 300 keystrokes, pooled, n = 900, every keystroke accounted
for. Mean ± sd first, because that is how both published bars are stated.

| | mean ± sd | worst | p50 | p99 |
|---|---|---|---|---|
| The app's JavaScript has returned | **1.52 ± 0.20** | 4.95 | 1.49 | 2.31 |
| **`app_cost`** — its JS plus the whole of the frame it caused, no cadence | **1.92 ± 0.22** | 5.30 | 1.88 | 2.88 |
| … the frame is **committed** — *the milestone REFERENCE §5.3's bar names* | **2.43 ± 0.66** | **15.61** | 2.33 | 3.77 |
| … the frame is **presented** (no display cadence at all) | 3.29 ± 0.84 | 17.27 | 3.16 | 6.21 |
| **The caret has stopped moving** (§4.1, at this pace) | **8.1** | 16.7 | 7.8 | 16.7 |
| Keystroke → presented, headless on a **60 Hz frame clock** | 10.08 ± 4.72 | 18.95 | 9.93 | 18.37 |
| Keystroke → presented, **on the user's own compositor** (§8) | 11.40 ± 4.85 | 31.02 | 11.36 | 19.84 |
| **Kernel keypress** → presented, real keys through /dev/uinput (§8) | **12.17 ± 5.01** | 34.01 | 12.21 | 21.03 |

**Against the app-internal bar (≤ 5 ms average, ≤ 16 ms worst case).** Quill is at **2.43 ms mean
(95 % CI 2.39–2.48) and 15.61 ms worst**. It clears both. That is inside Notepad++'s 4.3 ms and
GVim's 0.9–ish bracket in Fatin's table and **3.4× better than Sublime's 8.2** — on a 10,000-word
Markdown document with live tokenising, decorators and a mirrored render, not a plain text file.

**Against the photon bar (Sublime 32.5 ± 4.0 ms).** From a real key pressed into `/dev/uinput` to
the user's own compositor reporting the frame presented: **12.17 ± 5.01 ms**, everything in that
number measured end to end, including the kernel → libinput → Hyprland → Wayland → Chromium
delivery hop that every previous round excluded. What is *not* in it is the panel, because this
machine has no panel: §9 adds that term as arithmetic, states its bounds, and says which part is
cited rather than measured.

## 6. Every regime, in the bar's own statistics

`--reduced-motion --clock off`, phase concentration 0.00 (§3.4): the application's own work, no
display cadence. n = 900 each (revision has fewer text-affecting keystrokes per session because
half of its presses are chords). `✓`/`✗` is against **≤ 5 ms mean** and **≤ 16 ms worst**, scored on
`to_commit`.

| regime | `to_commit` mean ± sd | 95 % CI | worst | ≤5 | ≤16 | `app_cost` mean | `app_cost` worst |
|---|---|---|---|---|---|---|---|
| **prose_end_of_draft** | **2.43 ± 0.66** | 2.39–2.48 | 15.61 | ✓ | ✓ | 1.92 | 5.30 |
| markdown_syntax | 2.67 ± 0.59 | 2.64–2.71 | 9.54 | ✓ | ✓ | 2.13 | 3.87 |
| fence_flip (open/close a code block) | 2.68 ± 0.23 | 2.66–2.69 | 4.30 | ✓ | ✓ | 2.19 | 3.54 |
| paste_blocks (Ctrl+V of a paragraph) | 2.69 ± 1.71 | 2.58–2.81 | **18.03** | ✓ | ✗ | 1.98 | 5.08 |
| paragraph_breaks (Enter) | 2.71 ± 0.46 | 2.68–2.74 | 5.64 | ✓ | ✓ | 2.15 | 3.98 |
| letters_only (round 1's script) | 2.71 ± 0.24 | 2.70–2.73 | 4.53 | ✓ | ✓ | 2.21 | 3.89 |
| bursts_and_pauses | 2.71 ± 1.83 | 2.60–2.84 | **16.90** | ✓ | ✗ | 1.91 | 3.07 |
| prose_middle_of_draft | 2.73 ± 0.36 | 2.71–2.76 | 6.38 | ✓ | ✓ | 2.23 | 5.15 |
| revision (select back, replace, undo) | 2.76 ± 0.72 | 2.71–2.82 | 15.76 | ✓ | ✓ | 2.21 | 3.46 |
| prose_focus_sentence | 3.11 ± 0.30 | 3.09–3.13 | 4.89 | ✓ | ✓ | 2.60 | 4.13 |
| fast_typist (266 wpm) | **5.90 ± 4.80** | 5.59–6.23 | **18.00** | ✗ | ✗ | 2.04 | 3.25 |

**Ten of eleven paced regimes clear the ≤ 5 ms average; eight clear the ≤ 16 ms worst case.**
Out of the table on purpose, as in every round: **saturation_stress** (no pacing at all, ~2,000 wpm)
at 8.90 ± 4.54, worst 19.15 — keystrokes share frames there, so a per-keystroke latency is not
something a person could experience.

One number in that table went the wrong way: the headline regime's **worst case is 15.61 ms against
round 3's 14.25**, even though its mean fell from 5.73 to 2.43. That is the same effect as the four
misses below — a single keystroke that arrived while Chromium's frame source was asleep — and it is
now a rare outlier on an otherwise very tight distribution (p99 3.77, against round 3's 13.16)
rather than the shoulder of a broad one. It is still a worse worst case, and it is still inside the
bar.

**Every miss is `scheduler_wait`, and none of them is the application.** Look at the last two
columns: `app_cost` never exceeds **5.30 ms** in any of the 9,618 keystrokes in this table, and its
mean runs 1.55–2.60 in all twelve regimes including saturation. The four regimes that miss the
≤ 16 ms worst case are the four that pause: after ~1.4 s of idle, Chromium has to spin its frame
source back up, and that first keystroke pays up to a whole frame for it. Demonstrated rather than
asserted — the spike survives turning off **every animation the app has**, including the chrome
bars (`--settings '{"showChrome":false}'`: worst 17.49, unchanged) and the caret blink (cancelling
it on `keydown` instead of on the change: worst 16.76, unchanged). It is Chromium waking up.

## 7. Where the 2.43 ms goes

Per keystroke, from the trace, 10k-word document, application-cost regime:

| | mean, per keystroke |
|---|---|
| Chrome's own insertion of one character into a 53 KB `<textarea>` | **1,281 µs** |
| **Quill's `input` handler — the mirror diff, the tokenizer, the decorators** | **726 µs** |
| the frame's own style, layout, pre-paint and paint | 1,688 µs |
| *(between keystrokes, not in this path: `chrome.js`'s word count, idle-scheduled)* | *911 µs* |
| main thread busy, total | 3.89 ms per key, **4.1 % of wall clock** |

The app's JavaScript is finished **1.52 ms** after the key. `app_cost` — that plus the whole frame —
is **1.92 ms**. The frame is committed **2.43 ms** after the key. The 0.51 ms in between is
`scheduler_wait`: time in which *nothing is running at all*.

Round 3 spent a section explaining why its residual 2 ms could not be removed. It could: it was the
caret (§4.2). What is left is 0.5 ms of Chromium's scheduling, and that one really is not the
application's to spend.

## 8. On a real compositor, with real keys

`bin/quill` opens the app in a `chromium --app` window on the user's own Hyprland, on a virtual
output the compositor genuinely composites and presents, and `tools/latency.mjs` attaches to it.
**All twelve regimes, three sessions each, n = 900 apiece, every keystroke accounted for.**

| regime | keystroke → presented, mean ± sd | p50 | p99 | worst | per-session means |
|---|---|---|---|---|---|
| paste_blocks | 11.16 ± 4.75 | 11.14 | 19.89 | 27.83 | 11.30 / 11.21 / 10.96 |
| **prose_end_of_draft** | **11.40 ± 4.85** | 11.36 | 19.84 | 31.02 | 11.49 / 11.36 / 11.37 |
| paragraph_breaks | 11.62 ± 4.69 | 11.61 | 20.26 | 21.95 | 11.48 / 11.88 / 11.52 |
| markdown_syntax | 11.71 ± 5.01 | 11.77 | 21.38 | 50.33 | 11.60 / 11.54 / 12.00 |
| letters_only | 11.91 ± 4.69 | 11.73 | 20.16 | 21.35 | 11.96 / 11.97 / 11.79 |
| fence_flip | 11.93 ± 4.71 | 11.90 | 19.94 | 20.69 | 12.03 / 11.85 / 11.91 |
| prose_middle_of_draft | 11.94 ± 4.68 | 11.99 | 20.01 | 20.50 | 11.99 / 11.81 / 12.03 |
| prose_focus_sentence | 11.99 ± 4.63 | 11.63 | 20.15 | 26.14 | 12.08 / 11.92 / 11.96 |
| bursts_and_pauses | 12.43 ± 5.94 | 12.48 | 30.27 | 35.88 | 12.34 / 12.66 / 12.29 |
| revision | 13.36 ± 6.34 | 12.63 | 32.50 | 36.85 | 13.27 / 13.49 / 13.31 |
| fast_typist (266 wpm) | 20.85 ± 10.01 | 20.69 | 37.33 | 37.98 | 22.87 / 19.57 / 20.11 |
| saturation_stress (~2,000 wpm) | 24.47 ± 6.55 | 24.85 | 35.57 | 37.15 | 26.72 / 22.59 / 24.10 |

Of the 11.40 ms in the headline regime, **9.48 ms is Chromium** (keystroke → committed, of which
~7.5 ms is waiting for the compositor's next 60 Hz BeginFrame) and **1.98 ms is Hyprland** compositing
and reporting the frame presented (p50 1.85, sd 1.38).

**And the same regimes typed with a real keyboard.** `bin/quill --uinput`: the clock starts at a
`write(2)` to `/dev/uinput`, so the kernel → libinput → Hyprland → Wayland → Chromium hop is inside
every number. Five regimes (the ones a keyboard can express — no `Ctrl+V`, no `Shift+←`), three
sessions each, n = 900 apiece, all fifteen runs reconciled.

| regime | kernel `write(2)` → Chromium's event | **kernel → presented** | p50 | p99 | worst |
|---|---|---|---|---|---|
| **prose_end_of_draft** | 0.37 ± 0.15 | **12.17 ± 5.01** | 12.21 | 21.03 | 34.01 |
| paragraph_breaks | 0.36 ± 0.20 | 12.41 ± 5.10 | 12.27 | 22.57 | 34.29 |
| markdown_syntax | 0.37 ± 0.14 | 12.49 ± 5.26 | 12.20 | 25.29 | 44.49 |
| fence_flip | 0.34 ± 0.19 | 12.61 ± 5.07 | 12.40 | 20.93 | 45.54 |
| letters_only | 0.36 ± 0.14 | 12.72 ± 4.97 | 12.57 | 21.64 | 35.89 |

**The delivery hop is 0.36 ms.** That is the term round 2 and round 3 were told they were missing,
and it turns out to be a third of a millisecond — but it is now *measured* rather than argued away,
and it is inside every figure in the middle column.

## 9. Keyboard to photon: what is measured, what is arithmetic, and what is cited

**There is no display on this machine.** Not asleep — absent. Every DRM connector across seven
cards reads `disconnected` and Hyprland is running on a `FALLBACK` headless output
(`dev/shots/latency/r4-no-display.txt`). `bin/quill --panel N` still exists, still refuses to take a
screen off somebody who is using it (`tools/idle-check.py`), and still detects and refuses the
FALLBACK case by name. It cannot run here, and no amount of patience will change that.

So the photon figure is stated as a sum, with each term labelled:

| term | value | how |
|---|---|---|
| kernel keypress → the compositor reports the frame presented | **12.17 ms** (sd 5.01) | **measured**, §8, n = 900 |
| the frame waits for the panel's next vblank | **0 – 16.67 ms**, expectation **8.3** | **arithmetic**: a 60 Hz panel scans out at a vblank and not between two. Hyprland's *headless* output does not gate on one (§3.4 measures this: phase concentration 0.00, commit → present 1.98 ms), so this term is missing from the measurement and has to be added |
| panel pixel response | ≈ **4 ms** | **cited**, not measured — Fatin's monitor budget (REFERENCE §5.2: "refresh 0–17, pixel response ~4"). Nobody can measure it without a photodiode, and there is no panel here to point one at |
| **total, keyboard to photon, 60 Hz panel** | **≈ 24.5 ms** at the mean; **12.2–32.9 ms** as a hard bracket | |

Against **Hume's Sublime Text: 32.5 ± 4.0 ms** on a Dell P2415Q at 60 Hz. Quill's *mean estimate*
beats it by 8 ms, and **the worst end of the bracket — every keystroke missing a whole refresh —
only just reaches Sublime's mean.** Round 3's honest position was "we cannot tell you which side of
Sublime we are on." This round's is: the pessimistic end of our range is Sublime's average.

Three honesties about that sum:

* **The variance is now comparable.** sd 5.01 against Hume's 4.0, where round 3 was 10.72. Adding a
  uniform vblank term would take it to ≈ 6.9, which is TextEdit/Terminal class, not Electron class.
* **The vblank term is a model, not a measurement.** It is the right model — a frame cannot become
  photons before scan-out — but on a real panel Chromium's BeginFrames are driven by the compositor,
  so a commit tends to land early in a refresh period and the true wait is usually less than the
  uniform 8.3 ms. Quoting 8.3 is the conservative choice.
* **Everything else is inside the number.** Keyboard scan and USB polling are excluded from both
  sides: Hume drove his tester from a 1 kHz Teensy, this bench writes to `/dev/uinput`. Fatin's
  budget for that hardware is a further 8–22 ms for a real keyboard, and it applies to Sublime and
  to Quill equally.

## 10. Startup

`bin/quill` from a shell, chromium process spawn included, the 10,062-word document already in the
profile, on a real compositor. n = 12.

| | median | mean ± sd | range |
|---|---|---|---|
| **exec → the first frame a human could see** | **370 ms** | 382.6 ± 58.2 | 349.9–573.3 |
| exec → `window.__quillReady` (a JS marker, **before any frame exists**) | 309 ms | 312.6 ± 8.1 | 303.4–336.1 |

`startup_ready` in `dev/progress/latency.json` is the **370**, the frame, not the marker. It is 13 ms
slower than round 3's 357 and the marker is 36 ms slower than round 3's 273; nothing in this round's
changes touches boot, five node servers were running on the box this time where round 3 had two, and
both rounds' spreads overlap heavily (round 3's own range was 331–562). It is reported as measured
rather than explained away, and it is the one number in this report that did not improve.

## 11. Bigger documents, and slower machines

Application cost, `prose_end_of_draft`, n = 300 each:

| document | lines | `to_commit` mean ± sd | worst | `app_cost` mean | main thread per key |
|---|---|---|---|---|---|
| 2,112 words | 71 | 1.67 ± 0.67 | 11.29 | 1.36 | 2.40 ms |
| 10,062 words | 432 | 2.47 ± 0.76 | 13.65 | 1.95 | 3.90 ms |
| 26,841 words | 1,642 | 4.60 ± 0.73 | 15.25 | 3.50 | 7.18 ms |
| **53,684 words** | 3,285 | **7.66 ± 1.00** | **18.67** | **5.72** | **11.69 ms** |

A 55k-word manuscript is over the ≤ 5 ms average and inside the ≤ 16 ms worst case on `app_cost`,
and over the worst case on `to_commit` by 2.7 ms. Round 3 was 8.55 / 17.93 there; this is 7.66 /
18.67 with a far tighter sd. Where it goes (µs per keystroke at 55k): Chrome's own textarea
insertion 4,509, the frame's style/layout/paint 5,931, **Quill's own `input` handler 2,326**.
Roughly half is Chrome holding 288 KB of text in a `<textarea>`.

CPU throttled (`Emulation.setCPUThrottlingRate`), keystroke → presented on the 60 Hz clock:

| | prose_end | prose_middle | paragraph_breaks |
|---|---|---|---|
| 2× slower | 11.55 ± 4.64 | 11.94 ± 4.53 | 11.78 ± 4.78 |
| 4× slower | 12.39 ± 4.03 | 15.23 ± 4.67 | 14.61 ± 4.97 |

At 4× the app's own cost is 6.9–8.1 ms and the end-to-end number is still inside two 60 Hz frames.
This is a Chromium proxy for a slow machine, not a slow machine: it leaves GPU, memory bandwidth and
storage untouched, and it is the weakest evidence in this report.

## 12. Frames

An idle page produces 62.1 frames/s with the bench's display clock on and 59.7 with it off, dropping
0.5/s either way — so the drop rate belongs to the clock animation, not to Quill. Typing at 133 wpm
on the 60 Hz clock, **90.7 % of keystrokes are presented within one 60 Hz frame** and 100 % within
two; on the compositor, 81.3 % within one frame. In the application-cost regime, where there is no
cadence to be inside of, the whole distribution sits under 6.3 ms at the 99th percentile.

`files.js` flushed the document to `localStorage` 8 times in a 300-key run (400 ms after each pause,
never during a burst): 0.11 ms mean, 0.4 ms worst.

**Sustained writing.** 2,500 keystrokes with pauses, 4.4 minutes, headless on the 60 Hz clock —
keystroke → presented **10.25 ± 4.71**, p99 18.53, worst 21.04. Nothing drifts:

| quartile | 1 | 2 | 3 | 4 |
|---|---|---|---|---|
| mean ms | 10.23 | 10.22 | 10.26 | 10.29 |
| worst ms | 18.77 | 18.84 | 18.93 | 21.04 |

172 autosave flushes of the whole document (0.13 ms mean, 0.5 ms worst), JS heap 6,447 KB after,
and the caret placed statically in its own task on all 2,525 of them.

## 13. The deferred render is still correct

`dev/shots/latency/probes/correctness.mjs`, 55k-word manuscript, 3,285 lines, 1440×900 — the same 17
assertions round 3 introduced, re-run against this round's build: `AHEAD` computed from the viewport
(32 lines here), 1,608 lines deferred by one keystroke, caught up in one frame, **0 stale lines
visible immediately, 0 after scrolling 1,195 lines into the pending range mid-flight, 0 anywhere
after the catch-up**, and the mirror's text equal to the textarea's at every step.

## 14. What would move these numbers next, and what would not

* **Not removing more application work at 10k words.** `app_cost` is 1.92 ms against a 5 ms bar; the
  remaining `to_commit` is 0.5 ms of Chromium scheduling plus the frame itself.
* **A 55k-word manuscript still costs 11.7 ms of main thread per keystroke**, and that is where the
  next real win is. Half of it is Chrome's own `<textarea>`; the other half is the mirror, and the
  fix is not rendering line elements the reader cannot see — which cannot be done without a way to
  keep `#mirror`'s height exactly equal to the textarea's, and glyph alignment is the one thing this
  app may never break.
* **`count()` in `app/js/chrome.js`** is still a whole-document scan on every keystroke: 0.91 ms of
  main thread at 10k words, ~5 ms at 55k, idle-scheduled. It was off the critical path when a
  keystroke had 3.7 ms of slack; it has 0.5 ms now.
* **Anything that animates while the writer types costs every keystroke a frame** — that is the
  lesson of §4.2, and it is worth more than any micro-optimisation in this report. The chrome bars'
  opacity fades are the only ones left (37 of 446 samples, at the edges of a burst).
* **A physical panel, a photodiode, and a second machine.** Two of the three terms in §9 are not
  measured here and cannot be on hardware with no display attached.

## 15. Raw

Every run is a file under `dev/shots/latency/`, and every headline table can be recomputed from the
`samples_*_ms` arrays inside them: `r4-appcost.json` (12 regimes × 3 sessions, no cadence),
`r4-headless.json` (the same on a 60 Hz clock), `r4-virtual-1.json` (the compositor, 12 regimes ×
3 sessions), `r4-uinput-1.json` (real keys through the kernel, 5 regimes × 3 sessions),
`r4-caret-before.json` / `r4-caret-after.json` (the caret, four typing speeds),
`r4-size-*.json`, `r4-throttle*.json`, `r4-long.json`, `r4-coldlaunch-*.json`,
`r4-correctness.json`, and `r4-no-display.txt` — the proof that §9's missing term is missing because
there is nothing here to measure it on. Probes are in `dev/shots/latency/probes/`
(`caret-settle.mjs`, `animations.mjs`, `blink-clock.mjs`, `correctness.mjs`, `r4-summary.py`).
Both themes and all three faces were re-shot after the caret change: `r4-check-{light,dark}-{duo,quattro,mono}.png`.
