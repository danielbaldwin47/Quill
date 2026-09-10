# Mac re-capture on the original rig — #231 and #308

The re-shoot both tickets asked for after the 2026-09-09 follow-up
([`CAPTURE-2026-09-09.md`](CAPTURE-2026-09-09.md)) ran on a different machine and an external
monitor. This run is on the **built-in screen of the original 14-inch M1 MacBook Pro**, the
display the committed captures were taken on.

Both controls the tickets set as gates were shot first. **#231's positional control reproduces
exactly**; **#308's caret control does not**, and the cause is a clamping defect in
`rig/colour.normalise()` that this report identifies, bounds, and shows does not reach the ten
Category colours. The report records observations; it changes no Quill geometry, colour, Template
or Gate opponent.

All numbers are **device pixels** at backing scale 2, relative to the capture named, unless
labelled pt. Bounds are `{left,top,right,bottom}`; regions are `[x,y,w,h]` in logical points.
[The manifest](capture-original-mbp.json) names every frame, its original, both hashes, its region
and its colour check.

## Rig and provenance

| Item | This run | 2026-09-09 (Dell rig) | Earlier committed captures |
|---|---|---|---|
| Machine | MacBook Pro 14-inch, `MacBookPro18,3`, M1 Pro | a different MacBook | the same 14-inch M1 MacBook Pro |
| Display | **built-in** Color LCD, Liquid Retina XDR | external DELL P2723QE | built-in |
| Device / logical | 3024 × 1964 px, 1512 × 982 pt | 4608 × 2592 px, 2304 × 1296 pt | 3024 × 1964 px |
| Backing scale | 2.0 | 2.0 | 2.0 |
| Display profile | `Display #1 2025-12-15 13-06 2.4 F-S 1xCurve+MTX` (DisplayCAL), committed as [`rig/display-calibrated-2025-12-15.icc`](rig/display-calibrated-2025-12-15.icc) | not recorded | the display's stock profile, kept as [`rig/display.icc`](rig/display.icc) |
| macOS | **27.0 (26A5425a)** | 26.6.1 (25G76) | 27.0 (26A5406e) |
| iA Writer | 8.0.6 (80046) | 8.0.6 (80046) | 8.0.6 (80046) |
| Show scroll bars | Automatic | not recorded | not recorded |
| Window bounds | `{0,33,1512,982}` — 1512 × 949 pt | same | same |
| Full capture | `-R 0,33,1512,949` → 3024 × 1898 px | same | — |
| Control region | `-R 0,48,1470,380` → 2940 × 760 px, 30 px below the window top | — | the region states 1–16 used |
| Editor | Mono; System — Default; 64-character limit; Default text size (step 5) | same | same |
| Modes | Library and Preview hidden; Style Check and Authors off; Focus and Syntax off except their named states | same | same |

The originals, with their own ICC profiles, are in
[`../shots/mac-native/raw-original-mbp/`](../shots/mac-native/raw-original-mbp/); the normalised
copies sit beside the earlier captures. Each original went through `rig/colour.normalise()` into
`rig/display.icc` at relative colorimetric intent, and `rig/colour.check()` passes on every
populated Editor frame. An empty document has no body ink, so its paper alone is checked.

## The controls

### Positional — #231's gate: **reproduces**

| | Required (historical) | This run | The committed frames, read the same way |
|---|---:|---:|---:|
| Heading ink below the window top | 185 | **184** | **184** |
| Empty caret box top | 164 | **164** | 164 |

The 1 px on the heading is a reader difference, not a rig difference: `fast.ink_lines` reads
`y0 = 154` in the y48 control region on **this run's frame and on the committed
`mac-native-01-dark-caret-midword.png` alike**, and 154 + 30 = 184. NOTES' 185 came from
`iarig.ink_rows`, whose threshold crosses one row later. Both readings describe the same pixels.

The stronger check is that the caret is pixel-identical. In the y48 control region this run and
the committed frame both give the caret box **x 816 … 821, y 281 … 352, 426 accent pixels**, and
the following ink rows land at 301, 375 and 448 in both. The 32 px the Dell rig lost is the
machine or the display, as that report suspected; on this rig nothing moved.

### Chromatic — #308's gate: **the caret misses, and the rig's normalisation is why**

