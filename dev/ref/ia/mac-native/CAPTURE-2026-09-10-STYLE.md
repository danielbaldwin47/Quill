# Mac capture on the original rig — #354, the Style Check mark

The one state no committed capture had ever held. Every frame in
`dev/ref/ia/shots/mac-native/` up to this run was shot with Style Check **off** — the rig's own README
says to turn it off before shooting anything, because reading a style-check marker as a selection is
the mistake [#154](https://github.com/danielbaldwin47/Quill/issues/154) exists to stop repeating — so
the mark's colour, thickness and position had never been measured, and the only pictures of it were
marketing stills.

The six answers [#354](https://github.com/danielbaldwin47/Quill/issues/354) asked for:

- **The mark is not a line over the ink; it is a line *in* the ink.** A struck run is re-inked into
  the quiet marker tier — **`#7a7a78`** dark, **`#b5b3b0`** light — and a **2 px** rule of the same
  colour is drawn through it. Both values are already in the repo: they are the ink a link's
  brackets and destination carry (VERDICTS 4.2.13, `Role::Link` in `theme.rs`).
- **The rule sits on the middle of the x-height**, 2 px thick, its centre **10–11 px above the
  baseline** against an x-height of 22 px. It is not the face's own strikeout metric.
- **One mark for all three lists.** Fillers, Clichés and Redundancies draw the same colour, the same
  thickness and the same rows.
- **A struck word in a dimmed sentence dims with the sentence, and the two dims do not compound.**
  Out of focus a struck word is drawn at the ordinary Focus dim, `#707070` / `#c6c4c2`, exactly the
  value every other word out there carries.
- **A struck word loses its Category colour outright** under Syntax highlight, and is drawn at the
  struck ink — the same value it has with Syntax off.
- **The fill sits under the mark.** A selection changes neither the struck ink nor the rule, and the
  rule is drawn over the fill in the same value it has over paper.

And the question the ticket asked last: **iA strikes the word that can be deleted**, not a fixed one
of the pair. `basic` fundamentals, combined `together`, fell `down`, `past` history — iA's own
marketing page bolds `fundamentals`, and the app strikes `basic`.

All numbers are **device pixels** at backing scale 2 unless labelled pt or em. Bounds are
`{left,top,right,bottom}`; regions are `[x,y,w,h]` in logical points.
[The manifest](capture-2026-09-10-style.json) names all 19 frames, their originals, both hashes,
the region and the colour check; `rig/run_style.py` shot them and `rig/measure_style_354.py` re-reads
every number below off them into [`style-354-read.json`](style-354-read.json). The report records
observations. It changes no Quill colour, geometry or Gate opponent.

## Rig and provenance

| Item | This run | 2026-09-10 earlier (#343, #344) |
|---|---|---|
| Machine | MacBook Pro 14-inch, `MacBookPro18,3`, M1 Pro | the same |
| Display | **built-in** Color LCD, Liquid Retina XDR | the same |
| Device / logical | 3024 × 1964 px, 1512 × 982 pt | the same |
| Backing scale | 2.0 | the same |
| Display profile | `Display #1 2025-12-15 13-06 2.4 F-S 1xCurve+MTX` (DisplayCAL) | the same |
| macOS | 27.0 (26A428) | the same |
| iA Writer | 8.0.6 | the same |
| Window bounds | `{0,33,1512,982}` | the same |
| Editor | Mono; System — Default; Normal text size; 64-cell limit | the same |
| Modes | Library and Preview hidden, Typewriter and Authors off; **Style Check on** | Style Check off |
| Region | `[0,33,1512,949]` — the whole window | `[0,33,1512,949]` for #343 |

Every frame went through `colour.normalise()` into `rig/display.icc` and then `colour.check()`, which
fails unless the paper and the body ink land back on the § 4.2 values. All 19 pass.

The passage is [`dev/ref/style.md`](../../style.md), written by this ticket so that the capture, the
`style` Piece's judged state and the engine test's fixture cannot drift apart. It wraps to five rows
of the 64-cell measure, and the row texts are the reader's only assumption about the layout:

```
# Style check

Basically, the plan was pretty much finished, and we were sort of ready to get down to brass
tacks. Against all odds the team combined together the basic fundamentals into one very short
draft. The long and short of it: the draft was a little rough, and it fell down only where the
past history ran too long.
```

## Method: every reading is a difference between two frames

Each of the ticket's five states was shot twice — once with Style Check on, once with it off and
everything else identical — so no reading depends on knowing the face. The mark itself is read in
the **gap columns**: the columns a row's control frame leaves blank between two glyphs. Whatever is
drawn there in the Style Check frame is the mark and nothing else, so its colour is sampled off
paper, its thickness is a row count, and its position is a row band the same row's glyphs are
measured against.

| Frame | State | Ground |
|---|---|---|
| `24-{dark,light}-style-off` | S0, the control every other reading is a difference against | both |
| `24-{dark,light}-style-all` | S1, all three lists, caret at the end of the document | both |
| `24-light-style-list-{fillers,cliches,redundancies}` | S2, one list at a time | light |
| `24-{dark,light}-style-focus-sentence` + `-focus-sentence-nostyle` | S3, Focus Sentence, caret in the second sentence | both |
| `24-{dark,light}-style-syntax` + `-syntax-nostyle` | S4, Syntax highlight, all five Categories | both |
| `24-{dark,light}-style-selection` + `-selection-nostyle` | S5, `Against all odds` held | both |

### The menu will not say which lists are on

The four list items — Fillers, Clichés, Redundancies, Custom — carry **no `AXMenuItemMarkChar` in
either state**, and the menu drawn open shows no check beside them either. Their four parents *are*
verbs, so a parent's state is read off its name: `Enable`/`Disable Style Check`,
`Enable`/`Disable Focus Mode`, `Show`/`Hide Syntax`, `Show`/`Hide Authors`.

A list's state is therefore **measured, not asked**. `run_style.py` clicks a list, shoots, and keeps
the click only if the frame moved the way the click should move it; the walk from unknown to
all-off is in the manifest's `lists` block and reads as a ledger:

| Click | px differing from the Style-Check-off control, before → after |
|---|---|
| Fillers off | 35 191 → 22 970 |
| Clichés off | 22 970 → 6 209 |
| Redundancies off | 6 209 → 80 |
| Custom off | 80 → 80 — **no visible change either way**, so the click was taken back |

The residual 80 px is the caret's own antialiased fringe. **Custom is an empty list**: it moves
nothing in either direction, which is the one state this method cannot name, and it is recorded as
left-as-found rather than guessed at. Style Check was switched off and on between S3, S4 and S5, and
the three-list state came back identical every time — 35 191 px on dark and 34 880 px on light, to
the pixel.

## The mark

| | dark | light |
|---|---|---|
| Paper | `#1a1a1a` | `#f7f7f7` |
| Body ink | `#cccccc` | `#191919` |
| **Struck ink — the glyphs and the rule alike** | **`#7a7a78`** | **`#b5b3b0`** |
| Rule thickness | **2 px** = 1 pt = 0.047 em | the same |
| Rule, above the baseline | bottom edge **9–10 px**, top edge **11–12 px** | the same |
| x-height at this size | 22 px | 22 px — 21 where antialiasing takes the top row |

**The glyphs and the rule are one colour.** Sampling a struck run's glyph stems with the rule's own
rows excluded returns the same value as sampling the rule in the gap between two glyphs, on every
struck phrase in both grounds. Style Check does not draw a line over body ink; it re-inks the run
and rules it.

**The value is not new.** `#7a7a78` / `#b5b3b0` is the quiet marker tier already measured in state 17
and recorded in VERDICTS 4.2.13 — the ink a link's `[`, `]`, `(`, `)` and destination URL carry, and
already `Role::Link` in `quill-engine/src/theme.rs`. It is not a tint of the body ink either: the
per-channel alpha that would flatten `#cccccc` onto `#7a7a78` over dark paper is 0.53, and over light
paper 0.30 — two different numbers, and neither survives the selection state below. It is an opaque
colour of its own, very slightly warm (`b` two steps under `r` and `g` on dark, three on light).

**The rule is centred on the x-height, not set by the face.** `iAWriterMonoS-Regular.ttf` asks for
`yStrikeoutSize` 60/1000 em = 2.56 px and `yStrikeoutPosition` 309/1000 em = 13.18 px at this size;
the rule is drawn 2 px thick with its centre 10–11 px above the baseline, which is **half the
face's x-height** (516/1000 em = 22.02 px) to within a pixel — the row-to-row pixel of difference is
the 73 px pitch not landing on whole rows. The rendering ignores the face's own strikeout metrics
and uses the x-height.

**The rule covers the matched phrase's cells and stops at the space either side.** `Basically,`
takes 9.92 cells of 10 — the trailing comma included — `get down to brass tacks` 22.93 of 23,
`basic` 4.96 of 5, every one within a pixel of its own cell span. A phrase broken by a wrap is ruled
on each row over its own cells only: `sort of` is 4.02 cells at the end of row 1 and 1.95 at the
start of row 2, with nothing ruled in between.

## One mark for all three lists

Read off the three single-list frames, every rule is `#b5b3b0`, 2 px, on the same rows relative to
its row's baseline. There is no per-list colour, no per-list thickness and no per-list position, so
the spec's assumption of one mark holds.

## What each list owns

| List | struck in this passage |
|---|---|
| **Fillers** | `Basically,` · `pretty much` · `sort of` · `very` · `a little` · `only` · `too` |
| **Clichés** | `get down to brass tacks` · `Against all odds` · `long and short of it` · `past history` |
| **Redundancies** | `together` · `basic` · `down` · `past` |

Two things fall out of the table.

**Which word of a redundancy is struck: the one that can be deleted.** Not the first, not the
second — `basic` of `basic fundamentals` and `past` of `past history` are the first word,
`together` of `combined together` and `down` of `fell down` are the second. **This contradicts iA's
own marketing page, which bolds `fundamentals`.** The app strikes `basic`.

**Two lists can claim overlapping text, and the longer match wins where they overlap.** `past
history` is a cliché to the whole phrase and a redundancy to `past` alone; with both lists on the
rule runs the full 12 cells, the cliché's span.

**Two abutting spans draw as one unbroken rule.** `down` is a redundancy and `only` a filler, and
they sit either side of one space. With both lists on the rule is continuous across that space — all
46 px of the gap's mark rows are drawn — while the spaces at `fell|down` and `only|where` carry
nothing. The mark is drawn per contiguous struck range, not per word.

## S3 — under Focus Sentence, the dims do not compound

Caret in `the team`, so the second sentence is the focused one. Struck words fall on both sides of
the boundary: `together`, `basic` and `very` are inside it; `Basically,`, `pretty much`,
`sort of`, `get down to brass tacks` and `Against all odds` are outside.

| | dark | light |
|---|---|---|
| inside the sentence, unstruck | `#cccccc` | `#191919` |
| inside the sentence, struck | `#7a7a78` | `#b5b3b0` |
| **outside, unstruck** | `#707070` | `#c6c4c2` |
| **outside, struck** | **`#707070`** | **`#c6c4c2`** |

A struck word outside the focused sentence is drawn at **exactly** the Focus dim — the same value
state 13 measured and VERDICTS 4.2.6 and 4.2.5 record, and the same value the unstruck words beside
it carry. Style Check contributes no colour out there at all; only the rule itself survives, and it
is drawn in the dim too. **The spec's assumption that the mark dims with the word is right, and the
stronger claim follows: there is still one dim tier, and out of focus it wins outright.** On dark
the Focus dim is darker than the struck ink (`#707070` against `#7a7a78`) and on light it is lighter
(`#c6c4c2` against `#b5b3b0`) — in both grounds it is the further of the two from the body ink, so
this run cannot distinguish "Focus replaces" from "the dimmer wins".

## S4 — under Syntax highlight, the strike takes the word

All five Categories on. A struck word is drawn at the struck ink and **not** at its Category colour:

| word | Category colour, Syntax only | with Style Check on |
|---|---|---|
| `plan` — unstruck noun | `#ce896d` / `#bb512a` | unchanged |
| `finished` — unstruck verb | `#829ebf` / `#4675b5` | unchanged |
| `short` — unstruck adjective | `#ba9659` / `#9d6722` | unchanged |
| `long` — unstruck adverb | `#b490b0` / `#a6559f` | unchanged |
| `basic` — struck adjective | `#ba9659` / `#9d6722` | **`#7a7a78` / `#b5b3b0`** |
| `history` — struck noun | `#ce896d` / `#bb512a` | **`#7a7a78` / `#b5b3b0`** |
| `very` — struck adverb | `#b490b0` / `#a6559f` | **`#7a7a78` / `#b5b3b0`** |

`long` is the control the passage was built for: the adverb `long.` at the end keeps `#b490b0`
while `long and short of it` — the same word, inside a cliché — is grey. **The spec's assumption
that the strike takes the word's colour is wrong: the word takes the strike's colour.** A struck run
under Syntax highlight is indistinguishable from a struck run with Syntax off.

The ten Category values reproduce #308's table exactly, which is the frame pair's own control.

## S5 — over a selection, the fill sits under the mark

`Against all odds` held, which is a struck cliché of 16 cells.

| | dark | light |
|---|---|---|
| Selection fill, Style Check off | `#143c52` | `#cbedf7` |
| Selection fill, Style Check on | `#143c52` — unchanged | `#cbedf7` — unchanged |
| Selected text, Style Check off | `#cccccc` | `#191919` |
| Selected text, struck | `#7a7a78` | `#b5b3b0` |
| The rule over the fill | `#7a7a78`, 2 px, the same rows | `#b5b3b0`, 2 px, the same rows |

**The spec's assumption holds.** The fill is drawn first and unchanged, the struck ink and its rule
go over it, and a selection neither restores the full ink nor lifts the mark.

This is also where the struck ink proves opaque. Were it the body ink at an alpha, its value over
the fill would differ from its value over paper — flattening `#cccccc` at the dark ground's 0.53
over `#143c52` would give `#778a94`. The measured value over the fill is `#7a7a78`, the same as over
paper, to the digit.

## What this does not say

- **One text size, one face.** Everything here is Mono at the Normal step, where the em is 42.667 px
  and the x-height 22 px. The rule's 2 px and its 10–11 px offset are that size's numbers; whether the
  thickness is a fixed device value or a rounded fraction of the em needs a second step, and the
  `x-height / 2` rule is the reading that would survive one.
- **The Custom list was never seen doing anything.** It is empty on this machine, so nothing here
  says whether a custom match is marked like the other three.
- **Authors stayed off**, so nothing here says how a struck word inside an authored run is drawn.
- **Nothing was read from Preview.** Style Check is an Editor mark.
- **No claim about the tagger.** Which phrases each list matches is this passage's evidence and
  English's, not a specification; #5's nlp-tools research is the place that question belongs.

## Repeating the run

`rig/README.md` has the permissions, the normalisation and the burst. Four things this run learned
that the rig did not already record:

- **`screencapture` will not write a dotted filename.** A scratch frame named `.scratch-00.png`
  fails with "cannot write file to intended destination"; the same frame named `scratch-00.png`
  writes. `run_style.py` keeps its scratch frames in a `scratch/` directory and removes it at the
  end.
- **The Focus menu's item names are read lazily and the first read can be stale.** The very first
  query of that menu in a session came back with `Enable Style Sheck` and `Other` where every later
  one reads `Enable Style Check` and `Reference`. Read the menu once before trusting it, and click
  by the name the current read gives.
- **The four Style Check lists cannot be asked, only measured** — see § The menu will not say which
  lists are on. The four parents are verbs and can be.
- **Do not paste over the app's own sample documents.** `states.reset()` is Select-All then paste,
  and the window that happened to be frontmost held `Writing Tips for Writers.txt`. The run made a
  new library document (File → New in Library) and worked in that.

The rig needs `Pillow`, `numpy` and `pyobjc-framework-Quartz`, which on this machine live in
`.venv-rig/` at the repository root rather than in the system Python.

Restored after the run: dark appearance, Style Check off with its three lists remembered on, Focus
off, Syntax off, Normal text size, the scratch document emptied, window `{0,33,1512,982}`.
