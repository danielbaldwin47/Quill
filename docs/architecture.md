# Native Quill: architecture

The shape of the Rust rewrite, decided in [#11](https://github.com/danielbaldwin47/Quill/issues/11) on
top of ADRs 0001–0010. Vocabulary is `CONTEXT.md`'s; quality rules are `docs/agents/gate.md`; the
research each section rests on is under `docs/research/` on the `research/<name>` branches. Feature
specs refine this document, never contradict it; a contradiction is an ADR conversation.

## Workspace

Two crates at the repo root ([ADR 0008](adr/0008-engine-crate-without-gtk.md)), Rust edition 2024,
GTK ≥ 4.22 through `gtk4-rs` 0.11, MSRV whatever `gtk4-rs` pins (1.92 today):

- **`quill-engine`**: everything that can be tested without a display. No dependency on `gtk`; may
  depend on `pango`, `cairo`, `pulldown-cmark`, `aho-corasick`, `harper-brill`, `notify`, `toml`,
  `serde`, and Quill's own `extern "C"` declarations for enchant.
- **`quill`**: the binary. `GtkApplication`, windows, the Editor widget, Preview widget, Library
  sidebar, chrome, menus, command-line flags. Depends on `quill-engine`.

Every type both crates share is defined in the engine. `tools/gate check` runs `cargo test` on the
workspace; a test that needs a window is harness, not test.

Engine modules, one per concept: `document` (text and block index), `markdown` (the parser, one
shared `Options`), `annotate` (the Annotator trait, spans, run flattening), `library`, `settings`,
`template`, `render` (Pango layout for Preview, PDF and HTML), `stats`, `outline`, `spell` (the
`SpellChecker` trait and the enchant and `spellbook` implementations), `pos` (Syntax highlight),
`style` (Style check). App modules mirror the Pieces and features: `editor`, `caret`, `focus`,
`typewriter`, `theme`, `chrome`, `library`, `preview`, `export`, `flags`, `harness`.

## Text model

The `GtkTextBuffer` owns the text the writer edits (undo, IME preedit, clipboard, selection and
accessibility come with it). The engine keeps its own `String` copy per Document, spliced from the
buffer's `insert-text` and `delete-range` signals before any Annotator runs; at 288 KB for a 55,000-word
Document the splice is tens of microseconds, so there is no rope.

All engine offsets are **UTF-8 bytes** from the start of the Document, because that is what the
parser emits. The app maps a byte offset to a `GtkTextIter` by line number plus byte index within the
line (`set_line` then `set_line_index`), never by counting characters from the top.

A Document's block index is the list of top-level blocks, each `(byte range, kind, container
context)` where the context is any open fence, front-matter block or list nesting. Edits shift the
ranges after the edit by the edit's byte delta.

## Annotators and the keystroke path

An Annotator turns a byte range of the Document into spans, each `(byte range, mark)`. Four exist:
Markup (from the parser: which bytes are Markup, which are heading, emphasis, strong, code, link,
quote, list marker), Syntax highlight (a UPOS tag per word), Style check (a list name per match) and
Spell check (a misspelling per word, suggestions fetched on demand). Syntax highlight, Style check and
Spell check consume the **prose stream**: the parser's `Text` events with Markup, code spans, fenced
code, URLs and front matter removed. They never see a `#` or a `*`.

Two lanes, and the budget is the Gate's ≤ 5 ms mean, ≤ 16 ms worst from keystroke to presented frame:

- **Synchronous, on the keystroke**: splice the text, find the containing block, widen to its
  neighbours when the edit touches a blank line, a fence marker, a list marker or a setext underline,
  re-parse that slice with a fresh parser, rebase the ranges, splice the Markup spans, re-flatten the
  runs for the affected lines and retag the buffer. The spike measured 0.082 ms for a whole-document
  retag; the block re-parse must stay flat across Document sizes and under 200 µs at 55,000 words.
- **Asynchronous, on one worker thread**: Syntax highlight, Style check and Spell check re-run for the
  changed paragraphs only, debounced, viewport first. Each result carries the Document generation it
  was computed against; the main thread applies a result whose generation is current and discards the
  rest. Dictionary and tagger loading happen on that thread at first use, never on a keystroke or at
  startup.

Whole-document passes (link-reference and footnote definitions, Stats, the heading outline) run on
idle after the synchronous lane, never inside it.

**Tags.** Overlapping `GtkTextTag`s override a property by priority; they do not blend. So colour is
flattened: Markup tier × Focus tier × Syntax highlight resolve into non-overlapping runs, one
precomputed colour and alpha each, and the tag table holds one tag per distinct `(colour, alpha)`
and one per `(weight, slant)`, created lazily and never removed. Decorations are separate tags
layered over the runs: one `underline: error` tag for Spell check, one per Style check list, one for
selection-independent things such as the Focus dim of a heading marker. Underline and colour are
different properties, so those overlaps are safe.

