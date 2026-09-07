# `harper-brill` ships as it is, with its training data named

Syntax highlight tags words with `harper-brill`, pinned `=2.8.0`, and Quill ships the crate's
embedded tagger model unchanged. Two of the three treebanks that model was trained on are
CC BY-NC-SA 4.0, which is not a free licence; the model holds no line of any of them, and whether a
word→tag lexicon extracted from a treebank is a licensed derivative of it has never been
adjudicated anywhere. The owner accepts that exposure knowingly rather than discovering it after a
release. Decided 2026-09-07 from the research at
[#307](https://github.com/danielbaldwin47/Quill/issues/307)
(`git show research/harper-provenance:docs/research/harper-provenance.md`) and built at
[#311](https://github.com/danielbaldwin47/Quill/issues/311).

## Context

Syntax highlight needs one part-of-speech tag per word, offline, with no network and nothing trained
at build time (`docs/architecture.md` § Packaging). `harper-brill` is the one crate that answers
that: a Brill tagger — a lowercase word→tag lexicon plus ordered rewrite rules — with its trained
model compiled in.

**The code is clean.** `harper-brill` 2.8.0 is Apache-2.0, which is one-way compatible with
GPL-3.0-or-later. Nothing about the code path into a GPL binary raises a question.

**The data is the question.** The embedded `trained_tagger_model.json` is 659,686 bytes: 26,900
lowercase word→tag entries and 201 rewrite rules. It was trained on the union of three Universal
Dependencies English treebanks, which the research established by measurement rather than by taking
anyone's word for it — the model's vocabulary is 99.99 % covered by GUM + EWT + LinES together and
by no smaller combination:

| Treebank | Licence | |
|---|---|---|
| UD_English-EWT | CC BY-SA 4.0 | free, GPLv3-compatible one way only, and so incompatible with "or-later" |
| UD_English-GUM | CC BY-NC-SA 4.0 | not free — NonCommercial |
| UD_English-LinES | CC BY-NC-SA 4.0 | not free — NonCommercial |

`docs/architecture.md` § Packaging carries the rule this runs at: "no CC BY-SA data ships in Quill".
The rule is about data that ships. No corpus ships here and no sentence from one survives in the
model; what ships is a table of facts about words, derived from corpora. That is the distinction
nobody has settled, and it is the whole of the exposure.

## Decision

Depend on `harper-brill = "=2.8.0"` and ship the model as published.

**The pin is exact and not a matter of taste.** The crate embeds the model, so a version bump is a
recolouring of every page a writer has open and a new floor under
`quill-engine/tests/syntax_accuracy.rs`, which the shipped model clears by one word (25 of 29 scored,
86.2 %, against a floor of 86 %). A bump is a deliberate act with that test re-read, never a
`cargo update`.

**Do not retrain on EWT alone.** The earlier research proposed it as the clean-provenance escape and
it is not one: EWT is CC BY-SA 4.0, so under the same pessimistic reading that makes GUM and LinES a
problem, retraining converts a NonCommercial problem into a GPL-3.0-**only** problem and costs Quill
the "or-later". Under the optimistic reading it buys nothing, because the shipped model was already
fine. Replacing the base lexicon with a public-domain source is the only mitigation that would
actually clean the provenance, and the research measured what it costs: 54.3 % to 80.8 % agreement,
under the spec's floor and well under what the shipped model reaches.

## Considered options

**`harper-core` whole** is a grammar checker, which `CONTEXT.md` says Style check is deliberately
not, and it drags the dictionary and the linter set behind it for a tag per word.

**A hand-written or public-domain lexicon** was measured by the research at 54.3–80.8 % and cannot
reach the accuracy the feature is judged at; a most-frequent-tag lexicon has no context and cannot
tell `spot` the verb from `spot` the noun at all.

**Wait for a cleanly-trained model.** Nothing suggests one is coming, and the map has Syntax
highlight now.

## Consequences

**The build cost, measured on this machine** (`quill` at `--release`, a fresh worktree, `cargo clean`
between):

| | before | with `harper-brill` | with one reachable call |
|---|---:|---:|---:|
| `quill` binary | 4,632,192 B | 4,856,848 B | 5,672,112 B |
| crates compiled | 92 | 187 | 187 |
| cold `cargo build --release` | 32 s | 32 s | — |

Ninety-five crates arrive, `burn` and `burn-ndarray` among them: `harper-pos-utils` takes a neural
network framework as a non-optional dependency for a chunker Quill never calls, and there is no
feature to drop it. The wall-clock does not move, because that tree compiles alongside `gtk4-rs`
rather than after it; what it costs is cores, not minutes.

The binary grows 219 KiB from the crate merely being there and 0.99 MiB once anything reachable from
`main` calls the tagger — which is the number that will land when the Editor is wired up
([#316](https://github.com/danielbaldwin47/Quill/issues/316)); today nothing calls it and the linker
drops the model. The 1.4 MiB of chunker never arrives at all, for the same reason: nothing reaches
it.

**Apache-2.0 §4(a).** The published `.crate` carries no `LICENSE` file and the harper repository has
no `NOTICE`, so a Quill package that embeds this model has to carry the Apache-2.0 text itself,
beside `fonts/OFL.txt` under `/usr/share/licenses/quill/`. Packaging owns that before a release.

**If the reading ever changes**, the seam is one module: `quill_engine::pos` hands out
`(byte range, Category)` and nothing downstream knows a tagger exists. A replacement changes the
accuracy figure and this ADR, and no other file.
