# The Gate

What must be true before native Quill work lands. The owner does not read Rust: the Gate, not code
review, keeps quality honest, so every check here ends in evidence the owner can read without
opening the code. Decided in
[Gate spec: the checks every native change must pass](https://github.com/danielbaldwin47/Quill/issues/12);
the build spec is the `ready-for-agent` issue that resolution links.

The Gate has three tiers, keyed to what is landing. Every ticket names the Pieces it touches;
"Pieces: none" is a valid answer and must be written.

## Commit tier: every commit

`tools/gate check` is green: `cargo fmt`, applied — the step line names any file it changed, and the
change is the session's to commit with the rest — `cargo clippy --all-targets -- -D warnings` on the
default lint set, `cargo test` with no display attached, and — checking this repository rather than
a directory handed to it — every `tools/*-selftest.mjs`, none of which needs a window:
`tools/bench-selftest.mjs`, which holds the join `bench` turns two files into a number with;
`tools/keys-selftest.mjs`, which holds the assertion `keys` reads a typed shot with and runs it over
the two builds committed as pixels under `tools/keys-fixture/`; `tools/oracle-selftest.mjs` and
`tools/judge-selftest.mjs`, which hold the two halves of a blind pair — which states are shot and
frozen, and which opponent each is paired with; `tools/ticket-selftest.mjs`, which holds
`tools/ticket --graph` to a captured spec; and `tools/usage-selftest.mjs`, which holds
`tools/gate --help` to the subcommands the script dispatches and the endings their scripts print,
and this file to naming each one. A test that needs a window is harness, not test; it belongs under
`tools/gate judge`, `bench` or `keys`. An `#[allow(...)]` carries a one-line reason on the same
line; clippy's `pedantic` group stays off. A float becomes an integer only in the helpers
`tools/gate` lists (`round`); every `ADR NNNN`, `NOTES §`, `VERDICTS §` and `design.md row` citation
in `docs/design.md`, `dev/progress/state.json` and `dev/shots/oracle/states.json` resolves (`cite`); and
those two JSON files are Node's `JSON.stringify(v, null, 2)` byte for byte (`json`), so a scripted
edit is a small diff. There is no coverage number: each spec's Testing Decisions names the seams its
tests hold. A green run is its step lines and that last line and nothing else: each step's own
output waits in `target/gate/<step>.log` under the directory being checked, and is printed only for
the step that fails, above the line the owner reads.

Done when: `tools/gate check` prints its final `pass` line.

## Ticket tier: before a ticket closes

For every Piece the ticket names, the evidence below is committed and its summary lines are pasted
on the ticket.

### As run

For a judged Piece, whose brief and judged states are read first and in one call,
`tools/gate brief <piece>` (§ Blind judging):

1. `tools/gate shoot <piece>`, foreground, about a minute: the shots to look at, and one line per
   state saying whether its pixels moved since the last round. A state that has not moved costs the
   judge no critic. Several Pieces are one call — `tools/gate shoot --pieces a,b` or `--all` — on
   one stage: a shell loop opens and removes a headless output per Piece and hung Hyprland on
   2026-09-02.
2. `tools/gate judge <piece> --note "..."` in the background (`run_in_background`, output to a file
   under the job's tmp directory), then end the turn: every critic runs at once, so the run is one
   critic's five or six minutes plus the shots, and a run with more than one critic still outlasts a
   foreground call's ten-minute cap. `tools/gate bench` is the opposite: forty seconds, foreground,
   because a background run loses its keys. A hook, `.claude/hooks/gate-run-mode-guard.sh`, refuses
   the other arrangement of either. The display is one lock, whose path and wait `tools/harness.mjs`
   names (`LOCK`, `LOCK_WAIT_S`), held by the harness for the stage's life: a second `shoot`,
   `judge`, `keys` or `bench` waits its turn and says whose turn it is, and refuses once the wait is
   up; a judge releases it before its critics run.
3. The last line of the output file, and its per-state lines above it — the winner, the margin,
   `carried from round N` and the gap — go on the ticket; the JSON's `pick` is a letter, and the
   line is the reading. `gate keys <piece>` and `gate bench` follow when the ticket names them.

### Latency

**Latency** (any ticket naming the latency Piece; every ticket touching the Editor's keystroke path
names it). `tools/gate bench` runs the headline regime, `prose_end_of_draft` at 133 wpm on the
10,062-word `dev/shots/latency/doc10k.md`, with real keys through `/dev/uinput` and presentation from
`GdkFrameTimings` on the dedicated scale-2 headless output. Hard budget, uinput `write(2)` →
presented: **≤ 5 ms mean, ≤ 16 ms worst**. Cold start, `exec` → first complete frame with a non-zero
presentation time, the same document open: **≤ 250 ms**. `--all` runs all nineteen regimes of
`tools/regimes.mjs` instead — each on its own launch, at its own caret, Focus and pace — and
`--regimes a,b` runs only the subset named; both print one line per regime and write one summary
beside the per-regime results, and — when every regime ran and accounted for its keys — a last line
that passes only when every scored regime cleared the budget. A regime that pauses pauses clear of
the app's own idle timers: a pause ending within one refresh interval after a timer the app arms
from the last keystroke gives that timer's frame the refresh slot the key needs, so the key measures
the chrome's return rather than typing, and the regime then passes or fails on which side of a
refresh the collision lands on an unchanged build
([#349](https://github.com/danielbaldwin47/Quill/issues/349)); `PAUSE_MS` in `tools/regimes.mjs`
carries the value with the timers it clears, and `tools/bench-selftest.mjs` reads those timers out
of the app and holds it to them. Seventeen regimes are scored, including the headline regime's own
typing with Live on (`live_end_of_draft`), with the rendered page open in Split (`preview`), with
every Syntax highlight Category on (`syntax`), with Style check on and every List (`style`), with
Spell check on against the fixture dictionary (`spell`), and with all three of those Annotators on
at once (`annotators`).
`saturation_stress` is unpaced, so two keys land in every 16.7 ms frame and a per-keystroke figure
grows by construction; it runs, is recorded and must account for its keys like the rest, but its
lines say informational rather than pass or fail, as the oracle's own report kept it out of the
table. A run that lost focus or could not account for a regime's keystrokes stops at that regime: it
has no run-level verdict to give and refuses instead, and writes the keys it sent and the stamps the
app wrote beside that regime's result as `bench-<regime>-<stamp>.capture.json`, so the refusal can
be read key by key; a refused run's files are not committed. The pointer leaving the window mid-run
is refused the same way, with the keys accounted for: the app says so on stdout, it is the owner's
mouse crossing the stage, and from the leave on the chrome's opacity fade has GDK's frame clock
pacing every key's frame to the refresh grid — a whole refresh of waiting on the key that lands just
after a frame, which is the shape of #327's 17.97 ms. `--panel` measures on the physical panel
instead of the headless output, which is the one number a headless output cannot give: it takes
workspace 5 for the run and puts back what was there, refuses before it shows anything unless
`tools/idle-check.py` says nobody is at the machine and a panel is awake, and is informational
rather than a Gate condition — the panel is fractional-scale, so every line it prints says so, its
results are marked informational inside and named `bench-panel-<regime>-<stamp>.json`, a run of
several also writes `panel-summary-<stamp>.json`, and `judge latency` refuses one it is handed.
A launch paints frames of its own for 2.65–2.68 s after `exec` (#495's series), and a key inside the same refresh as
one waits a whole refresh (16.25 ms on an unchanged build, #489), so `--measure` says on stdout when
that work is over, in two lines ([#495](https://github.com/danielbaldwin47/Quill/issues/495)).
`launch settled` is the window's own launch work done (`Window::launching`: the caret's reveal, the
Annotators' first pass, the Preview's first layout); the warm-up starts there, because a key typed
during the Annotators' first pass throws the pass away and it is redone on key 0. `launch quiet` is
GTK's overlay scrollbar indicator hidden too; the warm-up is topped up with letter-and-Backspace
pairs until it, and a regime whose first measured key still went before it is refused. Work a
change arms at launch that paints — a timer, a pass, a first render — goes into `launching`, or it
reads as a spike in the first measured keys and nowhere else. One uinput keyboard types the whole
run, made while the stage warms.
Results land in `dev/shots/latency/` with the environment fingerprint `tools/fingerprint.mjs` records, and the run prints the summary lines that go on the ticket and nothing else. The bench runs
ours `--deterministic` (`tools/bench.mjs`), so the caret's blink and glide never run in a latency
measurement, and no animation is a bench number's cause (#345 was filed on the blink and falsified
by this line).

**The latency verdict** is arithmetic rather than a critic. `tools/gate judge latency` reads a whole
`--all` summary — the newest, or the one `--summary` names — and gives the Piece to ours when the
headline regime's mean is under the Parity oracle's 12.17 ms uinput → presented, the cold start
under its 370 ms, and every scored regime is inside the budget; `saturation_stress` is written into
the round with its numbers and decides nothing. The oracle's own run is `dev/progress/latency-report.md`
(2.43 ms mean, 15.61 ms worst, 370 ms cold), measured once and left as it was measured. The round is
`dev/progress/rounds/latency-r<N>.json` in the shape a blind round has, carrying `opponent` and `build`,
its one pair the summary against that report; the line and the exit codes are the ones below, and a
Piece once won is never lost here either. A `--regimes` subset is a measurement and not a verdict:
it is refused, as is a run that could not account for every keystroke, and one the pointer left the
window during (`regimes_the_pointer_left` in the summary).

### Blind judging

**Blind judging** (every other Piece). `tools/gate judge <piece>` shoots ours at the Piece's judged
states (identical theme, font, size, focus, caret, passage; 1440×900 at scale 2, captured with
`grim -T` per `docs/research/native-harness.md` on `research/native-harness`; font metrics hinted by
`harness::determine()`, so Mono sets 26.0 device px per cell where the shipped app and a
`mac-native` frame both set the ladder's 25.600 — a gap naming the advance or a line's width against
a `mac-native` opponent is the stage's, not the Piece's, the way `spell` lost round 7 on #416),
pairs each shot with the opponent's through `tools/blind.mjs`, runs one fresh-context critic per pair (`tools/critic.md`,
the gauntlet's critic prompt, Opus at high effort) — every state shot first, then every critic at
once — reveals, and writes `dev/progress/rounds/<piece>-r<N>.json` in the existing shape plus
`opponent`, `build` (the commit and the binary's hash) and `states` (each state's shots and their
hashes, pick, margin and gaps). **The same pixels are not judged twice**: a state whose shot of ours
and whose opponent hash to what an earlier round's critic was shown (a round from before the hashes
is compared with its files) carries that round's verdict
forward — the latest such round — marked `carried` with the round it came from, and spends no
critic; `--fresh` asks a critic about every state anyway, which is how the owner re-rolls a verdict
that looks wrong. `tools/gate shoot <piece>` takes the same shots with no critic, into
`target/gate/shoot/<piece>/`, and says per state whether the pixels moved — for looking before a
judge, and never evidence. A change to `tools/critic.md` or the critic's effort is tried first with
`node tools/critic-replay.mjs <piece>`, which puts the last rounds' committed pairs to the candidate
critic and counts its agreement with the verdicts on disk. The Piece is ours only when every one of
its states is. It ends in `gate judge <piece>: ours|theirs, round <N>`, and exits **0** for ours,
**1** for theirs, **2** for theirs when this Piece had been won before, and **3** when nothing was
judged — a command line it could not read, a Piece with no judged states, a state naming a flag the
app has not got, an opponent that is not frozen, a state naming an assertion this build cannot run,
a shot of an active caret-drawing state whose capture came back with no accent pixel in it — ours
had not painted itself active by the shutter, and the frame carries the ghost caret rather than the
bar — or a run that broke. That line, with the line above it that names the round which won a Piece
now lost, ends stdout; above them, one line per state names the winner, the margin and the round it
was carried from, and one the winner's gap, so a round is read from the output rather than from the
letters in its JSON. The trail to them is `target/gate/judge-<piece>.log`, said on stderr as well
only when the run refuses, because a round that reached a verdict has every state's detail in its
JSON. The judged states are `dev/shots/oracle/states.json`: `defaults`, then `pieces.<piece>.<state>` as
overrides, a state carrying `opponent` (a `mac-native` crop) or `assert` instead of the Parity pair,
and `keys.<piece>` the typed scripts; the Pieces' briefs and verdicts are `dev/progress/state.json`,
`pieces` a list of `{ id, title, what, judge }` — latency carries no `judge` — and `log` a list of
`{ at, msg }`. The opponent is the **Parity oracle**: the frozen shots under
`dev/shots/oracle/<piece>/<state>.png`, taken of the JavaScript app at that Piece's judged states. The
app is retired (`docs/architecture.md` § Repo migration), so the shots stand for good: `tools/gate
oracle <piece>` reads them `unchanged` while the states, passages and fixtures under them hold, and
fails a state that moves, which then takes a `mac-native` crop or an `assert`. A
state that follows a `docs/design.md` row has left the Parity oracle behind and names its own
opponent instead: `opponent` in `states.json` gives a capture under `dev/ref/ia/shots/mac-native/` — the
**Design oracle**, iA Writer for Mac as measured in `dev/ref/ia/mac-native/`
([ADR 0015](../adr/0015-the-design-oracle-outranks-the-parity-oracle.md)) — a rectangle in it, and
the matching rectangle in ours, and the pair is those two crops rather than the two whole windows.
Ours is shot in Mono at the type the `defaults` name for such a state, so the two grids compare cell
for cell, and `tools/gate oracle` passes it over: its opponent is committed under `dev/ref/ia/`, not
frozen. Ours wins at any margin. A state that **neither** oracle can
arbitrate names `assert` in `states.json` instead of an opponent, and is measured rather than shown
to anybody ([ADR 0017](../adr/0017-a-judged-state-neither-oracle-can-arbitrate.md)):
`tools/assert-state.mjs` reads the rule off ours' own pixels, the round records what it measured
where a critic's reasoning would be, and the state is won or lost on that answer like any other. The
rules are that file's `ASSERTIONS` — ghost, folded, split, full, pdf-split, pdf-full, dialog, syntax,
outline, spell, pinned, menu, settings and palette-control, as `tools/gate --help` lists them — and `states.json`'s `assert` note says what
each reads. The
bar for it is that neither oracle holds the subject — `caret/unfocused`, because iA draws no caret
on a deactivated window and the Parity oracle draws one at a column `docs/design.md` has overruled, and the
two `preview` states, because the Parity oracle has no rendered page at all and iA Writer for Mac's own
preview is that app's design rather than this one's
([#263](https://github.com/danielbaldwin47/Quill/issues/263) § Out of Scope puts Preview outside ADR
0015's reach) — and not that a round was lost. Four Pieces are asserted-only, every state naming
`assert` and none shown to a critic: `live`, `preview`, `export` and `settings`. `tools/gate oracle` passes such a state over, as it
does one carrying `opponent`. **A Piece once won is never lost**: a ticket naming a won Piece
re-judges it, and a loss blocks the ticket. Won means won natively — the rounds carrying `opponent`;
the gauntlet's r1 verdicts are the JavaScript app's against iA Writer, which is the reference era
rather than a native win. The two files a judge reads — this one and `dev/progress/state.json`, whose
`pieces[]` entry `{ id, title, what, judge }` carries the Piece's brief as the one string `judge` —
are read through `tools/gate brief <piece>`, never by `grep`: their briefs are single lines, and a
two-hit grep returned 25k characters (2026-09-11).

### Keys

**Keys** (any Piece with a keys script). `tools/gate keys <piece>` is the one condition that types.
It builds ours and opens it Live — not `--deterministic`, because the caret machine is what a typed
condition tests — on a headless output of the Gate's own, takes keyboard focus and proves it by
read-back, writes the Piece's scripted bursts through `/dev/uinput` with focus asked again between
every chunk as `bench` has it, and after each burst reads the Piece's assertions off the pixels. The
scripts are the `keys` entries of `dev/shots/oracle/states.json`, beside the judged states; most Pieces
carry none, and a ticket naming one that does runs it. An entry may be a list of scripts, each on
its own launch: `chrome` types the stats bar's and the Palette's, where Enter on a settings row
flips its switch with the Palette still up, on a fresh copy of the `settings` file the script
names so the writer's own is never written. Arithmetic rather than a critic, and no
oracle: it ends in `gate keys <piece>: pass` or
`gate keys <piece>: fail (<the assertion, measured against what it expected>)`, exit **0** and
**1**, and **3** with `refused (...)` when nothing was typed or nothing could be read — no script
for that Piece, no compositor, a binary that would not build, or focus taken away mid-run. **A Piece
is not won while its keys script fails.** The assertion reads the glass and never asks the app: on
#108 the app believed its own caret was at x=0, so a probe would have agreed with the defect in good
faith. That is the whole reason this condition exists — every other one here is a still or a clock,
and #108's bar was placed from a signal GTK does not emit for typing, so three judged states, two
rounds and six fresh critics all read correct while the bar never followed a keystroke. The
assertion itself is a pure function of a decoded PNG, so `tools/gate check` runs it on every commit
over the fixed and broken builds committed as pixels under `tools/keys-fixture/`.

Done when: every named Piece has a committed verdict, bench result or `keys` line from this ticket's
build, and the summary lines are on the ticket.

## Feature tier: before a feature ticket closes

The owner hand-tests the feature from the installed package, built and installed by the commands in
`README.md` § Install — written out in full in the Hand test, because
the owner runs it from that comment alone — then the feature's Hand test checklist. The checklist
lives in the feature spec, in this shape:

- Numbered "do X, see Y" steps, at most ten, each naming its concept with `CONTEXT.md` vocabulary.
- Every step observable in the running app; nothing that needs a terminal or a log.

The ported Pieces' checklists are `docs/agents/hand-tests.md`, one per Piece.

Done when: the owner comments `hand test: pass` with the `pacman -Q quill-writer` output on the feature
ticket. That comment closes the ticket.

Before a release, additionally: every scored latency regime clears the budget and
`saturation_stress` is recorded beside them (`tools/gate bench --all`), and every Piece's latest
verdict is ours.
