# What State 28's frames still hold — #436

Everything [CAPTURE-2026-09-13-LIBRARY.md](CAPTURE-2026-09-13-LIBRARY.md) § What this capture
does not measure named and did not read, plus the name, date and excerpt type sizes and the
date's right inset, read off the twenty committed `mac-native-28-*` frames by
[`rig/measure_library_436.py`](rig/measure_library_436.py). No new capture: the frames are
#379's.

All values are **device pixels at backing scale 2**; `pt` is half. The **frame** column names
the tag — `l1-rest` in the theme the column heads, `l1-rest-nolib` the same window with the
Library hidden, `l7-bare` the Sort bar, Filter bar and Text excerpts all off, `l3-folder`
`Drafts` opened, `o1-dragged` the pane at 500 pt, `l2-search` / `o2-search-the` a query typed.
Coordinates are the whole window's, so the Organizer is columns 0…258, the File List 259…719,
and every inset below is from **259**, the File List's own left edge.

**Type size is one named glyph's ink height**, cap height or x-height as each row says — a text
band's own height mixes ascenders, descenders and the icon beside it. Point sizes are not
readable from pixels.

## The title bar over the pane

| | light | dark | frame |
|---|---|---|---|
| Title bar height | **105 px = 52.5 pt**, rows 0…104 | the same | l1-rest |
| Its bottom rule, over the page | row 104, `#dddddd` | row 104, `#292929` | l1-rest |
| Its bottom rule, over the pane | **none** — the pane's toolbar runs on to row 170 | the same | l1-rest |
| Pane toolbar, title row + Sort bar | rows 0…170, **171 px = 85.5 pt** | the same | l1-rest |
| Rule under the pane toolbar | row 170, `#e0e0e0`, x 259…719 | row 170, `#252525` | l1-rest |
| Ground, over the Organizer | `#eaebeb` — the Organizer's own | `#1a1c1b` — the Organizer's own | l1-rest |
| Ground, over the File List | `#fcfcfc` — the List's own | **`#1c1c1c`**, against the List's `#151515` | l1-rest |
| Ground, over the page | `#fcfcfc`, against the page's `#f7f7f7` | `#222222`, against `#1a1a1a` | l1-rest |
| Library toggle, pane open | **74 × 74 px = 37 × 37 pt**, x 171…244, y 15…88 | 74 × 72 px, x 171…244 | l1-rest |
| Library toggle, pane hidden | the same size at **x 187…260** — it moves 16 px left when the pane opens | the same | l1-rest-nolib |
| Traffic lights | 32 px circles at x 34…65, 80…111, 126…157 — they do not move | the same | both |
| `‹` back button, over the File List | 74 × 74 px = 37 × 37 pt, x 275…348 | 74 × 72 px | l1-rest |
| `+ ⌄` pill, over the File List | 160 × 74 px = **80 × 37 pt**, x 547…706 | 160 × 72 px | l1-rest |
| `‹ │ ›` history pill, pane open | 146 × 74 px = **73 × 37 pt**, x 733…878 — **over the page**, 14 px past the pane's last column | the same box | l1-rest |
| `‹ │ ›` history pill, pane hidden | the same size at x 275…420 — where the pane's back button now stands | the same | l1-rest-nolib |
| Title over the File List | **`☁ iCloud` — the current Location**, not the document | the same | l1-rest |
| — its icon | 30 × 23 px = 15 × 11.5 pt, x 383…412 | 30 × 20 px | l1-rest |
| — its text | x 423…506, **cap height 22 px = 11 pt** (the C), bold | cap height 23 px = 11.5 pt | l1-rest |
| — its ink | `#545454` | `#c6c4c7` | l1-rest |
| Document title | stays in the title bar and **re-centres over the page**: centre x 1852, the page's centre 1871 | the same | l1-rest |
| Document title, pane hidden | centre x 1490, the window's centre 1511 | the same | l1-rest-nolib |
| A document count in the title bar | **none** | none | l1-rest |

## The Sort bar

| | light | dark | frame |
|---|---|---|---|
| Sort bar band | rows 105…170, **66 px = 33 pt** | the same | l1-rest |
| Sort pill | 290 × 50 px = **145 × 25 pt**, x 275…564, y 103…152 | 290 × 48 px = 145 × 24 pt, y 104…151 | l1-rest |
| — shape | a capsule; radius = half its height, 25 px = 12.5 pt | 24 px = 12 pt | l1-rest |
| — ground | `#f6f6f6` against the bar's `#fcfcfc` | `#3a3a3a` against `#1c1c1c` | l1-rest |
| — border | `#e8e8e8`, 1 px | `#686868`, 1 px | l1-rest |
| — left inset in the List | 16 px = 8 pt | the same | l1-rest |
| Sort text `Sort by Date Modified ⌄` | **cap height 17 px = 8.5 pt** (the S), x-height 12 px = 6 pt (the o) | cap 17 px, x-height 13 px | l1-rest |
| — ink | `#767676` | `#9c9c9c` | l1-rest |
| Pill while a query stands | widens to x 275…604, **330 px = 165 pt** (`Sort by Search Relevance ⌄`) | — | l2-search, o2-search-the |
| **A document count** | **none, in any frame** — right of the pill is bare ground in `l1-rest`, `l2-search`, `o2-search-the` and `l8-quick-search` | none | all |
| With the bars off | no toolbar rule and no pill: the list starts under the title bar at row 105 | — | l7-bare |

