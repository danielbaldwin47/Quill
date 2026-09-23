# #327, round 3: the pause band named, 2026-09-10 UTC

`bursts_and_pauses` scores eleven keys a run at 13.5–16.9 ms while 288 of its
other 289 sit under 4.03 ms — the band [#347's
session](https://github.com/danielbaldwin47/Quill/issues/327#issuecomment-5612599623)
handed to this ticket as its deterministic repro. Round 3 caught it under every
probe on the first attempt and named it end to end:

**The app's own chrome timer paints a frame two to five milliseconds before the
key, and GDK's frame clock will not start another frame until one refresh
interval after the last one was presented, so the key waits out the rest of that
interval before its own frame is even begun.**

`Typing::TITLE_MS` is 1,400 ms (`quill/src/chrome/typing.rs:28`) and the regime's
pause is 1,400 ms (`tools/regimes.mjs:150`). They are the same number, so every
one of the eleven pauses ends with the title bar's return frame landing just
ahead of the key that ends it. Two controls prove it: move the pause to 1,387 ms
and the band is gone; leave the pause at 1,400 ms and suppress the chrome's
return timer and the band is gone.

Nothing was changed in the app, the harness, the budgets or the scoring. What is
left is a Gate policy question, in [§ The question for the
owner](#the-question-for-the-owner).

Round 2's other two findings stand unchanged: the compositor's tail is still the
floor under every worst key, and the isolated 20–39 ms stall recurred here and is
now **split** — [§ revision's 37.77 ms key](#revisions-3777-ms-key-a-3531-ms-layout-phase).

## Build, environment and instrumentation

`ticket-327` at `7ed9c7f`, with round 2's
[instrumentation.patch](../round-2/instrumentation.patch) applied through
`git apply -3` (it predates `26005af`'s pointer-leave controller, so it needs the
three-way apply) and one probe added: a `tail` line whenever `Capture::drain`
asks for a frame, which is what makes the harness's own `TAIL` frame observable
rather than assumed. The whole of it is
[instrumentation.patch](instrumentation.patch) here.

The first instrumented binary was SHA-256
`d004984c2ecf0bcc3af3b285d59c90472ee1539fe2fa1fdd5d6ba33466ce5354`; control B
rebuilt it with the chrome timer suppressed, so the two are not the same binary
and neither is archived. Every instrumented number below is a diagnostic
observation and never acceptance evidence.

The bench launched the binary through round 2's
[quill-wrapper.sh](../round-2/tools/quill-wrapper.sh) (`WAYLAND_DEBUG=1
GDK_DEBUG=frames`, stderr to `wayland-<pid>.log`) with
[cargo-shim.sh](../round-2/tools/cargo-shim.sh) first on `PATH`, exactly as
round 2's § Reproduce and continue says.

Stage: the Gate's own headless output at 3200×2000, scale 2, 60 Hz, VRR off,
GTK's GL renderer, on GTK 4.22.4-1 / Hyprland 0.56.2-1 / Aquamarine 0.14.0-2.
The owner was away and the physical panel was off, which is why the stage's first
client cold-started at 850–920 ms rather than the ~200 ms round 2 saw; ours is
the launch after it and was measured at 164–226 ms throughout.
[clocks.log](clocks.log) holds one `realtime_ns monotonic_ns` pair per attempt,
for putting the wire log's wall-clock stamps on the monotonic scale.

## Every attempt

All figures in milliseconds. Result files are under [results/](results/) rather
than `dev/shots/latency/`; the raw directories are in [records.tar.gz](records.tar.gz).

| UTC | Command | Raw | Mean | Worst | p99 | Cold | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| `132802` | `bench bursts_and_pauses --sessions 2` | `capture/6Bj9zl` | 2.92 | **17.30** | 15.86 | 226.5 | **Fail**, 600 of 600 accounted. The capture. |
| `133514` | control A, `pauseMs: 1387` | `control-a/HxudIN` | — | — | — | 174.8 | Refused, focus lost mid-run. Diagnostic only. |
| `133700` | control A, `pauseMs: 1387` | `control-a/NAsyuB` | 2.38 | 9.92 | **4.07** | 176.3 | Pass, 600 of 600. The band is gone. |
| `133857` | control B, chrome timer off | `control-b/suaiX4` | 2.29 | 13.75 | **3.82** | 182.2 | Pass, 600 of 600. The band is gone. |
| `134207` | `bench syntax --sessions 2` | `syntax/Z2UcMb` | 2.39 | 4.63 | 4.14 | 178.2 | Pass, 600 of 600. |
| `134322` | `bench revision --sessions 2` | `revision/XsyY82` | 3.00 | **37.77** | 4.81 | 185.5 | **Fail**, 600 of 600. The stall, split below. |

Every accounted run saw 614 keys for 600 sent (`revision` 792 for 600); the extra
are the ones the existing join skips. `records.tar.gz` carries each attempt's
`capture-{0,1}.jsonl`, `capture-{0,1}.probe.jsonl`, `joined-{0,1}.json` — the
exact inputs to the unchanged production join — and the capture's three wire logs.

## The band, split

[tools/pause.py](tools/pause.py) on the capture, both sessions. `wait` is the
handler to the frame clock's `before-paint`, `draw` is `before-paint` to
after-paint, `comp` is after-paint to presented; `prevpaint` is how long before
the handler the previous frame was painted.

| Session | Keys | total | wait | draw | comp | prevpaint |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 11 pause-followers | 13.69–16.94 | 12.08–14.35 | 0.41–0.73 | 0.81–1.28 | 0.9–4.0 |
| 1 | 11 pause-followers | 13.92–16.35 | 12.04–14.18 | 0.40–0.76 | 0.70–2.37 | 0.2–3.1 |
| both | the other 289 a session | 1.3–9.4 | 0.36–0.80 | — | — | > 16.7 |

All of it is `wait`. The app's own drawing is half a millisecond and the
compositor is one. Session 0's index 203 is the exception that proves the split
works: `wait` 0.58, `comp` **15.72** — a compositor tail, round 2's finding, not
this one.

[tools/paced.py](tools/paced.py) states the rule over every accounted key of the
capture rather than the eleven, bucketed by `prevpaint`:

| Previous paint was | Keys | wait p50 | wait max | total p50 | total max |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0–2 ms before | 10 | 14.03 | 14.35 | 15.59 | 16.35 |
| 2–5 ms before | 12 | 12.21 | 14.01 | 14.08 | 16.94 |
| 10–16.7 ms before | 1 | 6.53 | 6.53 | 7.76 | 7.76 |
| 16.7–50 ms before | 3 | 0.36 | 0.59 | 2.03 | 4.19 |
| more than 50 ms before | 638 | 0.37 | 0.80 | 1.75 | 9.36 |

Those are the tool's own five rows, unmerged; the 5–10 ms bucket is empty.

The wait is one refresh interval minus however long ago the last frame was
presented, and it is nothing at all once a refresh has passed. That is GDK's
`min_next_frame_time`, and GDK says so itself: the key's frame in the wire log
below is `103: interval=14.9 (sleep) ... predicted=18.4`.

## What paints inside the pause

Every paint in one pause (capture, session 0, key 77), from the burst's last key
at T, read off the probe and confirmed against the wire log:

| After-paint at | Frame | Committed? | What |
| ---: | ---: | --- | --- |
| T+0.76 | 97 | yes | the burst's last key |
| T+500.72 | 98 | yes | the stats bar back — `STATS_MS` |
| T+502.67 | 99 | yes | the recount the stats timer owes, relabelling the bar |
| T+828.84 | 100 | **no** | a frame cycle that committed nothing |
| T+1401.28 | 101 | yes | the title bar back — `TITLE_MS` |
| T+1402.75 | 102 | **no** | a frame cycle that committed nothing |
| T+1405.40 | — | — | **the key**, 2.70 ms after frame 101 was *presented* (whose after-paint is the 4.12 ms row above) |
| T+1418.31 | 103 | yes | the key's frame, one refresh after frame 101 |

`STATS_MS = 500` and `TITLE_MS = 1400` are `quill/src/chrome/typing.rs:25` and
`:28`, armed by `Window::settle` (`quill/src/window.rs:2812`, from
`Typing::resumes_at` at `:2810`). They are the Parity oracle's own `chrome.js` `tA` and `tB`.
The regime's `pauseMs` is 1,400 — the same number — so the title bar's return
frame and the key that ends the pause arrive together, every pause, in both
sessions, by construction.

On the wire (`capture/wayland-416796.log`, local stamps, UTC+4 of the GDK log):

```
13:26:37.509617  -> wl_surface#44.commit()                     frame 101, the title bar
13:26:37.511094  wp_presentation_feedback#69.presented(... 45919 668566617 ...)
13:26:37.513724  wl_keyboard#43.key(80315, 45919670, 32, 1)    the key, 2.70 ms later
13:26:37.526637  -> wl_surface#44.commit()                     frame 103, 12.9 ms after the key
13:26:37.527883  wp_presentation_feedback#70.presented(... 45919 685300700 ...)
```

16.73 ms between the two presentations; 14.03 ms from the handler to the second.
Nothing came in from the compositor between them but an `xdg_wm_base.ping`.

**The frame callback was answered before the key arrived.** The plan asks this
of the last commit before the key, because GDK's Wayland backend will not start
a paint while a frame callback is outstanding, and a headless output with
nothing to render might hold one. It does not: frame 101's callback comes back
as `wl_callback#70.done(45919668)` at `13:26:37.511070`, **2.65 ms before** the
key's `wl_keyboard.key`, and its `wp_presentation_feedback` says `presented`
rather than `discarded` in the same microsecond. GDK was free to paint when the
key landed and chose to wait; the plan's third possible outcome — the backend
waiting on a callback the output withholds — is ruled out here, not deferred.

The
`comp` column above is the whole answer to whether the output has a vblank to
wait for: after-paint to presented is 0.7–2.4 ms whatever the phase, so this
headless output presents on commit and the whole 12–14 ms is GDK pacing the
client. On a real 60 Hz panel the same pacing would land on real vblanks and the
writer would wait the same 14 ms — the number is not an artefact, but it is not
the keystroke path either.

## The two controls

Both diagnostic, both uninstrumented in the sense that matters — the probes do
not touch the mechanism — and neither committed as a result. Reverted before
anything was measured for the record.

**Control A — the pause moved off `TITLE_MS`.** `tools/regimes.mjs:150`
`pauseMs: 1400` → `1387`, nothing else. The eleven pause-followers become
1.76–2.66 ms with a `wait` of 0.28–0.63 ms, and the last paint before each of
them is 887 ms earlier (the stats bar's, at T+500) instead of 3 ms earlier. p99
15.86 → **4.07**. The title bar's frame now lands *after* the key and pays the
pacing itself, where nobody is scored.

**Control B — the chrome's return timer suppressed.** `Window::settle`'s wake
timer never armed, `pauseMs` back at 1,400. Zero paints inside the pause: for
eight of the eleven the previous paint is the burst's own last key, 1,403 ms
earlier. The eleven become 1.37–3.37 ms with a `wait` of 0.25–0.62 ms. p99
15.86 → **3.82**.

## What it is not

- **Not the harness's `TAIL` frame.** The `tail` probe fires whenever
  `Capture::drain` asks for a frame. Across all eighteen probe files of the six
  instrumented attempts — every session's and every warmup's — it fired **zero**
  times. The
  burst's last key is drained about 30 ms after it, on the 100 ms timer, because
  its frame's timings are already complete by then; no stamp ever reaches the
  250 ms `TAIL`. The first suspect of the round-3 plan is eliminated.
- **Not an app source that draws nothing new.** The two frames that committed
  nothing (T+828.8, T+1402.7) are 570 ms and 2.7 ms from the key; the one 2.7 ms
  away had no effect, because the frame that paced the key is the one before it,
  which drew the title bar.
- **Not the caret.** `--deterministic` is on every bench launch
  (`tools/bench.mjs:333`) and `Caret::wants_tick` is false outside `Mode::Live`
  (#347), so the blink never asks for a frame. Confirmed here: no caret stage
  probe fired in any pause.
- **Not the Syntax worker.** `bursts_and_pauses` launches `--syntax off`, and no
  `worker-debounce` stage fired.
- **Not the compositor.** `comp` is 0.7–2.4 ms on every one of the eleven.

## revision's 37.77 ms key: a 35.31 ms layout phase

The isolated 20–39 ms stall recurred, on the first instrumented `revision` run,
and the probes split it. Session 0, keys 29 and 30 — `Shift` and `Left`, GDK
keycodes 50 and 113 — shared frame 42:

```
wait 1.06   draw 35.35   comp 0.86
  +1.06 ms  phase before       frame 42
  +1.10 ms  phase layout       frame 42
 +36.41 ms  phase paint-start  frame 42
 +36.41 ms  after-paint        frame 42
 +36.43 ms  busy               35.38 ms
```

35.31 ms inside the frame clock's **layout** phase, with the main loop's own
busy span covering exactly it. This is not pacing and not the compositor: it is
GTK re-laying-out inside the key's own frame. It is the shape of PR #325's
20.01 ms and 24.23 ms keys and of round 2's 38.82 ms key — one key, early in the
run, everything after the handler — but where those were unsplit, this one is
now on one phase.

It is not deterministic: session 1 typed the identical keycodes at the identical
indices with no key over 3 ms of draw, and the uninstrumented `revision` runs in
the same hour scored 4.57 ms worst. Two of 1,200 keys, in one of two sessions.
Naming the line inside the layout phase needs a probe this round did not have —
that is round 4, and it is the one thing standing between this ticket and its
original failures.

## What was changed, and why the fix this outcome names was not made

Nothing. No production code, no harness, no timestamp acceptance, no budget, no
scoring, no regime. The round-3 tooling under [tools/](tools/) reads archives and
runs nothing.

This is the round-3 plan's **outcome two** — "an app source paints inside the
pause and that paint paces the key" — and that outcome's fix is "fix that source
(a status tick that draws nothing new should not `queue_draw`, and so on)". The
parenthesis is the case this is not. The source here draws something new: at
T+1,400 the title bar goes from `TITLE_FADED` to full strength, which is a
visible change and has to reach the glass. Nor can the app hold the frame back
for the key: at T+1,400 nothing in the process knows a key is coming at
T+1,405 — the key has not left `/dev/uinput` — so any rule that deferred the
chrome's return until after it would have to be clairvoyant, and any rule that
deferred it by a fixed amount would only move the collision to a different pause
length.

What is left is `TITLE_MS` itself. 1,400 ms is the Parity oracle's own
`chrome.js` `tB`, and moving it is a design change to what a writer sees, which
belongs to the owner and to `docs/design.md` rather than to a latency ticket.
Round 2 settled the same question the same way for the chrome's opacity
transition: "it is what a writer sees, and the Gate's question is what the
writer's keys cost, not what the owner's mouse costs."

So the fix outcome two names is unavailable, and what remains is the Gate's own
fixture. That is the question below.

**Noticed in passing, not this ticket.** A `--sessions 2` result reports
`accounting.keys_sharing_a_frame: 300` where the same regime at `--sessions 1`
reports `0` — exactly one session's worth. Each session's frame counter starts
again at its own launch, so pooling two sessions makes every frame number occur
twice. The per-key numbers are unaffected (each key keeps its own handler and
presentation stamps, and `every_keystroke_accounted_for` is `true`), but the
field is counting launches rather than keys. It is in the production join, not
in anything this round changed.

## The question for the owner

`bursts_and_pauses`'s `pauseMs` is `TITLE_MS`, so eleven of its three hundred
keys measure the chrome's return colliding with the refresh grid rather than the
keystroke path, and the regime's worst key is decided by which side of a refresh
the collision falls on — 14.87 and 15.91 ms on the two idle runs below, 16.74 ms
on #347's run, 17.30 ms under probes. Either **(a)** the pause stays at 1,400 ms,
and the Gate keeps holding Quill to a 16 ms worst-key budget on a number that is
honest about a 60 Hz writer but says nothing about typing — it will pass and fail
at random on an unchanged build, as it already has four times; or **(b)** the
pause moves off 1,400 ms (control A used 1,387), and those eleven keys measure
what the other 289 measure — p99 4.07 ms — at the cost of the Gate no longer
measuring the collision at all, which is a real thing a writer pays, though never
on eleven keys in three hundred. Nothing else about the regime, the budget, the
scoring or the accounting changes either way. It is a Gate policy change, so it
is the owner's.

## Final uninstrumented measurements

Probes reverted (`git checkout HEAD -- quill tools`), the wrapper and shim
deleted, the release binary rebuilt by the bench itself
(`app.sha256 33f3f75dc03b1dd3`, `git_head 7ed9c7f`). The fingerprints say
`tree_was_dirty: true`: at the time of these runs the only modifications in the
worktree were the untracked files of this directory, and `git status` showed no
tracked file changed.

| Result | Mean | Worst | p99 | Cold | Verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| `dev/shots/latency/bench-bursts_and_pauses-20260910T134735.json` (`--sessions 2`) | 2.72 | 15.91 | 15.04 | 179.8 | pass |
| `dev/shots/latency/bench-syntax-20260910T134817.json` | 2.34 | 4.72 | 4.28 | 164.4 | pass |
| `dev/shots/latency/summary-20260910T135606.json` (`--all`) | — | — | — | — | **14 of 14 scored regimes pass** |

Inside that `--all`: `bursts_and_pauses` 2.64 mean / **14.87** worst / 14.06 p99,
`revision` 2.91 / 4.57, `syntax` 2.37 / 4.40, `markdown_syntax` 3.18 / 7.42,
every other scored regime under 7.6 ms worst, `saturation_stress` informational
at 24.67 mean. Every regime accounted for every key; no focus loss, no pointer
leave. `tools/gate judge latency`: **ours, round 7**
(`dev/progress/rounds/latency-r7.json`).

`RUST_TEST_THREADS=1 tools/gate check`: **pass**, all fourteen steps, on the
reverted tree.

The band is in that run's samples as plainly as anywhere: indices 25, 50, 75 …
275 read 14.81, 13.33, 13.42, 13.98, 13.60, 14.76, 14.06, 13.95, 14.87, 13.84,
14.00. Of the other 289, 288 read 1.34–4.03; the one exception is index 277 at
12.00 ms, which is not a pause-follower and is the sort of isolated key § revision's
37.77 ms key is about.

## Reproduce and continue

```sh
mkdir -p /tmp/r3 && tar xzf dev/progress/diagnostics/ticket-327/round-3/records.tar.gz -C /tmp/r3
R=dev/progress/diagnostics/ticket-327/round-3
python3 $R/tools/pause.py /tmp/r3/capture/quill-gate-6Bj9zl --session 0
python3 $R/tools/pause.py /tmp/r3/capture/quill-gate-6Bj9zl --session 0 --gap 1000 --events
python3 $R/tools/paced.py /tmp/r3/capture/quill-gate-6Bj9zl --session all
python3 $R/tools/worst.py /tmp/r3/revision/quill-gate-XsyY82 --session 0 --top 2 --window 50
```

The three read archives and nothing else; what they share — a session's capture
and probe files, the wait/draw/comp split, and one observation rendered as a
line — is [tools/evidence.py](tools/evidence.py) beside them, and a mistyped flag
is an error rather than a silently different measurement.

Round 2's [timeline.py](../round-2/tools/timeline.py) and
[split.py](../round-2/tools/split.py) read these archives too; `clocks.log`'s
first line belongs to the capture, and `timeline.py` prints an offset check that
says whether the line matches the log.

To capture again: apply [instrumentation.patch](instrumentation.patch) to an
isolated checkout with `git apply -3`, build, move `target/release/quill` to
`quill-real`, install [quill-wrapper.sh](../round-2/tools/quill-wrapper.sh) as
`target/release/quill`, put [cargo-shim.sh](../round-2/tools/cargo-shim.sh) first
on `PATH` as `cargo` — both hardcode the worktree path — and run
`QUILL_PROBE_ARCHIVE=/absolute/dir tools/gate bench <regime> --sessions 2`.

Round 4, if the owner wants the last stall named, is a probe inside the layout
phase on `revision`: `worst.py` says which frame and `busy` says the main loop
was in it, but nothing here says which widget's measure or allocate spent the
35 ms.
