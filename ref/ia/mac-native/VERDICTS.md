# Verdicts: every claim, against iA Writer for Mac running

One row per claim in `ref/ia/REFERENCE.md` §§ 3.5, 4.1 and 4.2 and in
[ADR 0012](../../../docs/adr/0012-editor-draws-its-own-selection.md),
[ADR 0013](../../../docs/adr/0013-caret-on-the-advance-boundary.md) and
[ADR 0014](../../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md), marked **confirmed**,
**contradicted** or **still unknown**, each naming the capture that decides it. Taken from iA Writer
8.0.6 on macOS 27.0 at backing scale 2.0; the method and the full measurements are in
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
| 4.2.13 | Wikilink brackets, link, content-block chip, autocomplete popup, library list | **split**: the **link** is now measured; wikilink brackets, the content-block chip, the autocomplete popup and the library list are **still unknown** — not covered | link text is body ink and underlined; its `[`, `]`, `(`, `)` and destination URL are **`#7a7a78`** dark / **`#b5b3b0`** light; the 4 px underline is **`#545452`** / **`#d5d3d1`** under both | 17-dark-marks, 17-light-marks |
| 4.2.14 | Syntax colours (verbs, adjectives, adverbs, conjunctions, nouns) | **still unknown** — not covered; Syntax was off for every state by the ticket's Method | — | — |
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