| Control | Committed (stock profile) | This run, normalised | This run, display profile |
|---|---|---|---|
| Caret, dark | `#00bfff` | **`#36bffa`** | `#00c8ff` |
| Selection band, light | `#ccedf8` | `#cbedf7` | `#cef1fb` |
| Selection band, dark | `#113d52` | `#143c52` | `#18475c` |

#308's brief predicted the diagnosis — "the fault is then in the rig's normalisation rather than
in the display" — and that is what it is. `colour.normalise()` converts *from* the display's
current profile *into* the stock one, and that direction **clamps at the gamut edge**. Converting
each oracle colour forward into the calibrated profile and back again isolates it exactly:

| Colour | stock → calibrated | → back to stock | stable |
|---|---|---|---|
| Caret `#00bfff` | `#00c8ff` | **`#36bffa`** | **no, 54 in red** |
| Every Category colour, both grounds (10 values) | — | unchanged | yes |
| Selection light `#ccedf8`, dark `#113d52` | — | unchanged | yes |
| Dim `#c7c4c2` / `#707070`, paper, ink | — | unchanged | yes |

The caret is the **only** oracle colour that clips, because it alone sits on the panel's gamut
edge at R = 0. Two consequences:

- **The 2026-09-09 caret reading was not evidence of a display difference.** Its `#51c0fe` and
  this run's `#36bffa` are two clamp residues of the same conversion, on two displays.
- **The caret's colour is Display P3 `#00bfff`.** `P3 #00bfff` maps forward onto exactly the
  `#00c8ff` measured here, and the stock profile reads `#00bfff` for it directly, the stock
  profile of this panel being P3-equivalent. In sRGB it clips to `#00c2ff`. The committed value
  was never wrong; it is not recoverable by an ICC round trip, and no rig whose display carries a
  non-stock profile can pass the gate as it was written.

The two selection bands are in gamut and reproduce to **Δ1 (light)** and **Δ3 (dark)** per
channel, against the Dell rig's Δ5 on the light band. The residue is compositing rounding: both
bands are translucent fills, and the composite happens in the display's space before capture.

**What this means for the Category values.** They are in gamut, they round-trip exactly, and each
is measured below within Δ1 per channel of the 2026-09-09 reading. The chromatic doubt #308 was
reopened on does not reach them.

## #231 — the page top

Steps 0, 5 and 13, passage and empty document, document top verified per frame; the default
control re-shot after the excursion. Text size set through View → Text Size: Make Text Normal
Size, then five Smaller for step 0 or eight Bigger for step 13.

| Step | Body pitch | Empty caret box | First ink | Ink − box |
|---:|---|---|---:|---:|
| 0 | 50, 49, 49 | **164** … 212 | 178 | 14 |
| 5 | 74, 73, 73 | **164** … 236 | 184 | 20 |
| 13 | 172, 172, 171 | **164** … 334 | 202 | 38 |
| 5, after the excursion | 74, 73, 73 | **164** … 236 | 184 | 20 |

**The page top is a constant 164 device px — 82 pt — and does not scale with the pitch.** Three
points at pitches 49, 73 and 172 give the same box top to the pixel, which is the question #227
could not answer from one point and #231 exists to settle. The default control holds at 164 and
184 after the size excursion, so the excursion moved nothing.

The heading's ink sits further below the box as the type grows (14 / 20 / 38 px) because a
heading's line box is taller than the body's, not because the page top moved.

### The title bar

View → Title Bar → **Always Show**, at step 5, both documents.

- **It is a real opaque band.** Rows 2 … 103 are a uniform `#222222` across the window's width,
  row 104 is a `#292929` separator, and the paper `#1a1a1a` begins at row **105**.
- **AX agrees to the pixel.** The toolbar reports position (0, 41) and size 1512 × 44 pt against a
  window at (0, 33): bottom 85 pt, **52 pt = 104 px** below the window's top edge.
- **The text does not move.** With the bar shown, the heading's ink is still at 184 and the empty
  document's caret box still at 164 … 236 — the same as with the bar faded out.

So the title bar's height is **T = 104 px = 52 pt**, and the 164 px band from the window's top
edge is 104 px of title bar plus **60 px = 30 pt** of the editor's own page top. That is the half
of the figure a window with opaque chrome has a counterpart to, and the half #227 could not read.

