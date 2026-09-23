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
state is `--select 153,171` in the middle of `dev/ref/sample.md` — six rounds and eighteen critics saw
nothing.

The assertion counts row bands rather than looking at the bottom one, so a walk that starts a row
late fails the same way a walk that stops a row early does. The bands touch — 223 then 224, 297
then 298 — so what tells one from the next is the column its fill ends at, which is why the script's
three rows are of three different lengths.

## The selection filling the container

- `fill-select-all.png` — the same third burst again, on the build that fills the **container**
  rather than each row's ink (#168). `fixed-select-all.png` above is its red half: that build is
  the one whose rows stopped at their own last glyph.
- `fill-newline-held.png` — the fourth burst, added by #168. It types over the standing selection,
  leaving a Document of `aa`, `bbbb` and the empty row a trailing newline opens, and presses
  `Control+a`. The selection then ends past a **newline** rather than past a glyph, which is the
  one shape no still in `dev/shots/oracle/` shows.

Taken by #168 with `tools/gate keys caret --shots`, on the same window and the same type as the
pair above, so the three bands stand at the same three heights and only their reach along each row
has moved.

| | row 1 | row 2 | row 3 |
|---|---|---|---|
| filling the ink | y 150..223, x 622..1616 | y 224..297, x 622..744 | y 298..371, x 622..877 |
| filling the container | y 150..223, x **622..2437** | y 224..297, x **442..2437** | y 298..371, x **442**..877 |
| the newline held | y 150..223, x 622..2437 | y 224..297, x 442..2437 | — |

The container is x 442 … 2438 of a 2880 px view — 442 either side of it — and that is what the two
assertions read: the first row starts at the anchor and the last stops at the focus, and every row
the selection runs past reaches the container's right edge, every row it entered from above the
container's left. `442 + 2437 + 1 = 2880` is the whole of the arithmetic. A row-band count cannot
see any of this, which is why `selection-rows` passes on both builds.

`fill-newline-held.png` has two bands and not three: the empty row the trailing newline opens is
inside the selection and holds none of it, so it is a fill of no width and nothing is painted for
it.

## The Spell check mark withheld until the space

- `spell-typing.png` — after the spell script's first burst: ` comittee` typed at the end of
  `dev/ref/spell.md` under `--spell on`, the caret still after the word.
- `spell-space.png` — after its second: one space, which releases the word.

Taken by #414 with `tools/gate keys spell --shots` on the build with #409's caret rule, and cut by
`cropPng` in `tools/crop.mjs` to [560, 500, 360, 150] of the 2880×1800 pages.

| | bar | word's ink | spell Role under the word |
|---|---|---|---|
| after ` comittee` | x 837..842, y 536..609 | x 626..836 | none |
| after the space | x 863..868, y 536..609 | x 626..836 | 544 px, y 590..593 |

Page coordinates; the crop subtracts 560 and 500. The row above ends in `mispelled`, whose own mark
sits at y 517..519, above the bar's rows, which is why `spell-mark` reads the bar's rows and the
typed word's columns and never the paragraph.

## What runs over them

`tools/gate check` runs `tools/keys-selftest.mjs` over all fourteen shots on every commit, with no
display attached: green on the two fixed caret pairs, on `fixed-select-all` for the row count and
on `fill-*` for the container, red on `broken-typing-*`, on `broken-select-all`, and on
`fixed-select-all` for the container it does not fill; each `spell-*` crop green on its own
burst's expectation and red on the other's.

## Enter on a Palette settings row

Three shots for `chrome`'s Palette script (#479), whose rules are `palette-up` and
`switch-flipped` in `tools/keys-assert.mjs`:

- `palette-query.png`, `palette-enter.png` — one run of `tools/gate keys chrome --shots` on the
  spec-467 branch (#475's Palette rows), the Palette's two bursts: `ask` typed into the Palette
  opened by `--menu palette` on an empty Document with the bars off, which lists the one settings
  row Always ask where to save with its switch off; then a newline, which is Enter, after which the
  switch is on and the Palette still up. The knob's centre goes from x 1816 to 1852.
- `palette-closed.png` — `palette-query.png` with the panel and its shadow (x 938..1942,
  y 209..496) painted over in the page's paper, `#f7f7f7`: the page a Palette that closed on Enter
  leaves, derived rather than taken because the build under test never closes it.

The red pairs are the knob that did not move (`palette-query` against itself) and the Palette gone
(`palette-query` against `palette-closed`).
