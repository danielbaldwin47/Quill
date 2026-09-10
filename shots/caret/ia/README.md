# The caret on the advance boundary — measured against iA Writer itself

*2026-09-10: the app measured here is iA Writer for **Windows**, run under Wine, and the owner has
ranked it below iA Writer for Mac running natively — `ref/ia/mac-native/` decides where the two
differ ([ADR 0015](../../../docs/adr/0015-the-design-oracle-outranks-the-parity-oracle.md)). The rig
below stays as method: it is how a running app was first driven and read, and `ref/ia/mac-native/rig/`
is its Mac port.*

Evidence for [ADR 0013](../../../docs/adr/0013-caret-on-the-advance-boundary.md), which moves the
bar onto the advance boundary, puts the selection's opening bar in the free caret's own column, and
paints both over the ink. It answers [#147](https://github.com/danielbaldwin47/Quill/issues/147),
whose own evidence is under `shots/caret/147/` and whose measurements the tables below carry over.

The new thing here is the opponent. Every earlier reading of iA's caret came from the eight
Microsoft Store stills in `ref/ia/shots/`, in all of which the caret sits at the end of the text —
so the question #147 asks was never in them. iA Writer for Windows 2.1.9644 now runs on this
machine under Wine and can be driven, so the app answers.

## Driving iA Writer

`wine/` holds the rig: `launch.sh` opens the app on a created headless output with `jump.md`,
`drive.sh <state>` puts one caret or selection state on screen and shoots it — `back-10` and
`fwd-10`, the same selection with the caret at either end, are the pair the table below rests on —
and `measure.py` reads the bar, the fill and the glyph ink out of the frame. Paths in them are this
machine's, and the wine prefix is the default one.

Three things it has to get right, each of which cost a wrong reading first:

- **Keys go through `ydotool`, not `wtype`.** The app is Wine under XWayland and saw only the
  kernel-level injection; `wtype`'s virtual keyboard reached the compositor and nothing else.
- **The caret blinks**, so a single frame is a coin toss. Every state is a burst of eight frames
  across more than one blink period, and `pickbright.py` keeps the one with the most accent in it.
- **Nothing is counted, and no cell grid is used.** `ydotool`'s key auto-repeat makes a run of
  Rights land where it likes, so an offset asked for is not an offset arrived at; and iA's default
  face is Duo, which is duospaced, so the cells are not all one width and a pitch cannot be fitted
  either. Every number below comes from one selection shot twice, which needs neither.

The comparison is taken at a text size that puts one cell at about 71 device pixels, four Ctrl+`+`
above the app's default. Positions survive that; **bar widths do not**, because iA's bar stays 5 px
at every zoom, so no width is compared across the two apps.

## What iA does

One selection, shot twice with the caret held at either end of it (`ia-head.png`, `ia-tail.png`).
The fill is the same both times, so its two boundaries are measured rather than computed:

| | measured |
|---|---|
| the fill, identical in both shots | 1681 … 1965 |
| caret at the head | one bar at 1680–1684 — **on the leading boundary** |
| caret at the tail | one bar at 1965–1969 — **on the trailing boundary** |
| the end the caret is *not* at | **no bar at all** |
| unfocused | fill at full strength, **no bar at all** |

The bar stands within one pixel of its boundary at either end. `caret::NUDGE` was 0.07 em, which is
eight pixels at this size — six times the whole uncertainty of the reading, and so not a thing iA
does. And iA has only **one** mark, the caret, which a selection moves to whichever end is active
rather than adding a second bar to; a mark that is the only one there cannot jump when a selection
opens.

Two findings are not in the change, and are written down for whoever takes them next: iA draws
**one** bar where we and the oracle draw two, and it drops the bar entirely when the window loses
focus rather than dimming it. Both are visible departures from the Parity oracle as well as from
ours, and both are their own decision.

## What ours does now

*As of ADR 0013, which is what this directory measures. The two shots holding a selection —
`ours-selection.png` and `ours-jump-select.png` — were then superseded by
[ADR 0014](../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md), which took the bars
off both of a selection's ends: `shots/caret/mac/` is what those states look like now. They are kept
here as 0013 shot them, because they are its evidence. The free caret is unchanged — `ours-caret.png`
re-shot after 0014 is byte-identical.*

Device pixels at scale 2, from `measure.py`'s repo-side twin in `shots/caret/147/measure147.py`.
`Δcell` is the bar's left edge minus its cell boundary; `solid` is how many of the bar's six columns
are the caret's blue all the way down the band, and `cut` the deepest bite any column takes out of
it — a bar drawn under the ink has the glyph showing through.

### The jump — mono 20 px, offset 10, the `t` of `test`, cell boundary 912

| | free caret | Δcell | solid | cut | opening bar | Δcell | **jump** |
|---|---|---|---|---|---|---|---|
| Parity oracle | 916 | +4 | 6/6 | 0 | 906 | −6 | **10 px** |
| ours before | 915 | +3 | **0/6** | **24** | 906 | −6 | **9 px** |
| **ours after** | **912** | **0** | **6/6** | **0** | **912** | **0** | **0** |
| iA Writer | — | **0**, ±1 px of 71 | — | — | — | **0**, ±1 px of 71 | **0** |

iA's row carries no `solid` or `cut`: in no state the app can be driven to does its bar meet ink,
because the boundary plus the side bearing keeps them apart, so there is nothing for the glyph to
cut. Its columns are relative to their own boundaries rather than to 912, since it is a different
app at a different size.

`sheet-jump.png` is these three at 3×, the free caret above and a selection opening at the same
offset below, so the jump is the distance between the two rows.

Before, the `t`'s stem was rasterised straight through the bar — `solid 0/6, cut 24` — which is what
#147 reported as the bar "sitting on" the leading glyph. It is not that the column was far wrong:
ours stood one device pixel from the oracle's. It is that ours was the only one of the three drawn
*under* the letter.

### The judged states — duo 20 px

| state | bar(s) before | after | boundary |
|---|---|---|---|
| `caret`, offset 403 | 1899 (Δ+3) | **1896 (Δ0)** | 1896 |
| `selection`, 153–171 | 942 (Δ−6) and 1404 (Δ0) | **948 (Δ0)** and 1404 (Δ0) | 948 and 1404 |

All four bars are `solid 6/6, cut 0` after. Offset 403 falls in a word space, which is why no critic
has ever seen the defect on the judged `caret` state: there is no glyph there for the bar to stand
on.

## The Gate

`tools/gate check` passes. The Ticket tier is **not** run here, on purpose.

`tools/gate judge caret`'s opponent is the Parity oracle, and this change leaves it deliberately:
the oracle carries both the nudge and the jump. A round would write
`progress/rounds/caret-r<N>.json` into the record either way, and a loss on a Piece already won
exits 2 and blocks. The critic judges which shot is better rather than which is more alike, so the
round is winnable — round 4 lost on the smear that the paint order now fixes — but it is the owner's
to run.

## Reproducing

    python3 shots/caret/ia/jobs.py /tmp/jobs.json
    node shots/caret/ia/shoot.mjs /tmp/jobs.json
    python3 shots/caret/147/measure147.py shots/caret/ia/ours-*.png

`shots.json` is what the shooter wrote: every shot's path and the SHA-256 of its bytes.
