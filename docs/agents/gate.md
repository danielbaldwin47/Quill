# The Gate

What must be true before native Quill work lands. The owner does not read Rust: the Gate, not code review, keeps quality honest, so every check here ends in evidence the owner can read without opening the code. Decided in [Gate spec: the checks every native change must pass](https://github.com/danielbaldwin47/Quill/issues/12); the build spec is the `ready-for-agent` issue that resolution links.

The Gate has three tiers, keyed to what is landing. Every ticket names the Pieces it touches; "Pieces: none" is a valid answer and must be written.

## Commit tier: every commit

`tools/gate check` is green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` on the default lint set, and `cargo test` with no display attached. A test that needs a window is harness, not test; it belongs under `tools/gate judge` or `bench`. An `#[allow(...)]` carries a one-line reason on the same line; clippy's `pedantic` group stays off. There is no coverage number: each spec's Testing Decisions names the seams its tests hold.

Done when: `tools/gate check` prints its final `pass` line.

## Ticket tier: before a ticket closes

For every Piece the ticket names, the evidence below is committed and its summary lines are pasted on the ticket.

**Latency** (any ticket naming the latency Piece; every ticket touching the Editor's keystroke path names it). `tools/gate bench` runs the headline regime, `prose_end_of_draft` at 133 wpm on the 10,062-word `shots/latency/doc10k.md`, with real keys through `/dev/uinput` and presentation from `GdkFrameTimings` on the dedicated scale-2 headless output. Hard budget, uinput `write(2)` → presented: **≤ 5 ms mean, ≤ 16 ms worst**. Cold start, `exec` → first complete frame with a non-zero presentation time, the same document open: **≤ 250 ms**. Beating the Parity oracle's own numbers (2.43 ms mean, 15.61 ms worst, 12.17 ms uinput → presented, 370 ms cold) is what wins the latency Piece blind, not the budget. The physical panel (DP-3) is recorded whenever `tools/idle-check.py` allows it and is never a Gate condition. Results land in `shots/latency/` with the environment fingerprint `tools/latency.mjs` records today.

**Blind judging** (every other Piece). `tools/gate judge <piece>` shoots ours and the opponent at the Piece's judged states (identical theme, font, size, focus, caret, passage; 1440×900 at scale 2, captured with `grim -T` per `docs/research/native-harness.md` on `research/native-harness`), pairs them with `tools/blind.mjs`, runs a fresh-context critic (the gauntlet's critic prompt, Opus at high effort), reveals, and writes `progress/rounds/<piece>-r<N>.json` in the existing shape. The opponent is the **Parity oracle** while `legacy/` exists, from frozen shots under `shots/oracle/<piece>/<state>.png` regenerated only when `legacy/` changes; once all nine Pieces are won and `legacy/` is deleted, and for any feature with no oracle state, the opponent is the iA reference crop in `ref/ia/`. Ours wins at any margin. **A Piece once won is never lost**: a ticket naming a won Piece re-judges it, and a loss blocks the ticket.

Done when: every named Piece has a committed verdict or bench result from this ticket's build, and the summary lines are on the ticket.

## Feature tier: before a feature ticket closes

The owner hand-tests the feature from the installed package: `makepkg -f && sudo pacman -U quill-*.pkg.tar.zst`, then the feature's Hand test checklist. The checklist lives in the feature spec, in this shape:

- Numbered "do X, see Y" steps, at most ten, each naming its concept with `CONTEXT.md` vocabulary.
- Every step observable in the running app; nothing that needs a terminal or a log.

Done when: the owner comments `hand test: pass` with the `pacman -Q quill` output on the feature ticket. That comment closes the ticket.

Before a release, additionally: all twelve latency regimes clear the budget, and every Piece's latest verdict is ours.

### Hand test checklists for the ported Pieces

Each ported Piece is its own feature ticket; these are its checklists.

**Page**: 1. Open `ref/sample.md`; the text sits in a measure of about 64 characters, centred, with calm margins. 2. Resize the window narrower and wider; the measure adapts and the text never touches an edge. 3. Open an empty Document; the caret waits on paper, no placeholder chrome.

**Type**: 1. Read a paragraph in Duo at the default size; lines are crisp and the leading reads about 1.7×. 2. Switch to Quattro, then Mono; each renders in its own face, none oblique. 3. Step the size up and down; the leading scales with it. 4. A bold and an italic word sit on the same baseline as their neighbours.

**Cursor and caret**: 1. Click into text; the caret is a single blue bar the full line height. 2. Type; the blink pauses while typing and resumes after. 3. Select a word by drag and by Shift+arrows; the selection is one continuous band beneath the glyphs, no bars at its ends. 4. Click another window; the caret shows its unfocused state.

**Focus and typewriter**: 1. Turn Focus on; everything outside the current sentence dims, the caret sentence stays full ink. 2. Type past a full stop; the dim moves with the sentence. 3. Switch scope to paragraph; the whole paragraph is lit. 4. Turn Typewriter on and type a screen of text; the caret line holds its vertical position.

**Dark and light**: 1. Toggle the theme; paper and ink swap instantly with no flash. 2. In each theme, Markup, dimmed Focus text, selection and caret read as designed, not inverted. 3. Quit and relaunch; the last theme is remembered.

**Markup rendering**: 1. Type `# Heading`; the marker hangs into the margin and the heading is bold at body size. 2. Type `*emphasis*` and `**strong**`; markers dim, text styles, nothing shifts horizontally. 3. Type a list, a blockquote, a fenced code block and a link; each marker is quiet and the prose stays readable.

**Chrome and menus**: 1. Start typing; the chrome fades. 2. Stop, move the mouse; the title and the stats return. 3. Open each menu; every in-scope feature is reachable with its shortcut shown.

**File handling**: 1. Open a folder as the Library; its Documents list in the sidebar. 2. Create a new Document, type, wait; it is on disk as plain Markdown with nothing else written beside it. 3. Edit the file in another editor; Quill shows the change. 4. Rename and delete from the sidebar; the disk agrees.

**Latency**: 1. Open `shots/latency/doc10k.md` and type in the middle of it; the caret and glyph appear together with no visible lag. 2. Hold a key; repeat is smooth. 3. Quit and relaunch with that Document; the window is readable in well under a second.
