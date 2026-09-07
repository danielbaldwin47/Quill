# Ship the pinned Harper tagger with its provenance recorded

**Status — 2026-09-07:** Accepted in [#311](https://github.com/danielbaldwin47/Quill/issues/311),
implementing the owner's #310 decision. The decision stands; its implementation is on this
branch pending merge. `main` does not yet contain the tagger dependency or this implementation.

Syntax highlight uses `harper-brill = "=2.8.0"`, an Apache-2.0 crate, as accepted by the owner
in [#310](https://github.com/danielbaldwin47/Quill/issues/310). Its embedded tagger was trained
on Universal Dependencies English EWT (CC BY-SA 4.0), GUM and LinES (both CC BY-NC-SA 4.0).
The model contains a word-to-tag lexicon and transformation rules, with no corpus sentences.
The owner accepts shipping that model as is, including the unsettled question of whether
the learned lexicon is a derivative of the training data. This records the accepted exposure;
it does not resolve it or change Quill's GPL-3.0-or-later licence. Quill vendors no corpus.

The evidence and rejected mitigations are the [#307 provenance research](https://github.com/danielbaldwin47/Quill/blob/research/harper-provenance/docs/research/harper-provenance.md).
Retraining on EWT alone would still retain its share-alike question. Replacing the crate is
unnecessary if a replacement model becomes desirable: the tagger's data and implementation
are separable. Pinning the version keeps upstream model changes deliberate. Harper's
Apache-2.0 text, taken from release commit `5d2053bec94f49daaed3cdeae4cc328b897036c6`, is
`packaging/harper-brill-LICENSE`; the package installs it beside Quill's licence.

## Build cost

The crate bundles a 659,686-byte (644.2 KiB) tagger model, a 52,717-byte Brill chunker model,
and a neural chunker's 806,312-byte weights plus 627,993-byte vocabulary: 1.42 MiB of chunker
data that Quill never calls. `harper-pos-utils` also requires `burn` and `burn-ndarray` without
an optional feature to remove them, so even tagger-only consumers compile that framework.
The built-in tagger deserialises once behind Harper's process-wide `LazyLock`, at first use
on the calling worker thread; neither chunker is initialised by Quill.

Measured on 2026-09-07, x86_64 Linux, Intel Core i5-13600K (20 logical CPUs), Rust 1.98.0:

| Measurement | Before dependency | With `pos::tag` | Difference |
|---|---:|---:|---:|
| Clean release engine and probe build | 15.229 s | 25.833 s | +10.604 s |
| Probe executable | 453,832 B | 1,621,176 B | +1,167,344 B |
| Probe executable after `strip` | 347,120 B | 1,317,952 B | +970,832 B |

Both builds used `cargo build --release -p quill-engine --example pos_build_cost` with
separate empty `--target-dir` directories; dependency downloads were already cached locally.
The temporary example read `std::env::args().nth(1).unwrap_or_default()` into `prose`.
Before the dependency, its body ended with `std::hint::black_box(prose)`; afterwards with
`std::hint::black_box(quill_engine::pos::tag(&prose))`. Bash `time` measured elapsed seconds,
`stat` the executable bytes, and `strip -o` made the stripped copies. The example is a
measurement harness, not shipped code. Other implementation worktrees were active on the
same machine, so these are single-run observations, not an isolated performance benchmark.
The measurements preceded the final guard distinguishing auxiliary `'s` from possessive `'s`.
The dependency, model and tokenizer were unchanged; the final executable size was not remeasured.

These are engine-consumer probe sizes, not application sizes: the application does not yet
call Syntax highlight at this ticket. A binary that never calls the seam can discard the
tagger too. The release linker also discards unused chunker code and data, so the bundled
2.05 MiB is not a claim about the executable's increase; the probe above measures the
tagger actually retained by a consumer.

## Accuracy

The fixture `quill-engine/tests/fixtures/pos-stills.toml` transcribes each coloured word from
the named TRIM and Alice stills. TRIM shows Verbs, Adjectives, Adverbs and Conjunctions;
Alice shows Verbs, Adjectives and Adverbs. Nouns are plain in both, and Alice contains no
conjunction to observe. Alice's `actually` is visibly plain and remains so in the fixture.

On the pinned model, TRIM agrees on 15 of 16 scored words, Alice on 10 of 12: **25/28,
89.3%**, above the required 86%. As #310 requires, the denominator is words Quill colours
in Categories visible in that passage, not every word or every reference-coloured word.
The disagreements are `Check` (Quill: Verb; still: plain), `actually` (Adverb; plain), and
`so-called` (Verb; Adjective). No fixture-specific correction is applied to the tagger.
