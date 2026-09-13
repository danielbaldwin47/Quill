# The text container is 78 cells, and only headings hang

The editor's text container is the 64-cell measure plus a **7-cell gutter on each side**, 78 cells
centred in the window. Headings hang their markers into the left gutter at level + 1 cells, so
`###### ` reaches the container's own edge — to within the pixel the last Consequence below is
about, which is why the gutter is seven cells and not six; blockquote and list markers do not hang
out into it and sit on the body column instead. A selection's rows fill this container: a held hard newline runs to its right edge, and
the interior rows of a multi-row selection span it whole. Measured on the Design oracle
(`ref/ia/mac-native/VERDICTS.md` rows 4.1.8–4.1.13 and the found-here table; captures
`09-select-all`, `14-gutters`, `14-blocks`, `10-newline-only`). Decided 2026-08-30 with the owner
under [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md).

*Stands as decided on 2026-08-31, with the selection Consequence below now landed by
[#168](https://github.com/danielbaldwin47/Quill/issues/168): `Editor::selection` reads the
container's edges off `quill_engine::typography::Column`, and `caret::NL_TAIL` and `caret::tail`
are gone rather than pending. The two shapes no judged still shows are typed instead — the
`selection-container-wide` and `selection-newline-to-edge` assertions in `tools/keys-assert.mjs`.
Nothing else here moves.*

*Narrowed on 2026-09-09 by
[#241](https://github.com/danielbaldwin47/Quill/issues/241): "only headings hang" is only headings
hang **out**. Four `mac-native-19-*-wrapped-markers` captures show that a wrapped list item's and a
wrapped quote's continuation rows hang **in**, under the item's own first word, by that paragraph's
own marker run — `- ` and `> ` two cells past the body column, `123. ` five — and that a wrapped
quote carries no second `>` (`ref/ia/mac-native/CAPTURE-2026-09-09.md` § #241, `NOTES.md` § State
14, `docs/design.md` row What hangs). No first row moves: every marker still sits where this ADR
put it, the gutter is still seven cells and still sized by `###### `, no quote rule is drawn, and
the measured-not-counted rule below now covers a marker run as well as a heading's. Nothing else
here moves.*

*Waits on [#419](https://github.com/danielbaldwin47/Quill/issues/419) and
[#420](https://github.com/danielbaldwin47/Quill/issues/420) since 2026-09-12, by
[#344](https://github.com/danielbaldwin47/Quill/issues/344): the `column()` Consequence below
stands on `main`, but the question it left is measured. The oracle's type gives first, on the
window's width alone, and the measure gives only once the limit no longer fits (`NOTES.md` § State
22). `docs/design.md` row Window limitation holds that rule. Nothing else here moves.*

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

**A heading hangs by what the layout advances its markers, measured — not by `level + 1` cells
counted again.** The two are not the same number, and which of them is right changes under the app.
Pango rounds a glyph's advance to a whole pixel when font metrics are hinted and does not when they
are not, and Quill is both: `--deterministic` pins hinting on in `quill::harness::determine`, and a
writer's own launch takes whatever the desktop and the display's scale settle on. At the default
step that is 13 px a cell in a judged shot against the ladder's true 12.798 in the app.

A hang is subtracted from the body column and then handed straight back by the `#`s set after it, so
a hang that is not what the layout will advance puts that heading's words off the column and the six
levels on to as many columns. **Round 10 of the Markup Piece was lost to exactly that** (#167), and
counting the cell either way would only have moved which of the two builds was wrong — the judged
shot or the shipped app. So `quill::tags::hang_markers` is handed the six widths that
`Editor::marker_advance` measured off the layout, and a `quill::tags::Measure` to ask a list item's
or a quote's own marker run for its width the same way, and hangs by what it is given — #241 put a
wrapped item's hang under the same rule, and a marker run counted off the cell would land its
continuation rows on the wrong column for the same reason a heading's did. `typography::Column::hang` is
the same rule in the ladder's own cell and stays what the seven-cell gutter is sized from: the gutter
is designed once, and the type is laid out per launch.

Measuring is the best each build can do, and it is not the same answer for both. Hinted, the advance
is already a whole pixel and the six levels land on one column **exactly** — 0.00 px of spread
across the ladder, which is what the Design oracle measures too. Unhinted it is fractional, and a
GTK `left-margin` is an integer, so the rounding leaves up to half a logical pixel — one device pixel
at scale 2 — and no integer margin can beat that. A writer's own launch therefore still has the six
heading words inside one device pixel of each other rather than on one column. That is the floor,
not a defect left in.

The disagreement underneath is left open: hinting a cell to a whole pixel makes a rendered line wider
than the measure it was laid out for, so the 64-cell measure does not hold 64 characters, and the
container and the text are counted in two different cells. That is the Typography Piece's, not this
one's, and wants a ticket.

**`quill-engine::typography::column()` centres 78 cells, not 64 inside a clamp.** Where 78 cells
exceed the window the container is the window and the gutters hold at 7 cells while the measure
shrinks, so `###### ` still hangs and a selection's edges stay the container's; the oracle is
window-limited past its step 7, and which of its two gives was not measured (NOTES § 11).

**The selection painter fills the container.** `caret::NL_TAIL` and the per-row ink extent were
replaced by the container's edges in #168: `Editor::selection` measures only the anchor and the
focus off glyphs and takes every edge between them from `Column`. #150 runs as written — its burst
counts painted rows, not their widths — so the two shapes it cannot see are two assertions beside
it, which is why the same burst now carries `selection-container-wide` and a fourth burst holds a
newline for `selection-newline-to-edge`.
