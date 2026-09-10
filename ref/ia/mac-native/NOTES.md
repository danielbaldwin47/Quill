# iA Writer for Mac, running — the capture notes

Every number in `ref/ia/REFERENCE.md` was read off a marketing still until now, and three of those
readings were wrong ([ADR 0013](../../../docs/adr/0013-caret-on-the-advance-boundary.md),
[ADR 0014](../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md)). These captures are the
app itself, driven and measured, for
[#154](https://github.com/danielbaldwin47/Quill/issues/154). The verdicts they carry are in
[VERDICTS.md](VERDICTS.md); this file is how each one was taken and what it measured.

Nothing here changes the spec. The evidence is put where a spec change can be argued from.

## The rig

**Follow-up rig, 2026-09-09:** [CAPTURE-2026-09-09.md](CAPTURE-2026-09-09.md) records the Mac halves of #231, #241, #261, #308 and #328, with per-frame metadata and untouched originals. It uses the same iA version but macOS 26.6.1 and a different display; its chromatic and page-top controls are qualified there. The table below describes the earlier run.

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

Three states need markup `ref/sample.md` does not contain, so they use passages kept beside this
file: [`passage-blocks.md`](passage-blocks.md) (heading, blockquote, list, emphasis) for states 9
and 14, [`passage-markers.md`](passage-markers.md) (all six heading levels) for the gutter ladder,
and [`passage-markup.md`](passage-markup.md) (every mark kind at once) for state 17.

On the Linux box a capture is measured with `magick` or with node and `pngjs` (`tools/ink-coverage.mjs`
is the template); `rig/`'s Python imports Quartz and runs only on the Mac.

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
- **A capture carries the display's profile, so a later capture is converted into the earlier one's**
  (`rig/colour.py`). States 1–16 were shot while the built-in display carried its stock profile;
  it now carries a calibration profile (DisplayCAL, `Display #1 2025-12-15 …`) and the same pixels
  come back as `#252525` paper and `#d1d1d1` ink. The stock profile is kept as `rig/display.icc`,
  taken out of `mac-native-14-dark-markup.png`, and every state-17 frame was converted into it
  before it was measured or committed. The conversion is checked rather than assumed: it lands the
  paper on `#1a1a1a` and the body ink on `#cccccc` exactly, which is what the § 4.2 rows already
  hold, and `colour.check()` fails if it stops doing so. Calibrating the display is the writer's,
  so the rig converts rather than asking for it back.

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

### The page top

**Settled on the original rig (#231).** [The re-capture](CAPTURE-ORIGINAL-MBP.md#231--the-page-top)
ran on the built-in screen of the 14-inch M1 MacBook Pro and reproduces the control to the pixel,
so both questions below are now answered:

- **The band does not scale with the pitch.** The empty document's caret box top is **164 px at
  every step** — pitches 49, 73 and 172 at steps 0, 5 and 13 — and holds at 164 when the default is
  re-shot after the excursion. It is a constant, not `k × pitch`.
- **The title bar is 104 px of it.** With Title Bar → Always Show the bar is an opaque `#222222`
  band over rows 2 … 103, a `#292929` separator at 104, and paper from 105; AX reports the toolbar
  bottom at 52 pt = **104 px** below the window's top edge, agreeing to the pixel. The text does not
  move when the bar is shown. So the editor's own page top, the part a window with opaque chrome has
  a counterpart to, is **164 − 104 = 60 px = 30 pt**.

The 2026-09-09 follow-up read box tops 126 / 132 / 112 at those steps, 32 px above the old control,
and did not reproduce it. That run was a different machine driving an external monitor; the
original rig moved nothing, so those figures describe that rig alone.

How far the first line stands below the top of the editor at scroll 0, at the default text size —
the second figure #227 asks for.

**The editor's top edge is the window's top edge.** `mac-native-00-dark-window-chrome.png` is the
whole window, and its very first row already carries body ink cut off mid-glyph: the text view runs
to the frame, and the title bar over it is transparent and draws nothing of its own. The window was
set to logical `{0, 33, 1512, 982}` for every state, so a capture taken from logical y = 48 begins
**30 device px** below that edge and one taken from y = 90 begins 114 px below it.

| capture | region y | in-capture | below the window's top edge |
|---|---|---|---|
| `mac-native-01-dark-caret-midword.png` | 48 | first ink 155 | ink **185** |
| `mac-native-01-light-caret-midword.png` | 48 | first ink 155 | ink **185** |
| `mac-native-08-dark-selection-inline.png` | 48 | first ink 155 | ink **185** |
| `mac-native-14-dark-markup.png` | 48 | first ink 155 | ink **185** |
| `mac-native-17-dark-marks.png` | 90 | first ink 71 | ink **185** |
| `mac-native-03-dark-caret-empty-document.png` | 48 | caret box 134 … 206 | box **164** |

Four region origins and both grounds agree on the ink. The two figures are two things, read off two
documents: **185 px** is where a *heading's* ink begins, `ref/sample.md` opening on
`# The Lighthouse`, and **164 px** is where the *line box* begins, given by the empty document,
whose caret is the pitch tall (134 … 206) and has no glyph to be read instead. The box is the
figure another app can hold to; the ink is what a shot shows. **The page top is 164 device px
(82 pt)**, at the default step and at scale 2.

Two things *these* captures cannot say, both answered by the re-capture at the head of this
section:

- **How much of the band is the title bar's.** The text runs to the frame, so the 164 px is measured
  from there; how much of it is room left for the invisible title bar over the text cannot be read
  out of a capture that never draws one. Shown, the bar is an opaque band **104 px** deep, leaving
  **60 px = 30 pt** of the editor's own page top.
- **Whether it scales with the pitch.** 164 px is 2.25 × the 73 px pitch, which one point cannot tell
  from a constant. No committed capture at another step is at the document top: every one of the
  fourteen `11-*` frames opens its first line near y 900, some 750 px below where four captures at
  the same step and the same region origin put it, so the sweep was not shot at the top of the
  document. (Its origin was never recorded either — `shots/oracle/states.json` § `opponent` says so,
  from #165.) Shot at steps 0, 5 and 13 at verified document top, the box holds at **164 at all
  three**: a constant.

The numbers are measured; porting them is #319's, and `docs/design.md` row Page top holds Quill at
two pitches until that lands.

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

**Wrapped continuation capture (#241, 2026-09-09):** [four new frames](CAPTURE-2026-09-09.md#241--wrapped-markers) show bullet/quote continuations retaining two cells of indentation and `123.` continuations retaining five, on both grounds and under H6. A wrapped quote does not repeat `>`. The earlier single-row measurements follow.

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

**A heading and the paragraph under it are the same two pitches**, which is the figure #227 asks
for as one number. Heading ink to the first paragraph's ink across a blank line is **146 px** here
(ink tops 155 and 301) and **146 px** again in a second passage
(`mac-native-17-dark-marks.png`, 71 and 217). The other way round, a body paragraph to the heading
under it, is **147 px** (301 → 448) — the extra pixel is the same taller line box as the
heading-to-heading 74. So a heading carries **no margin of its own above or below it**: it stands on
the body grid, and a blank line costs one pitch wherever it falls.

## State 15 — Typewriter mode

`mac-native-15-dark-typewriter.png`, captured as the whole window (3024 × 1898).

With Typewriter on and the caret driven to the end of a line deep in the document, the bar sits at
y 912 … 984, **mid-point y = 948.0 of a 1898 px window — 49.9 %**.

**The caret line is held at the vertical centre of the window.**

## State 16 — Preview: the rendered Markdown, and the split

**Follow-up (#261, 2026-09-09):** [light paper, Template switches, editor-size controls and scroll pairs](CAPTURE-2026-09-09.md#261--preview) measure light Modern at `#fcfcfc` / `#1a1a1a`, confirm centred Classic and Manuscript headings, show Web scaling with editor size and establish two-way scrolling on the longer fixture. The original three dark frames follow.

`mac-native-16-dark-preview-full.png`, `mac-native-16-dark-preview-split.png`,
`mac-native-16-dark-preview-pdf-full.png`

Not one of the ticket's fifteen states. The owner asked for them afterwards, as reference for what
the app makes of the Markdown it has been showing as source all through the states above. All three
are **the owner's own captures** — the window plus macOS's drop shadow, at the same 2× scale, so the
window content sits at x 111 … 3136, y 75 … 1987 (3026 × 1913) and its centre is x 1624.0. The
document is the same sample passage; the template is the app's own **Modern (Sans)**; Dark Mode is
**on** for all three.

### Web preview, Full

| | measured |
|---|---|
| preview paper | **#101010** |
| toolbar | #181818 |
| footer | #101010 |
| heading `The Lighthouse` ink | x 1452 … 1802, centre **1627.0** against a window centre of **1624.0** |

Two things the editor does not do:

- **The preview's paper is `#101010`, not the editor's `#1a1a1a`.** The rendered page is a distinctly
  darker ground than the pane the source is typed into.
- **Headings are centred.** In the editor a heading is left-aligned on the body column with its
  marker hanging into the gutter; rendered, it is centred in the measure — 3 px off the window's own
  centre, which is the glyph rounding.

The markers themselves are gone: `#`, `##`, `**` and `*` do not appear, the emphasis is carried by
weight and slope, and the `-` list becomes real bullets. The face is proportional, not Mono.

### Web preview, Split

| | measured |
|---|---|
| divider | x **1621** — **49.9 %** across the window, an even split |
| left (editor) pane ground | **#1a1a1a** |
| right (preview) pane ground | **#101010** |
| editor pane ink span | x 145 … 1501 (w 1357) |
| preview pane ink span | x 1769 … 2977 (w 1209) |

The split is the clearest single frame in this whole set for what the editor is and is not: the same
paragraph in Mono with its markers showing on the left, proportional and rendered on the right, and
**the two panes carry different papers** — `#1a1a1a` against `#101010` — side by side in one window.

### PDF preview, Full

| | measured |
|---|---|
| toolbar | **#ffffff** |
| surround | **#f7f7f7** |
| page | white, x 962 … 2284, **width 1323**, centre **1623.0** |

**The whole window goes light, with Dark Mode still on.** Web preview honours the dark appearance
and PDF preview does not: it renders paper as paper, and takes the toolbar, the surround and the
footer with it. The page is centred in the window and paginated — the page number `1` sits at its
foot.

The page's *height* cannot be read from this capture: its top edge is flush against the toolbar, so
the sheet is clipped by the viewport and no paper size can be fitted to it. Only its width and its
centring are measured here.

## State 17 — marker ink at rest

`mac-native-17-dark-marks.png` and `-light-marks.png`, from
[`passage-markup.md`](passage-markup.md), which puts every mark kind
[#198](https://github.com/danielbaldwin47/Quill/issues/198) asks about in one screenful: `#` and
`##`, inline code, `**bold**`, `*italic*`, a bare URL, a `[named](url)` link, `>`, `-`, `1.`, both
task boxes, `---`, and a fenced block with a `rust` info string. Region `(0, 90, 1470, 800)` in
logical points, shot by `rig/run_markup.py`.

**The caret is driven to the end of the document before each frame.** The caret's own line could
lift a marker, and *resting* ink is what this state measures. `-caret-on-heading.png`, both grounds,
is the control that says whether it does.

`rig/inks.py` reads the frames: it groups a line's glyph runs by the ink each carries, so a line
drawn in one colour prints one row and a line that changes ink prints one row per ink. A run's ink
is the colour furthest from the paper that the run holds at least six times, so a stem's covered
pixels answer and its antialiasing does not.

### Every mark kind, both grounds

| line | mark | dark | light |
|---|---|---|---|
| `# Heading one` | `#`, hung at cell −2.0 | `#cccccc` | `#191919` |
| `## Heading two` | `##`, hung at cell −3.0 | `#cccccc` | `#191919` |
| *(state 14)* | `###` … `######`, dark only | `#cccccc` | not shot |
| body | `` ` `` inline-code marks | `#cccccc` | `#191919` |
| body | `**` and `*` | `#cccccc` | `#191919` |
| body | bare URL `https://example.com/plain` | `#cccccc` | `#191919` |
| body | link text inside `[…]` | `#cccccc` | `#191919` |
| body | link `[`, `]`, `(`, `)` and the destination URL | **`#7a7a78`** | **`#b5b3b0`** |
| `> Quoted line one.` | `>` | `#cccccc` | `#191919` |
| `- bullet item` | `-` | `#cccccc` | `#191919` |
| `1. ordered item` | `1.` | `#cccccc` | `#191919` |
| `- [ ] task not done` | `-`, `[`, `]` and the text | `#cccccc` | `#191919` |
| `- [x] task done` | the whole row, marker and text | **`#7a7a78`** | **`#b5b3b0`** |
| `---` | all three hyphens | `#cccccc` | `#191919` |
| ` ```rust ` | the three backticks **and** the info string | `#cccccc` | `#191919` |
| ` ``` ` | the closing backticks | `#cccccc` | `#191919` |

**Every Markdown marker rests at the body ink, on both grounds.** `#cccccc` on `#1a1a1a` is
10.84:1 and `#191919` on `#f7f7f7` is 16.41:1 — the body's own contrast, to the unit, at every
mark kind. There is no resting marker grey, no quiet tier and no hair tier: `---` is drawn at the
same ink as a heading's `#`, and a fence's info string at the same ink as its backticks.

The two rows that are not body ink are not markers quieted. `- [x] task done` is **Settings →
Editor → Completed tasks → Fade**, which was on, and it fades the item's text with its marker; the
link value is the destination and its punctuation, not the syntax of a block. Both grounds put that
one value just above the focus dim tier — `#7a7a78` at 4.05:1 against dim's 3.51:1, `#b5b3b0` at
1.95:1 against dim's 1.62:1 — so it is its own ink, not the dim role reused.

### The control: the caret lifts nothing

`-caret-on-heading.png` is the same frame with the caret back on the H1's line, and it reads
identically, ink for ink, on both grounds. The only difference is the bar standing at the `#`'s
left edge, which takes the run's measured x0 from 641 to 643. **A marker's ink does not depend on
where the caret is.**

### Found here: the link's ink and the code ground

Three values fall out of the same two frames.

| | dark | light |
|---|---|---|
| link punctuation and destination URL | `#7a7a78` (4.05:1) | `#b5b3b0` (1.95:1) |
| link underline, 4 px tall | `#545452` (2.29:1) | `#d5d3d1` (1.39:1) |
| code ground, inline and fenced alike | `#252525` (1.14:1) | `#eeeeee` (1.08:1) |

The underline is the same colour under a full-ink bare URL as under the quieted destination, so it
is its own ink rather than a tint of the text above it.

**What it runs under is the URL and nothing else**, read off `17-dark-marks` a row at a time
(#198 phase 2, from this repo). The dark frame carries exactly two rules, each 4 px tall and each
`#545452` to the pixel:

| rule | rows | x | what is above it |
|---|---|---|---|
| the bare URL, first half | 402–405 | 1844 … 2045 | `https://`, where the row wraps |
| the bare URL, second half | 476–479 | 692 … 1123 | `example.com/plain` |
| the link's destination | 476–479 | 1639 … 2275 | `https://example.com/named` |

The link's own row reads `[` at x 1315, its **words** in body ink at 1336 … 1575, then `](`, the
destination and `)` in the link grey out to 2293. The rule starts at 1639 — after the `](` — and
stops at 2275, before the `)`. So the words carry no rule, the brackets carry no rule, and a bare
URL carries one over its whole length, wrap and all. This is the row `VERDICTS.md` 4.2.13 states,
and it corrects the phrase "link text is body ink and underlined" this section first carried.

The fenced block's ground runs x 680 … 2341, 11 px left of the body column at 691.0 and 12 px past
the measure's end at 2329.4 — about 0.43 cells of bleed each side — and 222 px tall over three
lines. An inline run's ground is the run's own cells plus about 3 px each side (x 944 … 1282 for
13 cells starting at cell 10) and 64 px tall against the 73 px pitch.

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

## State 18 — default-size narrow windows

[#328’s matched 960- and 1040-point captures](CAPTURE-2026-09-09.md#328--matched-narrow-windows) retain full Editor boundaries and selection edges. Default L adapts to width: about 22.7 px per cell and 63 px pitch at both requested widths, against 25.6 and 73–74 at 1512. The report records the geometry contradiction and a proposed full-width crop; no Gate opponent is changed.

## State 21 — Syntax categories and token boundaries

[#308’s ten frames](CAPTURE-2026-09-09.md#308--syntax) cover all five categories on both grounds, light isolation, Sentence dimming and contractions. The report and JSON manifest give the ten measured colours in both the original and reference profiles. Dim replaces category colour; contractions can split into differently coloured tokens. Chromatic portability is qualified by the caret control.

**Anchored on the original rig.** [The re-capture](CAPTURE-ORIGINAL-MBP.md#308--syntax) shoots the
same four states on the built-in screen of the 14-inch M1 MacBook Pro and lands **within 1 unit per
channel of all ten colours**, of both dim greys, and on the same five token splits. The caret
control the qualification rested on misses for a reason that reaches nothing else: it is the only
oracle colour on the panel's gamut edge, so `colour.normalise()` clamps it, while every Category
colour round-trips exactly. The caret's colour is **Display P3 `#00bfff`**. The ten values stand.
