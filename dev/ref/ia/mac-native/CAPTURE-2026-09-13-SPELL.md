# Mac captures on the original rig — #400, the misspelling mark

The mark macOS draws under a misspelled word, on both grounds and under everything else the Editor
can be wearing, shot on the **built-in screen of the original 14-inch M1 MacBook Pro**.

No committed capture showed a misspelling before this one: every state up to now was shot on
`dev/ref/sample.md`, which has none, and `VERDICTS.md` had no Spell row. The mark is **macOS's own**,
drawn by the text system rather than by iA, so its colour, its shape, its thickness and where it
sits against the baseline had never been measured — and neither had what the app does around it.
`quill-engine`'s `spell` Role ships a provisional red under Pango's `error` underline until this
lands (ADR 0017).

**Four of the spec's six assumptions hold and two do not.** The mark does **not** dim with a word
Focus has dimmed, and an all-caps word the dictionary does not hold is **not** marked.

All numbers are **device pixels** at backing scale 2 unless labelled pt. The region is the whole
window, `[0, 33, 1512, 949]` in logical points. [The manifest](capture-2026-09-13-spell.json) names
all 26 frames, their originals, both hashes and the switches each was shot under;
`rig/measure_spell_400.py` re-reads every number below off them. The report changes no Quill
geometry, colour or Gate opponent.

## Rig and method

Mono, System — Default, Normal text size, limit 64, Library and Preview hidden, Authors off, window
`{0,33,1512,982}`, on [`dev/ref/spell.md`](../../spell.md) — which is also the `spell` Piece's judged
state and the engine test's fixture, so the capture, the Piece and the test cannot drift.

