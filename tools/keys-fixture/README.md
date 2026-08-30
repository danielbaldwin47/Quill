# keys fixture: the caret following the writing

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

## The red fixture

- `broken-typing-16.png`, `broken-typing-38.png` — the same two bursts, on the build *without*
  `45d1434`.

Taken by #138 with `tools/gate keys caret --shots`, on the app as `dba7b74` left it (`b3dc404`, the
tip of main when the condition landed, changes nothing under `quill/` — #141 added this folder and
nothing else). The bar is at **x=675..680 in both of them**, which is the defect entire: 16
characters of ink end at x=1053 and 38 end at x=1581, and the bar has not moved between the two.

## The matched pair

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

`tools/gate check` runs `tools/keys-selftest.mjs` over all six shots on every commit: green on both
fixed pairs, red on the broken one.

Sizes differ between the first pair and the other two, which is why the assertion derives one glyph
advance from each shot rather than carrying a measured constant — see `glyphAdvance` in
`tools/keys-assert.mjs`.
