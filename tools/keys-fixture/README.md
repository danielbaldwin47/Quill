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

These are the **green fixture** for `tools/gate keys` and its selftest. The red fixture — the same
two shots of the build *without* `45d1434`, bar at x=0 — is taken when the condition lands, from a
checkout at `dba7b74` (main before #108's fix merged), with the same keys on the same stage.
