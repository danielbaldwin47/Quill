# Verdicts: every claim, against iA Writer for Mac running

One row per claim in `ref/ia/REFERENCE.md` §§ 3.5, 4.1 and 4.2 and in
[ADR 0012](../../../docs/adr/0012-editor-draws-its-own-selection.md),
[ADR 0013](../../../docs/adr/0013-caret-on-the-advance-boundary.md) and
[ADR 0014](../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md), marked **confirmed**,
**contradicted** or **still unknown**, each naming the capture that decides it. Taken from iA Writer
8.0.6 on macOS 27.0 at backing scale 2.0, except the dated 2026-09-09 follow-ups on macOS 26.6.1; the method and the full measurements are in
[NOTES.md](NOTES.md).

**This table is the argument, not the change.** Nothing under `quill/`, `quill-engine/`, `docs/` or
`ref/ia/REFERENCE.md` is touched by the branch that carries it; what the spec does with it is
[#154](https://github.com/danielbaldwin47/Quill/issues/154)'s triage.

"Still unknown" is used two ways and they are marked apart: a claim these states **cannot** decide
(a quotation's provenance), and a claim they simply **did not cover** (the syntax palette).

A short **§ 4.3** block follows § 4.2. It is beyond the sections #154 asks for, and it is here
because three Preview frames were added after the fifteen states at the owner's ask and they bear on
that section.

---

## REFERENCE.md § 3.5 — Caret

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 3.5.1 | No official caret spec is published | **still unknown** — not decidable by capture | — | — |
| 3.5.2 | "iA invested significant energy into making a blue wider caret…", source not retrievable | **still unknown** — not decidable by capture | — | — |
| 3.5.3 | Design Takes Time: the caret is one of the app's basic elements; a gradient cursor was tested and dropped | **still unknown** — not decidable by capture | — | — |
| 3.5.4 | Caret blue in the range #00b5ff–#01c4ff | **confirmed** as a range — but the app's value is a single flat colour it does not name | **#00bfff**, 220 flat px, identical in both themes | 01-dark, 01-light |
| 3.5.5 | Caret width ≈ 0.12–0.17 em | **contradicted** as a constant — holds near the default only | 0.141 em at the default; **0.080–0.186 em** across the 14 sizes; width quantises to 5/6/8/10 px | 11-00 … 11-13 |
| 3.5.6 | Caret height = full line pitch | **confirmed**, at every size, within 1 px | 72 px against a 73 px pitch at the default; 172/172 at the top | 01-dark, 11-00 … 11-13 |
| 3.5.7 | No visible rounding at these sizes | **confirmed** | square end; 1 px of corner antialiasing over the top and bottom two rows only | 01-dark |
| 3.5.8 | Blink not visible in stills; standard platform blink presumed | **confirmed** | **1.000 s period, 0.516 s on / 0.484 s off** at full strength, with ~0.09 s fade ramps | 04-f00 … f12, `blink-idle.tsv` |

## REFERENCE.md § 4.1 — Geometry

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 4.1.1 | Line-length limit 64 / 72 / 80, **64 the default** | **confirmed** | the popup offers exactly 64, 72, 80 with 64 checked | Settings → Editor |
| 4.1.2 | Default text size not published; samples 54.3 px, 41.3 px, 49.5 px | **still unknown** was the state; now **measured** | default em = **42.67 px = 21.33 pt**; the app offers **14 sizes**, em 14.50 pt … 62.58 pt | 11-00 … 11-13 |
| 4.1.3 | Line height 1.71× at 54 px, 1.77× at 41 px, 1.62× elsewhere; iA states spacing is "liquid" | **confirmed**, and it is a **falling curve**, not scatter | pitch/em runs **1.732 → 1.374** monotonically from step 2 up; 1.711 at the default | 11-00 … 11-13 |
| 4.1.4 | Font's own hhea line height is 1.30 em; the app adds ≈ +0.3–0.5 em of leading | **confirmed at the default, and it does not hold across the range** | hhea 1.30 em from the bundle's `iAWriterMono.ttf`; added leading is **+0.41 em** at the default but falls to **+0.07 em** at the largest size, so the +0.3–0.5 em band describes the small end only | font tables + 11-00 … 11-13 |
| 4.1.5 | Blank Markdown line = exactly one empty line; no extra paragraph margin | **confirmed** | paragraph break 145–146 px = 2 × the 73 px pitch | 14-gutters |
| 4.1.6 | Headings sit on the same grid as body lines | **confirmed** | heading-to-heading pitch 74 px against body's 73; the 1 px is the taller line box | 14-gutters |
| 4.1.7 | Character advance (Duo) = 0.6 em | **still unknown** — not covered; every state was shot in Mono, as the ticket's Method requires | **Mono** measures 0.6 em exactly (25.6 px advance, 42.67 px em), matching the font's 600/1000 | grid section |
| 4.1.8 | Text column is horizontally centred within the window | **confirmed** | container x 512 … 2511; centre **1512.0** = half of 3024 | 09-select-all |
| 4.1.9 | Text column = 64 cells = 38.4 em | **confirmed** | measure 691.0 … 2329.4 = 1638.4 px = 64 × 25.6 | 08-cells-05/10/20/40 |
| 4.1.10 | Hanging marker `#` + space = 2 cells | **confirmed** | 1.95 cells | 14-gutters |
| 4.1.11 | Hanging marker `##` = 3 cells | **confirmed** | 2.97 cells | 14-gutters |
| 4.1.12 | Hanging marker `####` = 5 cells | **confirmed** | 4.96 cells; the full ladder is **(level + 1)** — `###`=4, `#####`=6, `######`=7 | 14-gutters |
| 4.1.13 | Hanging marker `>` = 2 cells | **contradicted** | the `>` does **not hang at all** — it sits on the body column (695 against 691). List `-` and ordered `1.` do not hang either | 14-gutters, 14-blocks |
| 4.1.14 | Caret width 9 px / height 93 px at 54.3 px font; 6 × 70 at 41.3 px; 7 × 80; Windows 5 × 51 | **still unknown** for those stills' own sizes; this app's ladder is in NOTES § 11 | 6 × 72 at em 42.67 px | 11-00 … 11-13 |
| 4.1.15 | The caret **sits flush after the last glyph** | **contradicted** | at a hard line end the bar's centre is 1229.0 against an advance boundary of 1228.6, with **7 px of clear paper** between it and the last glyph's ink | 02-dark, 02-light |
| 4.1.16 | Heading style: bold, same size and colour as body | **confirmed** | heading ink `#cccccc`, identical to body; 40 px tall against body's 38, the same for all six levels | 14-gutters |
| 4.1.17 | Rounded window / editor corners present | **confirmed** | all four corners transparent; corner radius ≈ 35 px (17.5 pt) | 00-window-chrome |

## REFERENCE.md § 4.2 — Colours

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 4.2.1 | Editor background light **#f9f9f9**, or #f7f7f7 on ia.net | **split**: #f9f9f9 **contradicted**, #f7f7f7 **confirmed** | **#f7f7f7** | 12-light |
| 4.2.2 | Editor background dark **#1a1a1a** / #1b1b1b | **confirmed** (#1a1a1a) | **#1a1a1a** | 12-dark |
| 4.2.3 | Body text light **#1c1c1c**, or #191919 | **split**: #1c1c1c **contradicted**, #191919 **confirmed** | **#191919** | 01-light |
| 4.2.4 | Body text dark **#cccccc** | **confirmed**, exactly | **#cccccc** | 01-dark |
| 4.2.5 | Dimmed light **#cccccc**, or ≈#c6c4c3 | **split**: #cccccc **contradicted**, ≈#c6c4c3 **confirmed** | **#c6c4c2** | 13-light-sentence |
| 4.2.6 | Dimmed dark **#707070** | **confirmed**, exactly | **#707070** | 13-dark-sentence |
| 4.2.7 | Caret / accent light #00b5ff (webp #01c4ff, Windows #00b2ff) | **contradicted** | **#00bfff** | 01-light |
| 4.2.8 | Caret / accent dark #00b5ff or #00c3ff | **contradicted** | **#00bfff** — and the same value as light, not a per-theme accent | 01-dark |
| 4.2.9 | Selection dark: fill ≈ **#003e4c** | **contradicted** | **#113d52** | 08-dark, 12-dark |
| 4.2.10 | Selection dark: handles #00b5ff | **contradicted** | there are no handles; **no accent pixel at all** in any frame holding a selection | 07–10, both themes |
| 4.2.11 | Selection light: "—" (no value recorded) | **now measured** | **#ccedf8** active, **#dcdcdc** idle | 08-light, 07-light |
| 4.2.12 | Dimmed / focus tiers are plural | **contradicted** | there is **one** dim tier per theme; Sentence and Paragraph share it | 13-* |
| 4.2.13 | Wikilink brackets, link, content-block chip, autocomplete popup, library list | **split**: the **link** is now measured; wikilink brackets, the content-block chip, the autocomplete popup and the library list are **still unknown** — not covered | link text is body ink and carries **no** rule; its `[`, `]`, `(`, `)` and destination URL are **`#7a7a78`** dark / **`#b5b3b0`** light; the 4 px rule runs under the **destination alone** — not the words, not the brackets — at **`#545452`** / **`#d5d3d1`**, and under a bare URL at its full extent | 17-dark-marks, 17-light-marks |
| 4.2.14 | Syntax colours (verbs, adjectives, adverbs, conjunctions, nouns) | **measured and anchored** — the colour-space qualification is lifted | Ten values in [the re-capture table](CAPTURE-ORIGINAL-MBP.md#the-ten-category-colours), reproduced on the original rig within Δ1 per channel; dim wins (`#c6c4c2` light, `#707070` dark) and contractions split | 308-original-mbp-*-syntax-{all,focus,tokens}, five isolated categories |
| 4.2.15 | Authorship author colours | **still unknown** — not covered; Authors were hidden for every state | — | — |

### Marker ink, one row per mark kind

No § 4.2 claim covers a marker's colour, which is why
[#198](https://github.com/danielbaldwin47/Quill/issues/198) went out to measure it rather than port
the Parity oracle's ladder. Every row below is read off state 17, both grounds, with the caret
parked at the end of the document; the paper is `#1a1a1a` dark and `#f7f7f7` light, and "body ink"
is `#cccccc` dark / `#191919` light, 10.84:1 and 16.41:1 against those papers.

**There is no ladder.** Every mark kind rests at the body ink. The Parity oracle's 72 % quiet and
34 % hair have no counterpart in this app, and neither does any single resting marker grey.

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| — | *(no claim)* heading `#` … `######` | **no quiet tier** | body ink. `#` and `##` on both grounds; levels 3–6 on dark only, where 14-gutters already reads them at `#cccccc` | 17-*-marks; 14-gutters |
| — | *(no claim)* blockquote `>` | **no quiet tier** | body ink | 17-*-marks |
| — | *(no claim)* bullet `-` | **no quiet tier** | body ink | 17-*-marks |
| — | *(no claim)* ordered `1.` | **no quiet tier** | body ink | 17-*-marks |
| — | *(no claim)* task box `- [ ]` | **no quiet tier** | body ink, marker and box alike | 17-*-marks |
| — | *(no claim)* completed task `- [x]` | **quieted, and not as a marker** | the **whole row**, marker and text, at `#7a7a78` / `#b5b3b0`; this is Settings → Editor → Completed tasks → **Fade**, a task feature, not markup ink | 17-*-marks |
| — | *(no claim)* thematic break `---` | **no hair tier** | body ink — the same ink as a heading's `#`, where the Parity oracle drops it to 34 % | 17-*-marks |
| — | *(no claim)* fence marks ` ``` ` | **no quiet tier** | body ink, opening and closing alike, over the code ground | 17-*-marks |
| — | *(no claim)* fence info string | **no quiet tier** | body ink — the same ink as the backticks beside it, so the Parity oracle's split between the two is not drawn here | 17-*-marks |
| — | *(no claim)* inline code marks `` ` `` | **no quiet tier** | body ink, over the code ground | 17-*-marks |
| — | *(no claim)* emphasis `**` and `*` | **no quiet tier** | body ink | 17-*-marks |
| — | *(no claim)* bare URL | **no quiet tier** | body ink, underlined at `#545452` / `#d5d3d1` | 17-*-marks |
| — | *(no claim)* code ground, inline and fenced | **now measured** | **`#252525`** dark, **`#eeeeee`** light — one ground for both | 17-*-marks |
| — | *(no claim)* does the caret's own line lift a marker? | **contradicted** | the caret parked on the H1 reads ink for ink the same as the caret parked away | 17-*-marks-caret-on-heading |

## REFERENCE.md § 4.3 — Preview template

Beyond the sections #154 asks for. Three Preview frames were added after the fifteen states, at the
owner's ask, and they touch this section, so what they settle is listed rather than left loose.

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 4.3.1 | The built-in Modern / Classic / Academic template CSS ships only inside the bundle and is not public (GAP) | **still unknown** — the captures render the template, they do not open it | — | 16-preview-full |
| 4.3.2 | `Example.iatemplate`'s public `style.css` is 0 bytes; `GitHub.iatemplate` carries the real values | **still unknown** — not covered; nothing here reads the public template repo | — | — |
| 4.3.3 | Preview classes `night-mode`, `mac`, `content-size-*` | **still unknown** — not covered; the DOM was not opened | — | — |
| 4.3.4 | Light Modern Web paper is white | **contradicted on the 2026-09-09 rig** | `#fcfcfc` paper, `#1a1a1a` ink | 20-light-preview-modern-full/split |
| 4.3.5 | Template switches change alignment, sizes and spacing | **measured on the 2026-09-09 rig** | Classic and Manuscript (Duo) both centre headings; body pitch 68 vs 72 px; [measured line positions](CAPTURE-2026-09-09.md#261--preview) | 20-dark-preview-classic-full, manuscript-duo-full |
| 4.3.6 | Web Preview scales with editor text size | **confirmed** | Editor-controlled steps 0/13: body pitch 46/164 px, different wrapping | 20-dark-editor-step-00/13, preview-modern-step-00/13 |
| 4.3.7 | Scroll synchronization is two-way and scroll-position driven | **confirmed**; exact within-block interpolation remains unmeasured | Preview scrolling changes the Editor scrollbar, with caret still at document start | 20-dark-preview-long-sync-* |
| — | *(no claim)* the Web preview's paper | **now measured** | **#101010**, a distinctly darker ground than the editor's `#1a1a1a` | 16-preview-full |
| — | *(no claim)* rendered heading alignment | **now measured** | **centred** — ink centre 1627.0 against a window centre of 1624.0, where the editor left-aligns a heading on the body column | 16-preview-full |
| — | *(no claim)* PDF preview against Dark Mode | **now measured** | the **whole window goes light** with Dark Mode still on — toolbar `#ffffff`, surround `#f7f7f7` — where the Web preview stays dark | 16-preview-pdf-full |

## ADR 0012 — The Editor draws its own selection

Rows about how **Quill** paints are not iA claims and are out of this table's scope; what is listed
is every statement ADR 0012 makes about **iA Writer**.

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 0012.1 | "macOS iA adds iOS-style round knobs to its two bars" (from `appstore-mac-04`) | **contradicted** | no knobs and no bars; a held selection is fill only | 08, 09, 10, both themes |
| 0012.2 | "Bars at both ends are iA's, not a departure from it" | **contradicted** | zero accent pixels in every frame holding a selection | 07–10, both themes |
| 0012.3 | Bars inset inside the fill, as `msstore-win-04` has them | **contradicted** for the Mac app | that frame is a Windows style-check marker (ADR 0013); the Mac app draws neither an inset bar nor an outset one | 08-dark |
| 0012.4 | The caret is hidden while a selection stands | **confirmed** | no accent pixel in any selection frame, in either theme, at any of the four selection shapes shot | 07–10 |
| 0012.5 | Selection colours are a paint of the focus flag | **confirmed** for the fill | fill swaps `#113d52` → `#464646` (dark), `#ccedf8` → `#dcdcdc` (light); geometry unchanged | 07-dark, 07-light |
| 0012.6 | On losing focus the ends go to ink rather than a paler accent | **moot** — there are no ends; and the idle fill is a **neutral grey**, not a paler accent | `#464646` / `#dcdcdc` | 07-* |

## ADR 0013 — The caret stands on the advance boundary

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 0013.1 | The bar stands on the advance boundary, offset from it by nothing | **confirmed**, and sharpened: the 6 px bar is **centred** on the boundary, 3 px each side | centres 691.0 / 819.0 / 1229.0 against boundaries 691.0 / 819.0 / 1228.6 | 01, 02, 03 |
| 0013.2 | Consecutive offsets step by exactly one cell | **confirmed** | 688 → 944 → 1200: steps of exactly 256 px = 10 × 25.6 | 01-caret-offset-00/10/20 |
| 0013.3 | `NUDGE` = 0.07 em is not iA's | **confirmed** | the offset from the boundary is **0.000 em**; 0.07 em would be 3 px at this size and is not there | 01, 02 |
| 0013.4 | `REFERENCE.md` § 4.1's "flush after the last glyph" is measured wrong | **confirmed** | 7 px of clear paper between the last glyph's ink and the bar at a hard line end | 02-dark |
| 0013.5 | The clear paper at a line end is the last glyph's right side bearing | **confirmed** | ink ends 1218, boundary 1228.6 — 10.6 px of bearing, of which the bar covers the last 2.6; 7 px of paper is left showing | 02-dark |
| 0013.6 | iA's paint order cannot be settled — in no drivable state does its bar meet ink | **confirmed** for the Mac app too | mid-word `gap<` = 1 px, `gap>` = 0 px, **no ink under the bar**: they abut and never overlap | 01-dark |
| 0013.7 | iA has only one mark, which a selection moves to whichever end is active | **contradicted for the Mac app** | the Mac app moves the caret nowhere — it **removes** it for as long as the selection stands. (0013's own table measured the *Windows* build, where the claim holds; the two builds genuinely differ, which is what #154 exists to settle) | 07–10 |
| 0013.8 | Windows drops the caret when the window deactivates, which neither ours nor the oracle does | **confirmed, and it is the Mac app's behaviour too** | 0 accent pixels across a 14-frame burst, both themes, window visible | 06-dark, 06-light |

## ADR 0014 — A selection is a fill and nothing else

| # | Claim | Verdict | Measured | Capture |
|---|---|---|---|---|
| 0014.1 | A selection is its fill rows and nothing else | **confirmed** | four selection shapes × two themes, all fill-only | 07, 08, 09, 10 |
| 0014.2 | No bar at either end | **confirmed** | no accent pixel anywhere in any of those frames | 07–10 |
| 0014.3 | The caret stays out for as long as the selection stands | **confirmed** | same measurement — a bar is the brightest thing in any frame that holds one | 07–10 |
| 0014.4 | Selection fill is `#113d52` (dark) | **confirmed**, exactly | **#113d52** | 08-dark |
| 0014.5 | Caret is `#00bfff` | **confirmed**, exactly, and in **both** themes | **#00bfff** | 01-dark, 01-light |
| 0014.6 | Caret is 6 px wide | **confirmed** at the default size | 6 px; 5–10 px across the app's 14 sizes | 01-dark, 11-* |
| 0014.7 | Caret 63 px against a 60 px band is "the band-is-the-pitch rule inside the quantisation of a 6 px bar" | **confirmed** in shape at this size | caret **72**, band **70**, pitch **73** — the caret is the pitch, the band is 3 px short of it | 01-dark, 08-dark |
| 0014.8 | The idle swap is the fill's alone | **confirmed** | only the fill changes; geometry identical | 07-* |
| 0014.9 | A window that loses focus says what is held "with the paler band" | **contradicted in colour** | the idle band is not a paler accent but a **neutral grey** — `#464646` dark, `#dcdcdc` light | 07-dark, 07-light |
| 0014.10 | `appstore-mac-04`'s 44 px accent runs are touch grab-handles macOS has never drawn | **confirmed** indirectly | the running app draws no handle of any size in any selection state | 07–10 |
| 0014.11 | The active end of a growing selection is not marked | **confirmed** | nothing marks either end; the fill's own edge is all there is | 08, 09, 10 |

---

## Found here, recorded by no claim above

Measurements these states produced that neither `REFERENCE.md` nor an ADR has a row for. They are
listed so the triage can see them, not because anything asks for them.

| Finding | Measured | Capture |
|---|---|---|
| The editor's **text container** is the 64-cell measure plus a **7-cell hanging-marker gutter each side** — 78 cells — and is centred in the window | x 512 … 2511, centre 1512.0 of 3024 | select-all |
| The 7-cell gutter is exactly what `###### ` needs: the deepest heading hangs to x 513, the container's own left edge | 6.95 cells | 14-gutters |
| A held **hard newline** fills to the container's right edge, not a one-cell stub | x 1228 … **2511**, w 1284 | 10-newline-only |
| In a multi-row selection, interior rows fill the **whole container**, the first row starts at the anchor, the last runs from the container's left edge to the focus | 512 … 2511 / 920 … 2511 / 512 … 921 | 09-dark |
| A multi-row band is **vertically continuous** — a lone held row is 70 px, stacked rows abut at the full 73 px pitch with no gap | 292 px = 4 × 73 | 09-dark |
| The selection fill is **under the ink**: glyphs interrupt the band at their own columns | column scan | 09-dark |
| The blink is **suppressed while typing** and resumes 0.633 s after the last keystroke | solid on for 3.448 s | 05-*, `blink-typing.tsv` |
| Typewriter mode holds the caret line at the **vertical centre** of the window | mid-point 948.0 of 1898 = 49.9 % | 15-dark |
| **Typewriter is a value of the Focus scope popup**, alongside Sentence and Paragraph — not a separate mode | Settings → Editor | Settings |
| Past text-size step 7 the 78-cell container exceeds a 1512 pt window, so the column is window-limited rather than 64-cell limited | 78 × cell > 3024 px | 11-08 … 11-13 |
| Split view divides the window **evenly**, and the two panes carry **different papers** — the editor's `#1a1a1a` beside the preview's `#101010` | divider x 1621 = 49.9 % | 16-preview-split |
| In the rendered preview the Markdown markers are gone entirely — no `#`, no `**`, and the `-` list becomes real bullets — so the hanging-marker gutter has no counterpart there | — | 16-preview-full, 16-preview-split |
| The PDF preview paginates: the page is centred, 1323 px wide, with a page number at its foot (its height is clipped by the viewport and cannot be read) | page x 962 … 2284, centre 1623.0 | 16-preview-pdf-full |
| A heading carries **no margin of its own**: heading ink to the paragraph under it across a blank line is the same two pitches as paragraph to paragraph | **146 px** = 2 × 73, twice, in two passages; 147 px the other way round, the heading's taller line box | 14-markup, 17-marks |
| The **page top** — the first line at scroll 0, measured from the window's top edge, which is the editor's own: the text view runs to the frame under a title bar that draws nothing | line box **164 px** (the empty document's caret), a heading's ink **185 px**, at the default step | 01-dark, 01-light, 08-inline, 14-markup, 17-marks, 03-empty-document |
| 2026-09-09: wrapped list/quote continuation rows retain the marker’s indentation, including under H6 | 2 cells for `- ` and `> `; 5 for `123. `; no repeated quote marker | 19-light/dark-wrapped-markers[-h6] |
| 2026-09-09: the old page-top control does **not reproduce** on this rig | Default line box 132 and heading ink 153, both 32 px above the earlier full-window figures; no portable replacement established | 18-dark-page-top-*; [control analysis](CAPTURE-2026-09-09.md#231--page-top) |
| The page top is a **constant**, not a multiple of the pitch | Empty caret box top **164 px at steps 0, 5 and 13** (pitches 49 / 73 / 172), and 164 again when the default is re-shot after the excursion | 231-original-mbp-dark-page-top-empty-step-{00,05,13,05-after} |
| The title bar is an **opaque band 104 px (52 pt)** deep, so the editor's own page top is **60 px (30 pt)** | `#222222` over rows 2 … 103, `#292929` separator at 104, paper from 105; AX toolbar bottom 52 pt below the window top, agreeing to the pixel; the text does not move when the bar is shown | 231-original-mbp-dark-titlebar-always[-empty] |
| The **caret is Display P3 `#00bfff`**, the one oracle colour on the panel's gamut edge | `P3 #00bfff` maps forward onto the `#00c8ff` this rig captures; `colour.normalise()` clamps it back to `#36bffa`, while all ten Category colours, both selection bands and both dim greys round-trip exactly | 308-original-mbp-control-*; [the control analysis](CAPTURE-ORIGINAL-MBP.md#chromatic--308s-gate-the-caret-misses-and-the-rigs-normalisation-is-why) |
| A contraction **splits into differently coloured tokens**, reproduced on the original rig | `I`+`'ll` verb, `ca` verb + `n't` adverb, `it`+`'s` verb, `well` adverb + `known` verb, `writer` noun + `'s` plain | 308-original-mbp-light-syntax-tokens |
| 2026-09-09: default L adapts to window width; a 1040-point control is not the wide rig’s 25.6-pixel cell | 960 and 1040 pt both about 22.7 px/cell, 63 px pitch, 1770 px selection container; width/height controls separate the dependency | 18-light-page-narrow*, page-wide*, width/height/original-control-20-cells |
