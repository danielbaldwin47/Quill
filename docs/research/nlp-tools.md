# Part-of-speech tagging and style checking in Rust

Research for [#5](https://github.com/danielbaldwin47/Quill/issues/5). Collected 2026-08-27. Every
number below is either measured here (marked MEASURED), quoted from a primary source with its URL,
or marked ESTIMATE.

## Question

Syntax highlight needs an English part-of-speech tag per word (noun, verb, adjective, adverb,
conjunction). Style check needs cliché, filler and redundancy detection. Both must run inside the
native Rust Quill with no Python and no model server, must not blow the per-keystroke budget, and
must be redistributable under GPL-3.0-or-later ([ADR 0003](../adr/0003-gpl-3-license.md)).

## Recommendation

**Syntax highlight: `harper-brill`** (Apache-2.0, part of Automattic's Harper). A Brill tagger with a
644 KB embedded model emitting Universal POS tags, actively released (2.8.0 on 2026-08-13), which
maps exactly onto iA's five colours. Use `harper-brill` alone, not the whole Harper linting stack.

**Style check: Quill's own phrase lists matched with `aho-corasick`** (Unlicense OR MIT). No crate
implements iA's cliché/filler/redundancy check, and iA's own feature is literal phrase matching, not
grammar analysis. Seed the lists from permissively licensed sources and own them as Quill data.

**Reject `nlprule`** (18.3 MiB of English binaries, no release since 2021-04-24, and it is a
grammar-error engine, not a style checker) and **reject every ML approach** (hundreds of MB of
runtime and per-sentence latency in the tens of milliseconds, against a per-keystroke budget of
single-digit milliseconds).

## Options

| Option | POS tags | Style lists | On-disk | License | Maintained | Verdict |
|---|---|---|---|---|---|---|
| `harper-brill` 2.8.0 | UPOS, Brill tagger | no | 644 KB model | Apache-2.0 | yes (2026-08-13) | **adopt for Syntax highlight** |
| `nlprule` 0.6.4 | Penn Treebank, dictionary + disambiguation rules | no | 18.3 MiB (EN) | MIT/Apache-2.0 code, LGPL-2.1 data | no (2021-04-24) | reject |
| Own lexicon in an `fst::Map` | most-frequent-tag only | no | ~0.3–1 MB | depends on word list | n/a | fallback only |
| `harper-core` lints | via `harper-brill` | 2 filler words, 5 boring words | ~1.4 MB + crate | Apache-2.0 | yes | wrong shape (grammar checker) |
| `rust-bert` / `candle` / `ort` | state of the art | no | ~600 MB (libtorch + weights) | Apache-2.0 / MIT | yes | reject (weight and latency) |
| Quill phrase lists + `aho-corasick` | n/a | **yes, ours** | ~50–200 KB | ours (GPL-3) | n/a | **adopt for Style check** |

## What the feature actually has to do

iA's own support pages define the target, and they matter because Quill is judged against iA Writer.

Syntax highlight (<https://ia.net/writer/support/editor/syntax-highlight>): "Adjectives in brown /
Nouns in red / Adverbs in purple / Verbs in blue / Conjunctions in green". Five categories, one
colour per word — not a parse tree, not a grammar judgement.

Style check (<https://ia.net/writer/support/editor/style-check>) crosses out three categories, and
iA's own examples give the game away:

- Fillers: "basically, pretty much, sort of"
- Redundancies: "basic fundamentals, combine together, fall down"
- Clichés: "against all odds, brass tacks, long and short of it"

Every example is a literal word or phrase. `ref/ia/REFERENCE.md` records the same thing observed in
iA's screenshots: the struck-through spans are `too`, `very` and `a little`. Style check is a
multi-pattern literal match over the text, not a linguistic analysis. That single observation
decides the feature.

## Accuracy expectations on prose

From Jurafsky & Martin, *Speech and Language Processing* 3rd edition, chapter 18 "Sequence Labeling
for Parts of Speech and Named Entities" (draft of 2026-08-19,
<https://web.stanford.edu/~jurafsky/slp3/18.pdf>):

> The accuracy of part-of-speech tagging algorithms (the percentage of test set tags that match
> human gold labels) is extremely high. One study found accuracies over 97% across 15 languages from
> the Universal Dependency (UD) treebank (Wu and Dredze, 2019). Accuracies on various English
> treebanks are also 97% (no matter the algorithm; HMMs, CRFs, BERT perform similarly). This 97%
> number is also about the human performance on this task, at least for English (Manning, 2011).

> The most-frequent-tag baseline has an accuracy of about 92%. The baseline thus differs from the
> state-of-the-art and human ceiling (97%) by only 5%.

The chapter attributes the 92.3% figure to Abney et al. (1999). Figure 18.4 gives the ambiguity that
makes the difference:

| | WSJ | Brown |
|---|---|---|
| Unambiguous types (1 tag) | 44,432 (86%) | 45,799 (85%) |
| Ambiguous types (2+ tags) | 7,025 (14%) | 8,050 (15%) |
| Unambiguous tokens | 577,421 (45%) | 384,349 (33%) |
| Ambiguous tokens | 711,780 (55%) | 786,646 (67%) |

> most word types (85-86%) are unambiguous […] But the ambiguous words, though accounting for only
> 14-15% of the vocabulary, are very common, and 55-67% of word tokens in running text are
> ambiguous. Particularly ambiguous common words include *that*, *back*, *down*, *put* and *set*

This is the load-bearing number for the whole ticket. A pure lexicon lookup cannot beat ~92%,
because more than half of the words on screen are ambiguous and a lexicon has no context to resolve
them. `back` is J&M's own example and it appears in iA's own marketing screenshot coloured purple
(adverb) in "be placed back into their seats" — exactly the case a most-frequent-tag lexicon gets
wrong.

92% vs 97% is not an abstraction in this feature: every wrong tag is a visibly wrong colour, on
screen, while the user types. At 92%, roughly one word in twelve is miscoloured — about four errors
in a 50-word paragraph, which a reader will notice. At 97% it is roughly one in thirty. Any approach
that resolves ambiguity from context is worth real cost here; a bare lexicon is not good enough to
ship as the primary path.

## nlprule

<https://github.com/bminixhofer/nlprule>, <https://crates.io/crates/nlprule>.

**Maintenance — disqualifying.** Latest release 0.6.4, published 2021-04-24 (crates.io API). Last
push to the repository 2023-05-23; 670 stars, 27 open issues, not archived. Five years without a
release, against a 2021-era Rust ecosystem, for a crate that would sit on Quill's hot path.

**Size — MEASURED.** Downloaded the 0.6.4 release assets and decompressed them:

| Asset | Compressed | Uncompressed |
|---|---|---|
| `en_tokenizer.bin` | 7,165,515 B (6.83 MB) | 11,590,973 B (11.05 MiB) |
| `en_rules.bin` | 1,003,688 B (0.96 MB) | 7,555,031 B (7.21 MiB) |
| **Total** | **7.79 MiB** | **18.26 MiB** |

Both are `bincode`-serialized and are deserialized whole by `Tokenizer::new` / `Rules::new`, so the
uncompressed figure is roughly the resident cost, plus allocator overhead. 18 MiB of resident
dictionary for a typography-first editor is a poor trade when 1.4 MB does the job.

**Rule counts and speed.** The README's benchmark table gives English: 843 disambiguation rules
(100% of LanguageTool's) and 3,725 grammar rules (~85%), against LanguageTool v5.2, with nlprule at
relative time 1 versus LanguageTool's 1.7–2.0. The numbers are relative, not absolute, so they do
not answer the per-keystroke question directly.

**Tagging model.** Dictionary lookup from LanguageTool's morfologik English dictionary plus 843
disambiguation rules; the Penn Treebank tagset is confirmed by inspecting the binary (MEASURED:
`strings en_tokenizer.bin` yields NN, NNP, JJ, NNS, VB, VBN, VBD, VBG, VBP, RB, CD, VBZ, NNPS, IN,
MD, JJR, UH, DT, CC, RBR). This is a real disambiguating tagger, not a bare lexicon, so accuracy
would be respectable. It is simply the wrong package: 18 MiB and unmaintained.

**License — actually fine, and this is worth recording.** The README states: "nlprule is licensed
under the MIT license or Apache-2.0 license, at your option" and "The nlprule binaries (*.bin) are
derived from LanguageTool v5.2 and licensed under the LGPLv2.1 license." The FSF's license list
(<https://www.gnu.org/licenses/license-list.html>) says of LGPLv2.1: "It is compatible with GPLv2
and GPLv3." So the LGPL-2.1 rule data could ship inside a GPL-3 Quill. nlprule is rejected on weight
and maintenance, not on license.

## Harper — the recommendation for Syntax highlight

<https://github.com/Automattic/harper>. Apache-2.0. 14,749 stars, last push 2026-08-27.
`harper-core` 2.8.0 published 2026-08-13 (crates.io API). Maintained by Automattic.

Harper as a whole is a grammar checker, which `CONTEXT.md` explicitly says Style check is *not*.
But it is a Cargo workspace, and one member of it is exactly what Syntax highlight needs.

**`harper-brill`** (`harper-brill/Cargo.toml`, version 2.8.0, `license = "Apache-2.0"`) re-exports
`BrillTagger`, `FreqDict`, `Tagger` and `UPOS` from `harper-pos-utils`, and embeds its trained model
in the binary:

- `trained_tagger_model.json` — 659,686 B (644 KB), `include_str!`d and deserialized once through a
  `LazyLock<Arc<BrillTagger<FreqDict>>>`, so the cost is paid on first use, not at startup.
- `trained_chunker_model.json` — 52,717 B, and an optional Burn neural chunker
  (`vocab.json` 627,993 B + `model.mpk` 806,312 B). **Quill needs neither.** The chunker answers
  "is this word inside a noun phrase", which Syntax highlight does not ask. Its own source comments
  call neural-net inference "extremely expensive" and memoize it; skip it entirely.

**The tagset maps exactly onto iA's five colours.** `harper-pos-utils/src/upos.rs` implements the
17-tag Universal POS set (<https://universaldependencies.org/u/pos/index.html>):

| iA colour | UPOS |
|---|---|
| Nouns — red | `NOUN`, `PROPN` |
| Verbs — blue | `VERB`, `AUX` |
| Adjectives — brown | `ADJ` |
| Adverbs — purple | `ADV` |
| Conjunctions — green | `CCONJ`, `SCONJ` |
| uncoloured | `ADP`, `DET`, `INTJ`, `NUM`, `PART`, `PRON`, `PUNCT`, `SYM`, `X` |

`AUX` → verb is confirmed against iA's own rendering: in the TRIM sample transcribed in
`ref/ia/REFERENCE.md`, iA colours `can` and `'ll` blue.

**Accuracy.** A Brill transformation-based tagger is a lexicon-plus-context-rules design: it starts
from a most-frequent-tag `FreqDict` and applies learned patch rules that use surrounding words and
tags. It therefore sits between the 92% lexicon baseline and the 97% ceiling rather than at the
floor. Harper publishes no tagger accuracy figure; the author reports "~95% accuracy on
grammatically correct text" for the *chunker* task
(<https://elijahpotter.dev/articles/training_a_chunker_with_burn>). ESTIMATE: expect low-to-mid 90s
on clean prose, degrading on fragments and headings. Measure it before shipping — see Risks.

**Dictionary.** `harper-core/dictionary.dict` is 787,346 B (769 KB) holding 54,746 lines / 54,697
non-comment entries (MEASURED) in a hunspell-style affix-flagged format, expanded at build time.
`DictWordMetadata` carries per-word `noun`, `pronoun`, `verb`, `adjective`, `adverb`, `conjunction`,
`determiner`, `preposition` fields plus a `pos_tag: Option<UPOS>` "Generated by a POS tagger" —
Quill's five categories are first-class in the data model.

**Total footprint for the tagger path: ~1.4 MB** of embedded data (644 KB model + 769 KB
dictionary), against nlprule's 18.3 MiB. Harper's README claims it "take[s] milliseconds to lint a
document, take[s] less than 1/50th of LanguageTool's memory footprint" and is "even small enough to
load via WebAssembly."

## Lexicon-based tagging — the fallback, and the sizing

If `harper-brill` proves unusable, the fallback is Quill's own word→tag map in an `fst::Map`
(BurntSushi/fst, Unlicense OR MIT — the FSF lists the Unlicense as GPL-compatible).

BurntSushi's own writeup (<https://burntsushi.net/transducers/>) gives the sizing: a 119,095-key
dictionary of 1.1 MB input builds to a **324 KB** FST (29.4% of input) in 0.12s using 9.4 MB. A
larger 3.5M-term Gutenberg vocabulary of 41 MB builds to 22 MB. ESTIMATE for Quill: a ~150k-entry
English word→tag-bitmask map lands in the 300 KB–1 MB range, and lookup is O(length of key),
independent of dictionary size.

So the fallback is cheap in space and time. What it cannot do is beat 92%, per J&M above. Suffix
heuristics (`-ly` → ADV, `-tion`/`-ness` → NOUN, `-ing`/`-ed` → VERB, `-ous`/`-ive` → ADJ) only help
with *unknown* words; they do nothing for the ambiguous known words that are the actual problem, and
they misfire on `only`, `family`, `during`. Treat this as the degraded path, not the plan.

## ML approaches — rejected

Rejected on arithmetic, before any accuracy argument. Quill's whole premise is a fast native editor
packaged for Arch and later Flatpak; the JS oracle's per-keystroke budget is what the native app
must beat.

- **Runtime — MEASURED.** `rust-bert` needs libtorch. The official CPU Linux build
  `libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip` is 189,478,225 B (**180.7 MiB compressed**,
  more once unpacked), per an HTTP HEAD against download.pytorch.org.
- **Weights — MEASURED.** A representative English POS model,
  `vblagoje/bert-english-uncased-finetuned-pos`, ships `model.safetensors` at **417.7 MB**
  (HuggingFace API).
- **Total: ~600 MB against `harper-brill`'s 1.4 MB — a factor of ~430**, for the ~2 accuracy points
  between a Brill tagger and BERT, on a task where J&M reports that "HMMs, CRFs, BERT perform
  similarly" at 97%.
- CPU inference for a transformer runs in the tens of milliseconds per sentence. A 60 fps frame is
  16.7 ms and Quill's typing path must fit inside one. Even Harper's own tiny BiLSTM chunker is
  described in its source as "extremely expensive" and is memoized behind a 10,000-entry cache.
- `rust-bert` is Apache-2.0 and `candle` MIT/Apache — both GPL-3 compatible per the FSF list
  ("Apache License, Version 2.0 […] This is a free software license, compatible with version 3 of
  the GNU GPL"). License is not the obstacle; weight and latency are.

The decisive point: J&M records that HMMs, CRFs and BERT all land at 97% on English POS tagging. ML
buys nothing here that a Brill tagger cannot nearly match at 1/200th the size.

## Style check — ship Quill's own lists

**No crate does this.** Harper is the closest thing in the Rust ecosystem and it does not:
`harper-core/src/linting/filler_words.rs` defines its filler set as `WordSet::new(&["uh", "um"])` —
two disfluencies — and `boring_words.rs` as `very, interesting, several, most, many`. Harper's 335
lint modules are overwhelmingly grammar and usage corrections (`despite_of.rs`, `an_a.rs`,
`allow_to.rs`), each proposing a fix. That is the wrong shape twice over: `CONTEXT.md` defines Style
check as "Underlining clichés, fillers and redundancies in prose" and says *Avoid*: grammar check,
linting. iA's version strikes text through and offers no correction.

So Style check is Quill data plus a matcher, and that is a feature, not a shortfall: the lists are
product surface, they are what Quill is judged on, and iA ships "Custom Patterns" precisely because
the lists are the interesting part.

**The matcher is trivial.** `aho-corasick` (BurntSushi, Unlicense OR MIT) searches for all patterns
simultaneously in a single pass, linear in the length of the text and independent of the number of
patterns. `ref/sample.md`'s paragraphs are 40–69 words (MEASURED), i.e. a few hundred bytes.
Re-matching one paragraph against the ~1,225 phrases below on each keystroke is microsecond work,
and because Aho-Corasick cost does not scale with pattern count, the lists could grow tenfold
without touching the budget. Case-insensitive matching and word-boundary confirmation are
built in (`MatchKind::LeftmostLongest` plus an `ascii_case_insensitive` builder flag).

**Sourcing the lists.** Take care here: the lists ship inside a GPL-3 application, so provenance
matters. Prior art to draw on, all with permissive licenses, checked against the FSF list — the
Expat/MIT license and the Modified BSD license are both listed as "compatible with the GNU GPL":

- **`proselint`** (amperser/proselint, BSD-3-Clause, 4,568 stars, last push 2026-08-26) is the
  strongest seed and its lists are already plain text, one phrase per line — MEASURED:
  `checks/cliches/write-good` 697 phrases, `checks/cliches/garner` 79, `checks/redundancy/after-the-deadline`
  361, `checks/redundancy/garner` 88. That is **1,225 curated phrases** available under a
  GPL-compatible license before writing a single entry.
- `write-good` (btford/write-good, MIT) — weasel words, `too-wordy`, adverbs.
- The `retext-*` plugins (`retext-simplify`, `retext-intensify`, `retext-passive`, MIT).
- Wikipedia's list of English clichés is CC BY-SA 4.0, which Creative Commons declared **one-way
  compatible with GPLv3** in 2015 (<https://creativecommons.org/2015/10/08/cc-by-sa-4-0-now-one-way-compatible-with-gplv3/>):
  content may move into a GPLv3 project but not back. Usable, with attribution.

Curate rather than concatenate. iA's lists are small, opinionated and tuned for false-negative
tolerance — a wrong strike-through in the middle of a sentence is far more damaging in a
typography-first editor than a missed cliché. Store the lists as plain data files in the crate so
they can be reviewed, tested and eventually exposed as iA's "Custom Patterns" equivalent.

## Per-keystroke cost

The engine/UI boundary in [#11](https://github.com/danielbaldwin47/Quill/issues/11) already frames
Syntax highlight, Style check and Spell check as annotators over one token stream. Both recommended
approaches fit that shape and both are paragraph-local:

- **Style check**: one Aho-Corasick pass over the edited paragraph. Microseconds. ESTIMATE.
- **Syntax highlight**: Brill tagging of the edited sentence or paragraph — a dictionary lookup per
  token plus a bounded set of patch rules per token. Sub-millisecond for a 50-word paragraph.
  ESTIMATE; not measured here, because no Rust toolchain is installed on this machine
  (`pacman -Q rust cargo` reports neither package). **This is the first thing the prototype should
  measure.**
- Neither should run on the whole Document per keystroke. Re-annotate the changed paragraph, cache
  the rest. iA does the same: its Windows 2.0 notes say "Features like Spell Check, Syntax Control,
  and Style Check now begin analyzing text a few seconds after the application launches"
  (<https://ia.net/topics/ia-writer-for-windows-2-0>), and `harper-brill`'s `LazyLock` gives Quill
  that deferral for free.

## Risks

1. **Harper's tagger accuracy is unpublished.** No figure exists for `BrillTagger` on prose. Before
   committing, tag `ref/sample.md` and the passages transcribed in `ref/ia/REFERENCE.md` and compare
   the colours against iA's own screenshots — Quill already has iA's ground truth for the TRIM and
   Alice passages, which is a better test set for this app than a treebank.
2. **Training-data provenance.** Harper's models are trained on UD treebanks — the chunker article
   names "GUM + EWT + LINES". UD_English-EWT is CC BY-SA 4.0 (one-way GPLv3-compatible), but
   **UD_English-GUM is CC BY-NC-SA 4.0** — a non-commercial license, which is not free and not
   GPL-compatible. Automattic ships the models under Apache-2.0, and whether a trained model is a
   derivative of its corpus is unsettled, but Quill should not inherit the question silently.
   Mitigation: the training pipeline is in-repo and Apache-2.0 (`harper-pos-utils`, `harper-cli`),
   so a tagger can be retrained on EWT alone if provenance ever needs to be clean. Confirm which
   corpora the *tagger* (not the chunker) used before shipping.
3. **Depending on one workspace member.** `harper-brill` is versioned with the whole Harper
   workspace and released on Harper's schedule, at a fast clip (2.5.0 → 2.8.0 between June and
   August 2026). Pin the version and vendor the model file if churn becomes painful; it is 644 KB of
   JSON and the API surface Quill uses is two functions.
4. **Markup interaction.** Tagging must run on prose with Markdown syntax removed, or `*drowsy*`
   tags as an unknown word. Quill's tokenizer already separates Markup from prose; the tagger must
   consume the prose stream, not the raw line. Harper has its own Markdown parser, but Quill should
   use its own tokenizer and feed `harper-brill` words, keeping the engine boundary clean.
5. **Style-check false positives.** Every entry added to a list is a potential wrong strike-through
   on a user's sentence. Keep lists short, prefer multi-word phrases (`basic fundamentals`) over
   bare words (`very`), and make each list independently toggleable as iA does.
6. **No local measurement was possible.** No Rust toolchain on this machine, so every latency figure
   here is an estimate from documented complexity, not a benchmark. The architecture spike must
   measure tagging and matching on a real paragraph before these recommendations are locked.

## Sources

- iA Writer Syntax Highlight — <https://ia.net/writer/support/editor/syntax-highlight>
- iA Writer Style Check — <https://ia.net/writer/support/editor/style-check>
- iA Writer for Windows 2.0 — <https://ia.net/topics/ia-writer-for-windows-2-0>
- Jurafsky & Martin, SLP3 ch. 18 — <https://web.stanford.edu/~jurafsky/slp3/18.pdf>
- nlprule — <https://github.com/bminixhofer/nlprule>, <https://crates.io/crates/nlprule>
- Harper — <https://github.com/Automattic/harper>, <https://writewithharper.com/docs/contributors/architecture>
- Training a chunker with Burn — <https://elijahpotter.dev/articles/training_a_chunker_with_burn>
- Universal POS tags — <https://universaldependencies.org/u/pos/index.html>
- UD_English-EWT / UD_English-GUM READMEs — <https://github.com/UniversalDependencies/UD_English-EWT>, <https://github.com/UniversalDependencies/UD_English-GUM>
- proselint — <https://github.com/amperser/proselint>
- libtorch CPU builds — <https://pytorch.org/get-started/locally/>
- fst / transducers writeup — <https://burntsushi.net/transducers/>
- aho-corasick — <https://docs.rs/aho-corasick/>
- FSF license list — <https://www.gnu.org/licenses/license-list.html>
- CC BY-SA 4.0 one-way GPLv3 compatibility — <https://creativecommons.org/2015/10/08/cc-by-sa-4-0-now-one-way-compatible-with-gplv3/>
- Quill's own iA reference sheet — `ref/ia/REFERENCE.md`
