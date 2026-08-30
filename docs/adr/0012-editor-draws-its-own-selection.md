# The Editor draws its own selection

The Editor paints the selection itself — the fill, a bar at each end, and the caret — in the same
`snapshot_layer` pass beneath the glyphs, all four boxes cut from one function (`Editor::band`) so
that their rows are the same rows. GTK's `selection` sub-node keeps one stylesheet rule, which clears
its `background-color` and holds `color` at the page's ink so the desktop theme cannot take the ink
when it loses the ground. This supersedes one sentence of [ADR 0004](0004-gtktextview-editor.md):
"GTK draws selection beneath the glyphs already, so the hand-drawn selection rectangles go away",
and with it the claim that the caret is the one genuinely custom piece. The rest of 0004 — the
subclass, its considered options, its five consequences — stands. Decided in
[#108](https://github.com/danielbaldwin47/Quill/issues/108) and
[#140](https://github.com/danielbaldwin47/Quill/issues/140).

The owner asked for iA Writer's Windows look on the selection: a bar at each end, each matching the
selection's height exactly. Neither is reachable while GTK paints the ground. Its selection is its
own box on its own rows, and nothing lines it up with a band the Editor cuts, so a bar drawn beside
it is a second geometry disagreeing with the first by a pixel somewhere. Drawing the fill too is
what lets fill, bars and caret register: measured on the judged shot, the fill's rows and the bars'
rows are the same rows.

## Considered options

Keeping GTK's selection and drawing only the bars beside it fails on registration: GTK's selection
is its own rounded box on its own rows, its stylesheet sets its colour and nothing about its rows,
and the bars would be cut from the Editor's band, so the two disagree by a pixel somewhere on every
row. macOS iA adds iOS-style round knobs to its two bars; the owner declined them. Bars inset
inside the fill, as iA's own Windows capture has them
(`ref/ia/shots/msstore-win-04-dark-style-check-selection.png`, where the two bars and the fill share
a column span exactly), were built and lost round 4: a bar is 0.155 em and a glyph's left side
bearing at 20 px is less, so an inset bar covers the bearing whole and lands on the stem of the letter
it holds, and the critic read bar and letter as one smear. The bars bracket from outside, which is
the Parity oracle's own geometry (`setEdge(edgeA, firstEdge, -M.w)` in `legacy/app/js/caret.js`).

## Consequences

**The band is one device pixel lower than the constant alone gives.** Against ink that is 37 device
pixels ascender-to-descender in both our shot and the oracle's, a browser puts the baseline
15.25 logical pixels below the top of the ink and Pango puts it 14.75. `ABOVE_BASELINE` stays the
oracle's 11/16 and the half pixel is named for what it is (`BASELINE_DRIFT`), so the band sits
19 device pixels above the ascender and 16 below the descender, which is iA's and the oracle's.

**Bars at both ends are iA's, not a departure from it.** #38 § Problem Statement records that a
gauntlet critic named the oracle's end bars as "its one gap", and the port dropped them on that
sentence. Both of iA's own captures have two bars — `msstore-win-04` (Windows) and `appstore-mac-04`
(macOS, with the knobs) — so whatever that critic meant, two bars is the reference, and the sentence
is not grounds for dropping them again.

**The caret is hidden while a selection stands.** The two ends are the instrument, and a third bar
blinking inside the held cells reads as a second cursor; `place()` in `caret.js` hides its caret for
the same reason. The caret asks the frame clock for nothing until the selection is released.

**The selection is clipped to the viewport**, as the oracle clips it, with a cap on rows drawn: a
select-all scrolled deep into a Document draws a screenful of boxes, not the Document's.

**Selection colours are a paint of the focus flag**, `selection_paint(focused)`: the fill and the
ends both swap when the window loses focus, the ends going to ink rather than a paler accent. The
colours come from the colour table (`Role::Selection`, `Role::SelectionIdle`), which is why #110 has
only the scheme switch left to do.