**Leading.** Line pitch is `round(1.30 × size + 10.4)` pixels, split three ways as ADR 0004 requires:
`pixels-above-lines`, the same value as `pixels-inside-wrap`, and half of it as `pixels-below-lines`.
Font sizes are absolute pixels (`set_absolute_size`), never points.

## Documents and files

A Document is a path. Opening reads the file, parses it whole (cold-start cost, inside the 250 ms
budget), and records the file's modification time. **Autosave** writes the Document after one second
of idle, on window focus loss, on Library actions and on quit: to a temporary file in the same
directory, then renamed over the original with its permissions preserved. Markdown export is the
file's bytes verbatim; Quill never re-serialises Markdown.

The engine watches every open Document's file and the Library tree with `notify` (inotify). A change
on disk to a clean Document reloads it in place, keeping the caret's block; a change to a Document
with unsaved edits is a conflict, and the Library spec decides the writer-facing policy. Rename and
delete go through the Library and the disk agrees before the sidebar does.

## Library

The Library is the folder tree the writer pointed Quill at, plus recents. It is **in memory**: walked
on launch, watched with inotify, never persisted as an index ([ADR 0002](adr/0002-plain-markdown-documents.md)
leaves nothing in the folder, and there is no stale index to delete). Config remembers the location;
state remembers recents and per-Document caret positions. Content search, if the Library spec wants
it, scans `.md` files live. The Library is shared by all windows.

## Windows

`GtkApplication` with the id `io.github.danielbaldwin47.Quill`, single instance, `HANDLES_OPEN`: a
file opened from a file manager or the shell reaches the running instance. Single instance is a
writer's launch; a launch carrying a flag is not (see Command-line flags). One Document per window,
any number of windows; the Editor, Preview and Stats belong to a window, the Library and settings to
the application. Plain GTK4 without libadwaita ([ADR 0009](adr/0009-plain-gtk4-without-libadwaita.md));
theme `auto` follows the settings portal's colour scheme.

## Settings

One TOML file at `$XDG_CONFIG_HOME/quill/settings.toml` for what the writer chose; `$XDG_STATE_HOME/quill/`
for what the app observed ([ADR 0010](adr/0010-settings-in-toml-under-xdg.md)). Missing keys take
defaults; unknown keys and unknown tables are kept, so an older Quill never destroys a newer file.
Every write to either file goes through a temporary file beside it and a rename, so a write that
fails leaves the file it was replacing whole.

Config: `theme` (auto, light, dark), `face` (duo, quattro, mono), `size` (pixels, default 20),
`focus` (on/off) and `focus_scope` (sentence, paragraph), `typewriter` (on/off) and
`typewriter_anchor` (0–1, default 0.5), `chrome` (shown/hidden), `spell_check` (on/off, default on)
and `spell_language`, `[syntax_highlight]` (a table: `enabled` is the master, and the five category
toggles sit beside it), `[style_check]` (the same shape, one toggle per list beside `enabled`),
`template` (the current Template's name), `preview_layout` (split, full), `library` (the Library
path), and a `[shortcuts]` table of Command id → chords that
replaces the defaults in [`shortcuts.md`](shortcuts.md) ([ADR 0011](adr/0011-shortcut-precedence-on-linux.md)).

The settings file is watched with `notify` like a Document: a saved edit applies without a restart,
and a line that cannot be applied is logged once and skipped, never fatal.

State, in `state.toml`: the size of each window and whether it was maximized or full screen, the last
Document per window, caret position per recent Document, the recents list, and the Gate's blind keys
under `blind-keys/` beside it. Not window position: GTK4 gives a client no way to ask where its
window is or to put it back, on Wayland or on X11, so placement is the compositor's and Quill
remembers nothing it could not act on.

## Preview and Export

The engine's `render` module lays a whole Document out with Pango from the current Template ([ADR
0005](adr/0005-native-templates.md)): one pass produces the layouts the Preview widget snapshots and
the pages the PDF surface draws. Preview re-renders on idle after edits, debounced, and restores its
scroll to the block the caret is in. PDF export runs through `GtkPrintOperation` in export mode so
Print and Export to PDF are one path, with heading bookmarks from the outline and page geometry
(size, margins, header, footer, title page) owned by Export, not the Template. HTML export is the
parser's HTML plus the CSS the Template generates, inlined. Annotator marks never reach Preview or
Export.

Preview ships without tables, figures and footnote blocks first; they are the last renderer work and
sit behind the Gate like everything else.

## Fonts and data files

Before GTK initialises, startup calls `FcConfigAppFontAddDir` on the six Faces ([ADR 0007](adr/0007-quill-faces-renamed-and-private.md)).
Data files (fonts, Templates, the Style check lists, the tagger model, `OFL.txt`) are resolved from
one data directory: `$QUILL_DATA_DIR` if set, else the directory compiled in at build time
(`/usr/share/quill` for the package), else the repo root for a development build. Nothing is
downloaded at build time.

## Command-line flags

The harness drives the app through flags applied before the first frame; the Gate names the states
and the determinism settings, this document names the flags:

