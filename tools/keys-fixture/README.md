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

These are the **green fixture** for `tools/gate keys` and its selftest.

## The red fixture

- `broken-typing-16.png`, `broken-typing-38.png` — the same two bursts, on the build *without*
  `45d1434`.

Taken by #138 with `tools/gate keys caret --shots`, on the app as `dba7b74` left it (`b3dc404`, the
tip of main when the condition landed, changes nothing under `quill/` — #141 added this folder and
nothing else). The bar is at **x=675..680 in both of them**, which is the defect entire: 16
characters of ink end at x=1053 and 38 end at x=1581, and the bar has not moved between the two.
`tools/gate check` runs `tools/keys-selftest.mjs` over all four of these shots on every commit, and
it is red on this pair.

The two builds are not the same size on the glass, and that is worth knowing before reading the
numbers side by side: the green shots were taken at a glyph advance of 38.4 device pixels and the
red ones at the judged default size 20, which draws 24.0. So the assertion derives one advance from
each shot rather than carrying a measured constant — see `glyphAdvance` in `tools/keys-assert.mjs`.
