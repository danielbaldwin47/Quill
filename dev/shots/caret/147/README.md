# #147 — the caret's column, four shapes measured

Judging evidence for [#147](https://github.com/danielbaldwin47/Quill/issues/147),
which asks for a comparison rather than a change: the owner picks a shape from
it, and nothing here lands. No geometry moved on `main`, no oracle was
re-frozen, and no caret round was run.

## What was shot

Every shot is taken on the Gate's own stage — `tools/harness.mjs`, a headless
Hyprland output at 3200×2000, integer scale 2, captured with `grim -T` — so it
is comparable to the frozen Parity oracle under `dev/shots/oracle/caret/`.

**The three views the ticket asks for.** `caret` and `selection` are the judged
states verbatim (`dev/shots/oracle/states.json`, `pieces.caret`): duo at 20 px,
`dev/ref/sample.md`, chrome off. The **jump** pair is the ticket's own condition —
mono at 20 px on `jump.md`, where one cell is 24 device px and offset 10 is the
`t` of `test`, with the space before it in cell 9.

The third judged state, `unfocused`, was shot at every shape as well. It gets
no sheet, because the ticket's three views do not include it; it is here so
that "which judged states would this shape move" can be answered for all three
rather than two.

The jump view has no frozen oracle shot, because it is not a judged state, so
the oracle was shot at it into this directory (`oracle-jump-*.png`) rather than
into `dev/shots/oracle/`. `tools/gate oracle caret` still says `unchanged`.

**Five shapes.** The ticket names four; the fifth is what the measurement
turned up and is not one of the ticket's own.

| | shape | what changes |
|---|---|---|
| 1 | as-is | nothing — the oracle's geometry |
| 2 | no nudge | the free caret loses `caret::NUDGE`, so it stands on the cell boundary |
| 3 | nudged bars | each end bar stands where the free caret would stand at that end's offset |
| 4 | both | 2 and 3 together |
| 5 | oracle order | the caret and the end bars paint **over** the ink instead of under it |

All five come from one build, switched by `QUILL_CARET_SHAPE`; the patch is
`shapes.patch` and it is reverted on this branch. Shape 1 was additionally shot
from the binary built *before* the patch (`s1base-*.png`), and is the same
pixels either way — see the rig proof below.

## The rig is the Gate's

`compare -metric AE`, differing pixels:

| pair | AE |
|---|---|
| shape 1 vs `r6-caret-ours` / `r6-selection-ours` / `r6-unfocused-ours` | **0 / 0 / 0** |
| shape 1 vs the unpatched binary, all five shots | **0** |
| shape 1 vs `dev/shots/oracle/caret/caret` / `selection` / `unfocused` | 37392 / 37428 / 37350 |

So shape 1 is, pixel for pixel, the round the Piece stands on today, and the
shape switch moves nothing at its default.

The last row is what "the selection is identical to the oracle's" does and does
not mean. It is identical **in the mark**: both draw their bars at 942 and 1404
and their fill from 948 for 456 px, which is what round 6's critic reported and
what the tables below repeat. It is not identical **in the frame** — some 37,000
pixels differ on every state, in the type, because a browser and Pango raster
the same Face differently. That difference is the Piece's standing margin and
has nothing to do with the caret; `verify147.sh` prints these rows so the claim
is read as the one and not the other.

## The measurements

Device pixels, at scale 2. `gap<` and `gap>` are the clear pixels to the
nearest glyph ink either side. `solid` is how many of the bar's six columns are
the caret's blue *all the way down* the 72-row band, and `cut` is the deepest
bite any one column takes from it — a bar drawn over the ink is solid on every
column, and one drawn under it has the glyph showing through.

### Judged `caret` state — duo 20 px, offset 403

Offset 403 falls in a **word space**, which is why no critic has ever seen what
#147 reports: there is no glyph for the bar to stand on.

The cell boundary is **1896**. `Δcell` is the bar's left edge minus it.

| | bar x | w | Δcell | gap< | gap> | solid | cut |
|---|---|---|---|---|---|---|---|
| Parity oracle | 1900 | 6 | +4 | 7 | 16 | 6/6 | 0 |
| 1 as-is | 1899 | 6 | +3 | 6 | 17 | 6/6 | 0 |
| 2 no nudge | 1896 | 6 | **0** | 3 | 20 | 6/6 | 0 |
| 3 nudged bars | 1899 | 6 | +3 | 6 | 17 | 6/6 | 0 |
| 4 both | 1896 | 6 | **0** | 3 | 20 | 6/6 | 0 |
| 5 oracle order | 1899 | 6 | +3 | 6 | 17 | 6/6 | 0 |

Dropping the nudge puts the bar on the boundary: 3 px left of where it stands,
and 4 px from the oracle's.

### Judged `selection` state — duo 20 px, select 153–171

The fill runs 948–1403; every bar is 6 px wide. The cell boundary at the
selection's start is **948** and at its end **1404**, and `Δcell` is each bar's
left edge minus its own end's boundary.

| | opening bar | Δcell | gap< | gap> | solid | cut | closing bar | Δcell | solid | cut |
|---|---|---|---|---|---|---|---|---|---|---|
| Parity oracle | 942 | −6 | 22 | 2 | 6/6 | 0 | 1404 | 0 | 6/6 | 0 |
| round 4 (**lost**) | 948 | 0 | 28 | 0 | 1/6 | 3 | 1398 | −6 | 2/6 | 14 |
| 1 as-is | 942 | −6 | 22 | 1 | 6/6 | 0 | 1404 | 0 | 6/6 | 0 |
| 2 no nudge | 942 | −6 | 22 | 1 | 6/6 | 0 | 1404 | 0 | 6/6 | 0 |
| 3 nudged bars | **951** | **+3** | **0** | **0** | **0/6** | **24** | 1407 | +3 | 6/6 | 0 |
| 4 both | **948** | **0** | 28 | **0** | **1/6** | **3** | 1404 | 0 | 6/6 | 0 |
| 5 oracle order | 942 | −6 | 22 | 1 | 6/6 | 0 | 1404 | 0 | 6/6 | 0 |

A negative `Δcell` is a bar standing outside the fill, in the gap before the
cell; zero is a bar on the boundary; positive is a bar inside the cell, on the
glyph.

Shape 4's opening bar is round 4's opening bar, to the pixel and to the count:
x=948, solid 1/6, cut 3. Shape 3's is further in again.

### The jump — mono 20 px, offset 10, the `t` of `test`

Cell 10 begins at **912**. The jump is the free caret's column minus the
opening bar's, at the same offset.

Every bar is 6 px wide. The free caret and the opening bar are both measured
against cell 10's boundary at 912.

| | free caret | w | Δcell | gap< | gap> | solid | cut | opening bar | w | Δcell | **jump** |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Parity oracle | 916 | 6 | +4 | 0 | 0 | 6/6 | 0 | 906 | 6 | −6 | **10 px** |
| 1 as-is | 915 | 6 | +3 | 0 | 0 | 0/6 | 24 | 906 | 6 | −6 | **9 px** |
| 2 no nudge | 912 | 6 | **0** | 26 | 0 | 1/6 | 3 | 906 | 6 | −6 | **6 px** |
| 3 nudged bars | 915 | 6 | +3 | 0 | 0 | 0/6 | 24 | 915 | 6 | **+3** | **0** |
| 4 both | 912 | 6 | **0** | 26 | 0 | 1/6 | 3 | 912 | 6 | **0** | **0** |
| 5 oracle order | 915 | 6 | +3 | 0 | 0 | 6/6 | 0 | 906 | 6 | −6 | **9 px** |

`gap<` and `gap>` read 0 for every shape whose bar is inside the `t`'s cell,
because the crossbar's ink runs to both sides of it. That is what `solid` and
`cut` are for: the oracle's bar and shape 5's have the same zero clearance and
are still solid blue all the way down, because they are drawn over the ink
rather than under it.

Two things the ticket could not have known:

1. **The oracle has the jump too, and it is bigger than ours** — 10 device px
   against our 9. Closing it makes us deliberately unlike iA, which is the
   opposite of what the complaint assumes.
2. **The free caret's column is not what makes it read as sitting on the `t`.**
   Ours is 1 device px from the oracle's, but ours is `solid 0/6, cut 24` and
   the oracle's is `solid 6/6, cut 0`. The oracle paints its caret **over** the
   glyph and we paint ours **under** it, so the `t`'s stem is drawn straight
   through our bar. `dev/legacy/app/css/caret.css` names the three layers and their
   order — `#sel-layer` beneath `#mirror`, `#caret-layer` above it — and ours
   puts all three in `snapshot_layer(BelowText)`.

### The one written claim that disagrees

`dev/ref/ia/REFERENCE.md` § 4.1 says of the caret: *"Sits flush after the last
glyph."* Every shot here says otherwise, and so does the oracle's.

At the jump offset the preceding `a`'s ink ends at x=885 and the oracle's caret
stands at 916–921 — **30 device px** past it, a whole 24 px cell plus the
nudge, and inside the cell of the glyph that follows. On the judged `caret`
state the oracle's bar has 7 px of clear paper behind it and 16 ahead. Flush
after the last glyph is neither.

The wording is not edited here: #147 puts it out of scope and asks only that
the contradiction be written down. It is the reference's own claim about iA,
measured against iA's port, and it is the claim that is wrong — the caret sits
in the *following* cell, nudged `caret::NUDGE` past its boundary.

### Which judged states each shape moves

`compare -metric AE` against shape 1:

| shape | `caret` | `selection` | `unfocused` | needs `gate judge caret` + a round |
|---|---|---|---|---|
| 1 as-is | 0 | 0 | 0 | no |
| 2 no nudge | 182 | 0 | 54 | yes |
| 3 nudged bars | 0 | 496 | 0 | yes |
| 4 both | 182 | 320 | 54 | yes |
| 5 oracle order | **0** | **0** | **0** | **no** |

Shape 5's three judged shots are byte-identical to as-is: at offset 403 and at
the selection's two ends no glyph sits under a bar, so paint order changes
nothing a judged state can see.

## The sheets

- `sheet-caret.png` — the judged `caret` state at the insertion point, oracle
  first, then all five shapes, 3× nearest-neighbour.
- `sheet-selection-open.png`, `sheet-selection-close.png` — the judged
  `selection` state's two ends, with round 4's inset bar beside them.
- `sheet-jump.png` — the free caret above, a selection opening at the same
  offset below, so the jump is the distance between the two rows.
- `pair-*.png` — ours beside the oracle, whole frame, for context.

The shots themselves are `s<shape>-<state>.png`, with `s1base-*` the same four
states shot from the unpatched binary and `oracle-jump-*.png` the oracle at the
jump pair. `shots.json` is what the shooter wrote: every shot's path and the
SHA-256 of its bytes, which is where the byte-identical claims come from before
`verify147.sh` re-checks them with `compare`.

## Reproducing it

From the repo root, with `npm i` done here and in `dev/legacy/`:

```
git apply dev/shots/caret/147/shapes.patch
cargo build --release
python3 dev/shots/caret/147/jobs147.py /tmp/jobs.json
node   dev/shots/caret/147/shoot147.mjs /tmp/jobs.json
node   dev/shots/caret/147/oracle147.mjs
python3 dev/shots/caret/147/measure147.py dev/shots/caret/147/s1-caret.png ...
python3 dev/shots/caret/147/sheets147.py
bash   dev/shots/caret/147/verify147.sh
git checkout -- quill/src/editor.rs
```

`probe147.py` prints per-column blue and ink counts across a band, which is how
`solid` and `cut` were checked against the pixels rather than against a
downscaled sheet.

## The shape that landed, and how its acceptance was measured

Everything above is the evidence for the *decision*, shot in 2026-08-30 when
this ticket still asked for one. What the rewritten ticket asked to be built —
the bar **centred** on the advance boundary, `docs/design.md` row Caret column —
is measured here, at the default step on the Gate's own stage.

The boundary is not asserted, it is read. `Editor::selection` builds its fill
rows from the same `iter_location` x values the caret is placed by, and this
change does not touch it, so a fill from offset a to offset b spans exactly
`boundary(a) .. boundary(b)` and is an independent ruler:

```
python3 dev/shots/caret/147/boundary.py \
  select=dev/shots/caret/147/boundary-select-18-28.png \
  a=dev/shots/caret/147/boundary-caret-18.png \
  b=dev/shots/caret/147/boundary-caret-28.png
```

```
fill              x 622..881  (boundary a = 622, boundary b = 882)
bar at offset a   x 619..624  w 6  centre 622.0  boundary 622  delta +0.0
bar at offset b   x 879..884  w 6  centre 882.0  boundary 882  delta +0.0
```

Mono at the default step, light, chrome off, 1440x900 at scale 2. The bar is
6 device px wide with 3 px each side of the boundary, which is the Design
oracle's own bar to the pixel (`dev/ref/ia/mac-native/VERDICTS.md` 0013.1, and
`NOTES.md` § The three offset frames: 688…693 on 691.0, 944…949 on 947.0,
1200…1205 on 1203.0).
