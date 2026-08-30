# Hand test checklists for the ported Pieces

Each ported Piece is its own feature ticket, closed by the owner's `hand test: pass` (`docs/agents/gate.md` § Feature tier); these are its checklists. `/to-spec` merges a Piece's checklist with the spec's additions into the spec's **Hand test** section, so the owner tests from that comment alone.

**Page**: 1. Open `ref/sample.md`; the text sits in a measure of about 64 characters, centred, with calm margins. 2. Resize the window narrower and wider; the measure adapts and the text never touches an edge. 3. Open an empty Document; the caret waits on paper, no placeholder chrome.

**Type**: 1. Read a paragraph in Duo at the default size; lines are crisp and the leading reads about 1.7×. 2. Switch to Quattro, then Mono; each renders in its own face, none oblique. 3. Step the size up and down; the leading scales with it. 4. A bold and an italic word sit on the same baseline as their neighbours.

**Cursor and caret**: 1. Click into text; the caret is a single blue bar the full line height. 2. Type; the blink pauses while typing and resumes after. 3. Select a word by drag and by Shift+arrows; the selection is one continuous band beneath the glyphs with a bar at each end, the same height as the band, and the caret is hidden while the selection stands. 4. Click another window; the caret shows its unfocused state.

**Focus and typewriter**: 1. Turn Focus on; everything outside the current sentence dims, the caret sentence stays full ink. 2. Type past a full stop; the dim moves with the sentence. 3. Switch scope to paragraph; the whole paragraph is lit. 4. Turn Typewriter on and type a screen of text; the caret line holds its vertical position.

**Dark and light**: 1. Toggle the theme; paper and ink swap instantly with no flash. 2. In each theme, Markup, dimmed Focus text, selection and caret read as designed, not inverted. 3. Quit and relaunch; the last theme is remembered.

**Markup rendering**: 1. Type `# Heading`; the marker hangs into the margin and the heading is bold at body size. 2. Type `*emphasis*` and `**strong**`; markers dim, text styles, nothing shifts horizontally. 3. Type a list, a blockquote, a fenced code block and a link; each marker is quiet and the prose stays readable.

**Chrome and menus**: 1. Start typing; the chrome fades. 2. Stop, move the mouse; the title and the stats return. 3. Open each menu; every in-scope feature is reachable with its shortcut shown.

**File handling**: 1. Open a folder as the Library; its Documents list in the sidebar. 2. Create a new Document, type, wait; it is on disk as plain Markdown with nothing else written beside it. 3. Edit the file in another editor; Quill shows the change. 4. Rename and delete from the sidebar; the disk agrees.

**Latency**: 1. Open `shots/latency/doc10k.md` and type in the middle of it; the caret and glyph appear together with no visible lag. 2. Hold a key; repeat is smooth. 3. Quit and relaunch with that Document; the window is readable in well under a second.