## The foot

| | light | dark | frame |
|---|---|---|---|
| Rule above the field | rows 1818…1819 — **2 px**, `#dbdbdb`, x 259…719 | `#2e2e2e` | l1-rest |
| Filter field | 424 × 52 px = **212 × 26 pt**, x 278…701, y 1832…1883 | 432 × 60 px with its focus ring, x 274…705 | l1-rest |
| — insets in the List | 19 px = 9.5 pt left, 18 px = 9 pt right | 15 / 14 px, the ring 4 px wider each side | l1-rest |
| — shape | a capsule; radius 26 px = 13 pt | the same | l1-rest |
| — ground | `#fcfcfc` — the List's own | `#161616`, against the List's `#151515` | l1-rest |
| — border | `#dbdbdb`, 1 px, at rest | **`#438587`…`#488a8c` — a focus ring**; the resting border is unshot | l1-rest |
| Filter icon | 26 × 26 px = **13 × 13 pt**, x 294…319 | x 288…319 | l1-rest |
| — icon ink | **`#7e7e7e`** | `#939393` | l1-rest |
| Prompt `Filter` | x 327…385, **cap height 19 px = 9.5 pt** (the F) | the same box | l1-rest |
| — prompt ink | **`#999999`** | `#757575` | l1-rest |
| A typed query's ink | `#1c1c1c` | — | l2-search |
| Caret in the field | absent — the field is at rest | `#36bffa`, 9 × 32 px at x 323…331 | l1-rest |
| Gap below the field to the window's foot | 14 px = 7 pt of List ground | 10 px | l1-rest |
| **A status line below the field** | **none** — no ink between the field and the window's edge | none | l1-rest |
| With the bars off | no rule and no field: the list runs to the window's foot | — | l7-bare |

## A file row

| | light | dark | frame |
|---|---|---|---|
| Pitch, excerpts on | **136 px = 68 pt** — separators at 251, 387, 523, 659, 795, 931, 1067, … | the same | l1-rest |
| Pitch, excerpts off | **64 px = 32 pt** — 185, 249, 313, 377, … | — | l7-bare |
| The List's top inset | **16 px = 8 pt** before the first row | the same | l1-rest, l7-bare |
| Icon | 24 × 30 px = **12 × 15 pt**, x 302…325 | 22 × 28 px, x 303…324 | l1-rest |
| Icon left inset | **44 px = 22 pt** | the same | l1-rest |
| Icon ink | a pale document raster — sheet `#fcfcfc`/`#f5f5f5`, edge `#dbdbdb` and `#d7d7d7` | **the same values**: the icon does not invert | l1-rest |
| Name left inset | **81 px = 40.5 pt** (x 340) | the same | l1-rest |
| Name ink | `#191919` | `#b9b9b9` | l1-rest |
| Name type | **cap height 20 px = 10 pt** (the T of `The Lighthouse.txt`); x-height 15 px = 7.5 pt (the n); ascender 20 px | the same | l1-rest |
| Date | `3:16 AM`, x 593…684 | the same | l1-rest |
| Date right inset | **35 px = 17.5 pt** | the same | l1-rest |
| Date type | **digit height 20 px = 10 pt** (the 3); cap 19 px = 9.5 pt (the A) — **the name's size** | the same | l1-rest |
| Date ink | `#999999` | `#757575` | l1-rest |
| Excerpt type | **cap 20 px = 10 pt** (the W), x-height 16 px = 8 pt (the n) — the name's size again | the same | l1-rest |
| Excerpt ink | `#999999` | `#757575` | l1-rest |
| Separator | **2 px**, `#ededed`, x 340…691 | 2 px, `#212121` | l1-rest |
| Separator insets | 81 px left — the name's — and 28 px = 14 pt right | the same | l1-rest |
| Insets at a 500 pt pane | unchanged: icon 44 px, name 81 px, date right 34 px | — | o1-dragged |

## A folder row

