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

**Reject `nlprule`** (18.3 MiB of English binaries, ~350 ms to deserialize, no release since
2021-04-24 with the author on record that he is not returning, and it is a grammar-error engine,
not a style checker) and **reject every ML approach** (hundreds of MB of
runtime and per-sentence latency in the tens of milliseconds, against a per-keystroke budget of
single-digit milliseconds).

## Options

| Option | POS tags | Style lists | On-disk | License | Maintained | Verdict |
|---|---|---|---|---|---|---|
| `harper-brill` 2.8.0 | UPOS, Brill tagger | no | 644 KB model | Apache-2.0 | yes (2026-08-13) | **adopt for Syntax highlight** |
| `nlprule` 0.6.4 | Penn Treebank, dictionary + disambiguation rules | no | 18.3 MiB (EN), ~350 ms load | MIT/Apache-2.0 code, LGPL-2.1 data | no (2021-04-24) | reject |
| Own lexicon in an `fst::Map` | most-frequent-tag only | no | ~0.3–1 MB | depends on word list | n/a | fallback only |
| `harper-core` lints | via `harper-brill` | 2 filler words, 5 boring words | ~1.4 MB + crate | Apache-2.0 | yes | wrong shape (grammar checker) |
| `rust-bert` / `candle` / `ort` | state of the art | no | ~860 MB unpacked; 25–36 ms/sentence | Apache-2.0 / MIT | `rust-bert` stale | reject (weight and latency) |
| Quill phrase lists + `aho-corasick` | n/a | **yes, ours** | ~50–200 KB | ours (GPL-3) | n/a | **adopt for Style check** |

## What the feature actually has to do

iA's own support pages define the target, and they matter because Quill is judged against iA Writer.

