# The text container is 78 cells, and only headings hang

The editor's text container is the 64-cell measure plus a **7-cell gutter on each side**, 78 cells
centred in the window. Headings hang their markers into the left gutter at level + 1 cells, so
`###### ` reaches the container's own edge; blockquote and list markers do not hang and sit on the
body column. A selection's rows fill this container: a held hard newline runs to its right edge, and
the interior rows of a multi-row selection span it whole. Measured on the Design oracle
(`ref/ia/mac-native/VERDICTS.md` rows 4.1.8–4.1.13 and the found-here table; captures
`09-select-all`, `14-gutters`, `14-blocks`, `10-newline-only`). Decided 2026-08-30 with the owner
under [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md).

## What changed the answer

The Markup Piece was won (#86, #89, #102) hanging every marker — bullets, numbers and a quote's
`>` by the width of the marker run — because the marketing frames show `#` in the margin and the
JavaScript app, in a textarea, could hang nothing at all, so the port had no oracle for the rest and
took the frames' rule as far as it went. The running app hangs headings only, and its container is
not the measure inside a CSS gutter clamp but a fixed 78 cells: the same seven cells give the
deepest heading its room and give a selection its edges.

The selection painter (#108) draws a 0.5 em stub past a held newline and fills each row to its ink
extent because there was no container to fill to. With one, the stub is the wrong shape.

## Considered options

**Keep everything hanging and adopt only the container.** Keeps the won Markup rounds; leaves list
and quote markers in a margin the Design oracle leaves empty, and makes the left gutter carry two
rules.

**Headings only, 78 cells, the fill is the container.** Chosen. One geometry serves the marker
gutter, the selection and the centring, and it is the measured one.

## Consequences

**Markup is re-judged.** The Piece's judged states that show a list or a quote change; those states
name a `mac-native` crop as their opponent (ADR 0015).

**`quill::tags`' list gutters go.** The `list-N` hanging tags and `LIST_CELLS` have no
counterpart. The Design oracle draws no quote rule (`14-blocks`), so none is drawn.

**A heading hangs by the cell the text engine advances, not by the one the container is measured
in.** The two are not the same number here and are on the oracle: the app asks GTK for
`gtk-hint-font-metrics`, so Pango rounds a cell to a whole pixel before it lays anything out, while
the container is counted off the ladder's true cell — 13 px against 12.798 at the default step. A
hang is subtracted from the body column and then handed straight back by the `#`s set after it, so
a hang that is not what Pango will advance puts that heading's words off the column, and the six
levels on to six columns. Round 10 of the Markup Piece was lost to exactly that (#167). So
`Column::hang` is `level + 1` **hinted** cells, and `###### ` lands within four pixels of the
container's left edge rather than on it — outside it where the cell rounds up, inside where it
rounds down. The gutter is still what the deepest heading needs; what it needs is now measured in
the pixels the text is actually set in. A marker a few pixels into undrawn margin is a wobble in the
margin; a split body column is a wobble in the sentence. The underlying disagreement — hinted
metrics make every rendered line wider than the measure it was laid out for, so the 64-cell measure
does not hold 64 characters — is the Typography Piece's, not this one's, and is unresolved.

**`quill-engine::typography::column()` centres 78 cells, not 64 inside a clamp.** Where 78 cells
exceed the window the container is the window and the gutters hold at 7 cells while the measure
shrinks, so `###### ` still hangs and a selection's edges stay the container's; the oracle is
window-limited past its step 7, and which of its two gives was not measured (NOTES § 11).

**The selection painter fills the container.** `caret::NL_TAIL` and the per-row ink extent are
replaced by the container's edges. #150 runs as written: its burst counts painted rows, not their
widths.
