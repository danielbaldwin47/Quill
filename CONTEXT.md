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
Mode that dims everything except the current sentence or paragraph.
_Avoid_: zen mode, distraction-free

**Typewriter**:
Mode that keeps the caret's line at a fixed vertical position by scrolling the Editor.

**Preview**:
The rendered (HTML-like) view of a Document, opened beside or in place of the Editor.

**Export**:
Writing a Document out in another format (PDF, HTML, Markdown copy).

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

## Quality

**Piece**:
One of the nine judged facets of the app: page, type, cursor and caret, focus and typewriter, dark and light, markup rendering, chrome and menus, file handling, latency.

**Parity oracle**:
The original JavaScript app, kept in `legacy/` until the native app wins every Piece against it blind.

**Gate**:
What must be true before a change lands: clippy clean, engine unit tests green, latency bench within budget, blind judging per Piece, and a hand test from the installed package.

**Blind judging**:
A critic with fresh context picks between two unlabelled screenshots (ours vs the reference) for a Piece and names the biggest gap of each.
