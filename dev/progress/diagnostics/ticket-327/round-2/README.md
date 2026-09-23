# #327, round 2: two over-budget keys captured and split, 2026-09-10 UTC

Fourteen instrumented `tools/gate bench syntax` attempts on the merged spec
branch (`5627ac0`, which carries `main` at `1b852e0`): eleven accounted for all
300 keys, three were refused on focus loss, and two of the eleven went over the
16 ms worst-key budget with every key accounted for — 38.82 ms and 17.97 ms.
Both are captured across the whole handler-to-presentation interval, on the
app's side by frame-clock phase probes and a main-loop tracer, and on the wire
by the client's own Wayland log. The owner was at the machine throughout and
took focus twice.

What the observations establish, in order of strength:

1. **The 17.97 ms key is explained end to end.** Its handler ran at +0.0 ms;
   GDK's frame clock did not start the frame until +16.27 ms; drawing took
   0.42 ms and the compositor presented 0.97 ms after the commit. The wait is
   the frame clock pacing: the pointer left the window 17.7 ms before the key
   (the owner's mouse), the chrome answered with its CSS `opacity` transition
   (`quill/src/chrome.rs`, `.chrome { transition: opacity ... }`), and GDK ran
   two animation cycles (frames 152 and 153, update phase only, no commit).
   `gdkframeclockidle.c` then sets `min_next_frame_time` to the next slot of
   the refresh grid after every cycle, so the key, landing 0.4 ms after frame
   153, was painted a whole refresh later. GDK's own `GDK_DEBUG=frames` line
   for the key's frame says `interval=16.7 (sleep)`. The compositor was not
   involved: presentation feedback arrived 0.97 ms after the commit.
2. **The compositor's tail is the floor under every worst key.** Across the
   eleven accounted runs, the after-paint-to-presented stage has a median of
   0.6–1.1 ms, a p99 of 4.3–6.6 ms and a maximum of 6.1–12.65 ms, while the
   app's own stages stay under 1 ms at p99 (`split.py`). Presentation
   feedback is real on this Hyprland: every frame got `presented`, none was
   `discarded`, and no accepted timestamp was later revised or in the future
   for a scored key. The headless output's frame timer was idle at every slow
   key (the previous frame was ~89 ms earlier), so the tail is time Hyprland
   spends before it renders the stage: it is single-threaded, the owner's
   4K panel was live, and the GPU is shared. This is environmental; nothing
   in the app is on that path.
3. **The 38.82 ms key is client-side and not yet named.** Its handler finished
   by +0.43 ms (the text-input commit is the last thing the key handler
   sends); the frame's `wl_surface.frame` request went out at +37.09 ms and
   presentation came 1.41 ms after the commit. No Wayland message arrived or
   left in between, no frame-clock cycle ran in the 89 ms before it, and none
   of the Syntax stages (submit, drain, retag) fired inside the interval. This
   run predates the phase probes and the main-loop tracer, so it cannot say
   whether the paint idle was late or the paint itself was slow. In the eight
   accounted runs made with those probes, no main-loop span longer than 4.2 ms
   coincided with a scored key, and no key's drawing took more than 4.1 ms.

The original 20.01 ms and 24.23 ms failures on PR #325 remain unexplained by
name; both fit "one refresh of pacing plus the compositor's tail", which is the
shape of the 17.97 ms key, and neither retained the observations to say so.

## Build, environment and instrumentation

Installed: GTK `4.22.4-1`, Hyprland `0.56.2`, Aquamarine `0.14.0-2`. The Gate
stage was `3200×2000` at scale 2, 60 Hz, `render:new_render_scheduling` off in
Hyprland (read with `hyprctl getoption`). The owner's own output `DP-3` was
`3840×2160` at scale 1.6 and in use.

The temporary changes are [instrumentation.patch](instrumentation.patch),
which adds to round 1's probes: a mark at every frame-clock phase (flush,
before-paint, update, layout, paint, resume), a mark when the main loop next
goes idle after a paint, a poll-function tracer that records every span between
two polls longer than a millisecond, and stage probes on every main-loop source
the app owns (the session's file-watch drain, the worker debounce, autosave,
save, the status tick, the fade's begin, settle and ticks, the caret tick).
The bench launched the binary through [tools/quill-wrapper.sh](tools/quill-wrapper.sh),
which keeps the pid, sets `WAYLAND_DEBUG=1 GDK_DEBUG=frames` and writes the
app's stderr to `wayland-<pid>.log`; [tools/cargo-shim.sh](tools/cargo-shim.sh)
put the wrapper back after the bench's own `cargo build --release` uplifted the
binary over it. The first attempt ran the real binary (SHA-256
`a5b00189212939b74b42911cf151981c2404445beb99a36f2743ff06428b4679`) before the
wrapper was in place; every later result's `fingerprint.app.sha256` is the
wrapper script's hash, not the instrumented binary's, and the instrumented
binaries were rebuilt twice as probes were added and not archived. All
instrumented numbers are diagnostic observations, never acceptance evidence.

[clocks.log](clocks.log) holds one `realtime_ns monotonic_ns` pair read before
each attempt; the wire log's wall-clock stamps are put on the monotonic scale
with it, and the offset is checked against every compositor stamp in the log
(receipt minus stamp is 0.02–2.7 ms, never negative).

## Every attempt

Each command was `tools/gate bench syntax` with `QUILL_PROBE_ARCHIVE` set; the
result files are under [results/](results/) rather than `dev/shots/latency/`. All
figures in milliseconds; the raw directory is the one in [records.tar.gz](records.tar.gz).

| UTC stamp | Raw | Mean | Worst | Cold | Outcome |
| --- | --- | ---: | ---: | ---: | --- |
| `021140` | `s9QPvX` | — | — | 166.3 | Refused, focus lost after 25 keys (the owner). |
| `021258` | `pSvSzl` | 2.43 | **38.82** | 159.4 | Fail, all 300 accounted; wire log, no phase probes. |
| `021913` | `XjgRYX` | 2.28 | 7.34 | 167.1 | Pass, all 300 accounted. |
| `022029` | `9LwPGS` | 2.19 | 7.44 | 170.7 | Pass, all 300 accounted. |
| `022109` | `P7y0Z2` | 2.71 | 8.62 | 166.6 | Pass, all 300 accounted. |
| `022124` | `7YrJMU` | — | — | 158.8 | Refused, focus lost before the first key (the owner). |
| `022252` | `QTmEu9` | 2.60 | 10.73 | 170.7 | Pass, all 300 accounted. |
| `022649` | `tjmAvP` | — | — | 161.6 | Refused, focus lost after 100 sent, 2 aligned (the owner). |
| `022736` | `qUWyCn` | 2.70 | 8.88 | 179.9 | Pass, all 300 accounted; first run with the source probes. |
| `023017` | `1JLZcQ` | 2.73 | **17.97** | 168.1 | Fail, all 300 accounted; every probe on. |
| `023336` | `hdKR1i` | 2.52 | 9.02 | 160.3 | Pass, all 300 accounted. |
| `023513` | `2MhlPe` | 2.72 | 10.51 | 173.9 | Pass, all 300 accounted. |
| `023604` | `6IIEom` | 2.67 | 8.43 | 161.2 | Pass, all 300 accounted. |
| `023957` | `B9LGeQ` | 2.41 | 6.37 | 169.4 | Pass, all 300 accounted. |

Every accounted run saw 307 keys for 300 sent; the seven extra are the ones the
existing join skips. `records.tar.gz` carries each attempt's `capture-0.jsonl`,
`capture-0.probe.jsonl` and `joined-0.json` (the exact inputs to the unchanged
production join), and the two over-budget runs' wire logs.

## The two over-budget keys

Times are milliseconds from the key's handler; `tools/timeline.py` prints them.

**38.82 ms, `pSvSzl`, key 31, frame 129** (uinput 38.82, handler 38.56):

```
 -0.036  <- wl_keyboard.key press
 +0.098  retag 0.004 ms                      (the only app stage in the interval)
 +0.43   -> zwp_text_input_v3.commit         (the key handler's last request)
+37.09   -> wl_surface.frame, wp_presentation.feedback, offset; Mesa's queue dispatch
+37.157  after-paint (frame 129)
+37.277  main loop idle
+39.18   <- frame callback done(5476147); presented at +38.563
```

Nothing crossed the wire between +0.43 and +37.09. The previous frame was at
−89.4 ms, so the frame clock was not pacing (its `min_next_frame_time` was long
past), and no pointer event had happened for 7.5 s. Presentation followed the
commit by 1.4 ms.

**17.97 ms, `1JLZcQ`, key 59, frame 154** (uinput 17.97, handler 17.66):

```
-17.666  <- wl_pointer.leave                 (the owner's mouse)
-17.573  before-paint frame 152, update, layout; after-paint; no commit
 -0.420  before-paint frame 153, update; after-paint; no commit
 -0.041  <- wl_keyboard.key press
 +0.119  retag 0.005 ms; settle-fade 0.000 ms
+16.268  flush-events; before-paint frame 154       (the frame clock's next grid slot)
+16.690  after-paint; commit
+17.702  <- frame callback; presented at +17.664
   gdk frame 154: interval=16.7 (sleep) paint_start=0.0 frame_end=0.4 present=18.1
```

The `(sleep)` is GDK's own `slept_before`: the paint idle was scheduled with a
delay to `min_next_frame_time`, which `gdk_frame_clock_paint_idle` sets after
every cycle to the next refresh-grid slot while phases are still being
requested. Frames 152 and 153 were update-phase cycles with nothing to draw:
a CSS transition ticking. The only transition the app declares is the chrome's
`opacity`, and the pointer leaving is what changes the chrome's state.

## Every key, split into three stages

`tools/split.py` puts each accounted key into wait (handler to the frame clock's
before-paint), draw (before-paint to after-paint, which is GTK's layout,
snapshot, GL render and commit) and comp (after-paint to presented). Over the
eight runs with phase probes:

| Stage | p50 | p99 | Max |
| --- | ---: | ---: | ---: |
| wait | 0.5 | 0.9–1.1 | 1.2–1.4, and the 16.27 above |
| draw | 0.3 | 0.6–0.9 | 3.8–4.1 (frames that also ran a layout phase) |
| comp | 0.6–1.1 | 4.3–6.6 | 6.1–12.65 |

The app's own stages over all runs: drain at most 3.0 ms (mean 0.6), submit
0.28, retag 0.34, and the rest under 0.05 ms. No drain, submit, save or
autosave fell inside either over-budget interval.

## What was changed, and why it is not a manufactured pass

The pointer leaving the window mid-run is now a refusal, alongside focus loss:
`quill/src/harness.rs` prints `pointer left the window at <us> us` on stdout
from a capture-phase motion controller under `--measure`, `tools/bench-join.mjs`
reads it (`pointerLeft`, with a selftest), and `tools/bench.mjs` refuses the
regime, stops a run of several there, records `the_pointer_left_the_window` in
the result and `regimes_the_pointer_left` in the summary. The signal is the
owner's mouse, observed before any number is known, and it refuses a run that
would have passed exactly as it refuses one that would have failed; the keys
stay accounted for and no timestamp, budget or scoring changed. The chrome's
transition itself is untouched: it is what a writer sees, and the Gate's
question is what the writer's keys cost, not what the owner's mouse costs.

Nothing was changed for the compositor's tail or for the 38.82 ms stall,
because neither is attributed to a line of the app's code.

## Final uninstrumented measurements

Probes removed (`git apply --reverse instrumentation.patch`), the wrapper and
shim deleted, the release binary rebuilt by the bench itself, the owner still at
the machine:

| Result | Mean | Worst | p99 | Cold | Verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| `dev/shots/latency/bench-syntax-20260910T024503.json` | 2.63 | 6.60 | 6.04 | 159.75 | pass |
| `dev/shots/latency/bench-prose_end_of_draft-20260910T024538.json` | 2.31 | 8.19 | 7.04 | 164.70 | pass |

Both accounted for 300 of 300 keys. The full `tools/gate bench --all` was not
run: it takes about twenty-five minutes and the owner took focus three times in
forty; it is the owner's to run when the machine is idle.

## Reproduce and continue

```sh
mkdir -p /tmp/r2 && tar xzf dev/progress/diagnostics/ticket-327/round-2/records.tar.gz -C /tmp/r2
R=dev/progress/diagnostics/ticket-327/round-2
python3 $R/tools/split.py /tmp/r2/records/quill-gate-*
python3 $R/tools/timeline.py /tmp/r2/records/quill-gate-1JLZcQ \
  /tmp/r2/records/quill-gate-1JLZcQ/wayland-165036.log "$(sed -n 10p $R/clocks.log)" 12 100 3
python3 $R/tools/pointer.py /tmp/r2/records/quill-gate-1JLZcQ \
  /tmp/r2/records/quill-gate-1JLZcQ/wayland-165036.log "$(sed -n 10p $R/clocks.log)" 7
```

`clocks.log` line 2 belongs to `pSvSzl` and line 10 to `1JLZcQ`; the offset
check at the top of `timeline.py`'s output says whether the line matches the
log. To repeat capture, apply `instrumentation.patch` to an isolated checkout,
build, move `target/release/quill` to `quill-real`, install the wrapper as
`target/release/quill`, put `cargo-shim.sh` first on `PATH` as `cargo`, and run
`QUILL_PROBE_ARCHIVE=/absolute/dir tools/gate bench syntax`. A recurrence of
the 38.82 ms shape under these probes will say whether the paint idle was late
(a `busy` span before `before`) or the paint was slow (`before` to after-paint),
and `GDK_DEBUG=frames` will show it as `layout_start`/`paint_start`/`frame_end`.
