# Quill

A long-form writing app for Linux in the spirit of iA Writer: plain Markdown files, typography first, chrome that stays out of the way.

## Language

**Document**:
One Markdown file on disk. The file is the only source of truth; Quill stores nothing about it elsewhere.
_Avoid_: note, page, buffer

**Library**:
The Locations the writer has added, browsed from the sidebar.
_Avoid_: workspace, vault, project

**Location**:
One folder the writer has added to the Library; the sidebar shows each with its full tree.
_Avoid_: root, workspace, vault, project

**Pinned**:
A Document or folder the writer has pinned; the sidebar lists them above the Locations.
_Avoid_: favourite, starred

**Editor**:
The text surface where a Document is written. Renders markup styling inline without changing glyph advance widths.
_Avoid_: text view, canvas

**Markup**:
The Markdown syntax characters themselves (`#`, `*`, `[`), drawn in the body's own ink; Live folds them off every block but the caret's.

**Focus**:
Mode that dims everything except the current sentence or paragraph; its scope is Sentence or Paragraph. Independent of Typewriter.
_Avoid_: zen mode, distraction-free

**Typewriter**:
Mode that keeps the caret's line at a fixed vertical position by scrolling the Editor. Independent of Focus, never a Focus scope.

**Live**:
The Editor with markup rendered in place: markers folded away except on the caret's block and the blocks a selection touches, headings scaled, bullets, checkboxes and hairlines drawn where their markers were. The file is source throughout; the Template never enters.
_Avoid_: live preview (Preview is the pane), WYSIWYG, rich text, reading view

**Preview**:
The pane showing a Document laid out, opened beside or in place of the Editor. It has two modes: Web, the rendered (HTML-like) sheet, and PDF, the pages Export writes drawn as a column.
_Avoid_: live preview (that is Live, the Editor's own mode)

**Export**:
Writing a Document out, laid out in the current Template: to a file as PDF, HTML or Markdown, to the clipboard as HTML, or to a printer. Page furniture (paper, margins, header, footer, title page) is Export's; typography is the Template's.
_Avoid_: print preview (Preview's PDF mode is the preview of what Export writes), Save As (that re-points the Document, Export never does)

**Face**:
One of Quill's three shipped typefaces — Duo, Quattro, Mono — each a Roman file and an Italic file, derived from the iA Writer fonts.
_Avoid_: font family (that is the fontconfig name), font (ambiguous with the size setting)

**Template**:
A named typographic design (faces, sizes, rhythm, light and dark palettes) that Preview and Export apply to a rendered Document, independent of the Editor's face and size.
_Avoid_: theme (that is the Editor's dark/light), style, look, skin

**Syntax highlight**:
Coloring words by part of speech (noun, verb, adjective, adverb, conjunction) to show sentence texture.
_Avoid_: code highlighting, grammar coloring

**Style check**:
Striking through fillers, redundancies and clichés in prose, from Quill's own phrase lists; the strike is the whole interaction, with no suggestion offered.
_Avoid_: grammar check, linting, underlining (that is Spell check's mark)

**Spell check**:
Underlining misspellings against a system dictionary, with suggestions; a toggle, on by default.

**Stats**:
The Statistics the stats bar reads over the Document, or over the selection while one stands, counted on the prose stream. Not an Annotator: it marks nothing.

**Statistic**:
One number Stats counts, each behind its own check in the Stats menu: Words, Characters, Characters Without Spaces, Sentences, Paragraphs, Reading Time.
_Avoid_: stat, metric, count (a Statistic's value)

**Outline**:
A Document's headings in reading order, each with its level, as the Editor shows them.
_Avoid_: table of contents (that is `{{TOC}}`, a Preview/Export feature), tree, structure

**Heading navigation**:
Jumping the caret to a heading chosen from the Outline; it moves the caret and nothing else.
_Avoid_: outline view, folding (iA Windows' hiding of a section, which Quill does not do)

**Chrome**:
The two bars around the Editor: the title bar (Library toggle, Document title and menu, View menu) and the stats bar. Both step back while the writer types.
_Avoid_: toolbar, header bar, UI

**Command**:
One named thing Quill can do (`focus.toggle`, `file.save`); its id is how the menus, the Palette, the shortcut table and the writer's rebindings all refer to it.
_Avoid_: action (GTK's term), function

**Palette**:
The searchable list of every Command, opened from the View menu; the way to a Command that has no shortcut.
_Avoid_: command palette, launcher, quick open

**Shortcut**:
A key chord bound to a Command; every Command's is rebindable by the writer.
_Avoid_: accelerator (GTK's term), hotkey, keybinding

**Annotator**:
Anything that marks spans of a Document for the Editor to style: Markup, Live, Syntax highlight, Style check and Spell check are the five. The last three see the prose stream, never the Markup characters.
_Avoid_: highlighter, linter, decorator

**Category**:
One of the five parts of speech Syntax highlight colours, each behind its own toggle: Nouns, Verbs, Adjectives, Adverbs, Conjunctions. Syntax highlight is the master toggle over all five; the two are separate state.
_Avoid_: part-of-speech class, tag (that is the tagger's output, of which a Category groups several)

**List**:
One of the three phrase lists Style check matches, each behind its own toggle: Fillers, Redundancies, Clichés. Style check is the master toggle over all three; the two are separate state. Each ships as a plain-text file under `data/style/`.
_Avoid_: dictionary, ruleset, pattern

**Well**:
The ground a code block is drawn on, run past both edges of the measure so the block reads as sunk into the page rather than as a stripe the width of the prose. A code span has the same ground without being a Well: it is padded, not sunk.
_Avoid_: code background, block highlight

## Quality

**Piece**:
One of the nine judged facets of the app: page, type, cursor and caret, focus and typewriter, dark and light, markup rendering, chrome and menus, file handling, latency.

**Parity oracle**:
The original JavaScript app, kept in `legacy/` until the native app wins every Piece against it blind. What the port ports; where it and the Design oracle disagree, `docs/design.md` says which Quill follows.
_Avoid_: the reference

**Design oracle**:
iA Writer for Mac running natively, as captured under `ref/ia/shots/mac-native/` and measured in `ref/ia/mac-native/`. Outranks every marketing frame elsewhere under `ref/ia/shots/`, and outranks the Parity oracle wherever `docs/design.md` says so.
_Avoid_: the reference, iA stills, the vision

**Gate**:
What must be true before native work lands, in three tiers: every commit (format, clippy, tests), every ticket (latency within budget and Blind judging for each Piece it names), every feature (the owner's Hand test).
_Avoid_: CI, checks, definition of done

**Hand test**:
The owner's numbered "do X, see Y" walk through a feature from the installed package; its pass comment closes the feature ticket.
_Avoid_: QA, acceptance test, manual test

**Judged state**:
One named configuration (passage, theme, Face, size, Focus, caret, chrome) at which a Piece is shot on both sides for Blind judging, the other side being the Parity oracle's frozen shot or the Design oracle crop the state names; each Piece has a fixed list of them.
_Avoid_: scenario, screenshot, fixture

**Capture ticket**:
A ticket for the owner's Mac naming the Design-oracle states a spec needs and no capture holds, in the shape `docs/design.md` § Adding or changing a row gives; until it lands the row stays unwritten and the state keeps the Parity oracle as opponent.
_Avoid_: screenshot request, reference ticket, research ticket (that is `wayfinder:research`: reading, never shooting)

**Blind judging**:
A critic with fresh context picks between two unlabelled screenshots (ours vs the opponent's) for a Piece and names the biggest gap of each.
