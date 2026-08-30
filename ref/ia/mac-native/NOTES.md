# iA Writer for Mac, running — the capture notes

Every number in `ref/ia/REFERENCE.md` was read off a marketing still until now, and three of those
readings were wrong ([ADR 0013](../../../docs/adr/0013-caret-on-the-advance-boundary.md),
[ADR 0014](../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md)). These captures are the
app itself, driven and measured, for
[#154](https://github.com/danielbaldwin47/Quill/issues/154). The verdicts they carry are in
[VERDICTS.md](VERDICTS.md); this file is how each one was taken and what it measured.

Nothing here changes the spec. The evidence is put where a spec change can be argued from.

## The rig

| | |
|---|---|
| iA Writer | 8.0.6 (build 80046), `pro.writer.mac` |
| macOS | 27.0 (26A5406e) |
| Display | built-in Liquid Retina XDR, 3024 × 1964 device px, 1512 × 982 logical pt |
| Backing scale | **2.0**, verified: `screencapture -R` of a 200 pt region returns 400 px |
| Window | set to logical `{0, 33, 1512, 982}` for every state — 3024 × 1898 device px |
| Typeface | **Mono** throughout, so the grid can be fitted (Duo is duospaced and no cell pitch fits it) |
| Typography | System — Default |
| Line length limit | 64 characters (the app's default; the menu offers 64 / 72 / 80) |
| Style Check | **off** for every state |
| Syntax highlight | **off** for every state |
| Authors | **hidden** for every state |
| Focus Mode | **off** except states 13 and 15, which name it |
| Passage | `ref/sample.md`, except where a state needs a block `sample.md` has not got |

**Every number below is in device pixels at scale 2.0.** Divide by two for logical points.

Two states need markup `ref/sample.md` does not contain, so they use passages kept beside this file:
[`passage-blocks.md`](passage-blocks.md) (heading, blockquote, list, emphasis) for states 9 and 14,
and [`passage-markers.md`](passage-markers.md) (all six heading levels) for the gutter ladder.

### How things were measured

The Linux rig's vocabulary is kept so the two evidence sets compare
(`shots/caret/ia/wine/measure.py`): *band*, *bar*, *fill*, *ink*, `gap<`/`gap>`, *solid*, *cut*.

- **Boundaries come off a selection fill, never off a character count.** A selection of *n* cells is
  laid down and its fill measured; the cell advance is the difference between two such fills, which
  needs no assumption about the face.
- **The caret blinks, so a still is a coin toss.** Every caret frame is the brightest of a burst of
  14 (`pickbright.py`'s idea, ported), scored by accent-strength pixel count.
- **States 6 and 7 need the window deactivated but not covered**, so Finder's windows are closed
  first and Finder is then activated: focus moves, nothing is drawn over the frame. (`screencapture
  -l <windowid>` returns a black frame for an occluded window on this macOS, so it cannot be used.)
- The blink series were sampled through Quartz at ~103 Hz, not `screencapture`, which is far too
  slow per frame to time a blink.

## The grid at the default text size

Measured off selection fills of 5, 10, 20 and 40 cells, all four starting at the same x
(`mac-native-08-dark-selection-cells-05.png` … `-40.png`):

| n cells | fill x | width | implied advance |
|---|---|---|---|
| 5 | 690 … 819 | 130 | — |
| 10 | 690 … 947 | 258 | (258−130)/5 = **25.6** |
| 20 | 690 … 1203 | 514 | (514−258)/10 = **25.6** |
| 40 | 690 … 1715 | 1026 | (1026−514)/20 = **25.6** |

**Cell advance = 25.6 px exactly.** The fill runs 1 px proud of the cell on each side (width is
`25.6n + 2` every time), so the true leading boundary of the first cell is **x = 691.0**.

iA Writer Mono's own tables (from the app bundle) give `unitsPerEm` 1000 and an advance of 600, i.e.
**0.6 em per cell**, so at this size **1 em = 42.67 px = 21.33 pt**, and the hhea line height is
1.30 em. That is the first point value this repo has for the app's default text size.

### The text container

Select-all (`mac-native-09-dark-selection-select-all.png`) fills one unbroken block from
**x = 512 to x = 2511**, 2000 px wide. Its centre is x = 1512.0, exactly half the 3024 px window:
**the container is centred in the window**.

- 64-cell measure: 691.0 … 2329.4
- gutter left: 691 − 512 = 179 px = **6.99 cells**
- gutter right: 2512 − 2329.4 = 182.6 px = **7.13 cells**

So the container is the 64-cell measure plus a **7-cell hanging-marker gutter on each side**, and the
gutter is exactly the width `###### ` needs — see the ladder under state 14.

---

## State 1 — caret mid-word, light and dark

`mac-native-01-dark-caret-midword.png`, `mac-native-01-light-caret-midword.png`
Caret at offset 5 of `The lamp had been lit …`, inside the word `lamp`.

| | dark | light |
|---|---|---|
| bar | x 816 … 821, **w 6** | x 816 … 821, **w 6** |
| height | y 281 … 352, **h 72** | y 280 … 353, h 74 |
| colour | **#00bfff** | **#00bfff** |
| centre | **819.0** | **819.0** |
| advance boundary at offset 5 | 691.0 + 5 × 25.6 = **819.0** | same |

**The bar is centred on the advance boundary**, 3 px each side of it — not set to one side of it and
not offset from it. The core is 220 px of flat `#00bfff` in both themes with one blended column at
each edge; the light frame's h 74 against dark's h 72 is the accent threshold catching two more
antialiased rows over a pale ground, not a taller bar.

Against the glyphs either side, with the bar's own columns excluded from the ink:

- `gap<` = **1 px**, `gap>` = **0 px**, ink under the bar: **none**.

The bar covers the whole left side bearing of the glyph that follows and stops exactly where its ink
starts. Because the two abut and never overlap, **iA's paint order still cannot be read** — which is
what ADR 0013 said of the Windows build and is equally true here.

## State 2 — caret at the end of a line

`mac-native-02-dark-caret-line-end.png`, `mac-native-02-light-caret-line-end.png`
Caret at ⌘→ on `if about little else.`, the paragraph's last line — a **hard** line end. (A wrapped
line will not answer this: its last display cell is a trailing space, and the caret then sits a
whole cell past the last glyph for a reason that has nothing to do with bearings.)

| | measured |
|---|---|
| bar | x 1226 … 1231, w 6, centre **1229.0** |
| the line is 21 characters | boundary after cell 21 = 691.0 + 21 × 25.6 = **1228.6** |
| last glyph ink (the `.`) ends at | x **1218** |
| clear paper between ink and bar | **7 px** = 0.27 cell |

**The caret stands on the advance boundary, not against the ink.** `REFERENCE.md` § 4.1's "sits
flush after the last glyph" is measured wrong: what looks flush in a still is the last glyph's right
side bearing, which is exactly the misreading ADR 0013 named.

## State 3 — caret in an empty document

`mac-native-03-dark-caret-empty-document.png`, `mac-native-03-light-caret-empty-document.png`

| | measured |
|---|---|
| bar | x 688 … 693, w 6, centre **691.0**, h **73** |
| colour | #00bfff |
| anything else drawn | **nothing** |

The left edge of the measure is **the same x = 691.0** as in a document full of text, so an empty
document is not a special case. **A caret alone has no band**: the frame holds the bar and the paper
and nothing between.

## State 4 — caret blink

`mac-native-04-dark-caret-blink-f00.png` … `-f12.png` (13 frames, **0.10 s apart**, tight crop on the
bar), and [`blink-idle.tsv`](blink-idle.tsv) — 1240 samples over 12 s at 103 Hz, `t<TAB>accent px`.

Reading the series at two thresholds:

| threshold | on | off | period | duty |
|---|---|---|---|---|
| full strength (426 px) | 0.516 s | 0.484 s | **1.000 s** | **51.6 %** |
| any accent pixel at all | 0.691 s | 0.305 s | 0.996 s | 69.4 % |

**Period 1.000 s, half on and half off** — the macOS platform blink, which § 3.5 presumed but could
not see. The gap between the two rows is a **fade of about 0.09 s at each edge**: the waveform is
426 → 356 → 0 and back, so the bar is not hard-switched. The frame set shows it directly: the 356
frames are the fade, the 426 frames the bar at full strength.

## State 5 — typing

`mac-native-05-dark-typing-*.png` (8 frames, named in the order they were taken), and
[`blink-typing.tsv`](blink-typing.tsv), whose marks are `type-start` 2.508 s and `type-end` 5.432 s
for ten characters typed at ~0.18 s intervals.

| | measured |
|---|---|
| before typing | blinking normally |
| first keystroke | bar goes on and **stays on** |
| through the typing window | **solid on for 3.448 s** — no blink at all |
| blink resumes | **0.633 s after the last keystroke** |

**The blink is suppressed while a hand types** and the caret is held on for one full on-phase after
the last key before the cadence starts again.

## State 6 — window deactivated, caret only

`mac-native-06-dark-deactivated-caret.png`, `mac-native-06-light-deactivated-caret.png`
Caret set, then Finder activated with no Finder window open, so the editor stays visible.

**The caret goes. Entirely.** A burst of 14 frames scored **0 accent pixels in every frame**, in both
themes, with the editor's own ground (`#1a1a1a` / `#f7f7f7`) filling the frame — so the window was
visible and simply had no bar in it.

This is what the Windows build does under Wine, and it is what **neither Quill nor the Parity oracle
does**.

## State 7 — window deactivated, selection standing

`mac-native-07-dark-deactivated-selection.png`, `mac-native-07-light-deactivated-selection.png`

| | active | idle |
|---|---|---|
| dark | `#113d52` | **`#464646`** |
| light | `#ccedf8` | **`#dcdcdc`** |
| geometry (dark, 18 cells) | x 690 … 1151, w 462, y 282 … 351, h 70 | **identical** |

Only the colour moves; the band does not. And the idle colour is **not a paler accent** — it is a
neutral grey with no blue in it at all.

## State 8 — selection inside one row

`mac-native-08-dark-selection-inline.png`, `mac-native-08-light-selection-inline.png`
18 cells held from the start of a body line.

| | measured |
|---|---|
| fill | x 690 … 1151, **w 462** = 25.6 × 18 + 2 |
| band | y 282 … 351, **h 70**, against a line pitch of **73** |
| colour | dark **#113d52**, light **#ccedf8** |
| accent pixels in the frame | **none** |

`#113d52` is the owner's own reading of `owner-mac-03` to the digit. `REFERENCE.md` § 4.2's
`≈ #003e4c` is a marketing-frame value and is not this.

## State 9 — selection across rows, over a heading and a quote block

`mac-native-09-dark-selection-multirow.png`, `mac-native-09-light-selection-multirow.png`
Uses [`passage-blocks.md`](passage-blocks.md). Row bands are taken from the *unselected* frame, where
glyph ink still separates the lines, then read back out of the selected one.

| row | fill |
|---|---|
| `The lamp had been…` (first held row) | x **920** … 2511 — from the anchor to the container's right edge |
| `## What the sea keeps` (interior row) | x **512** … 2511 — **the whole container**, gutter and all |
| `> There are things…` (last held row) | x **512** … 921 — container's left edge to the focus |

- **The band is continuous between rows.** Sampled down a column in the right gutter, where no glyph
  can interrupt it, the fill runs y 282 … 573 with **no internal gap** — 292 px, exactly four
  73 px pitches. A single held row draws 70 px; held rows in a run abut into one unbroken band.
- **Markers in the gutter are simply inside the band.** The heading's hanging `##` sits at x 615 and
  the fill starts at 512, so the marker is covered like any other glyph; nothing is done specially
  for it.
- The apparent gaps in the fill at columns that carry text are **the glyphs themselves**: the ink is
  painted over the band, so the fill is under the ink.
- **No accent pixel in the frame.**

## State 10 — selection running through a trailing newline

`mac-native-10-dark-selection-newline-only.png` (the newline alone),
`mac-native-10-dark-selection-trailing-newline.png`, `mac-native-10-light-selection-trailing-newline.png`

Held at the hard end of `if about little else.` (21 cells, boundary 1228.6):

| held | fill |
|---|---|
| the newline alone | x **1228** … 2511, **w 1284** |
| the last six glyphs and the newline | x 1074 … 2511, w 1438 |

**A held newline does not draw a stub.** It fills from the boundary after the last glyph to the
**right edge of the text container** — 2511, the same edge select-all reaches, seven cells past the
64-cell measure. The width is a property of the container, not of the newline.

## State 11 — every text size the app offers

`mac-native-11-dark-text-size-00.png` … `-13.png`. The Text Size menu steps rather than names a
value, so the range was walked from the bottom until the geometry stopped moving: **14 distinct
sizes**. Per size the advance is measured off a 20-cell fill, the pitch off the line tops, and the
caret from a burst.

`em` is derived from the advance at Mono's own 0.6 em per cell; `pt` is `em / 2` at this scale.

| step | cell px | em px | em pt | pitch px | pitch / em | band px | caret w | caret h | caret w / em |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 17.4 | 29.00 | 14.50 | 49 | 1.690 | 46 | 5 | 49 | 0.172 |
| 1 | 18.3 | 30.50 | 15.25 | 52 | 1.705 | 50 | 5 | 53 | 0.164 |
| 2 | 19.4 | 32.33 | 16.17 | 56 | 1.732 | 54 | 6 | 56 | 0.186 |
| 3 | 20.6 | 34.33 | 17.17 | 59 | 1.718 | 58 | 6 | 59 | 0.175 |
| 4 | 23.1 | 38.50 | 19.25 | 66 | 1.714 | 64 | 6 | 67 | 0.156 |
| **5 — Default** | **25.6** | **42.67** | **21.33** | **73** | **1.711** | **70** | **6** | **72** | **0.141** |
| 6 | 30.7 | 51.17 | 25.58 | 86 | 1.681 | 84 | 8 | 86 | 0.156 |
| 7 | 35.7 | 59.50 | 29.75 | 98 | 1.647 | 94 | 8 | 98 | 0.134 |
| 8 | 40.7 | 67.83 | 33.92 | 109 | 1.607 | 108 | 8 | 109 | 0.118 |
| 9 | 45.7 | 76.17 | 38.08 | 120 | 1.575 | 118 | 10 | 120 | 0.131 |
| 10 | 53.1 | 88.50 | 44.25 | 135 | 1.525 | 134 | 10 | 135 | 0.113 |
| 11 | 60.4 | 100.67 | 50.33 | 149 | 1.480 | 146 | 10 | 148 | 0.099 |
| 12 | 67.8 | 113.00 | 56.50 | 161 | 1.425 | 158 | 10 | 161 | 0.088 |
| 13 | 75.1 | 125.17 | 62.58 | 172 | 1.374 | 170 | 10 | 172 | 0.080 |

Three things come out of it.

**The "liquid" line height is real and it is a falling curve.** `pitch / em` goes from about **1.73
at the small end to 1.374 at the large**, monotonically from step 2 down. Line spacing is not a
constant multiple: the bigger the type, the tighter the leading, proportionally. The wobble across
steps 0–3 is whole-pixel quantisation on a 29 px em.

**The caret's height is the line pitch at every size**, within the 1 px the two roundings can
differ (49/49, 53/52, 72/73, 148/149, 172/172).

**The caret's width is not a constant fraction of the em.** It quantises to 5, 6, 8, 10 px and stops
there, so `caret w / em` falls from **0.172 to 0.080** across the range. § 3.5's "≈ 0.12–0.17 em"
holds around the default and fails at both ends.

*Caveat, and it matters for the last six rows:* 64 cells plus two 7-cell gutters is 78 cells, which
needs 78 × cell px of window. Past step 7 that exceeds this window's 3024 px, so from step 8 on the
column is limited by the window, not by the app's line-length setting. Pitch, advance and caret are
unaffected; the measure's own width is not the app's from step 8 up.

## State 12 — paper, ink, selection band and accent, both themes

`mac-native-12-dark-palette.png`, `mac-native-12-light-palette.png`, and the state frames each colour
was sampled from.

| Role | Light | Dark |
|---|---|---|
| Paper (editor background) | **#f7f7f7** | **#1a1a1a** |
| Body ink | **#191919** | **#cccccc** |
| Heading ink | #191919 | **#cccccc** — identical to body |
| Selection band, active | **#ccedf8** | **#113d52** |
| Selection band, idle | **#dcdcdc** | **#464646** |
| Caret / accent | **#00bfff** | **#00bfff** |
| Dimmed (Focus Mode) | **#c6c4c2** | **#707070** |

The accent is **the same `#00bfff` in both themes** — not the `#00b5ff` and `#00c3ff` that § 4.2
carries from two different stills.

## State 13 — Focus mode, sentence and paragraph, both themes

`mac-native-13-{dark,light}-focus-{sentence,paragraph}.png`

There is **one dim tier, not several**: `#707070` on dark and `#c6c4c2` on light, and the focused run
is left at the ordinary body ink. Sentence and Paragraph differ only in how much stays lit — the two
colours are the same in both. Focus also scrolls the focused line toward the middle of the window.

In the app's own Settings, **Typewriter is a third value of the same "Focus scope" popup** as
Sentence and Paragraph, not an independent toggle.

## State 14 — markup rendering

`mac-native-14-dark-markup.png` (blocks, from [`passage-blocks.md`](passage-blocks.md)),
`mac-native-14-dark-markup-gutters.png` and `-blocks.png` (all six heading levels, from
[`passage-markers.md`](passage-markers.md)), plus `-lower.png` and `-deep.png`.

The hanging-marker ladder, measured as the distance from the body column (691.0) to the row's first
ink, in cells of 25.6 px:

| marker | measured | cells |
|---|---|---|
| `# ` | 641 | **1.95 → 2** |
| `## ` | 615 | **2.97 → 3** |
| `### ` | 589 | **3.98 → 4** |
| `#### ` | 564 | **4.96 → 5** |
| `##### ` | 538 | **5.98 → 6** |
| `###### ` | 513 | **6.95 → 7** |
| `> ` | 695 | **none — no hang at all** |
| `- ` | 697 | **none** |
| `1. ` | 693 | **none** |

**Headings hang by (level + 1) cells** — the marker and its space — so the heading's text lands on
the body column. `###### ` hangs 7 cells to x 513, which is the container's own left edge at 512:
**the gutter is 7 cells wide because that is what the deepest heading needs.**

**Blockquote and list markers do not hang.** They sit on the body column and push their text inward.
`REFERENCE.md` § 4.1's "`>` = 2" is not what this app draws.

Heading ink is 40 px tall against body's 38 at the default size, the same for every level, and the
same `#cccccc`: **bold, and the same size and colour as body**, as § 4.1 says.

Paragraph spacing: a blank Markdown line is exactly one empty line. Body-to-body pitch is 73 px and
a paragraph break measures 145–146 px = 2 × 73, with **no extra paragraph margin**. Heading-to-heading
pitch is 74 px, the one pixel being the heading's taller line box.

## State 15 — Typewriter mode

`mac-native-15-dark-typewriter.png`, captured as the whole window (3024 × 1898).

With Typewriter on and the caret driven to the end of a line deep in the document, the bar sits at
y 912 … 984, **mid-point y = 948.0 of a 1898 px window — 49.9 %**.

**The caret line is held at the vertical centre of the window.**

## Window chrome

From `mac-native-00-dark-window-chrome.png`, the whole window with its alpha kept: all four corners
are fully transparent and the first opaque pixel on the top row is at x = 35, on the left column at
y = 35 — a **corner radius of about 35 px (17.5 pt)**. Rounded window corners are present, as § 4.1
says.

## The three offset frames

`mac-native-01-dark-caret-offset-00.png`, `-10.png`, `-20.png` — the caret at offsets 0, 10 and 20 of
the same line, which is what says the bar tracks the grid rather than happening to land on it once:

| offset | bar | centre | boundary 691.0 + 25.6 n |
|---|---|---|---|
| 0 | x 688 … 693 | 691.0 | 691.0 |
| 10 | x 944 … 949 | 947.0 | 947.0 |
| 20 | x 1200 … 1205 | 1203.0 | 1203.0 |

Steps of **exactly 256 px** between them, which is 10 cells to the pixel.
