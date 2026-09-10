# Syntax assertion fixtures

These are native GTK captures for #317's `tools/judge-selftest.mjs` cases. The five
named pairs use the corresponding `shots/oracle/states.json` states. Each `ours-lit`
companion keeps the same flags with `syntax: off`; its name follows the Gate's
existing second-shot convention. `capture.json` records the build and every flag.

Reproduce from the repository root:

```sh
cargo build --release
node tools/syntax-fixture/capture.mjs
node tools/judge-selftest.mjs
```

`all-light-ours-lit.png` is the real `--syntax off` negative: the selftest passes it
as the primary image under the registered all-light rule and requires failure.
Further variants remove a Category, add an unexpected pigment, colour a dim glyph,
move a coloured pixel off the text column, and remove the Live heading's colour.
The unscaled all-light pair must also fail under the Live rule.

The rule compares actual pixels against provisional pigments restated from
`quill-engine/src/theme.rs`, `Colours::LIGHT` and `Colours::DARK`. Full-opacity
Category cores establish presence; unchanged companion pixels establish glyph
coverage and Focus bands independently of the colour. The two-channel-value
allowance accounts for 8-bit antialiasing quantisation, with no spatial allowance.
The real-pair selftest prints elapsed time for the bounded image walks.

`ref/sample.md` has no code or URL. The separate `protection.md` passage supplies
those subjects, alongside a heading marker, strong markers and coloured link
words. Its on/off captures use the same defaults with chrome off and caret zero.
They are regression fixtures, not a sixth judged state. Protection rectangles are
measured in those captures and recorded beside the selftest that consumes them.

`protection-shifted.png` preserves the broken Syntax-on capture from commit
`63552b4abc772a49bfdf9133a0838d5a6d86edf4`, binary SHA-256
`2783e0a0857139322fa973bbfa5642ed29429f1fc7aeea95c5e90d233d5fa8d9`.
Its flags are the protection-on capture's. Full retagging after a worker result
moved the indented code and following prose; the rule rejects the first changed
protected pixel at (592, 1110). The capture runner preserves this negative. The
fixed on/off pair and a wrong-colour variant for each protected subject must
respectively pass and fail through the same production rule.

The five Piece states take the standard asserted Gate round under ADR 0017. No
critic is spent. #319 remains pending the owner's Mac captures from #308.
