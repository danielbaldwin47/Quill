# Syntax assertion fixtures

These are native GTK captures for #317's `tools/judge-selftest.mjs` cases. The five
named pairs use the corresponding `dev/shots/oracle/states.json` states. Each `ours-lit`
companion keeps the same flags with `syntax: off`; its name follows the Gate's
existing second-shot convention. `capture.json` records the build and every flag.

Since #319 three of the five states — `all-light`, `all-dark` and `focus-sentence` —
name a `mac-native` opponent rather than an `assert`, and a state is answered one way
(ADR 0017), so the runner takes their companion as the same flags with Syntax off
instead of asking a rule for its second shot. The selftest derives the two rules they
used to carry from the two that remain, so a Category added to the Piece still reaches
all four. Their captures moved to `dev/ref/ia/mac-native/passage-syntax.md`, the passage
the Design oracle's own frames were shot on.

Reproduce from the repository root:

```sh
cargo build --release
node tools/syntax-fixture/capture.mjs
node tools/judge-selftest.mjs
```

`all-light-ours-lit.png` is the real `--syntax off` negative: the selftest passes it
as the primary image under the all-light rule and requires failure.
Further variants remove a Category, add an unexpected pigment, colour a dim glyph,
move a coloured pixel off the text column, and remove the Live heading's colour.
The unscaled all-light pair must also fail under the Live rule.

The rule compares actual pixels against the pigments restated from
`quill-engine/src/theme.rs`, `Colours::LIGHT` and `Colours::DARK`, which #308 measured
off the Design oracle and #319 ported (`docs/design.md` row Syntax colours). Full-opacity
Category cores establish presence; unchanged companion pixels establish glyph
coverage and Focus bands independently of the colour. The two-channel-value
allowance accounts for 8-bit antialiasing quantisation, with no spatial allowance.
The real-pair selftest prints elapsed time for the bounded image walks.

`dev/ref/sample.md` has no code or URL. The separate `protection.md` passage supplies
those subjects, alongside a heading marker, strong markers and coloured link
words. Its on/off captures use the same defaults with chrome off and caret zero.
They are regression fixtures, not a sixth judged state. Protection rectangles are
measured in those captures and recorded beside the selftest that consumes them.

`protection-shifted.png` preserves the broken Syntax-on capture from commit
`63552b4abc772a49bfdf9133a0838d5a6d86edf4`, binary SHA-256
`2783e0a0857139322fa973bbfa5642ed29429f1fc7aeea95c5e90d233d5fa8d9`.
Its flags are the protection-on capture's. Full retagging after a worker result
moved the indented code and following prose. It is kept as the record of that defect
and **is no longer fed to the rule**: #241 made the page top a constant, which moved
every band 88 px, and #319 replaced the provisional Category pigments with the
measured ones, so the production rule now rejects that file at the first pixel of a
colour it does not know rather than at the moved block — the right verdict for the
wrong reason. The selftest puts the same defect back on the current capture instead,
translating the indented-code rectangle, because the defect is geometry and geometry
survives a repalette. The fixed on/off pair and a wrong-colour variant for each
protected subject must respectively pass and fail through the same production rule,
and the protected rectangles are the #317 ones less that 88 px.

Two of the five Piece states — `adjectives-adverbs` and `live` — take the standard
asserted Gate round under ADR 0017, and no critic is spent on them: no `mac-native`
frame holds two Categories together, and none holds Live at all. The other three are
blind pairs against the Design oracle since #319.
