# `harper-brill`: what trained the tagger, and whether Quill can ship it

Research for [#307](https://github.com/danielbaldwin47/Quill/issues/307), raised by Risk 2 of
`docs/research/nlp-tools.md` on the `research/nlp-tools` branch, which flagged that Harper's models
may be trained on UD_English-GUM (CC BY-NC-SA) and asked for confirmation about the *tagger* as
opposed to the neural chunker.

Measured 2026-09-07 against `harper-brill` 2.8.0 as published on crates.io.

## Answer, in short

The concern is **confirmed, not dismissed**. The Brill tagger model was trained on the union of
three Universal Dependencies English treebanks — **EWT, GUM and LinES** — and **two of the three,
GUM and LinES, are CC BY-NC-SA 4.0**: non-commercial, which the FSF classes as non-free.

The crate itself is clean: `harper-brill` is **Apache-2.0**, which is one-way compatible with GPLv3,
so the *code* path into a GPL-3.0-or-later app raises nothing. The exposure is entirely upstream, in
the data the embedded 644 KiB JSON was derived from, and it is unadjudicated: whether a word→tag
lexicon extracted from a treebank is a licensed derivative of that treebank has never been settled.

**Verdict: prototype with it, do not ship it unexamined.** Use `harper-brill = "=2.8.0"` to build and
measure [#28](https://github.com/danielbaldwin47/Quill/issues/28) against iA's ground truth, but
before Syntax highlight reaches a release either accept the exposure explicitly in an ADR or replace
the model's base lexicon with a public-domain source. **Do not retrain on EWT alone** — the
mitigation `nlp-tools.md` proposed does not work, for the reason given under
[The EWT escape hatch is not one](#the-ewt-escape-hatch-is-not-one).

## What is actually in the crate

`harper-brill` 2.8.0, published 2026-08-13, is the current release. Its `Cargo.toml` declares:

```toml
[package]
name = "harper-brill"
version = "2.8.0"
description = "The language checker for developers."
license = "Apache-2.0"
repository = "https://github.com/automattic/harper"
```

`.cargo_vcs_info.json` in the published tarball pins it to harper commit
`5d2053bec94f49daaed3cdeae4cc328b897036c6`. Every release back to 1.7.0 carries the same
`Apache-2.0`. The repo's root `LICENSE` is the Apache License 2.0 text; there is **no `NOTICE` file
and no third-party data notice anywhere in the repository**, and the published `.crate` tarball
ships **no `LICENSE` file at all** — so a binary distribution that embeds this model has to carry
the Apache-2.0 text itself to satisfy Apache-2.0 §4(a).

`master` is already at `version = "2.9.1"`, unreleased at the time of writing. Releases run fast:
2.5.0 (2026-06-11), 2.7.0 (2026-07-28), 2.8.0 (2026-08-13).

### The embedded artifacts

Four files, all `include_str!`/`include_bytes!` at module scope in `harper-brill/src/lib.rs`. Sizes
are exact bytes from the published 2.8.0 tarball:

| File | Bytes | KiB | What it is |
|---|---:|---:|---|
| `trained_tagger_model.json` | 659,686 | 644.2 | **The Brill POS tagger — the one #28 wants** |
| `trained_chunker_model.json` | 52,717 | 51.5 | Brill noun-phrase chunker |
| `finished_chunker/model.mpk` | 806,312 | 787.4 | Neural (Burn) NP chunker weights |
| `finished_chunker/vocab.json` | 627,993 | 613.3 | Neural chunker vocabulary |
| **Total** | **2,146,708** | **2,096.4** | ≈ 2.05 MiB embedded in any binary linking the crate |

So `nlp-tools.md`'s "644 KB of JSON" is right to the byte for the tagger, but it is not what the
crate costs: the tagger comes bundled with 1.4 MiB of chunker the app has no use for. There are no
Cargo features to drop them.

**A second cost the earlier research did not name:** `harper-pos-utils`, the crate `harper-brill`
depends on, takes `burn = "0.19.1"` and `burn-ndarray = "0.19.0"` as **non-optional** dependencies.
A Quill build that only ever calls `brill_tagger()` still compiles a neural-network framework. For a
GTK text editor that wants a part-of-speech tag per word, that is a large tree to take on.

### The model's shape

```json
{"base": {"mapping": {"the": "DET", ...}}, "patches": [ ... 201 rules ... ]}
```

- `base.mapping` is a flat **word → single UPOS tag** map. **26,900 entries**, all keys lowercase, 16
  distinct tag values. No frequency counts are stored, despite the Rust type being
  `BrillTagger<FreqDict>`.
- `patches` is an ordered list of **201** Brill transformation rules. One example verbatim:

```json
{"from": "AUX", "to": "VERB", "criteria": {"Combined": {"a": {"WordIs": {"relative": -1, "word": "there"}}, "b": {"WordIsTaggedWith": {"relative": -1, "is_tagged": "PRON"}}}}}
```

That shape matters to the licence question: **the model contains no corpus text**. It is a lexicon
of 26,900 word→tag facts plus 201 rewrite rules. It reproduces no sentence from any treebank.

## Which corpora trained it

The exact training command was never recorded. `harper-cli`'s trainer takes an arbitrary list of
files —

```rust
TrainBrillTagger {
    /// Path to a `.conllu` dataset to train on.
    #[arg(num_args = 1..)]
    datasets: Vec<PathBuf>,
},
```

— no corpus is vendored in the harper repository, the `justfile` has no training recipe (`grep -ci
train justfile` → 0), and no build script or CI job regenerates the model. The model file has been
touched by exactly two commits in the repo's history:

```
7c338eb6 2025-07-04 feat(core): write linter to detect erroneous use of plurality (#1486)
db89187c 2025-06-16 feat(brill): train and use Brill tagger (#1344)
```

So the provenance has to be established from statements plus measurement.

### The documentary evidence

**Automattic/harper PR #1344**, the commit that added `trained_tagger_model.json`, in the author's
own words:

> The tagger itself (as of writing this comment) has been trained for two hours to achieve a 95%
> accuracy on the **UD_English_GUM** dataset. I plan to train further overnight or over the weekend.

The accompanying blog post the harper docs link to is vaguer, and names no treebank:

> To identify the baseline model's errors, we'll use an off-the-shelf tree-bank from the Universal
> Dependencies project.

And the *chunker* post — the one `nlp-tools.md` had already read — names **"GUM + EWT + LINES"**.
That is the sentence the ticket wanted separated from the tagger. It does not settle the tagger.

### The decisive evidence: a vocabulary fingerprint

The tagger's `base.mapping` is a lexicon extracted from whatever `.conllu` files were passed in, so
its 26,900 words should be almost exactly the token types of the training corpora. Measured against
the surface FORMs of the train+dev+test splits of each candidate treebank (comment lines,
multi-word-token range rows and empty nodes excluded; lowercased to match the model's keys):

| Hypothesis | Share of the model's 26,900 words the corpus explains |
|---|---:|
| EWT alone | 63.72% (17,140) |
| GUM alone | 61.99% (16,676) |
| LinES alone | 35.28% (9,490) |
| GUM + EWT | 91.59% (24,638) |
| **GUM + EWT + LinES** | **99.99% (26,898)** |

And in the other direction — how much of each treebank's own vocabulary the model absorbed — the
train splits alone give EWT **99.50%**, LinES **99.84%**, GUM **95.02%**. Each treebank also
contributes thousands of words neither of the others has: 7,074 model words come only from EWT,
6,266 only from GUM, 2,260 only from LinES. No treebank is redundant, and none is a rounding error.

Exactly **two** of the 26,900 model words are in none of the three, and both are extraction
artefacts rather than real gaps: `by-sa` (only ever inside GUM's own licence URL in a comment line)
and `cellfone` (only in an EWT `# text =` comment whose token row was hand-corrected).

That is not "one treebank plus noise" and it is not a larger external word list bolted on — the
model's 26,900 words are *smaller* than the 27,826 types in the three train splits combined. It is
the signature of training on the three UD English treebanks pooled, exactly as the chunker was.

**Conclusion: the tagger was trained on UD_English-EWT + UD_English-GUM + UD_English-LinES.** This
is strong inference from the artefact, not a quotation from upstream; upstream has never written the
command down. But no other hypothesis survives the coverage table.

## The corpora and their licences

All three licence lines are quoted from the treebanks' own repositories and from the UD project's
published treebank metadata.

| Corpus | Size | Licence | Free? |
|---|---:|---|---|
| UD_English-EWT | 16,622 sentences / 254,820 tokens | **CC BY-SA 4.0** | Free, GPLv3-compatible **one way only** |
| UD_English-GUM | 14,353 sentences / 252,284 tokens | **CC BY-NC-SA 4.0** | **Not free** — non-commercial |
| UD_English-LinES | 5,696 sentences / 105,137 tokens | **CC BY-NC-SA 4.0** | **Not free** — non-commercial |

**So yes: a CC BY-NC-SA corpus is among the tagger's training data — two of them.**

UD_English-EWT's README:

> The annotations and database rights of the Universal Dependencies English Web Treebank are
> licensed under a Creative Commons Attribution-ShareAlike 4.0 International License.

UD_English-LinES' `LICENSE.txt`:

> The treebank UD_English_LinES is distributed as a citation corpus under the Creative Commons
> license Attribution-NonCommercial-ShareAlike 4.0 International.

UD_English-GUM's README metadata block says `License: CC BY-NC-SA 4.0`, and its `LICENSE.txt`
explains that the repo-level label is the most restrictive of a mixture:

> The treebank is licensed under the Creative Commons License
> Attribution-NonCommercial-ShareAlike 4.0 International. […] The documents from Wikimedia
> (Wikinews, including interviews and bios, and Wikivoyage) are available under a CC-BY attribution
> license, as are open access textbooks from OpenStax, YouTube Creative Commons vlogs and the
> political speeches, which are in the public domain. **However wikiHow texts are made available
> under a CC-BY-NC-SA license (non-commercial, share alike), as are the fiction texts, meaning that
> commercial and/or non-open source use of those texts is prohibited.**

Note the phrase "and/or non-open source use" — the NC genres inside GUM are wikiHow and fiction, and
they are the reason the whole treebank ships as NC. Nothing in the tagger model records which of its
26,900 words came from which genre, so the mixture cannot be unpicked after the fact.

## Does any of that reach Quill?

Three links in the chain, and only the middle one is in doubt.

**Link 1 — the crate into a GPL-3.0-or-later app: clean.** The FSF's licence list on Apache-2.0:

> This is a free software license, compatible with version 3 of the GNU GPL.

and the ASF's own statement:

> The Free Software Foundation considers the Apache License, Version 2.0 to be a free software
> license, compatible with version 3 of the GPL.

Apache-2.0 imposes no reciprocal condition and, unlike CC BY-SA 4.0, carries no compatible-licence
list that would strand the "or later". Quill can link `harper-brill` and stay GPL-3.0-or-later.

**Link 2 — the corpora into the model: unsettled, and the whole of the risk.** Two readings:

- *The licence does not reach it.* The model reproduces no corpus text. It is 26,900 word→tag facts
  and 201 rules; facts are not copyrightable, and a most-frequent-tag lexicon of English is about as
  close to a table of facts as an artefact gets. This is the position Automattic — a commercial
  company shipping Harper in commercial products — implicitly takes by publishing the file under
  Apache-2.0.
- *The licence does reach it.* CC 4.0 licences grant **sui generis database rights** explicitly, and
  EWT's README says so in as many words ("The annotations **and database rights** … are licensed
  under"). A lexicon covering 95–99.8% of each treebank's token types is an extraction of a
  substantial portion of the database's contents, which is the exact act the database right governs
  in the EU and UK. CC BY-NC-SA 4.0 grants that permission **for NonCommercial purposes only**:

  > reproduce and Share the Licensed Material, in whole or in part, for NonCommercial purposes only;
  > and produce, reproduce, and Share Adapted Material for NonCommercial purposes only.

  > NonCommercial means not primarily intended for or directed towards commercial advantage or
  > monetary compensation.

**Link 3 — if link 2 holds, Quill cannot comply.** Not a labelling problem: a fatal one. The GPL
gives every recipient permission to run and redistribute the program for any purpose, including
commercially, and GPLv3 §7 forbids imposing further restrictions on that. Quill could not pass on a
NonCommercial condition even if it wanted to, so on the pessimistic reading Quill could not
distribute the binary at all. That is a sharper consequence than `nlp-tools.md` anticipated — it
treated this as a provenance footnote, and it is a distribution question.

The FSF's view of the licence class, for the record:

> This license does not qualify as free, because there are restrictions on charging money for
> copies.

### The EWT escape hatch is not one

`nlp-tools.md`'s mitigation was: "a tagger can be retrained on EWT alone if provenance ever needs to
be clean." It does not work, and the reason is a rule that document itself wrote down two sections
earlier.

Retraining on EWT alone is only *necessary* under the pessimistic reading of link 2 — that the
corpus licence reaches the model. But that reading applies to EWT too, and EWT is CC BY-SA 4.0. That
document's own conclusion about CC BY-SA 4.0:

> Because Creative Commons lists only version 3 of the GNU GPL on its compatible licenses list, it
> means that you can not license your adapted CC BY-SA works under the terms of "GNU GPL version 3,
> or (at your option) any later version." […] **This same trap applies to any future corpus, so it
> is worth stating as a rule: no CC BY-SA data in Quill unless the project first decides to drop
> "or-later."**

So retraining on EWT converts a fatal NonCommercial problem into a licence-change problem —
GPL-3.0-**only** instead of GPL-3.0-or-later. Better, but not clean, and not free. Under the
optimistic reading of link 2 the retrain buys nothing at all, because the existing model was already
fine. **Either way, EWT-only is the wrong target.**

## Verdict

There is **no GPL-compatibility problem in the crate**, and there **is** a real, confirmed, and
unadjudicated data-provenance exposure in the model — the thing #307 asked about. Ranking the three
options the ticket offered:

1. **Ship the model as is — acceptable to prototype with, not to release on unexamined.** The
   exposure is small in probability (the aggrieved parties would be wikiHow, the GUM fiction
   authors, or Språkbanken) and large in consequence (undistributable, not merely mislabelled). More
   to the point, Quill has already written down a rule that treats *CC BY-SA* corpus data as
   disqualifying; CC BY-NC-SA is strictly worse, so accepting it silently would be inconsistent with
   a decision the project has already made.
2. **Vendor a retrained model — the right long-term answer, but not retrained on EWT.** Note what is
   actually at risk: a **26,900-word lexicon**. That is the cheap half of the model, and a
   public-domain source supplies it outright — Moby Part-of-Speech II (public domain, already on
   `nlp-tools.md`'s source list) carries ~230,000 entries, and WordNet's licence is permissive. Only
   the 201 patch rules are corpus-learned, and `harper-pos-utils` is Apache-2.0 with the trainer
   in-repo, so they can be re-derived on whatever data Quill is comfortable with.
3. **Swap crates — unnecessary.** `harper-brill` is not the problem; its embedded data file is, and
   that file is separable. The API surface Quill uses is two functions.

**Recommended course for [#28](https://github.com/danielbaldwin47/Quill/issues/28):**

- Pin `harper-brill = "=2.8.0"` and build the prototype. Measure tagging accuracy on `ref/sample.md`
  and the iA passages against `nlp-tools.md`'s ~86% threshold **first** — if the tagger does not
  clear the bar, the licence question is moot and this document is filed rather than acted on.
- If it clears the bar, record the decision in an ADR before Syntax highlight ships in a release.
  The ADR should state which reading of link 2 the project is taking, and carry the Apache-2.0 text
  for the embedded artefacts (the `.crate` ships none).
- Budget the swap. The replacement is a lexicon, not a rewrite: `BrillTagger` deserializes from JSON
  and `harper_brill::brill_tagger()` is one function, so a Quill-built `{"base": …, "patches": …}`
  file drops into the same type.
- Weigh the 2.05 MiB of embedded artefacts and the non-optional `burn` dependency against what the
  tagger buys. If a Quill-built lexicon is on the table anyway, a dictionary-first tagger with no
  neural framework behind it may be the cheaper end state on every axis.

## What to pin

```toml
harper-brill = "=2.8.0"   # Apache-2.0; embeds a 644.2 KiB Brill tagger + 1.4 MiB of chunker
```

Pin exactly. The workspace releases on Harper's schedule, the model has been retrained in place
before (`7c338eb6`), and PR #3717 — open, unmerged as of 2026-09-07 — proposes replacing both the
Brill tagger and the Burn chunker with a single joint neural model, dropping
`harper-brill::brill_tagger()` onto a different implementation and the embedded footprint from
~2.05 MiB to ~787 KiB. A floating version would take that change unannounced. It would also change
the provenance answer: that PR's training example "auto-fetches the UD treebanks", so the successor
model inherits the same question this document just answered.

## Sources

Primary, all read 2026-09-07.

- `harper-brill` on crates.io — <https://crates.io/crates/harper-brill>; version metadata via
  `https://crates.io/api/v1/crates/harper-brill`; tarball
  `https://static.crates.io/crates/harper-brill/harper-brill-2.8.0.crate`
- `harper-brill` on docs.rs (2.8.0 API surface) — <https://docs.rs/harper-brill/2.8.0/harper_brill/>
- `harper-brill/Cargo.toml`, `src/lib.rs` —
  <https://github.com/Automattic/harper/tree/master/harper-brill>
- `harper-pos-utils/Cargo.toml` (the non-optional `burn` dependency) —
  <https://github.com/Automattic/harper/blob/master/harper-pos-utils/Cargo.toml>
- `harper-cli/src/main.rs`, `TrainBrillTagger` —
  <https://github.com/Automattic/harper/blob/master/harper-cli/src/main.rs>
- Automattic/harper PR #1344, "feat(brill): train and use Brill tagger" (the UD_English_GUM
  quotation; commit `db89187c` added the model) —
  <https://github.com/Automattic/harper/pull/1344>
- Automattic/harper PR #1486 (commit `7c338eb6`, the one later retrain) —
  <https://github.com/Automattic/harper/pull/1486>
- Automattic/harper PR #3717, the joint-model successor —
  <https://github.com/Automattic/harper/pull/3717>
- Harper docs, Brill Tagging — <https://writewithharper.com/docs/contributors/brill>
- Elijah Potter, "Transformation-Based Learning" —
  <https://elijahpotter.dev/articles/transformation-based_learning>
- Elijah Potter, "Training a Chunker with Burn" (the "GUM + EWT + LINES" line) —
  <https://elijahpotter.dev/articles/training_a_chunker_with_burn>
- UD_English-EWT — <https://github.com/UniversalDependencies/UD_English-EWT>,
  <https://universaldependencies.org/treebanks/en_ewt/index.html>
- UD_English-GUM, README and `LICENSE.txt` —
  <https://github.com/UniversalDependencies/UD_English-GUM>,
  <https://universaldependencies.org/treebanks/en_gum/index.html>
- UD_English-LinES — <https://github.com/UniversalDependencies/UD_English-LinES>,
  <https://universaldependencies.org/treebanks/en_lines/index.html>
- CC BY-NC-SA 4.0 legal code —
  <https://creativecommons.org/licenses/by-nc-sa/4.0/legalcode.en>
- FSF licence list (Apache-2.0 and the NonCommercial class) —
  <https://www.gnu.org/licenses/license-list.html>
- ASF, "Apache License v2.0 and GPL Compatibility" —
  <https://www.apache.org/licenses/GPL-compatibility.html>
- Quill's own prior research, `git show research/nlp-tools:docs/research/nlp-tools.md`

### Method note

The corpus fingerprint in [The decisive evidence](#the-decisive-evidence-a-vocabulary-fingerprint)
was computed locally: the published 2.8.0 `.crate` was unpacked, `base.mapping`'s 26,900 keys were
compared against the surface FORMs of every split of `en_ewt`, `en_gum` and `en_lines` taken from
the treebanks' `master` branches. It is reproducible from the URLs above with a few lines of Python.
Upstream has published no statement naming the tagger's training set, so this is the strongest
available evidence and it should be read as inference, not as a quotation.
