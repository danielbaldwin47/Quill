# Style check lists: GPL-compatible sources, and the three seeded files

Research for [#353](https://github.com/danielbaldwin47/Quill/issues/353), under the
[Style check spec](https://github.com/danielbaldwin47/Quill/issues/29) whose resolution comment
fixes the file shape and the matching rules. Collected 2026-09-10. Every number below is either
**MEASURED** here (by re-running the extraction over the fetched source, not by trusting a
README), **QUOTED** from a primary source with its URL, or **ESTIMATE**.

The prior decision this builds on is
[`research/nlp-tools`](https://github.com/danielbaldwin47/Quill/tree/research/nlp-tools)
§ Recommendation and § Style check: Style check is literal phrase matching with `aho-corasick`
over Quill's own lists, seeded from permissively licensed sources and owned as Quill data. That
document named no source and gave no file. This one does both.

## Answer

Four sources seed the three lists, all of them GPL-3.0-or-later compatible, all of them data that
separates cleanly from the code that reads it:

| List | Entries | Sources |
|---|---|---|
| `data/style/fillers.txt` | **259** | npm `fillers` 63, `hedges` 41, `weasels` 8, `too-wordy` 50 (all MIT), Quill 97 |
| `data/style/redundancies.txt` | **530** | proselint `after-the-deadline` 500, proselint's own sets 20, plainlanguage.gov 9 (BSD-3-Clause and CC0), Quill 1 |
| `data/style/cliches.txt` | **702** | npm `no-cliches` 697 (MIT), Quill 5 |

**1,491 phrases in total** (MEASURED — the counts are what `data/style/*.txt` holds after
de-duplication, the cross-list ownership pass and the false-fire pruning below). Attribution and
licence texts are in `data/style/SOURCES.md` beside them; the fixture passage the spec fixes is
`dev/ref/style.md`.

One caveat needs the owner's eye and is set out in § Provenance caveats: the 500-entry redundancy
block comes to us under proselint's BSD-3-Clause but originates in After the Deadline, whose server
is GPL **version 2** with no "or later".

## The licence rule this was held to

`research/nlp-tools` § Style check states it, and it is stricter than "permissive":

> **no CC BY-SA data in Quill unless the project first decides to drop "or-later."**

Because Creative Commons lists only GPL version 3 on its compatible-licences list, adapting CC BY-SA
material would pin Quill to GPL-3.0-**only**, against [ADR
0003](../adr/0003-gpl-3-license.md)'s GPL-3.0-**or-later**. So: MIT/Expat, Modified BSD, Apache-2.0
and public-domain dedications are in; CC BY-SA and CC BY-NC are out, which rules out every Wikipedia
and Wiktionary list without further argument. (Independently, `research/nlp-tools` establishes that
the Wikipedia list does not exist: `List of clichés` is a redirect and `Category:Clichés` is not an
English Wikipedia category.)

## Sources surveyed

### Adopted

**npm `fillers` 2.0.1, `hedges` 2.0.1, `weasels` 2.0.1** — MIT, `Copyright (c) 2014 Titus Wormer`,
the `words/` organisation. Three flat arrays of English words and short phrases exported from
`index.js`: 83, 162 and 116 entries respectively, 289 unique across the three (MEASURED; the
pairwise overlaps are 30, 23 and 31). `fillers` is intensifier and discourse padding
(`absolutely`, `basically`, `literally`, `very`); `hedges` is uncertainty (`kind of`, `more or
less`, `presumably`, `in my opinion`); `weasels` is write-good's vague-attribution set, which is
a different job (see below). Data is one ES-module array with no dependency on the matcher —
fully separable. Attribution: the MIT notice, reproduced as
`data/style/LICENSE-MIT-wooorm.txt`.

**npm `too-wordy` 0.3.6** — MIT, `Copyright (c) 2021 Matt Blair`. 236 entries (MEASURED) of wordy
constructions with no replacements — `due to the fact that`, `at the present time`, `in the event
that`. Separable: a single `let wordyWords = [...]` in `too-wordy.js`, with the matcher in a
sibling file. Only its 117 multi-word entries were even considered for Fillers (its single words —
`abundance`, `accelerate`, `aircraft` — are simplification targets, not fillers), and 50 survived
the pruning in § Phrases that fire too often.

**npm `no-cliches` 0.3.6** — MIT, same copyright. 698 clichés (MEASURED), one flat array in
`cliches.js`. This is the whole of Quill's cliché list bar five. Attribution:
`data/style/LICENSE-MIT-duereg.txt`.

**proselint** at `dbed789` (2026-06-22) — BSD-3-Clause, `Copyright © 2014–2015, Jordan Suchow,
Michael Pacer, and Lara A. Ross`. Its checks are Python modules, but the large corpora are plain
data files beside them (`checks/redundancy/after-the-deadline`, `checks/redundancy/garner`,
`checks/cliches/write-good`), each a `bad phrase,replacement` CSV or a bare list — separable
without touching a line of Python. Taken from it: `redundancy/after-the-deadline`, **361 pairs**
(MEASURED), and proselint's own small sets inline in `redundancy/ras_syndrome.py` (17 pairs) and
`redundancy/misc.py` (Nordquist 4, Wallace 2). Attribution:
`data/style/LICENSE-BSD-3-Clause-proselint.txt`; its third clause forbids using the authors' names
to endorse a derived product, which Quill does not do.

**plainlanguage.gov** at `fd76947` (2025-09-23), the GSA/PLAIN repository behind
<https://www.plainlanguage.gov>. Its `LICENSE.md` reads, verbatim (QUOTED): "As a work of the
United States Government, this project is in the public domain within the United States.
Additionally, we waive copyright and related rights in the work worldwide through the CC0 1.0
Universal public domain dedication." The usable list is
`_pages/guidelines/words/use-simple-words-phrases.md`, a **238-row** "Don't say / Say" table
(MEASURED) in Markdown — separable by reading the table. It is a *simplification* list, not a
redundancy list, and it shows: only **9 of its 238 rows** reduce to a redundancy, i.e. rows where
the recommended replacement is a word-subset of the phrase (MEASURED — `[in order] to`, `until
[such time as]`, `under [the provisions of]`). Those nine are in `redundancies.txt`. The other 229
were left out rather than mis-filed as Fillers.

**iA Writer's published examples** — <https://ia.net/writer/support/editor/style-check>, fetched
2026-09-10. The floor, as the ticket says, not a source to copy. What the page actually gives
(QUOTED, MEASURED as three examples per category and no more): "Fillers (basically, pretty much,
sort of)", "Redundancies (basic fundamentals, combine together, fall down)", "Clichés (against all
odds, brass tacks, long and short of it)", plus "Style Check is currently available for English,
French, Spanish and German" and a description of user-defined Custom Patterns with regular
expressions. The page publishes **no entry count** — the ticket's "hundreds of filler words" is not
on it, so treat that target as the ticket's, not iA's (see § Sizing). All nine example phrases are
in Quill's lists; `pretty much`, `a little` and `brass tacks` appear in **no** surveyed source and
were written for Quill.

### Rejected

**write-good 1.0.8** (MIT) — surveyed and not used *as a source*, because it has no data of its
own: `lib/` is checks, and its word lists are exactly the npm packages above, which Quill takes
directly at their own versions. Adopting write-good would add a dependency edge, not entries.

**proselint `checks/cliches/write-good`** (697 entries) — the same corpus as npm `no-cliches`:
**696 of them are identical** (MEASURED; `no-cliches` alone has `like the plague`, proselint alone
has `you're the boss`, and the two differ on one trailing space). Taking it would add one cliché at
the cost of mixing BSD-3-Clause provenance into an MIT list. Rejected as redundant.

**proselint `checks/cliches/garner` (79) and `checks/redundancy/garner` (88)** — phrase selections
from *Garner's Modern American Usage*, a copyrighted usage manual, as proselint's own source
comment records (`source: Garner's Modern American Usage`). proselint relicenses them BSD-3-Clause;
the selection upstream is somebody's copyrighted editorial work. `research/nlp-tools` had already
called this out and this research agrees: rejected on provenance, not on licence text. Cost of the
rejection: two of the four fixture redundancies (`basic fundamentals`, `combine together`) are in
the garner list, but both are also in `after-the-deadline`, so nothing was lost.

**proselint `checks/cliches/diction` (27)** — derived from GNU diction, which is GPL. GPL data
inside a GPL-3.0-or-later application is fine, but proselint redistributes it as BSD-3-Clause,
which is a conflict upstream, and 27 entries are not worth inheriting it. Rejected.

**npm `retext-simplify` 8.0.0** (MIT) — 122 patterns (MEASURED) of phrase → simpler alternatives.
Same shape as `too-wordy` and the same objection: simplification is not what any of iA's three
lists is. Not used. It is the obvious source if a fourth list is ever wanted.

**npm `passive-voice` 0.1.0** and **npm `e-prime` 0.10.4** (both MIT) — participles and forms of
"to be". Out of scope: #29 fixes three lists and nothing else.

**Harper (`harper-core`)** — Apache-2.0 and already in the tree for Syntax highlight. Its
`linting/filler_words.rs` defines the filler set as `WordSet::new(&["uh", "um"])` and
`boring_words.rs` as `very, interesting, several, most, many` (QUOTED via `research/nlp-tools`).
Seven words. Nothing to take.

**LanguageTool** — LGPL-2.1, and its rules are an XML grammar engine that needs a server. Ruled out
by #5 before this ticket.

**After the Deadline directly** (<https://open.afterthedeadline.com>) — Automattic's, GPL. See
below; Quill takes the corpus through proselint, not from here.

**Wikipedia, Wiktionary and every CC BY-SA list** — rejected by the rule above, without inspection.

## Provenance caveats

**One, and it is worth the owner's minute.** `checks/redundancy/after-the-deadline` is the largest
redundancy corpus with any permissive grant on it, and it is 500 of `redundancies.txt`'s 530
entries. proselint ships it under BSD-3-Clause with no source attribution recorded for it beyond
the filename (MEASURED — `redundancy/misc.py` carries `source:` comments for its garner, Nordquist
and Wallace sets and none for this one). The name points at Automattic's After the Deadline, whose
`atd-server-next` repository carries the bare GNU GPL **version 2** text with no per-file "or
later" statement (MEASURED, <https://github.com/Automattic/atd-server-next>). GPL-2.0-**only** data
in a GPL-3.0-or-later application would be a conflict; GPL-2.0-or-later would not be.

Quill takes it under proselint's BSD-3-Clause grant, which is the licence of the artefact actually
copied, and re-curates rather than reproduces: every pair was converted to the spec's bracket form,
filtered to those where the recommendation is a word-subset of the phrase, extended with inflected
forms, and re-ordered. Individual redundant-phrase pairs are short phrases and facts about English
rather than expression; what a compilation can protect is its selection and arrangement, and
neither survives the conversion. That is the reasoning, not a legal opinion.

**What it would cost to drop it.** The block is delimited in `redundancies.txt` by its own
`# --- ... after-the-deadline ---` comment and can be deleted in one edit. Redundancies would fall
from 530 to 30, and three of the four fixture redundancies (`combine together`, `fall down`,
`past history`) would have to be written into the Quill block by hand — about ten minutes of work
and a list that no longer earns the name. The recommendation is to keep it and record the caveat,
which `data/style/SOURCES.md` does.

## The three files

Shape, exactly as #29's resolution comment fixes it: one phrase per line, `#` comments, lower case,
ASCII apostrophes, inflected forms spelled out as their own lines, redundancies with the struck
words in square brackets. Validated mechanically — 1,491 entries, **0** with a non-ASCII byte, an
upper-case letter, stray whitespace, unbalanced brackets, or (in `redundancies.txt`) no bracket at
all (MEASURED).

Each file groups its entries under a `# --- <source> — <n> entries ---` comment, so "which entries
came from which source" is answerable from the file rather than only from `SOURCES.md`.

### Bracketing

Every redundancy in a source is a `bad phrase,replacement` pair. The conversion is mechanical: walk
the phrase's words, greedily match the replacement's words in order, bracket the runs that are left
over. `adequate enough,adequate` → `adequate [enough]`; `affirmative yes,yes` → `[affirmative]
yes`. A pair whose replacement is not a word-subsequence of the phrase (`bo staff,bō`) cannot be
bracketed and was dropped: **351 of 361** after-the-deadline pairs converted (MEASURED), the other
ten being spelling substitutions rather than redundancies.

One entry disagrees with its source on purpose. After the Deadline reduces `basic fundamentals` to
`fundamentals`, which brackets as `[basic] fundamentals`; #29 fixes the entry as `basic
[fundamentals]`. The spec wins, and the entry is attributed to Quill.

### Inflections

The spec asks for inflected forms as their own lines. Doing that for 530 redundancies by hand is
not on, and doing it for all of them mechanically would be wrong (`[past] history` has no verb).
The rule applied: an entry of the form `<verb> [<particle>]`, where the particle is one of
`together, down, up, out, off, in, back, away, over, forward, ahead, in advance` — **51 entries**
(MEASURED) — expands to its third-person, past, past-participle and progressive forms. Regular
spelling rules, an explicit doubling set (`drop`, `skip`, `plan`, `refer`, `confer`), British
variants where they differ (`cancelled [out]` beside `canceled [out]`), and a hand-written
irregular table for the seven verbs that need one (`fall` → `falls, fell, fallen, falling`; also
`sink`, `meet`, `kneel`, `dive`, `rise`, `shrink`). That is where the fixture's `fell down` comes
from. The expansion took redundancies from 351 converted pairs to 500 (MEASURED).

Fillers and clichés need none: they are not verb-headed.

### One list owns each phrase

All three lists take part in every match (#29), leftmost-longest wins, and matches never overlap —
so a phrase in two lists would be tagged by whichever the matcher reached first, and a toggle would
appear not to work. Twelve phrases were in two lists after the merge (MEASURED): `head honcho` in
redundancies and clichés, `in order to` and seven others in fillers and redundancies, `needless to
say` and two others in fillers and clichés. Precedence is clichés > redundancies > fillers, on the
grounds that the more specific list is the more informative strike. After the pass: **0**
cross-list duplicates and **0** within-list duplicates (MEASURED).

## Phrases that fire too often

This is the part the sources are worst at, because none of them was built for a typography-first
editor that strikes text through with no explanation and no way to dismiss a mark. write-good and
proselint both report to a console with a message and a suggestion; a false positive there costs a
line of output. In Quill it puts a line through a word the writer meant.

**How the sources handle it: they do not.** npm `weasels` — write-good's set — contains `all`,
`about`, `back`, `close`, `better`, `works`, `smelled`, `watched`, `found`. It is a list of words
that *precede* vague attribution, meant to be read with a sentence in front of you, and striking
its members is indefensible. npm `hedges` contains every modal (`can`, `may`, `might`, `must`,
`will`, `would`). proselint's answer is different and instructive: it does not carry these lists at
all as bare words, and where it must (`weasel_words.py`) it uses a regex with a negative
lookahead — `very(?! well)` — which #29's matcher deliberately does not have. iA's answer is a
third one: a short opinionated list plus a user-editable Custom Patterns escape hatch, which #29
puts out of scope as fog.

**What Quill did instead: measure, then cut.** The lists were run over 12,518 words of the
repository's own English prose — `dev/ref/sample.md`, `README.md`, `docs/architecture.md`,
`docs/design.md`, `dev/legacy/BRIEF.md` and `CLAUDE.md`, none of it written with a style checker in
mind — with the same leftmost-longest, whole-word matcher #29 specifies. First pass: **91 strikes,
7.3 per 1,000 words** (MEASURED), and they were not spread out. Seven entries produced 74 of them:

| Entry | Fires | Why it is wrong |
|---|---|---|
| `rather` | 24 | almost every one is `rather than` |
| `it is` | 23 | `too-wordy` wants `it is X that`; the matcher sees `it is` |
| `found` / `finds` / `find` | 16 | an ordinary verb, from `weasels` |
| `it was` | 7 | as `it is` |
| `real` | 4 | the adjective, every time |

Those seven were dropped, along with 59 of `too-wordy`'s 118 multi-word entries that say something
rather than pad — `comply with`, `benefit from`, `similar to`, `prior to`, `is responsible for`,
`on the other hand` — and thirteen content words the hedge list carries for its own purposes
(`estimate`, `guess`, `bunch`, `couple`, `doubt`). Weasels went from 116 candidates to **8 kept**
(MEASURED).

Second pass, same corpus: **18 strikes, 1.4 per 1,000 words** (MEASURED), one strike every 700
words, and no redundancy or cliché fires at all. The remainder is `too` ×5, `exactly` ×4, `at
least` ×2, `just` ×2, and one each of `actually`, `honestly`, `usually`, `truly`, `all of` — every
one of which is a filler a writer might genuinely want flagged.

**`too` is the deliberate exception**, and the ticket names it. It is one of the six Filler phrases
the fixture requires, it is 5 of the 18 remaining strikes, and literal matching cannot tell `ran
too long` from `too many`. There is no fix inside #29's matching rules: no lookahead, no POS, no
exceptions file (fog). It stays in, because the spec's fixture requires it, and it is the strongest
argument in the repository for the user-exceptions file #29 defers. `down` — the ticket's other
example — never enters any list on its own; it appears only inside `fall [down]` and the other 50
particle redundancies, so it never fires alone.

**Two structural rules do the rest of the work.** Single ordinary words are excluded on sight:
177 of the 289 candidate filler words (MEASURED) were dropped by a stoplist of modals, attribution
verbs (`appear`, `seem`, `suggest`, `believe`, `consider`, `think`) and high-frequency function
words. And nothing was taken from a simplification list into Fillers unless it is padding: 186 of
`too-wordy`'s 236 entries were left behind, and 229 of plainlanguage.gov's 238.

## Sizing

The ticket asks for "on the order of iA's 'hundreds of filler words'". iA's support page does not
publish a count (MEASURED — the page gives three examples per category and nothing more), so the
target was read as "hundreds, not dozens". `fillers.txt` holds **259**: 162 from the MIT lists
after pruning, 97 written for Quill — 32 intensifiers and adverbs the MIT lists miss
(`utterly`, `downright`, `ostensibly`, `seemingly`, `indeed`) and 65 discourse-filler
phrases (`the fact of the matter is`, `it goes without saying`, `truth be told`, `when you think
about it`). Adding those is what took the file past 200 without touching the measured fire rate,
which stayed at 1.4 per 1,000 words.

Growth is free at the matcher: `research/nlp-tools` § Style check establishes that Aho-Corasick
cost does not scale with pattern count, so the lists could grow tenfold inside the per-keystroke
budget. The constraint is editorial, not computational.

## Verification

The fixture passage `dev/ref/style.md` was matched with a leftmost-longest, whole-word, case-insensitive
matcher written to #29's rules. All thirteen required phrases match, each tagged by the list the
spec assigns it (MEASURED):

```
   0  fillers       basically
  24  fillers       pretty much
  58  fillers       sort of
  87  cliches       brass tacks
 100  cliches       against all odds
 126  redundancies  combined [together]
 148  redundancies  basic [fundamentals]
 176  fillers       very
 198  cliches       long and short of it
 234  fillers       a little
 257  redundancies  fell [down]
 282  redundancies  [past] history
 299  fillers       too
```

Two of them match through the inflection rule rather than the base entry — `combined [together]`
and `fell [down]` — which is the rule earning its place. No entry in any list overlaps a fixture
phrase in a way that would widen the struck span, so the engine test #29 asks for ("every phrase of
the passage is caught by its list, and leftmost-longest holds") will pass on these files as they
stand.

## Open questions for the implementation ticket

1. **The After the Deadline provenance** above — keep the 500-entry block with the caveat recorded,
   or cut it. Recommendation: keep.
2. **Which word of a redundancy is struck** is [#354](https://github.com/danielbaldwin47/Quill/issues/354)'s
   capture, and the bracket in these files encodes an answer — the bracketed words are the ones to
   strike. If the capture shows iA strikes the whole phrase, the reader ignores the brackets and
   nothing in the data changes.
3. **The user-exceptions file** #29 lists as fog: `too` is the case that argues for it. Worth
   revisiting once Style check has been used on real writing.
4. **Clichés with internal commas** (`no pain, no gain` — 17 entries) match literally, which is
   correct, but they are the only entries whose text contains punctuation. Worth one test.

## Sources

- npm `fillers` 2.0.1, `hedges` 2.0.1, `weasels` 2.0.1 — <https://github.com/words>, MIT
- npm `too-wordy` 0.3.6 — <https://github.com/duereg/too-wordy>, MIT
- npm `no-cliches` 0.3.6 — <https://github.com/duereg/no-cliches>, MIT
- npm `write-good` 1.0.8 — <https://github.com/btford/write-good>, MIT
- npm `retext-simplify` 8.0.0 — <https://github.com/retextjs/retext-simplify>, MIT
- proselint `dbed789` — <https://github.com/amperser/proselint>, BSD-3-Clause
- plainlanguage.gov `fd76947` — <https://github.com/GSA/plainlanguage.gov>, public domain / CC0 1.0
- After the Deadline — <https://open.afterthedeadline.com>, <https://github.com/Automattic/atd-server-next>, GPL v2
- iA Writer, Style Check — <https://ia.net/writer/support/editor/style-check>
- FSF licence list — <https://www.gnu.org/licenses/license-list.html>
- `research/nlp-tools:docs/research/nlp-tools.md` § Recommendation, § Style check