| | light | dark | frame |
|---|---|---|---|
| Height | **64 px = 32 pt**, against a file row's 136 px in the same list | the same | l1-rest |
| — with excerpts off | **64 px = 32 pt — unchanged**: the excerpt switch does not reach it | — | l7-bare |
| Icon | 30 × 26 px = **15 × 13 pt**, x 299…328 | 28 × 24 px, x 300…327 | l1-rest |
| Icon ink | the system blue folder, `#72b9e5`…`#7dc2ec` | `#70b7e3` — the same icon | l1-rest |
| Name | x 342, **cap height 19 px = 9.5 pt** (the D) — a file name's size | the same | l1-rest |
| Date | **none** | none | l1-rest |
| `›` chevron | 10 × 17 px, x 671…680 — right inset 39 px = 19.5 pt | the same | l1-rest |
| `⌄` chevron, folder open | 17 × 10 px, x 667…683 | — | l3-folder |
| Child row indent | **24 px = 12 pt** — icon x 326 against 302, name x 365 against 340, separator x 364 against 340 | — | l3-folder |
| Child row pitch | 136 px = 68 pt — a full file row, date and excerpt included | — | l3-folder |

## Folders at the top, or interleaved

| | reading | frame |
|---|---|---|
| Folders in the list | **one**, `Drafts` | l7-bare (the whole list, 11 rows) |
| Its place | first | l1-rest, l7-bare |
| The dates under it | `3:16 AM` ×6, `3:10 AM`, `12:01 AM`, `9/10/26`, `8/30/26` — Newest on Top | l7-bare |
| Verdict | **undecidable from these frames.** One folder, whose own date is the newest in the list, so *Pin folders to top* and a plain date sort put it in the same row. | — |

## The Organizer

| | light | dark | frame |
|---|---|---|---|
| Section heads | `Locations` y 115…131, `Favorites` 249…265, `Smart Folders` 457…474, `Hashtags` 591…611 | the same rows | l1-rest |
| Head type | **cap height 16 px = 8 pt** (the L), bold, x 33 — left inset 33 px = 16.5 pt | the same | l1-rest |
| Head ink | `#7f8080` | **`#6a6c6b`** | l1-rest |
| Head to its row's top | **17 px = 8.5 pt** — `Locations`' ink ends at 131 and the pill's ground starts at 148. Under `Smart Folders` the gap can only be inferred (474 to a row top of ≈ 490), because a `Recents` row draws no ground. | the same | l1-rest |
| Current-Location pill | 220 × 64 px = **110 × 32 pt**, x 20…239, y 148…211 | the same box | l1-rest |
| — insets in the Organizer | 20 px = 10 pt left, 19 px = 9.5 pt right | the same | l1-rest |
| — corner radius | ≈ 11 px = 5.5 pt — a rounded rectangle, not a capsule | the same | l1-rest |
| — ground | `#d2d3d3`, against the Organizer's `#eaebeb` | `#393b3a`, against `#1a1c1b` | l1-rest |
| — icon | `☁`, 33 × 23 px = 16.5 × 11.5 pt, x 39…71 | the same | l1-rest |
| — label `iCloud` | x 84…160, **cap height 21 px = 10.5 pt** (the C), bold | the same | l1-rest |
| — label ink | `#262626` | `#cacaca` | l1-rest |
| Row (`Recents`) icon | 29 × 24 px = 14.5 × 12 pt, x 41…69 | 30 × 24 px | l1-rest |
| Row text | x 84…178, **cap height 19 px = 9.5 pt** (the R) | the same | l1-rest |
| Row ink | `#262626` — lighter than the File List's `#191919` | `#c2c3c3` | l1-rest |
| Row height | **64 px = 32 pt**, read off the pill — the only row carrying a ground | the same | l1-rest |
| Row pitch | **not measurable** — no section holds two rows | — | all |
| Empty-section prose | x 32…193, x-height 17 px = 8.5 pt, two or three lines | x-height 16 px | l1-rest |
| — prose ink | `#7f8080` — the head's own grey | `#6a6c6b` — likewise | l1-rest |

## Two values the capture report has wrong

| | the report says | the frames say | frame |
|---|---|---|---|
| The Filter prompt's grey | `#7e7e7e` light | **`#999999`** — `#7e7e7e` is the **icon** beside it. The prompt is exactly the pane's secondary grey, `#999999` light and `#757575` dark, the date's and the excerpt's. | l1-rest |
| The Organizer head's dark ink | `#393b3a` | **`#6a6c6b`** — `#393b3a` is the **current-Location pill's ground**. | l1-rest (dark) |

## What these frames cannot show

| | why |
|---|---|
| A Favorites row | no Favorite could be made from the rig — #379's own note, unchanged |
| The Organizer's row-to-row pitch | no section on any frame holds two rows |
| Folders pinned to the top | one folder, and its date is the list's newest |
| The Filter field's resting dark border | the dark `l1-rest` frame has the field **focused** — teal ring, blue caret — so light and dark differ in state, not only in theme |
| The Sort bar's height with only the excerpts off | `l7-bare` turns the Sort bar, the Filter bar and the excerpts off together; no frame separates the three switches |
| The pane toolbar's own extent on light | its ground equals the List's (`#fcfcfc`), so only the dark frame shows where it ends |
| A count anywhere in the pane | none stands in any of the twenty frames — but no frame shoots a Location large enough to force one |
| Point sizes, families and weights | every type value here is an ink height; a pixel cannot name a font |