## #308 — Syntax

`ref/ia/mac-native/passage-syntax.md` for S1–S3 and `passage-syntax-tokens.md` for S4, Mono at the
default step, window `{0,33,1512,982}`, Syntax on.

### The ten Category colours

Each is the repeated solid glyph colour, read off the all-five frame and confirmed against the
single-Category frame, which gives the **identical pixel count** — so the Categories do not
overlap and no colour is an antialiased edge.

| Category | Light, normalised | Light, display | Solid px | Dark, normalised | Dark, display | Solid px |
|---|---|---|---:|---|---|---:|
| Noun | **`#bb512a`** | `#c6542f` | 8318 | **`#ce896d`** | `#d68e72` | 8095 |
| Verb | **`#4675b5`** | `#487ebe` | 6977 | **`#829ebf`** | `#87a6c7` | 6725 |
| Adjective | **`#9d6722`** | `#a76d24` | 3433 | **`#ba9659`** | `#c39c5d` | 3306 |
| Adverb | **`#a6559f`** | `#af5aa8` | 1925 | **`#b490b0`** | `#bc96b8` | 1845 |
| Conjunction | **`#51812f`** | `#588a33` | 990 | **`#89a474`** | `#90ac79` | 962 |

**Every one is within 1 unit per channel of the 2026-09-09 reading**, on a different machine and a
different display. Those values stand; this run anchors them.

### Dim wins over colour

Focus Sentence on, all five Categories on, caret in the final sentence. In the dimmed rows there
is **no Category colour at all** — the whole row is one grey.

| Ground | Dimmed ink | Dimmed px in a full row | Focused row still carries |
|---|---|---:|---|
| Light | **`#c6c4c2`** | 8158 | noun, verb, adjective, adverb |
| Dark | **`#707070`** | 7991 | noun, verb, adjective, adverb |

The spec's assumption holds, and the greys match the 2026-09-09 pair (`#c7c4c2` / `#707070`) to
Δ1 and Δ0.

### A contraction splits

`passage-syntax-tokens.md` read cell by cell against the 62-character line (advance 25.371 px,
ink x 695 … 2267):

```
I'll write, but I can't stop. it's a well-known writer's book.
.VVVVVVVVV. CCC . VVAAA VVVVV ..VV ..AAAA.VVVVVVNNNNNN...NNNN.
```

| Token | Split |
|---|---|
| `I'll` | `I` plain, `'ll` **verb** |
| `can't` | `ca` **verb**, `n't` **adverb** |
| `it's` | `it` plain, `'s` **verb** |
| `well-known` | `well` **adverb**, `-` plain, `known` **verb** |
| `writer's` | `writer` **noun**, `'s` plain |

All five match the 2026-09-09 reading. A whole-word rule loses every one of these distinctions.

## Repeating the run

`rig/README.md` has the permissions, the normalisation functions and the burst selection. Three
things this run learned that the rig did not already record:

- **The caret reads as a Verb.** It is a saturated cyan bar whose hue falls in the blue band, so
  any scan that classifies glyph colours by hue counts it as a fifth Category and a Category set
  can never be verified. Drop it by colour — no Category colour comes near its blue, so
  `b ≥ 0xe0 and b − r ≥ 0x80` removes it and nothing else.
- **A Category toggle needs about 2.5 s to settle.** iA re-tags the document asynchronously;
  a frame shot sooner shows the state before the toggle, which reads as a chaotic menu.
- **Set the Category state on the syntax passage, then swap the document.** A Category with no
  matching word in the passage is indistinguishable from one that is off, so the state cannot be
  verified on the tokens passage. `states.reset()` replaces the text without touching the toggles.

Focus Mode is Focus → Enable Focus Mode; enable it **before** clicking the scope, or the scope
click swallows the next one. The dedicated capture document is `The Lighthouse.txt` in iA's own
library; every state pastes over it.

Restored after the run: dark appearance, Syntax off, Focus off, Title Bar Fade In/Out, Normal text
size, empty capture document, window `{0,33,1512,982}`. Syntax's remembered Category set is now
all five, where it was Nouns and Conjunctions before.
