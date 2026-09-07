# Ship the pinned Harper tagger with its provenance recorded

Syntax highlight uses `harper-brill = "=2.8.0"`, whose crate code is Apache-2.0.
The owner accepts shipping its embedded tagger model as supplied: the model was
trained on the Universal Dependencies English EWT, GUM and LinES treebanks;
EWT is CC BY-SA 4.0, and GUM and LinES are CC BY-NC-SA 4.0. Whether the derived
lexicon and rules inherit the corpora's restrictions is unadjudicated. This is
an accepted upstream data exposure, not a finding that those restrictions are
compatible with Quill's GPL-3.0-or-later licence.

The model contains 26,900 word-to-tag entries and 201 transformation rules,
with no corpus sentences or other corpus text. Quill vendors no external
corpus. The rejected EWT-only retrain does not resolve the licence question;
the provenance research supersedes that earlier proposed mitigation. Before
distributing a binary with the model, include the Apache-2.0 licence text:
the published crate declares the licence but omits its text.

Evidence and the training-data analysis are recorded in
`git show research/harper-provenance:docs/research/harper-provenance.md`
([#307](https://github.com/danielbaldwin47/Quill/issues/307)); the adopted
decision is [#310](https://github.com/danielbaldwin47/Quill/issues/310).
The published crate identifies upstream commit
`5d2053bec94f49daaed3cdeae4cc328b897036c6`.

## Build cost measured for #311

On 2026-09-07, Intel Core i5-13600K (20 logical CPUs), Linux x86-64,
`rustc 1.98.0 (88d9e12ae 2026-08-18)`, two separate initially empty target
directories built `cargo build --release -p quill`, timed with Bash
`TIMEFORMAT='elapsed_seconds=%R'`. Sources were already downloaded; this
measures compilation and linking, not fetching. Before is commit `68b5513`;
after adds this ticket's dependency and tagger implementation.

| Measurement | Before | After | Change |
|---|---:|---:|---:|
| Wall time | 29.138 s | 32.121 s | +2.983 s |
| Release app binary | 4,632,208 bytes | 4,859,048 bytes | +226,840 bytes |
| Engine release archive | 5,817,046 bytes | 5,983,466 bytes | +166,420 bytes |

These are one-shot observations with cold Cargo artifact caches and warm
filesystem caches, not an isolated benchmark: another comparison session was
active on the machine. The sibling ticket's Gate completed before the after
measurement. The app does not call the tagger yet, so its binary delta is not
the final feature's footprint. A separate stripped, optimised Rust probe that
calls `pos::tag` on a command-line argument and prints its spans was
1,325,232 bytes; running it on `Alice can sing.` returned a Noun and two Verbs.
That is the whole probe binary, not a size delta or the future app size.

The crate embeds a 659,686-byte tagger (644.2 KiB), plus chunker artifacts of
52,717, 806,312 and 627,993 bytes (about 1.4 MiB combined). Its non-optional
`harper-pos-utils` dependency compiles `burn` and `burn-ndarray` even though
Quill never calls a chunker. Embedded source sizes are not linked binary
sizes: link-time elimination can remove unused artifacts. The model is
deserialised once by Harper's process-wide `LazyLock` on the first `pos::tag`
call; the later worker ticket supplies the thread that makes that call.
