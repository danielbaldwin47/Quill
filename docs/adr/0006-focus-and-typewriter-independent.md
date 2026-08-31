# Focus and Typewriter are independent modes

*Narrowed by [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md): the first of the
"two Quill refinements" in § Consequences — the near tier — is withdrawn, and Focus has one dim tier
per theme (`docs/design.md` § Dim tiers). The edge band and the independence stand.*

*Under the same ADR, the Focus & typewriter judged states that follow the Dim tiers row name a
`mac-native` crop as their opponent, where § Considered options says the Piece is judged against the
Parity oracle's frozen shots.*

Focus (dim everything but the current sentence or paragraph) and Typewriter (hold the caret's line at a
fixed height by scrolling) are two settings the writer switches on and off separately, and both can be
on at once. This is the model iA Writer ships on Windows and the one the JavaScript app already has.
Settled in [#15](https://github.com/danielbaldwin47/Quill/issues/15).

## Considered options

iA Writer on Mac folds Typewriter into Focus as a third scope beside Sentence and Paragraph: one mode,
one `⌘D`, and in the Typewriter scope "the text will not be highlighted or dimmed", so dimming and
centring never coexist. iA's own support page explains the coupling as a workaround: Focus's attempt to
keep the caret centred fights pointer-driven edits and makes "the screen jump vertically". The
JavaScript app solved that conflict directly (a pointer-placed caret is only nudged back into a band,
never yanked to centre), so the reason for the coupling does not apply, and the coupling would forbid the
dimmed-and-centred state writers use most. The Focus & typewriter Piece is judged blind against the
Parity oracle, whose frozen shots include that state. Behaviour research in
`docs/research/ia-behaviour.md` on `research/ia-behaviour`.

## Consequences

**Two settings, four remembered values.** Focus on/off, Focus scope (Sentence or Paragraph), Typewriter
on/off and the Typewriter anchor (a fraction of the viewport height, 0.5 by default) all persist across
launches; the Settings spec decides where.

**Shortcuts.** `Ctrl+D` toggles Focus off and back on at its last scope; `Ctrl+Shift+D` swaps Sentence
and Paragraph, switching Focus on if it was off; `Ctrl+T` toggles Typewriter. Picking a scope from the
menu also switches Focus on. The menu reads Focus (toggle, then the two scope radios), a separator, then
the Typewriter check item, so Typewriter is never mistaken for a third scope.

**Focus is a rendering mode only.** It touches no chrome: the Library and Preview panes stay where they
are (iA Mac dismisses them).

**The two Quill refinements stay.** In Sentence scope the neighbouring sentences sit one shade above the
far dim (the "near" tier), and with Focus on and Typewriter off the Editor still keeps the active line
away from the viewport edges.
