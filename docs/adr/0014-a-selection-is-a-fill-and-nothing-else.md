# A selection is a fill and nothing else

*Confirmed against iA Writer for Mac running, on evidence independent of the four captures it was
decided from — [`dev/ref/ia/mac-native/VERDICTS.md`](../../dev/ref/ia/mac-native/VERDICTS.md) § ADR 0014.
Fill-only selections at four shapes in both themes, no bar or handle at either end, the caret out
for as long as a selection stands, `#113d52` and `#00bfff` exact. One row is contradicted in colour:
0014.9's "paler band" for a window that has lost focus is a neutral grey — `#464646` on dark,
`#dcdcdc` on light — with no blue in it. The swap is still the fill's alone.*

*Overtaken in two places by [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md).
§ Consequences' "a lost round on this Piece reads as the decision it is": ADR 0015 gives the caret
states a `mac-native` opponent (the Gate's per-state key is #161), so a round is winnable and a
loss is a loss again. And its "wider question is open, not answered here" is answered there —
`docs/design.md` decides the writing surface row by row.*

*Narrowed on 2026-09-09 by [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md):
§ Considered options' "one bar, at the active end only" is what iA Writer for **Windows** showed
under Wine, which is not an oracle, so "a real iA Writer" and "the two builds genuinely differ"
stand as unmeasured until a native Windows capture says so. The option was rejected on the owner's
ask and nothing here moves.*

A selection is drawn as its fill rows and nothing else: no bar at either end, and the caret stays
out for as long as it stands. This supersedes the "a bar at each end" of
[ADR 0012](0012-editor-draws-its-own-selection.md) — its opening sentence, its
§ Consequences paragraph "Bars at both ends are iA's, not a departure from it", and the two-colour
`selection_paint` of the paragraph after it — and it narrows
[ADR 0013](0013-caret-on-the-advance-boundary.md), which put the free caret *and* the two end bars
on the advance boundary, to the free caret alone. 0013's boundary, its paint order and its
measurements are otherwise untouched. Decided from the owner's captures of iA Writer running
natively on macOS and the owner's ask that Quill's caret be that app's.

## What changed the answer

Every "iA brackets a selection with two bars" reading in this repo came from a marketing still. ADR
0013 disposed of the Windows one: `msstore-win-04` is a style-check marker, not a selection. The
macOS one is `appstore-mac-04`, and it does not show two bars either. Measured, its accent runs are
**44 device pixels wide** at x 500–543 and x 807–850, sitting above-left and below-right of the
fill: round touch grab-handles. A caret bar at that capture's type size is 7–9 px. `REFERENCE.md`
§ 1 catalogued them honestly all along — "text selection with iOS-style blue handles" — and ADR
0012 read them as the Mac app's selection anyway. macOS has never drawn those handles.

The owner captured the app running, on 2026-08-30. The four frames are
`dev/ref/ia/shots/owner-mac-0{1..4}`, and they are unanimous:

| capture | measured |
|---|---|
| caret mid-word | one bar, `#00bfff`, 6 × 63 device pixels |
| caret at a line's end | one bar, the same colour and the same 6 × 63 |
| selection inside a row | fill `#113d52`, 478 × 60 px — and **not one pixel of `#00bfff` in the frame** |
| selection over three rows, through a heading and a quote block | one contiguous fill band — and again **no accent pixel anywhere** |

That last column is the whole finding, and it needed no grid and no cell pitch: a bar is the
brightest thing in any frame that holds one, so a frame with no accent pixel in it holds no bar.
Both selection frames also hold no caret, which is the same measurement.

The caret's own numbers agree with what ours already draws — 63 px against a 60 px band is the
band-is-the-pitch rule inside the quantisation of a 6-px bar — so nothing about the free caret moves
here.

## Considered options

**Keeping both ends and hiding only the caret** is what shipped, and it is the shape
[#147](https://github.com/danielbaldwin47/Quill/issues/147) was opened against. It survives every
correction ADR 0013 made and still draws two marks the reference app does not draw.

**One bar, at the active end only.** This is what iA Writer for Windows does — ADR 0013's table
measured it directly: one bar, at whichever end the caret is at, and none at the other. It is a
real option and it is a real iA Writer. It loses on the owner's ask, which names the Mac app; and
the two builds genuinely differ here, which is [#154](https://github.com/danielbaldwin47/Quill/issues/154).

**Keeping the ends for the sake of the active end.** With no mark at all, a selection growing under
Shift+Right says which cells are held but not which end is moving. That is a real loss and it is
named under § Consequences rather than argued away: the Mac app accepts it, and the fill's own
moving edge is what it accepts instead.

## Consequences

**The active end of a growing selection is no longer marked.** Extending with Shift+arrows shows the
band change size, and nothing says which edge moved. This is the one thing the end bars did that the
fill does not, and it is given up deliberately.

**The `caret` Piece leaves the Parity oracle further.** ADR 0013 already left it on the boundary and
the paint order; this drops two boxes the oracle draws. The `selection` judged state changes again,
so the Piece is re-judged and re-frozen once — and a blind round against the oracle can only be won
on grounds the oracle is being asked to differ on. The owner's ask is the tiebreak the round cannot
supply. Recorded here so that a lost round on this Piece reads as the decision it is.

**The idle swap is the fill's alone.** `selection_paint` becomes `selection_fill`, `Role::Accent`
leaves the selection's paint, and `IDLE_ENDS` (0.22 of the ink, from
`#caret-layer.idle .sel-edge` in `dev/legacy/app/css/caret.css`) is deleted. A window that loses focus
now says what is held with the paler band and nothing else.

**One walk per frame, not two.** ADR 0013 split the paint across two layers and paid a second
`Editor::selection` walk for it. The fill is alone below the text and the caret alone above, and the
two never coexist — the caret is out while a selection stands — so the split costs nothing now.

**The selection's geometry gets simpler in a way worth naming.** `Selection` is a `Vec` of rows;
there is no head or tail to track through the walk, and an end clipped out of the viewport no longer
needs a rule of its own, because a clipped end and an unclipped one draw the same thing.

**The wider question is open, not answered here.** iA Writer for Mac being the design reference
where `dev/legacy/` is the porting reference is the owner's framing and it reaches much further than the
caret — `dev/ref/ia/REFERENCE.md` is built almost entirely on marketing stills. #154 collects the
evidence for that sweep and is `needs-triage`. This ADR decides one Piece from four captures and
claims nothing beyond it.
