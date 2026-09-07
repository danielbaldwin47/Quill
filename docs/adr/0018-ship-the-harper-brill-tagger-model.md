# Quill ships the pinned Harper Brill tagger model

Quill uses `harper-brill = "=2.8.0"` for Syntax highlight and ships its embedded tagger model.
The crate is Apache-2.0; the model was trained on UD English EWT, GUM and LinES, including two
CC BY-NC-SA 4.0 corpora, but contains no corpus text. The owner accepts the residual provenance
risk. Decided 2026-09-07 with the owner, from
[#311](https://github.com/danielbaldwin47/Quill/issues/311) and the provenance research behind it.

## Context

Syntax highlight needs one local English part-of-speech tagger whose Universal POS output maps to
the five Categories the writer sees. `harper-brill` 2.8.0 provides that seam and an embedded model.
Its published crate declares Apache-2.0. The model is a 26,900-entry lowercase word-to-tag lexicon
plus 201 transformation rules; it reproduces no sentence or other corpus text.

Upstream recorded no training command. A vocabulary fingerprint accounts for 26,898 of the
model's 26,900 words from the pooled UD English EWT, GUM and LinES treebanks, and each contributes
thousands of otherwise absent words. That is strong artefact-derived evidence that all three
trained the tagger. EWT is CC BY-SA 4.0. GUM and LinES are CC BY-NC-SA 4.0.

Whether those corpus licences reach the extracted lexicon and learned rules is unsettled. The
optimistic reading treats the entries and rules as facts that reproduce no protected corpus text;
the pessimistic reading treats the near-complete vocabulary extraction as exercising database
rights, which would make the NonCommercial restriction incompatible with Quill's unrestricted
GPL-3.0-or-later distribution. Automattic publishes and commercially uses the model as
Apache-2.0, but that does not adjudicate the database-right question.

The dependency is larger than the tagger Quill calls. The tagger JSON is 659,686 bytes
(644.2 KiB). The crate also embeds 52,717 bytes of Brill chunker data and 1,434,305 bytes of neural
chunker weights and vocabulary, about 1.4 MiB Quill never calls. `harper-pos-utils` also makes
`burn` and `burn-ndarray` non-optional dependencies; no Cargo feature removes either cost.

## Decision

**Ship the model under the optimistic reading and record the exposure.** The absence of corpus
text, the factual shape of the model and upstream's Apache-2.0 publication are enough for Quill's
owner to accept the residual database-right risk. This is an explicit provenance decision, not a
claim that the corpus-licence question has a settled legal answer.

**Pin `harper-brill` exactly at 2.8.0.** Harper has retrained the embedded model in place and is
developing a replacement tagger architecture. A floating compatible release could silently change
both behaviour and provenance. Quill depends on the tagger crate alone and never calls either
chunker.

**Pay model initialization only at first use.** Quill's part-of-speech module holds the shared
tagger behind a lazy cell. Syntax highlight startup therefore does not deserialize the model; the
first worker request does.

## Measured build cost

Measured on the owner's machine on 2026-09-07 with clean, separate target directories and
`cargo build --offline -p quill` in the `dev` profile, first at `implement-spec-310-sol-high` and
then with this decision implemented:

| | Before | After | Change |
|---|---:|---:|---:|
| Clean wall-clock compile | 23.01 s | 24.90 s | +1.89 s (+8.2%) |
| `target/debug/quill` | 108,127,520 B | 143,943,976 B | +35,816,456 B (+33.1%) |

The binary delta is a debug-build cost, dominated by the mandatory Burn dependency's code and
debug information; it is not the embedded-data size. As a narrower cross-check, the clean
`quill-engine` build rose from 10.91 s to 13.50 s, while its rlib rose from 42,633,508 to
42,729,006 bytes because dependency rlibs remain separate until final linking.

## Considered options

**Retrain on EWT alone.** Rejected: under the pessimistic reading that motivates retraining, EWT's
CC BY-SA 4.0 database rights would still reach the model, and Creative Commons' compatibility list
would require GPL-3.0-only rather than Quill's GPL-3.0-or-later.

**Build a replacement model from public-domain or permissive sources.** This remains the cleanest
way to remove the exposure, but it is a model-data project rather than a different Syntax highlight
interface. The pinned Harper seam permits that later substitution without changing Category spans.

**Choose another crate.** Rejected for this Piece: the crate code is permissively licensed and the
model data is replaceable. Swapping the whole tagger does not by itself answer provenance or meet
the measured accuracy bar.

## Consequences

Syntax highlight may ship with this model. A release distribution that contains Harper's code or
embedded artefacts also carries the Apache License 2.0 text; the published crate tarball does not
include it, so packaging must not assume Cargo supplied the notice.

Quill accepts approximately 2.05 MiB of embedded tagger and unused chunker artifacts, the mandatory
Burn dependency tree, and the measured compile and debug-binary cost. Only
`harper_brill::brill_tagger()` is called.

Any tagger version or embedded-model change reopens this ADR: remeasure Category accuracy, inspect
the new training provenance, and measure build cost before changing the exact pin.