- Judged state: `--text <file>`, `--theme light|dark`, `--font duo|quattro|mono`, `--size <px>`,
  `--focus off|sentence|paragraph`, `--typewriter`, `--chrome on|off`, `--caret <offset>|end`,
  `--select <from>,<to>`, `--scroll <fraction>`, `--nocaret`, `--w <px> --h <px>`.
- Harness: `--deterministic` (animations off, blink off, manual font rendering with pinned antialias,
  slight hinting, no subpixel, 96 dpi, hinted metrics, no client-side decorations),
  `--measure <out.jsonl>` (key capture in the capture phase, `GdkFrameTimings` presentation times,
  cold start against `QUILL_T0_NS`).

Every flag has a matching setting or a harness-only effect; none creates state a writer cannot reach.

A launch carrying any of them is the harness's rather than a writer's, and that decides three things
about it. It runs non-unique, so a judged shot or a bench is served by the process that was launched
even when a writer's Quill is already open. It overrides the settings for that launch alone and
writes nothing back to `settings.toml`. And it neither reads nor writes `state.toml`, so it opens at
the shape its flags name rather than at the window a writer left, the same command line is the same
window twice, and a bench at 1440×900 is not a writer resizing anything.

## Packaging

`PKGBUILD` builds the workspace with `cargo build --release --locked` from the working tree
(`cargo fetch` in `prepare`, so `makepkg` needs the network only there), `arch=('x86_64')`,
`license=('GPL-3.0-or-later' 'OFL-1.1')`, `depends=('gtk4' 'enchant' 'hicolor-icon-theme')`,
`makedepends=('cargo')`, `optdepends=('hunspell-en_us: English spell checking')`. It installs the
binary as `/usr/bin/quill`, data under `/usr/share/quill/`, the `.desktop` file and icon under the
application id, `fonts/OFL.txt` beside the fonts and under `/usr/share/licenses/quill/`. With no
dictionary installed, Spell check shows a "no dictionary" state rather than failing.

Flatpak comes later (the map's fog) and this design keeps it cheap: fonts are private, enchant and
its English dictionary are in `org.gnome.Platform`, data resolves through one directory, and nothing
runs at build time that needs the network.

**Licence rule.** Every dependency and data file is GPL-3.0-or-later compatible; no CC BY-SA data
ships in Quill, because it is compatible with GPL-3.0 only and breaks "or-later".

## Repo migration

One commit, once the Cargo workspace is standing beside the JavaScript app. The owner agreed at
[#52](https://github.com/danielbaldwin47/Quill/issues/52)'s creation that the workspace lands first,
so the move finds Rust already at the root rather than clearing the ground for it:

- `app/`, `tools/{serve,shoot,crop,latency,smoke}.mjs`, `bin/quill`, `package.json` and the Node
  lockfile move under `legacy/` unchanged; `legacy/bin/quill` still launches the JavaScript app from
  the checkout. `tools/blind.mjs`, `tools/thumb.mjs`, `tools/progress.mjs`, `tools/uinput-keys.py`
  and `tools/idle-check.py` stay at the root: the Gate uses them for the native app.
- `Cargo.toml` (workspace), `quill-engine/`, `quill/`, `tools/fontbuild.py` and `fonts/` are at the
  root by then; `tools/gate` joins them with the Gate tooling.
- `legacy/LICENSE` is ISC, the licence the Node manifest always named; the root `LICENSE` stays
  GPL-3.0-or-later, and the two halves of the tree are licensed apart.
- `PKGBUILD` switches to the native binary in the same commit. The Feature tier hand-tests from the
  installed package, and the legacy app needs no installation to serve as the Parity oracle.
- `README.md` and `CLAUDE.md` are rewritten for the new layout; `BRIEF.md` and `NOTES.md` move to
  `legacy/` with the app they describe.

`legacy/` is the Parity oracle: `shots/oracle/<piece>/<state>.png` is generated from it with
`legacy/tools/shoot.mjs` and regenerated only when `legacy/` changes. **Retirement** is its own
ticket, opened when every Piece's latest verdict in `progress/rounds/` is ours and all nine ported
Pieces' Hand tests have passed; the owner's `hand test: pass` on that ticket is the declaration. That
ticket deletes `legacy/` and switches `tools/gate judge`'s opponent to the iA reference.

## Port order

Ticket zero of the build effort is the Gate tooling ([#19](https://github.com/danielbaldwin47/Quill/issues/19));
no Piece can close without it. Then, each Piece a feature ticket closed by its Hand test in
`docs/agents/gate.md`:

1. The type
2. The page
3. Markup rendering
4. Cursor & caret
5. Dark & light
6. Focus & typewriter
7. Latency
8. Chrome & menus
9. File handling

The spike proved 1–4 and 6, so they land first and give the Gate something to judge; theme precedes
focus because the dim tiers are palette colours; latency is measured once the keystroke path is
complete and before chrome adds anything that animates; file handling is the largest new code and the
least judged by pixels. Features follow the Pieces, in the order their specs are ready.
