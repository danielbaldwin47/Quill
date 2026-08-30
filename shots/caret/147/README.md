# #147 — the caret's column, four shapes measured

Judging evidence for [#147](https://github.com/danielbaldwin47/Quill/issues/147),
which asks for a comparison rather than a change: the owner picks a shape from
it, and nothing here lands. No geometry moved on `main`, no oracle was
re-frozen, and no caret round was run.

## What was shot

Every shot is taken on the Gate's own stage — `tools/harness.mjs`, a headless
Hyprland output at 3200×2000, integer scale 2, captured with `grim -T` — so it
is comparable to the frozen Parity oracle under `shots/oracle/caret/`.

**Three views.** `caret` and `selection` are the judged states verbatim
(`shots/oracle/states.json`, `pieces.caret`): duo at 20 px, `ref/sample.md`,
chrome off. The **jump** pair is the ticket's own condition — mono at 20 px on
`jump.md`, where one cell is 24 device px and offset 10 is the `t` of `test`,
with the space before it in cell 9. The judged `unfocused` state was shot too,
to say which shapes move it, but has no sheet of its own.

The jump view has no frozen oracle shot, because it is not a judged state, so
the oracle was shot at it into this directory (`oracle-jump-*.png`) rather than
into `shots/oracle/`. `tools/gate oracle caret` still says `unchanged`.

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

So shape 1 is, pixel for pixel, the round the Piece stands on today, and the
shape switch moves nothing at its default.

## The measurements

Device pixels, at scale 2. `gap<` and `gap>` are the clear pixels to the
nearest glyph ink either side. `solid` is how many of the bar's six columns are
the caret's blue *all the way down* the 72-row band, and `cut` is the deepest
bite any one column takes from it — a bar drawn over the ink is solid on every
column, and one drawn under it has the glyph showing through.

### Judged `caret` state — duo 20 px, offset 403

Offset 403 falls in a **word space**, which is why no critic has ever seen what
#147 reports: there is no glyph for the bar to stand on.

| | bar x | w | gap< | gap> | solid | cut |
|---|---|---|---|---|---|---|
| Parity oracle | 1900 | 6 | 7 | 16 | 6/6 | 0 |
| 1 as-is | 1899 | 6 | 6 | 17 | 6/6 | 0 |
| 2 no nudge | 1896 | 6 | 3 | 20 | 6/6 | 0 |
| 3 nudged bars | 1899 | 6 | 6 | 17 | 6/6 | 0 |
| 4 both | 1896 | 6 | 3 | 20 | 6/6 | 0 |
| 5 oracle order | 1899 | 6 | 6 | 17 | 6/6 | 0 |

The cell boundary is **1896**: dropping the nudge puts the bar on it, 3 px left
of where it stands and 4 px from the oracle's.

### Judged `selection` state — duo 20 px, select 153–171

The fill runs 948–1403. The cell boundary at the selection's start is **948**.

| | opening bar | solid | cut | closing bar | solid | cut |
|---|---|---|---|---|---|---|
| Parity oracle | 942 | 6/6 | 0 | 1404 | 6/6 | 0 |
| round 4 (**lost**) | 948 | 1/6 | 3 | 1398 | 2/6 | 14 |
| 1 as-is | 942 | 6/6 | 0 | 1404 | 6/6 | 0 |
| 2 no nudge | 942 | 6/6 | 0 | 1404 | 6/6 | 0 |
| 3 nudged bars | **951** | **0/6** | **24** | 1407 | 6/6 | 0 |
| 4 both | **948** | **1/6** | **3** | 1404 | 6/6 | 0 |
| 5 oracle order | 942 | 6/6 | 0 | 1404 | 6/6 | 0 |

Shape 4's opening bar is round 4's opening bar, to the pixel and to the count:
x=948, solid 1/6, cut 3. Shape 3's is further in again.

### The jump — mono 20 px, offset 10, the `t` of `test`

Cell 10 begins at **912**. The jump is the free caret's column minus the
opening bar's, at the same offset.

| | free caret | solid | cut | opening bar | **jump** |
|---|---|---|---|---|---|
| Parity oracle | 916 | 6/6 | 0 | 906 | **10 px** |
| 1 as-is | 915 | 0/6 | 24 | 906 | **9 px** |
| 2 no nudge | 912 | 1/6 | 3 | 906 | **6 px** |
| 3 nudged bars | 915 | 0/6 | 24 | 915 | **0** |
| 4 both | 912 | 1/6 | 3 | 912 | **0** |
| 5 oracle order | 915 | 6/6 | 0 | 906 | **9 px** |

Two things the ticket could not have known:

1. **The oracle has the jump too, and it is bigger than ours** — 10 device px
   against our 9. Closing it makes us deliberately unlike iA, which is the
   opposite of what the complaint assumes.
2. **The free caret's column is not what makes it read as sitting on the `t`.**
   Ours is 1 device px from the oracle's, but ours is `solid 0/6, cut 24` and
   the oracle's is `solid 6/6, cut 0`. The oracle paints its caret **over** the
   glyph and we paint ours **under** it, so the `t`'s stem is drawn straight
   through our bar. `legacy/app/css/caret.css` names the three layers and their
   order — `#sel-layer` beneath `#mirror`, `#caret-layer` above it — and ours
   puts all three in `snapshot_layer(BelowText)`.

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

## Reproducing it

From the repo root, with `npm i` done here and in `legacy/`:

```
git apply shots/caret/147/shapes.patch
cargo build --release
python3 shots/caret/147/jobs147.py /tmp/jobs.json
node   shots/caret/147/shoot147.mjs /tmp/jobs.json
node   shots/caret/147/oracle147.mjs
python3 shots/caret/147/measure147.py shots/caret/147/s1-caret.png ...
python3 shots/caret/147/sheets147.py
bash   shots/caret/147/verify147.sh
git checkout -- quill/src/editor.rs
```

`probe147.py` prints per-column blue and ink counts across a band, which is how
`solid` and `cut` were checked against the pixels rather than against a
downscaled sheet.
