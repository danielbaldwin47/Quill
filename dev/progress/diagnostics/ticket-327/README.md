# #327: bounded latency investigation, 2026-09-08 UTC

Round 2, 2026-09-10, captured two over-budget keys with every probe on and
attributed one of them: [round-2/README.md](round-2/README.md).

The Syntax worst-key failure remains unresolved. Two fully accounted diagnostic
runs did not reproduce an over-budget key. A third lost focus, ending window
testing. No production fix or acceptance change is justified by this pass, and
#327 stays open. These observations do not clear the original 20.01 ms and
24.23 ms failures retained on PR #325.

## Build and environment

The worktree starts at PR #325 head `d295ed941633d835a03e47da18ccdb305f1718ce`
(`spec-310-syntax`). The uninstrumented baseline binary hash prefix was
`e494c8369057d7c1`. The diagnostic binary SHA-256 was
`43fbaa1c5ba8539fae80f85649038139e7058936d247cfdeb6c513bb914347b1`.
The exact temporary changes are [instrumentation.patch](instrumentation.patch).
All four patched production files were restored afterward.

Installed packages: GTK `1:4.22.4-1`, Aquamarine `0.14.0-2`, Hyprland `0.56.2-1`.
The Gate stage was 3200×2000, scale 2, 60 Hz, variable refresh off, with GTK's
OpenGL renderer and a 1440×900 logical window. Each retained result contains its
environment fingerprint. The diagnostic runs followed an eight-second successful
`python3 tools/idle-check.py 8` check; this did not guarantee later inactivity.

## Every attempt

Each command was `tools/gate bench syntax`; diagnostic commands additionally set
`QUILL_PROBE_ARCHIVE=/home/diggle/.local/state/quill/ticket-327/diagnostic`.
All results below are milliseconds. Instrumented passes are diagnostic observations,
never replacement acceptance evidence.

| UTC result stamp | Build | Mean | Worst | Cold | Accounting and outcome |
| --- | --- | ---: | ---: | ---: | --- |
| `20260908T011220` | Uninstrumented | — | — | 170.73 | Refused: 300 sent, 310 seen, 31 aligned; focus-loss flag false. |
| `20260908T011506` | Instrumented | 2.58 | 6.14 | 163.23 | 300 sent, 307 seen, 300 presented; seven extra events skipped by the existing join. |
| `20260908T011608` | Instrumented | 2.25 | 13.26 | 167.42 | 300 sent, 307 seen, 300 presented; seven extra events skipped by the existing join. |
| `20260908T011629` | Instrumented | — | — | — | Refused after focus loss: 50 sent, 47 seen, 45 aligned of 300 planned. |

The first refusal's cause is unclassified. Its misaligned subtraction results are
not usable latency measurements. The two refused result/capture pairs and logs
remain outside git, as the Gate requires, under
`/home/diggle/.local/state/quill/ticket-327/refused/`. The last attempt's raw probe
directory is `/home/diggle/.local/state/quill/ticket-327/diagnostic/quill-gate-WJ21Kf/`.
These local paths are durable on this machine but are not portable evidence.

## What the observations establish

The two completed runs' raw captures, exact inputs to the unchanged production
join, result fingerprints, and probe sidecars are in [records.tar.gz](records.tar.gz).
Their directory names are `quill-gate-rliPve` and `quill-gate-JcJdlB`, respectively.
[observations.json](observations.json) retains their maximum scored keys and
timestamp observations, recomputed by [analyze.mjs](analyze.mjs).

The 13.262622 ms maximum is scored key 215, carrying frame 313. Its handler
timestamp is `2925012686`, after-paint is `2925013487`, and presentation is
`2925025587`, all monotonic microseconds. Thus handler to after-paint took
0.801 ms, and after-paint to recorded presentation took 12.100 ms. Only one
instrumented stage overlapped the whole handler-to-presentation interval:
retagging from `2925012777` to `2925012780` (3 microseconds). No instrumented
Syntax submission or result application overlapped that interval. This is a
passing key with post-paint variation, not an explanation of the historical failures.

Each completed run also accepted one future **cold-start** timestamp. In the first,
frame 0 was complete when observed at `2840117688`, with presentation timestamp
`2840134306`: 16.618 ms in the future. The same retained timings object was read
36 more times through `2840617723`; its timestamp never changed. The second
run's cold timestamp was 15.615 ms ahead of observation. Neither completed run
contained a future per-key acceptance or an observed timestamp revision.
Future acceptance is established for these cold frames; a revision race causing
an offending Syntax key is not established. The probes do not observe Wayland
flushes, compositor receipt, or scheduling, so they cannot attribute the remaining
post-paint interval to any of those stages.

The probe buffers complete global Editor submit, drain, and retag intervals,
including early returns and work after paint. It records after-paint frame identity,
bounds around acceptance reads, and subsequent reads of cloned selected timings
objects for at least 500 ms. That retention period is diagnostic only: it does not
delay acceptance or modify a timestamp. The original key capture and scoring are
unchanged. Probe overhead can affect timing; all instrumented results remain diagnostic.

## Reproduce and continue

Extract `records.tar.gz` into an empty directory, then run from this repository root:

```sh
node dev/progress/diagnostics/ticket-327/analyze.mjs /path/to/extracted-records
```

The output reproduces `observations.json` using the production `align`, `latencyMs`,
and `handlerMs` functions. To repeat capture, use an isolated checkout of `d295ed9`,
apply `instrumentation.patch`, create an absolute archive directory, and run:

```sh
QUILL_PROBE_ARCHIVE=/absolute/archive tools/gate bench syntax
```

Retain every attempt, stop if focus is lost, and remove the patch with
`git apply --reverse instrumentation.patch` before acceptance measurements.
An offending, fully accounted key with observation bounds and later same-frame
reads is still missing. If its global Syntax intervals cannot explain its delay,
the next diagnostic pass needs submission/compositor timestamps to distinguish
the remaining hypotheses. No debounce change, optimization, timing correction,
or regression test for a guessed cause was added.

Final uninstrumented Syntax/headline measurements and the full latency Gate were
not run after focus loss. There is no latency verdict or ticket-close claim from
this investigation.

## Validation

`RUST_TEST_THREADS=1 tools/gate check` passed after removing the probes, including
the complete display-free Rust test suite and all Gate self-tests. Serial Rust
tests follow PR #325's existing portal-test workaround. The targeted
`node tools/bench-selftest.mjs` also passed. Recomputing the observations through
the production join produced byte-identical `observations.json`, including replay
from the committed archive. The release binary was rebuilt after probe removal
and matched the original baseline SHA-256:
`e494c8369057d7c19254f6d1e6c641a49c5db439367c01782c78c27b70e178a9`.

Independent Standards and Spec reviews found no issues in the bounded diagnosis
deliverable. Both reviewers independently reproduced the archived observations.
The Spec review explicitly leaves reproduction, causal attribution, a supported
fix with regression coverage, and final latency acceptance unresolved.
