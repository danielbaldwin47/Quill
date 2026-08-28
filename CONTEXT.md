# Quill

A long-form writing app for Linux in the spirit of iA Writer: plain Markdown files, typography first, chrome that stays out of the way.

## Language

**Document**:
One Markdown file on disk. The file is the only source of truth; Quill stores nothing about it elsewhere.
_Avoid_: note, page, buffer

**Library**:
The folder tree Quill has been pointed at, browsed from the sidebar.
_Avoid_: workspace, vault, project

**Editor**:
The text surface where a Document is written. Renders markup styling inline without changing glyph advance widths.
_Avoid_: text view, canvas

**Markup**:
The Markdown syntax characters themselves (`#`, `*`, `[`), styled dimmer than prose so they read as structure, not content.

**Focus**:
Mode that dims everything except the current sentence or paragraph; its scope is Sentence or Paragraph. Independent of Typewriter.
_Avoid_: zen mode, distraction-free

**Typewriter**:
Mode that keeps the caret's line at a fixed vertical position by scrolling the Editor. Independent of Focus, never a Focus scope.

**Preview**:
The rendered (HTML-like) view of a Document, opened beside or in place of the Editor.

**Export**:
Writing a Document out in another format (PDF, HTML, Markdown copy).

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
Underlining clichés, fillers and redundancies in prose.
_Avoid_: grammar check, linting

**Spell check**:
Underlining misspellings against a system dictionary, with suggestions; a toggle, on by default.

**Stats**:
Word count, character count and reading time for the Document or the selection.

**Heading navigation**:
The outline of a Document's headings, used to jump between sections.
_Avoid_: table of contents (that is a Preview/Export feature)

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
Anything that marks spans of a Document's prose for the Editor to style: Markup, Syntax highlight, Style check and Spell check are the four. An Annotator sees the prose stream, never the Markup characters.

**Well**:
The ground a code block is drawn on, run past both edges of the measure so the block reads as sunk into the page rather than as a stripe the width of the prose. A code span has the same ground without being a Well: it is padded, not sunk.
_Avoid_: code background, block highlight
_Avoid_: highlighter, linter, decorator

## Quality

**Piece**:
One of the nine judged facets of the app: page, type, cursor and caret, focus and typewriter, dark and light, markup rendering, chrome and menus, file handling, latency.

**Parity oracle**:
The original JavaScript app, kept in `legacy/` until the native app wins every Piece against it blind.

**Gate**:
What must be true before native work lands, in three tiers: every commit (format, clippy, tests), every ticket (latency within budget and Blind judging for each Piece it names), every feature (the owner's Hand test).
_Avoid_: CI, checks, definition of done

**Hand test**:
The owner's numbered "do X, see Y" walk through a feature from the installed package; its pass comment closes the feature ticket.
_Avoid_: QA, acceptance test, manual test

**Judged state**:
One named configuration (passage, theme, Face, size, Focus, caret, chrome) at which a Piece is shot on both sides for Blind judging; each Piece has a fixed list of them.
_Avoid_: scenario, screenshot, fixture

**Blind judging**:
A critic with fresh context picks between two unlabelled screenshots (ours vs the reference) for a Piece and names the biggest gap of each.