**Eight states were shot as pairs** — S1 to S4 on both grounds — each once with *Check Spelling
While Typing* on and once with it off and nothing else changed, so every colour, row and dot below
is a difference between two frames and nothing is assumed about what the paper under a mark would
otherwise hold. That is [#354](CAPTURE-2026-09-10-STYLE.md)'s method, and the same reason stands:
the mark is drawn under the glyphs, so a reading off the marked frame alone cannot tell the mark's
ink from a descender crossing it. **S5, S6 and the autocorrect run carry no control** and none is
possible: each of them changes the text, so there is no second frame of the same page to difference
against. They are read as behaviour — what the frame shows — and nothing in the mark's measurements
rests on them.

**Setting the switch is not enough.** A document already on the screen is not re-checked when the
switch changes, and *Check Document Now* is no substitute — it finds the **next** misspelling rather
than marking them all, which left the first run of this capture with one mark on the page instead of
seven. The passage is pasted again under each setting, which is also how a writer meets the mark.

## The mark

| | light | dark |
|---|---|---|
| Ink, on the paper | **`#ed766b`** | **`#cf807e`** |
| Shape | **dots** — 6 px lit, 8 px period | the same |
| Thickness | **6 px** = 3 pt | the same |
| Below the baseline | **12 … 17 px** = 6 … 8.5 pt | the same |
| Extent | the word's own cells: 9.92 of 10 for `definately`, 3.05 of 3 for `Teh` | the same |

**It is a row of dots, not a wave and not a line.** Every mark on every state reads the same:
6 px of ink, 2 px of paper, repeating on an 8 px period — **3 pt dots on a 4 pt pitch**. The band is
six rows deep and each dot is six columns wide, so the dots are round to the pixel.

**It is not the face's underline.** `iAWriterMonoS-Regular.ttf` asks for `underlineThickness`
60/1000 em — 2.56 px here — at `underlinePosition` −110/1000 em, 4.69 px below the baseline. The
mark is 6 px thick and sits 12 px below. macOS draws its own thing at its own size.

**The two grounds do not share an ink.** `#ed766b` on light and `#cf807e` on dark are not one colour
on two papers: flattening the light value onto the dark paper at any coverage gives nothing near
`#cf807e`. It is a dynamic system colour with a value per appearance, which is what a port has to
carry — two values, not one with an alpha.

## What the mark does under everything else

### Under Focus it does not dim — and the word does

| | word's ink | mark |
|---|---|---|
| light, lit sentence | `#191919` | `#ed766b` |
| light, dimmed sentence | **`#c6c4c2`** | **`#ed766b`** |
| dark, lit sentence | `#cccccc` | `#cf807e` |
| dark, dimmed sentence | **`#707070`** | **`#cf807e`** |

The caret stood in the second sentence, so `Teh`, `seperate` and `mispelled` kept the body ink while
`definately`, `recieved`, `comittee` and `accomodate` fell to the dim tier § 4.2 already holds
(4.2.5, 4.2.6). **Their marks did not move at all** — same hex, same rows, same dots. **The spec
assumed the mark dims with the word; it does not.**

### Under Syntax highlight the Category stays and the mark is unchanged

The marked words carry their Category colours — `#a6559f`, `#4675b5`, `#bb512a`, `#9d6722` on light;
`#b490b0`, `#829ebf` and the rest on dark — and the mark is `#ed766b` / `#cf807e` exactly as at rest.
**Both halves of the spec's assumption hold.**

### Over a selection the fill sits under it, and shows through

| | ground | mark |
|---|---|---|
| light, on paper | `#f7f7f7` | `#ed766b` |
| light, over the fill | `#cbedf7` | **`#e2726b`** |
| dark, on paper | `#1a1a1a` | `#cf807e` |
| dark, over the fill | `#143c52` | **`#cd8486`** |

The fill is under the mark — the selection state's own control frame shows the fill unbroken where
the dots land — but **the mark is not opaque**: the same dots render differently over the two
grounds. Solved on the raw frames, where the algebra is valid because normalising is a non-linear
conversion, a straight composite accounts for it at a coverage of about **0.7 on light and 0.85 on
dark**; it does not close to one number, so the dots are antialiased rather than painted flat. What
a port needs is the pair of rendered values above. **The spec assumed the fill sits under it, and it
does.**

## What is marked, and what is not

Seven words of the passage are marked: `definately`, `recieved`, `comittee`, `Teh`, `seperate`,
`mispelled`, `accomodate`.

**`DRAFFT` is not marked.** An all-caps word the dictionary does not hold is left alone. **The spec
assumed it is marked; it is not.** This is the one finding that changes a count: Quill's `spell`
Piece asserts **eight** waves on this passage, and the oracle draws **seven**.

**`2b` is not marked**, and neither is `Q3`. A token carrying a digit is left alone, which is what
the spec assumed.

## The word being typed

| state | what the frame shows |
|---|---|
| `comittee` typed, no space | **no mark.** The word carries a pale blue pending-correction fill and a rounded pill under it reading `Comittee ×` — macOS's suggestion, with a dismiss |
| a space typed after it | autocorrect capitalises it to `Comittee`, and **the dots appear at once**, the caret still on the same line |
| the caret clicked away | the dots stand, the fill and the pill gone |

**A word is marked when a boundary ends it, not while it is being typed** — the spec's assumption.
Ending it is what matters, not the caret leaving: the space alone brings the mark up with the caret
still beside the word.

**Escape and ⌘↑ do not end it.** While the pill is up it takes both, and two runs of this state shot
the caret still sitting in the word before a mouse click was used instead. A port that wants to
reproduce the pill will have to reproduce that too.

## Autocorrect — for the map's fog entry, not for the spec

| typed | after the space | after one Backspace |
|---|---|---|
| `teh` | **`Teh`** — capitalised, *not* corrected to `the` | the space goes; the word returns to the pending-correction fill and pill |
| `recieve` | **`Receive`** — corrected *and* capitalised | the space goes; the pill now reads **`recieve ↺`**, offering what was typed back |

**The replacement is underlined, and not with the misspelling mark.** Autocorrect draws a **solid**
pale-blue rule under the word it replaced; the misspelling mark is red dots. The two are different
marks and a reader can tell them apart at a glance.

**The first Backspace does not revert.** It deletes the space and re-opens the pending-correction
state, with the revert offered in the pill rather than taken.

## The correction menu

A right-click on `definately` selects the word — which **keeps its dots over the selection fill** —
and opens, in order:

1. **`definitely`**, **`defiantly`** — two suggestions, no more
2. `Report a Concern`
3. — separator — `Ignore Spelling`, `Learn Spelling`
4. — separator — `Look Up “definately”`, `Translate “definately”`
5. — separator — `Cut`, `Copy`, `Paste`
6. — separator — `Paste As ›`, `Paste Edits From ›`, `Mark As ›`
7. — separator — `Show Writing Tools`, `Proofread`, `Rewrite`, …

Rows 4 to 7 are macOS's own context menu, which any text view gets; rows 1 to 3 are what spell check
adds. The menu is chrome and Quill's own, so it is on record rather than to be copied.

**Aim at the word, not at the row.** The mark is dotted, so a row's differing columns run from the
first word's first dot to the last word's last one, and their midpoint is a word in between: the
first run of this state right-clicked `Tuesday` and opened a data detector's menu.
