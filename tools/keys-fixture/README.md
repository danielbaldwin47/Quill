# keys fixture: the caret following the writing, and the selection reaching the foot

Two defects, two pairs. The first six shots are #108's — the bar that never followed a keystroke —
and the last two are #146's, the bottom row of a Document the selection left unpainted. Both were
invisible to every judged state, which is why both are held by pixels here rather than by a critic.

## The caret following the writing

Two shots of the native build **with** `45d1434` ("caret: the bar follows the writing, not only
the mark that was moved") applied, taken on the Gate's stage (`tools/harness.mjs`, 1440×900 at
scale 2) with real keys through `tools/uinput-keys.py`, Live mode, an empty document:

- `fixed-typing-16.png` — after `dfdfsdfsdfsdfsdf` (16 characters).
- `fixed-typing-38.png` — after `fefefefefefsfesfesfesf` on top of it (38 characters).

In both, the caret's bar stands just right of the last glyph's ink, and between the two it has
moved right. Before `45d1434` the bar was placed from `mark-set`, which GTK does not emit for the
insert mark carried along by an insertion, so both shots had the bar at **x=0**; every judged state
was a still, so the Piece won three states with that defect in it (#108, #138).

These are the **green fixture** for `tools/gate keys` and its selftest. They were taken before the
condition existed, at a glyph advance of 38.4 device pixels — a larger setting than the script now
opens — which is why there is a matched pair below as well.

### The red fixture

- `broken-typing-16.png`, `broken-typing-38.png` — the same two bursts, on the build *without*
  `45d1434`.

Taken by #138 with `tools/gate keys caret --shots`, on the app as `dba7b74` left it (`b3dc404`, the
tip of main when the condition landed, changes nothing under `quill/` — #141 added this folder and
nothing else). The bar is at **x=675..680 in both of them**, which is the defect entire: 16
characters of ink end at x=1053 and 38 end at x=1581, and the bar has not moved between the two.

### The matched pair

- `fixed-size20-typing-16.png`, `fixed-size20-typing-38.png` — the build *with* `45d1434`, at the
  judged default size 20 the script actually opens.

Taken the same way, in the same run of the same command, so that the red pair and this one differ in
the build and in nothing else. Side by side they are the whole ticket in four numbers:

| | ink | bar after 16 | bar after 38 |
|---|---|---|---|
| with `45d1434` | 674..1053, 674..1581 | 1059 | 1587 |
| without it | 674..1053, 674..1581 | 675 | 675 |

Same ink, same derived advance (25.3 and 24.5 device pixels); the bar is the only thing that moves,
and on the broken build it does not move at all.

Sizes differ between the first pair and the other two, which is why the assertion derives one glyph
advance from each shot rather than carrying a measured constant — see `glyphAdvance` in
`tools/keys-assert.mjs`.

## The selection reaching the foot of a Document

- `fixed-select-all.png` — the build **with** `27f5a21` ("caret: the last row of a Document is a
  row the selection reaches").
- `broken-select-all.png` — the same build with that one commit reverted and nothing else changed.

Both are the third burst of the caret's keys script: on top of the two typing bursts it presses
Enter, types `ffff`, presses Enter, types `ssssssssss` and then `Control+a`, leaving a Document of
three rows — 38, 4 and 10 characters — with all of it selected and the caret out (ADR 0014).

Taken by #150, in one worktree at `7113df0`, with `tools/gate keys caret --shots`: the green shot
first, then the revert applied with `git apply -R`, then the red shot, then the revert undone. Same
window, same type, same keys, one commit apart.

| | row 1 | row 2 | row 3 |
|---|---|---|---|
| with `27f5a21` | y 150..223, x 622..1616 | y 224..297, x 622..744 | y 298..371, x 622..877 |
| without it | y 150..223, x 622..1616 | y 224..297, x 622..744 | — |

The bottom row is the whole difference, and it is the whole defect: `Editor::selection` read GTK's
display-line moves for whether they had moved, which is not what they answer, so the last row of a
Document measured no width and drew no fill. Every row above it painted, and the judged `selection`
state is `--select 153,171` in the middle of `ref/sample.md` — six rounds and eighteen critics saw
nothing.

The assertion counts row bands rather than looking at the bottom one, so a walk that starts a row
late fails the same way a walk that stops a row early does. The bands touch — 223 then 224, 297
then 298 — so what tells one from the next is the column its fill ends at, which is why the script's
three rows are of three different lengths.

## What runs over them

`tools/gate check` runs `tools/keys-selftest.mjs` over all eight shots on every commit, with no
display attached: green on the two fixed caret pairs and on `fixed-select-all`, red on
`broken-typing-*` and on `broken-select-all`.
