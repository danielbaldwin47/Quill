# The caret stands on the advance boundary

*Narrowed by [ADR 0014](0014-a-selection-is-a-fill-and-nothing-else.md): the selection's two end
bars are gone, so what stands on the boundary is the free caret alone. Everything below about where
the bar goes, what it is painted over, and how the Windows app was measured stands.*

*Sharpened, not disturbed, by iA Writer for Mac running —
[`ref/ia/mac-native/VERDICTS.md`](../../ref/ia/mac-native/VERDICTS.md) § ADR 0013. The 6 px bar is
**centred** on the advance boundary, 3 px each side, and the offset from the boundary measures 0.000
em; the clear paper at a hard line end (7 px), the one-cell step between consecutive offsets and the
bar that never overlaps ink are all confirmed. One row narrows: 0013.7's "iA has only one mark,
which a selection moves to whichever end is active" holds for the Windows build its table measured
and not for the Mac app, which removes the caret for as long as the selection stands. And one line
is overtaken: § Consequences' "`ref/ia/REFERENCE.md` § 4.1 still says 'flush after the last glyph'"
— correcting it was its own change, and that change is
[#157](https://github.com/danielbaldwin47/Quill/issues/157), so § 4.1 now carries the 7 px of clear
paper and the capture it is measured from.*

The bar — the free caret and both of the selection's end bars — stands on the advance boundary
between two cells, offset from it by nothing, and is painted over the ink rather than under it. The
free caret at an offset and the selection's opening bar at that same offset are therefore the same
column, so opening a selection under the caret leaves the mark where the writer's eye already was.
This supersedes `caret::NUDGE` (0.07 em, deleted here) and the opening bar's
`setEdge(edgeA, firstEdge, -M.w)` inherited from the Parity oracle, and it replaces the reading of
`ref/ia/shots/msstore-win-04-dark-style-check-selection.png` that
[ADR 0012](0012-editor-draws-its-own-selection.md) § Considered options rests one paragraph on.
Decided from [#147](https://github.com/danielbaldwin47/Quill/issues/147) and the owner's ask that
the caret be iA Writer's.

## What changed the answer

Every earlier measurement of iA's caret was taken from the eight Microsoft Store stills in
`ref/ia/shots/`. In all eight the caret sits at the end of the text, so the one question #147 asks —
where the bar goes when a glyph follows it — was never in the evidence, and two readings were made
of the stills that the stills could not support:

- **`NUDGE = 0.07` em.** Its doc comment recorded "0.060–0.073 em of iA's own captures". At the end
  of a run of text the clear paper behind the bar is the last glyph's **right side bearing**; read
  as a caret offset it becomes a nudge that does not exist. `ref/ia/REFERENCE.md` § 4.1 read the
  same pixels the other way — "sits flush after the last glyph" — and is wrong in the other
  direction. One gap, two contradictory constants, neither of them the caret's.
- **"iA insets its selection bars."** `msstore-win-04` is a **style-check** capture, as its own file
  name says: the dark box with a strikethrough through "so-called" is a flagged phrase and the two
  bright caps are that marker's ends. It is not a selection and neither cap is a caret.

iA Writer for Windows 2.1.9644 now runs on this machine under Wine, so the app answers instead of
its screenshots. Driven by hand on the Gate's own stage — a created headless output, keys through
`ydotool`, the caret's blink beaten by keeping the brightest frame of a burst — at a text size that
puts one cell at about 71 device pixels.

The reading is one selection, shot twice with the caret held at either end of it. That is what makes
it safe: the fill is the same both times, so the two cell boundaries are **measured off the fill**
rather than computed from a pitch — and a pitch could not be assumed anyway, because iA's default
face is Duo, which is duospaced.

| | measured |
|---|---|
| the fill, identical in both shots | 1681 … 1965 |
| caret at the selection's head | one bar at 1680–1684 — **on the leading boundary** |
| caret at the selection's tail | one bar at 1965–1969 — **on the trailing boundary** |
| the end the caret is *not* at, either shot | **no bar at all** |

Two things follow, and neither needs a grid. The bar stands within one pixel of its boundary at both
ends; `NUDGE` is 0.07 em, which is eight pixels at this size, so it is six times the whole
uncertainty of the measurement and plainly not what iA draws. And iA has only **one** mark — the
caret — which a selection moves to whichever end is active rather than adding a second bar to. A
mark that is the only one there cannot jump when a selection opens: there is nothing else for it to
become.

Ours, at the same offsets, carried a 0.117-cell nudge and a 0.375-cell jump, and the oracle's a
0.167-cell nudge and a 0.42-cell jump. The complaint in #147 is real and neither number was iA's.

## Considered options

**Keeping the nudge and closing only the jump** (#147's shape 3) puts both marks 0.07 em inside the
cell, on the glyph, and makes the thing the ticket complains about worse at both ends rather than
one. **Closing the jump by moving the free caret out to meet the opening bar** keeps the two equal
and puts the caret a bar's width into the gap before its own cell, which is neither app's and reads
as belonging to the letter behind.

**Leaving the paint order alone.** Under-the-ink was chosen in
[ADR 0012](0012-editor-draws-its-own-selection.md) on the grounds that "a caret drawn over a letter
is a caret that hides one", and it is why round 4's inset bars were read as a smear: a bar is 0.155
em, a glyph's left side bearing at 20 px is less, and the letter is then rasterised straight through
the bar so the two become one mark. On the boundary the overlap is small — three rows of the `t`'s
crossbar on the judged passage — but it is the same failure in miniature, and it is the mechanism
behind #147's "sits on the leading glyph": ours measured `solid 0/6, cut 24` where the oracle's
measured `solid 6/6, cut 0`. The oracle paints over (`legacy/app/css/caret.css` puts `#caret-layer`
above `#mirror`) and this now does too. iA's own order could not be settled: in no state the app can
be driven to does its bar meet ink, because the boundary plus the side bearing keeps them apart.

## Consequences

**This leaves the Parity oracle on purpose, and it is the first Piece to do so.** The oracle's
opening bar sits a bar-width outside the fill and its caret carries the nudge; ours now does
neither. The `caret` and `unfocused` judged states move, and so does `selection`, so the Piece has
to be re-judged and re-frozen — a blind round the oracle can only win on the geometry, because the
geometry is what it is being asked to differ on. The owner's ask is the tiebreak the round cannot
supply, and it is recorded here as the reason a lost round on this Piece is not a regression.

**The layer above the text is no longer free.** ADR 0012 held it for a future Annotator's marks. The
bars and the caret are drawn there now, so those marks will have to sort against them.

**`ref/ia/REFERENCE.md` § 4.1 still says "flush after the last glyph".** It is measured wrong in the
other direction and is left alone here; correcting it is its own change against its own evidence.

**The fill is still under the ink.** Only the bars moved. The ink of a held word is the ink of any
other word and a fill painted over it would tint it, which is ADR 0012's reason and is untouched.
