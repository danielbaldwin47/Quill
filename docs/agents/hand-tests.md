# Hand test checklists for the ported Pieces

Each ported Piece is its own feature ticket, closed by the owner's `hand test: pass` (`docs/agents/gate.md` § Feature tier); these are its checklists. `/to-spec` merges a Piece's checklist with the spec's additions into the spec's **Hand test** section, so the owner tests from that comment alone.

**Page**: 1. Open `ref/sample.md`; the text sits in a measure of about 64 characters, centred, with calm margins. 2. Resize the window narrower and wider; the measure adapts and the text never touches an edge.

**Type**: 1. Read a paragraph in Duo at the default size; lines are crisp and the leading reads about 1.7×. 2. Switch to Quattro, then Mono; each renders in its own face, none oblique. 3. Step the size up and down; the leading scales with it. 4. A bold and an italic word sit on the same baseline as their neighbours.

**Cursor and caret**: 1. Click into text; the caret is a single blue bar the full line height. 2. Type; the bar moves with the text, and the blink pauses while typing and resumes after. 3. Select a word by drag and by Shift+arrows; the selection is one continuous band beneath the glyphs, with no bar at either end and no caret anywhere while it stands (ADR 0014). 4. Click another window; the caret shows its unfocused state. 5. Hold a key down and watch the bar through the repeat; it stays solid the whole time and never blinks under the hand. 6. Let go and watch it; it fades out about half a second later and is blinking on a turn of about a second, lit about as long as it is dark. A bar still solid a second after the last key is the defect.

**Focus and typewriter**: 1. Turn Focus on; everything outside the current sentence dims, the caret sentence stays full ink. 2. Type past a full stop; the dim moves with the sentence. 3. Switch scope to paragraph; the whole paragraph is lit. 4. Turn Typewriter on and type a screen of text; the caret line holds its vertical position.

**Dark and light**: 1. Toggle the theme; paper and ink swap instantly with no flash. 2. In each theme, Markup, dimmed Focus text, selection and caret read as designed, not inverted. 3. Quit and relaunch; the last theme is remembered. 4. With the palette set up (`README.md` § Theme Quill with the desktop) and Quill open, `omarchy theme set` a dark theme; paper, ink and accent take the theme's colours in one repaint, no relaunch. 5. `omarchy theme set` a light theme with `theme = "auto"`; Quill lands on light and the light ground is the theme's. 6. Toggle the theme; the other ground is Quill's built-in, because the template wrote one table. 7. Delete the rendered `quill.toml`; the built-ins return. 8. Launch `quill --theme dark`; the built-in dark ground, whatever the file says. 9. Misspell one value in the file; every other colour still paints and only that role is the built-in.

**Markup rendering**: 1. Type `# Heading`; the marker hangs into the margin and the heading is bold at body size. 2. Type `*emphasis*` and `**strong**`; markers dim, text styles, nothing shifts horizontally. 3. Type a list, a blockquote, a fenced code block and a link; each marker is quiet, the list and quote markers sit on the body column with no rule beside the quote (`docs/design.md` § What hangs), and the prose stays readable.

**Chrome and menus**: 1. Start typing; the chrome fades. 2. Stop, move the mouse; the title and the stats return. 3. Open each menu; every in-scope feature is reachable with its shortcut shown. 4. Open an empty Document; both bars stand, the stats bar reads zero words, and nothing in the frame is waiting on text to fill in.

**File handling**: 1. Open a folder as the Library; its Documents list in the sidebar. 2. Create a new Document, type, wait; it is on disk as plain Markdown with nothing else written beside it. 3. Edit the file in another editor; Quill shows the change. 4. Rename and delete from the sidebar; the disk agrees.

**Latency**: 1. Open `shots/latency/doc10k.md` and type in the middle of it; the caret and glyph appear together with no visible lag. 2. Hold a key; repeat is smooth. 3. Quit and relaunch with that Document; the window is readable in well under a second.
