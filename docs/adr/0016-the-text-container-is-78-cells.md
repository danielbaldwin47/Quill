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

**`quill-engine::typography::column()` centres 78 cells, not 64 inside a clamp.** At text sizes
where 78 cells exceed the window the column is window-limited, as the oracle's is past its step 7.

**The selection painter fills the container.** `caret::NL_TAIL` and the per-row ink extent are
replaced by the container's edges; #150's keys burst to the foot asserts the new shape.