Syntax highlight (<https://ia.net/writer/support/editor/syntax-highlight>): "Adjectives in brown /
Nouns in red / Adverbs in purple / Verbs in blue / Conjunctions in green". Five categories, one
colour per word — not a parse tree, not a grammar judgement.

Style check (<https://ia.net/writer/support/editor/style-check>) crosses out three categories, and
iA's own examples give the game away:

- Fillers: "basically, pretty much, sort of" — and iA notes "There are hundreds of filler words"
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

**Maintenance — disqualifying, and stated by the author.** Latest release 0.6.4, published
2021-04-24 (crates.io API). Last commit on `main` 2022-02-04; 670 stars, 27 open issues, not
archived. Asked "project dead?" in issue
[#88](https://github.com/bminixhofer/nlprule/issues/88) (2024-02-28), the author replied in full:

> Hi, unfortunately yes, I am not working on this at the moment and don't plan to return soon. I am
> maintaining this project (fixing bugs) but not more.

No commit has landed since, so treat even the bugfix promise as lapsed. It does still compile —
crates depending on `nlprule ^0.6` were published as recently as 2026-08 and build on docs.rs — but
this is a frozen 2021-era dependency for a crate that would sit on Quill's hot path.

**Size — MEASURED.** Downloaded the 0.6.4 release assets and decompressed them:

| Asset | Compressed | Uncompressed |
|---|---|---|
| `en_tokenizer.bin` | 7,165,515 B (6.83 MB) | 11,590,973 B (11.05 MiB) |
| `en_rules.bin` | 1,003,688 B (0.96 MB) | 7,555,031 B (7.21 MiB) |
| **Total** | **7.79 MiB** | **18.26 MiB** |

Both are `bincode`-serialized and are deserialized whole by `Tokenizer::new` / `Rules::new`, so the
uncompressed figure is roughly the resident cost, plus allocator overhead. 18 MiB of resident
dictionary for a typography-first editor is a poor trade when 1.4 MB does the job.

**Load time — the real killer, and it was investigated and never fixed.** `Tokenizer::new` and
`Rules::new` are whole-blob `bincode::deserialize_from` calls with no lazy or mmap path. Criterion
measurements posted in [PR #70](https://github.com/bminixhofer/nlprule/pull/70) (English, Ryzen 7
3700X) give **`load tokenizer` 350.94 ms** and **`load rules` 32.82 ms** — roughly 330 ms combined
after the further ~12% improvement that landed in 0.6.4. The cost is not regex compilation (regexes
are lazy) but rebuilding the tagger's bimaps from an FST. Issue
[#56](https://github.com/bminixhofer/nlprule/issues/56) opens "The biggest issue using this library
currently is the fact, that on each startup a _lot_ of regular expressions are compiled"; the fix
discussed there (zero-copy via rkyv or capnproto) was judged "clearly out of scope" and never done.

**Per-sentence cost — actually fine.** Issue [#6](https://github.com/bminixhofer/nlprule/issues/6)
benchmarks 10,000 Tatoeba sentences at 12.542 s, i.e. **~1.25 ms per sentence** including PyO3
binding overhead. The README's table gives English 843 disambiguation rules (100% of
LanguageTool's) and 3,725 grammar rules (~85%), at relative time 1 against LanguageTool's 1.7–2.0.
So nlprule's steady-state speed is not the objection — its 18 MiB and its ~350 ms cold load are.

**Tagging model.** Dictionary lookup from LanguageTool's morfologik English dictionary plus 843
disambiguation rules; the Penn Treebank tagset is confirmed by inspecting the binary (MEASURED:
`strings en_tokenizer.bin` yields NN, NNP, JJ, NNS, VB, VBN, VBD, VBG, VBP, RB, CD, VBZ, NNPS, IN,
MD, JJR, UH, DT, CC, RBR, plus LanguageTool's own `NN:U`, `NN:UN`, `PCT`, `ORD`, `RB_SENT`). This
is a real disambiguating tagger, not a bare lexicon, so accuracy would be respectable.

Three properties would still hurt Syntax highlight, and they are worth recording because they apply
to any LanguageTool-derived tagger:

- **It returns a tag *set*, not a tag.** `Word::tags()` is documented as "Multiple pairs of (lemma,
  part-of-speech) associated with this token. Order is in general not significant" — so `tags()[0]`
  is not a "best" tag, and Quill would have to collapse the set to one colour itself.
- **`IN` conflates preposition with subordinating conjunction**, so *if / as / since / because*
  cannot be cleanly separated from *of / to / with*. Conjunction-green would be systematically wrong
  one way or the other. `harper-brill`'s UPOS keeps `ADP` and `SCONJ` apart, which is exactly the
  distinction iA's green needs.
- **Out-of-dictionary words are tagged literally `UNKNOWN`** with no statistical fallback.

**License — actually fine, and this is worth recording.** The README states: "nlprule is licensed
under the MIT license or Apache-2.0 license, at your option" and "The nlprule binaries (*.bin) are
derived from LanguageTool v5.2 and licensed under the LGPLv2.1 license." The FSF's license list
(<https://www.gnu.org/licenses/license-list.html>) says of LGPLv2.1: "It is compatible with GPLv2
and GPLv3." Independently, LGPL-2.1 §3 lets a copy be converted to the ordinary GPL — "(If a newer
version than version 2 of the ordinary GNU General Public License has appeared, then you can specify
that version instead if you wish.)" — which reaches GPLv3 directly. Either route works, and Quill
being copyleft sidesteps the unresolved argument in issue
[#81](https://github.com/bminixhofer/nlprule/issues/81) about whether `include_bytes!` of LGPL data
binds a *proprietary* downstream binary. **nlprule is rejected on weight and maintenance, not on
license.**

**One packaging trap worth remembering regardless.** `nlprule-build`'s `BinaryBuilder` performs a
**blocking HTTP GET to GitHub Releases from `build.rs`** to fetch the `.bin` files. That breaks
offline, sandboxed and reproducible builds — precisely the conditions of a PKGBUILD and a Flatpak
manifest (open issue [#84](https://github.com/bminixhofer/nlprule/issues/84), "Be more responsible
about network requests"). Any future dependency that downloads model data at build time should be
rejected on this ground alone; vendored, committed data is the only shape that packages cleanly.

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
dictionary), against nlprule's 18.3 MiB — and for scale, the entire `harper-ls` binary for
x86_64-linux-gnu, grammar rules and all, is 9.8 MiB. Harper's README claims it "take[s] milliseconds
to lint a document, take[s] less than 1/50th of LanguageTool's memory footprint" and is "even small
enough to load via WebAssembly"; `demo.md` claims "For most documents, Harper can serve up
suggestions in under 10 ms, faster that Grammarly." Harper's whole premise — local, small, fast,
private — is Quill's premise.

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

- **Runtime — MEASURED.** `rust-bert` needs libtorch, and its README pins the version exactly
  ("This package requires `v2.4`"). `libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip` is
  189,478,225 B (180.7 MiB) to download and **762 MiB unpacked**, of which `libtorch_cpu.so` alone
  is **472 MiB**. ONNX Runtime is far lighter — `libonnxruntime.so` is 27.2 MiB — but the weights
  dominate either way.
- **Weights — MEASURED.** `rust-bert`'s own default POS model
  (`mrm8488/mobilebert-finetuned-pos`) is **98.8 MB**, downloaded at first run. A BERT-base POS
  model (`vblagoje/bert-english-uncased-finetuned-pos`) is **438 MB**; DistilBERT token
  classification ~266 MB.
- **Total: ~860 MB unpacked for the lightest `rust-bert` path, against `harper-brill`'s 1.4 MB.**
  That is for the ~2 accuracy points between a Brill tagger and BERT, on a task where J&M reports
  that "HMMs, CRFs, BERT perform similarly" at 97%.
- **`rust-bert` is also stale**: 0.23.0, released 2024-09-29, and its optional ONNX path pins
  `ort` 1.16.3 while `ort` is at 2.0.0-rc.13.
- **`candle`** (MIT OR Apache-2.0) is the one genuinely embeddable runtime — pure Rust on CPU — but
  it ships **no POS or token-classification example**, so Quill would port the classification head
  itself and still carry hundreds of MB of weights.
- **Latency is the decisive number.** HuggingFace's own CPU benchmark
  (<https://huggingface.co/blog/infinity-cpu-performance>) puts *vanilla* DistilBERT — the small
  model — at 49 requests/sec at sequence length 8 and 28/sec at 64, i.e. **20 ms at 8 tokens and
  36 ms at 64 tokens**, batch size 1, on a server-class Ice Lake Xeon. A typical sentence is 20–40
  tokens, so **25–36 ms per sentence**: 1.5–2× over a 16.7 ms 60 fps frame and 3–4× over an 8 ms
  target, before Quill does anything else. BERT-base is roughly 2× slower again, and a laptop is
  slower than that Xeon.
- Harper's own 806 KB BiLSTM chunker is described in its source as "extremely expensive" and is
  memoized behind a 10,000-entry LRU cache. If that is expensive, a 438 MB transformer is not in
  the conversation.
- **iA does not use ML for this either.** From <https://ia.net/writer/how-to/edit-and-polish>:
  "Style Check only runs on your device—Writer doesn't send your text to any servers or cloud. It
  doesn't use AI either." The product Quill is judged against solves this with lists and a tagger.
- `rust-bert` is Apache-2.0 and `candle` MIT/Apache — both GPL-3 compatible per the FSF list
  ("Apache License, Version 2.0 […] This is a free software license, compatible with version 3 of
  the GNU GPL"). License is not the obstacle; weight and latency are.

The decisive point: J&M records that HMMs, CRFs and BERT all land at 97% on English POS tagging. ML
buys nothing here that a Brill tagger cannot nearly match at 1/200th the size.

## Style check — ship Quill's own lists

**No crate does this, and the Rust ecosystem is genuinely thin here.** There is no Rust port of
proselint. A crates.io sweep for prose/style/cliché/readability turns up `writing-analysis` (301
downloads), `textstat` (135, readability formulas only), `text-statistics` (41) and `Rust_Grammar`
(254) — all toys. `languagetool-rust` (52,729 downloads) is an HTTP client to a LanguageTool server,
which the ticket rules out. Harper is the closest real thing and it does not do this either:
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

**Prefer the MIT npm word lists.** They are the same corpora proselint ships, with cleaner
provenance, and MIT is "compatible with the GNU GPL" per the FSF. Entry counts MEASURED:

| List | Entries | Package | License |
|---|---|---|---|
| Clichés | **698** | `no-cliches` | MIT |
| Wordiness | **236** | `too-wordy` | MIT |
| Simplification | **327** | `retext-simplify` | MIT |
| Passive participles | **175** | `passive-voice` | MIT |
| Hedges | **162** | `hedges` | MIT |
| Weasels | **116** | `weasels` | MIT |
| Fillers | **83** | `fillers` | MIT |

`proselint` (BSD-3-Clause) is worth raiding for **redundancies**, which the MIT set lacks:
`checks/redundancy/after-the-deadline` 361 pairs and `checks/redundancy/garner` 88, stored as
`bad phrase,replacement` CSV. Its cliché list is *the same list* — proselint's
`checks/cliches/write-good` (697 entries) and npm `no-cliches` (698) overlap on 696 — so take that
one from npm under MIT and avoid proselint's mixed provenance: its `diction` sub-list derives from
GNU diction (GPL, fine here but not neutral) and its `garner` lists are phrase selections from a
copyrighted usage manual.

**Do not use Wikipedia or Wiktionary for this.** Two reasons, and the second is the one that
matters:

1. The list does not exist. There is no "List of English-language clichés" article; `List of
   clichés` is a redirect to the prose article, and `Category:Clichés` does not exist on English
   Wikipedia.
2. **CC BY-SA 4.0 would break ADR 0003.** Creative Commons made BY-SA 4.0 one-way compatible with
   GPLv3 in 2015, but the FSF's license list spells out the catch verbatim: "Because Creative
   Commons lists only version 3 of the GNU GPL on its compatible licenses list, it means that you
   can not license your adapted CC BY-SA works under the terms of 'GNU GPL version 3, or (at your
   option) any later version.'" Quill is **GPL-3.0-or-later**. Ingesting CC BY-SA text would pin
   Quill to GPL-3.0-**only** — a licence change, not a footnote. Wikimedia text is dual CC BY-SA 4.0
   and GFDL, so this applies to Wiktionary's idiom categories too.

This same trap applies to any future corpus, so it is worth stating as a rule: **no CC BY-SA data in
Quill unless the project first decides to drop "or-later".**

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
- write-good and the MIT word lists — <https://github.com/btford/write-good>, <https://github.com/words>
- iA Writer, Introducing Style Check — <https://ia.net/topics/introducing-style-check>
- iA Writer, Edit and polish — <https://ia.net/writer/how-to/edit-and-polish>
- HuggingFace CPU latency benchmark — <https://huggingface.co/blog/infinity-cpu-performance>
- rust-bert — <https://github.com/guillaume-be/rust-bert>
- libtorch CPU builds — <https://pytorch.org/get-started/locally/>
- fst / transducers writeup — <https://burntsushi.net/transducers/>
- aho-corasick — <https://docs.rs/aho-corasick/>
- FSF license list — <https://www.gnu.org/licenses/license-list.html>
- CC BY-SA 4.0 one-way GPLv3 compatibility — <https://creativecommons.org/2015/10/08/cc-by-sa-4-0-now-one-way-compatible-with-gplv3/>
- Quill's own iA reference sheet — `ref/ia/REFERENCE.md`
