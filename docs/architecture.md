# Native Quill: architecture

The shape of the Rust rewrite, decided in [#11](https://github.com/danielbaldwin47/Quill/issues/11) on
top of ADRs 0001–0010. Vocabulary is `CONTEXT.md`'s; quality rules are `docs/agents/gate.md`; where
the Design oracle and the Parity oracle disagree on the writing surface, `docs/design.md` decides
([ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md)); the research each section
rests on is under `docs/research/` on the `research/<name>` branches. Feature specs refine this
document, never contradict it; a contradiction is an ADR conversation.

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
shared `Options`), `front_matter` (the `title`, `author` and `date` Export reads out of a
Document's metadata block, and nothing written back), `annotate` (the Annotator trait, spans, run
flattening), `focus` (sentence
segmentation, the bright tier and the one dim tier —
[ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md)), `library`, `settings`, `template`,
`render` (Pango layout for Preview, PDF and HTML), `paginate` (a rendered page cut into pages of
paper under Export's geometry), `html` (the standalone export page and the body fragment Copy as
HTML carries), `draw` (one page of paper painted onto a cairo context, for both of Export's page
sinks), `pdf` (the PDF file: the surface, the metadata and the bookmarks), `stats`, `outline` (the
heading list a bookmark and Heading navigation are made of), `spell` (the `SpellChecker`
trait and the enchant and `spellbook` implementations), `pos` (Syntax highlight), `worker` (the
asynchronous Annotators' thread and generation-checked results), `style` (Style
check), `typography` (the pitch, the measure, the 78-cell text container and its gutters —
[ADR 0016](adr/0016-the-text-container-is-78-cells.md) — and the page margins), `theme` (the two
grounds' colour table and the rule that resolves one, and the rule that reads a change of the
desktop's against it), `commands` (`docs/shortcuts.md` as data: every Command with its chords
and menu rows, and the reserved and off-limits chord lists), `shortcuts` (the `[shortcuts]` table
checked against that registry and overlaid on its defaults). App modules mirror the Pieces and
features: `editor` (which also installs the display's stylesheet, where the engine table's colours
are painted from), `caret`, `focus`,
`typewriter`, `portal` (the settings portal: the desktop's colour scheme, read before the first frame
and listened to after it), `chrome`, `library`, `preview`, `export`, `flags`, `harness`.

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

An Annotator turns a byte range of the Document into spans, each `(byte range, mark)`. Five exist:
Markup (from the parser: which bytes are Markup, which are heading, emphasis, strong, code, link,
quote, list marker), Live (from the Markup spans and the caret: which marker bytes are folded away,
which bytes are a heading's and at what level, and what furniture stands in a folded marker's
cells), Syntax highlight (a Category per word — Nouns, Verbs, Adjectives, Adverbs or Conjunctions;
the Universal POS tag it reads them from never leaves `quill_engine::pos`), Style check (a List per
match — Fillers, Redundancies or Clichés, matched over the union of the three shipped phrase lists
whatever a toggle says, and a redundancy emitting only the words it strikes) and Spell check (a misspelling per word, suggestions fetched on demand). Syntax
highlight, Style check and Spell check consume the **prose stream**: the parser's `Text` events with
Markup, code spans, fenced code, URLs and front matter removed. They never see a `#` or a `*`.

Stats is not an Annotator and not on the worker: `quill_engine::stats` counts its six Statistics in
one walk of the parser's events — the prose plus the code the writer typed, which is the prose
stream widened by code spans and fenced blocks — on the main thread, on the idle pass after typing
stops and synchronously on a selection change.

Two lanes, and the budget is the Gate's ≤ 5 ms mean, ≤ 16 ms worst from keystroke to presented frame:

- **Synchronous, on the keystroke**: splice the text and advance the Document generation, find the
  containing block, widen to its neighbours when the edit touches a blank line, a fence marker,
  a list marker or a setext underline,
  re-parse that slice with a fresh parser, rebase the ranges, splice the Markup spans, re-flatten the
  runs for the affected lines and retag the buffer. The spike measured 0.082 ms for a whole-document
  retag; the block re-parse must stay flat across Document sizes and under 200 µs at 55,000 words.
- **Asynchronous, on one worker thread**: Syntax highlight, Style check and Spell check re-run for the
  changed paragraphs only, debounced, viewport first. Each result carries the Document generation it
  was computed against; the main thread applies a result whose generation is current and discards the
  rest. Dictionary, tagger and Style check list loading happen on that thread at first use, never on
  a keystroke or at startup. `quill_engine::worker::Worker` starts the thread on its first request and reuses it;
  dropping it closes its channels without joining on the main loop.

The worker's `Request` carries the Document `generation`, the Annotators `wanted` (any of Syntax
highlight, Style check and Spell check), the changed `Paragraph`s (each an `index` and its `prose`
text), and the `viewport` paragraph-index range. It answers one `ParagraphResult` per paragraph:
the same `generation`, the `paragraph` index, and three span sets whose byte ranges address that
paragraph's requested prose — `categories` for Syntax highlight, `lists` for Style check, the
misspelled words for Spell check, the set of an Annotator the request did not want left empty, so a
paragraph is sent once and tagged once whichever Annotators are on. The channel carries two kinds of
message, a request and a dictionary edit (Add, Ignore, a language change); an edit is applied in
order before the next request, so a re-request after an Add checks against the list it just grew.

**Spell check** holds one `SpellChecker` (`quill_engine::spell`, enchant behind Quill's own `extern
"C"` block) in a mutex shared by the worker and the main thread. The worker creates it at first use
and replaces it whole on a language change, and locks it per paragraph, never across a request, so
the main thread waits for one paragraph at most. The worker's constructor makes the handle empty and
a getter shares it, since the app reaches the thread only through requests. Suggestions are the one
main-thread call, under that lock, on a right-click or the menu chord and never on the keystroke
lane; a right-click before the first load finds the handle empty and shows no corrections. Add,
Ignore and a language change go to the worker as edits, each followed by a whole-Document
re-request, viewport first. A pure engine function walks a paragraph's prose into words by the
dictionary's word-character rule (start, middle, end), so the dictionary decides whether an
apostrophe or a hyphen is inside a word; a token holding a digit is skipped, and all-caps and
CamelCase are checked. The engine's spans are complete: the **caret rule** is the Editor's, applied
at paint by a second pure function — the span the caret stands inside is withheld while the
character before the caret is a word character, and returns when the caret leaves the span or a
non-word character is typed after it — so the painting carries the caret's paragraph offset always,
Live or not, and moving the caret out of a withheld word repaints that paragraph's decorations. The
language resolves on the main thread (§ Settings). `spell_check` off sends no Spell request and
takes the tag off the buffer at once; on re-requests the Document.

**Corrections** extend GTK's own context menu through the text view's extra menu: one section at its
top — up to five suggestions, then Add to Dictionary, then Ignore — backed by a `spell` action group
on the Editor with three actions, replace (the suggestion as a string target), add and ignore. A
secondary press first puts the caret at the press point and selects the misspelled span it lies in,
and the section is built for the word at the caret, so a menu opened by the Menu key or `Shift+F10`
reads the same word as the pointer's; a caret in no misspelled span leaves the extra menu empty, and
GTK shows Cut, Copy and Paste alone. Replace substitutes the span inside one user-action pair, so it
is one Undo step, through the window's ordinary splice, retag, furniture and autosave, and leaves the
caret after the new word. Add writes enchant's personal list for the current tag
(`~/.config/enchant/<tag>.dic`, shared with every enchant application), Ignore enchant's session
list; there is no permanent Ignore and no stored replacement. Viewport paragraphs arrive first, with request order kept within each
group. The app's `Syntax::accept` delegates to the paragraph's engine `SpanStore::apply`, generic
over the span's kind: it checks the current Document generation, then takes its own kind out of the
answer and maps it into source-relative spans, leaving the other kind for its own store; pending or
stale answers leave its last spans in place. The app owns the prose-to-source mapping and paragraph-index changes after structural
edits. Dirty-block extraction uses the Document's resolved link marks to omit reference labels
whose definitions live in other blocks. Document insert, delete and reload advance the
generation; equality compares the Document's content and indexes, excluding that edit history.
The app uses `worker::DEBOUNCE` for the quiet period after the last keystroke and drains
`Worker::try_recv` on idle. The engine owns neither a timer nor a main-loop source (ADR 0008).

Whole-document passes (link-reference and footnote definitions, Stats, the heading outline) run on
idle after the synchronous lane, never inside it. Heading navigation reads the outline off the block
index when the Palette opens on it and drops it as the panel closes; PDF bookmarks read it off the
rendered page, whose words carry Number Headings' numbers; neither runs on a keystroke.

**Tags.** Overlapping `GtkTextTag`s override a property by priority; they do not blend. So colour is
flattened: Markup tier × Focus tier × Syntax highlight × Style check resolve into non-overlapping
runs, one precomputed colour and alpha each, and the tag table holds one tag per distinct
`(colour, alpha)` and one per `(weight, slant)`, created lazily and never removed. **Style check is
in the flattening and not over it**: the Design oracle re-inks a struck run to the quiet tier rather
than ruling a line over the ink it had, so a struck word loses the Category it was carrying — an
ordering between the two Annotators, Style check last — and takes the Focus dim like any other run
(#354, `ref/ia/mac-native/VERDICTS.md` § The Style Check mark). Decorations are separate tags
layered over the runs: one `underline: error` tag for Spell check, coloured through
`underline-rgba` from the `spell` Role and carrying no ink of its own, so a Focus dim, a Category
and a strike on the same word all still show and the Editor's selection fills sit under it; one per Style check List — three
identical strikes, split so that a List switched off takes its own tag off the page and leaves the
other two, never so that the Lists read differently, and each carrying no colour of its own so the
rule is drawn in the run's — one for selection-independent things such as the transparent underline
a dim URL takes (`focus.css:41`).
Focus's own dim is not among them: it is a colour, so it resolves into the run rather than layering
over it (`quill_engine::annotate::paint`, #126). Syntax highlight is the third tier and enters the
same flattening as an ink laid over the Markup runs rather than a mark resolved with them
(`quill_engine::annotate::paint_tagged`, #313), which is what keeps a Category off a marker and off
a link's plumbing. Underline and colour are different properties, so those overlaps are safe.

**Leading.** Line pitch is the ladder's pitch per step (`ref/ia/mac-native/NOTES.md` § 11; 1.711 em
at the default), in device px at scale 2 and `round(value × scale / 2)` at any other: iA's liquid
leading as measured on the Design oracle, not a fitted curve (`docs/design.md` § Line pitch). The air
a row leaves over, `pitch − row`, is split three ways as ADR 0004 requires, and the split is fixed by
the two gaps GTK actually draws: `pixels-inside-wrap` carries all of it, because it alone separates
two rows of one paragraph, and the paragraph gap is halved, because only the sum of its two halves
separates two paragraphs. The three therefore do not sum to the air. GTK is set to that sum as
`pixels-above-lines` and to nothing below, because a bottom band is where it aborts on a line holding
invisible bytes (ADR 0004's status line, #279); the page's margins move to match, and the code well's
boundary rows are given the lower half back through tags. Font sizes are
absolute pixels (`set_absolute_size`), never points, and the em is the ladder's value in logical px
— 21.33 at the default step — so it is no longer an integer (`docs/design.md` § Text sizes).

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
leaves nothing in the folder, and there is no stale index to delete). Config remembers the Locations and what is Pinned;
state remembers recents (25, newest first) and per-Document caret positions. Content search, if the Library spec wants
it, scans `.md` files live. The Library is shared by all windows. The pane's right edge is a divider — a
narrow grab zone that paints nothing (`quill::sidebar`) — and dragging it sets one width for every
window, pulled into range by `quill_engine::settings::library_width` so that the pane stays a Library and
the page keeps room to write in, remembered in state as `library_width` and put back to the default by a
double-click.

## Windows

`GtkApplication` with the id `io.github.danielbaldwin47.Quill`, single instance, `HANDLES_OPEN`: a
file opened from a file manager or the shell reaches the running instance. Single instance is a
writer's launch; a launch carrying a flag is not (see Command-line flags). One Document per window,
any number of windows; the Editor, Preview and Stats belong to a window, the Library and settings to
the application. Plain GTK4 without libadwaita ([ADR 0009](adr/0009-plain-gtk4-without-libadwaita.md));
theme `auto` follows the settings portal's colour scheme.

Two windows besides: `Ctrl+?` is a `GtkShortcutsWindow` listing every Command with the chord the
effective map leaves it on, grouped as the menus are and built afresh on every open; `Ctrl+,` is a
Settings window, one grid of the rows that have no menu home — the Typewriter anchor, Follow System,
the Spell-check language, the Library's own rows, the Preview pane's Mode, the `[export]` group a printed or exported page is
laid out on, a button that opens `settings.toml` in the system editor, and whatever the
last read of that file could not apply — a file that is not TOML says so there, above the entries it
refused. Both are transient for the window they were opened from, and no row
of either sets a value on the session: a row writes the file and the watch below applies it.

## Settings

One TOML file at `$XDG_CONFIG_HOME/quill/settings.toml` for what the writer chose; `$XDG_STATE_HOME/quill/`
for what the app observed ([ADR 0010](adr/0010-settings-in-toml-under-xdg.md)). Missing keys take
defaults; unknown keys and unknown tables are kept, so an older Quill never destroys a newer file.
Every write to either file goes through a temporary file beside it and a rename, so a write that
fails leaves the file it was replacing whole.

Config: `theme` (auto, light, dark), `face` (duo, quattro, mono), `step` (the text-size ladder, 0–13,
default 5 = 21.33 logical px; an old `size` in px becomes the nearest step at or above it once,
`docs/design.md` § Text sizes),
`focus` (on/off) and `focus_scope` (sentence, paragraph), `typewriter` (on/off) and
`typewriter_anchor` (0–1, default 0.5), `live` (on/off, default off: the Editor rendering the markup
it is not being typed in), `chrome` (shown/hidden), `spell_check` (on/off, default on)
and `spell_language` (an enchant tag, `en_US` or `de`, or empty for System default), `[syntax_highlight]` (a table: `enabled` is the master, and the five category
toggles sit beside it), `[style_check]` (the same shape, one toggle per list beside `enabled`),
a `[stats]` table (`show`, the Statistics the stats bar shows as a list of their names in any order,
default `["words", "characters", "readingTime"]`, a name Quill does not know dropped with a note and
an empty list an empty bar; and `bar`, shown or hidden, default shown — hiding the bar is a separate
choice from checking none of them),
a `[template]` table (`name`, one of the five Templates, default `modern`; and `center_headings`,
default true and the only input to heading alignment, `number_headings` and `indent_paragraphs`,
the three toggles that bend one), a
`[preview]` table (`layout`, split or full; `mode`, web or pdf, default web, which of the pane's two
modes it draws; and `zoom`, a whole percentage from 50 to 200, default 100; a scalar `template` or
`preview_layout`, which is how each was written before it was a table, is read as its table's value
and rewritten as the table on the next write, as a scalar `library` is),
an `[export]` table (`paper`, one of `auto`, `a4`, `letter` and `legal`, default `auto`, which is
resolved to the desktop locale's paper at the moment a page is laid out and never written back as a
size; `margin` in whole millimetres, 0 to 50, default 20; `text_size` in whole points, 9 to 18,
default 12; and `title_page`, `header` and `footer`, the three pieces of furniture outside the text,
all default false), `palette` (the file the grounds take their colours from, `design.md` § The palette is a
file; empty is the built-ins), a `[library]` table (`locations` and `pinned`, two lists of paths,
and `show_hidden`, `show_extensions`, `confirm_move` and `ask_where_to_save`, four booleans that
default to false; a scalar `library` naming one folder, which is how the Library was written before
it was a set of Locations, is read as its first Location and rewritten as the table on the next
write), and a `[shortcuts]` table of Command id → chords that
replaces the defaults in [`shortcuts.md`](shortcuts.md) ([ADR 0011](adr/0011-shortcut-precedence-on-linux.md)).
A path — `palette`, or an entry in `locations` or `pinned` — written with a leading `~/` is read as
under the home directory, because that is how a hand writes one, and is written back expanded.

**Spell check's language** resolves on the main thread when a Document opens and on a language
change, over the installed tags enchant lists through a throwaway broker with no dictionary load;
the worker is handed the exact tag, or none, and loads it at first use. System default wants the
locale's tag (`LC_ALL`, then `LC_MESSAGES`, then `LANG`; `en_US` from `en_US.UTF-8`, and `C`,
`POSIX` or empty want `en_US`); an explicit `spell_language` wants itself. The ladder, one pure
engine function over a given tag list: the wanted tag exactly, else its bare language, else the
first installed tag of that language in enchant's listing order, else the **"no dictionary"
state**, which remembers the tag it wanted. In that state Spell check stays on, no request is sent
and nothing is underlined; the View › Writing tools row and the Palette row stay sensitive; the
status line carries one line once per process, when the first Document opens, in Export's transient
confirmation shape behind a process-wide flag. Installing a dictionary and toggling Spell check, or
changing the language, leaves it. libenchant's provider warnings go through a GLib log handler for
the `libenchant` domain at debug level, so a launch writes nothing to the journal.

The Settings window's Writing tools group carries Spell check under Style check's rows: a switch
bound to `spell_check`, the language dropdown bound to `spell_language` listing System default and
every installed tag (filled when the window opens, so a dictionary installed mid-session appears on
the next open), and, in the "no dictionary" state, the dropdown reading "No dictionary installed"
with one line under it naming the wanted tag and the package to install (`hunspell-en_us` on Arch,
`README.md` § Spell check in other languages for the rest).

The settings file is watched with `notify` and a debouncer whose window is
`quill_engine::watch::DEBOUNCE`, the
directory rather than the file, because a save is a write beside it and a rename over the top. The
app drains the watch from its main context and re-reads the file whole: every setting applies
without a restart, `[shortcuts]` included, and the flags of a launch that carries any stay over the
top of what the file says. Only a save is re-read: `notify` reports a file being opened as readily
as one being written, so the watch sends a file on only when its length or write time has moved
since it last did, read with a `stat` — an editor re-reading the file, and Quill's own re-read, are
not saves. Every value a key can move is written to the file as the key is pressed, so that the
file is never behind what is on screen and a re-read puts nothing back. A file that is not TOML
at all leaves the settings Quill is running on where they are; a line that cannot be applied is one
`g_warning` under the domain `quill-settings`, said once per distinct line per version of the file,
and never fatal, and the Settings window shows the same lines. The palette file `palette` names is
the watch's second subject, on the same receiver: a save of it is a re-read of the palette, and
because a desktop theme tool does not save into the directory but removes it and moves a fresh one
into its place, or re-points a link, the directory above it is watched too and the watch is armed
again when the directory is replaced, with the file's new version sent on. The watch follows the
setting — re-pointed when `palette` changes, dropped when it is unset — and a directory that is
not there yet is not polled: the add says so, and the app adds the file again on the settings
watch's next event. A palette that moved — the file saved, its directory replaced, the `palette`
line edited — is applied through the pass a theme toggle uses (`quill::window::repaint`: the table,
the stylesheet and the tag table together, on the main context), so a palette change and a scheme
change are the same one repaint; a re-read that yields the same palette is not a repaint, told by
the palette's equality rather than by the watch. The palette never chooses the ground: the scheme is
still the flag, then the setting, then the portal, then `last_scheme`, and the palette colours
whichever ground that lands on. The Documents and the Library join the same watch when they are
built.

State, in `state.toml`: the size of each window and whether it was maximized or full screen, the last
Document per window, caret position per recent Document, the recents list, `last_scheme` (the ground
the last session ended on, which an `auto` launch paints where the portal has no answer), and the
Gate's blind keys under `blind-keys/` beside it. Not window position: GTK4 gives a client no way to ask where its
window is or to put it back, on Wayland or on X11, so placement is the compositor's and Quill
remembers nothing it could not act on.

## Preview and Export

The engine's `render` module lays a whole Document out with Pango from the current Template ([ADR
0005](adr/0005-native-templates.md); what a Template takes from iA and what stays its own is
[ADR 0019](adr/0019-a-template-starts-from-ia-and-is-then-quills-own.md)): one pass produces the layouts the Preview widget snapshots and
the pages the PDF surface draws. Preview has two modes over that one pass, `[preview].mode`: Web
draws the rendered sheet, and PDF draws the same pages Export writes, stacked as a column with the
page under the column's top edge on the stats bar beside the counts. The mode is one setting for the
app: View › Panes' Web and PDF rows and the Settings window's Mode row write it, and every open pane
reads it on the refresh that follows. Preview re-renders on idle after edits, debounced, and restores its
scroll to the block the caret is in. `paginate` cuts that one tall rendered page into pages of paper
under the page geometry (size, margins, header, footer, title page) owned by Export, not the
Template: a heading never ends a page and moves with the block after it, a paragraph splits between
lines with at least two on each side or moves whole, a code block splits at a line boundary with its
Well ground carried on to the next page, a quotation splits at a line boundary and has no ground to
carry, and neither a thematic break nor the line after a hard break ever opens one. There is no page-break syntax. `draw` paints one such page onto
any cairo context, and the three page sinks are fed by it: PDF export is the engine's own
`cairo::PdfSurface` at the paper's size (`pdf`), with the document metadata and the heading
bookmarks from `outline` on it; `GtkPrintOperation` is Print's sink alone, the drawer called
from its `draw-page`; and Preview's page column in PDF mode is the third, the drawer called on the
widget's own cairo context. HTML export is the parser's HTML plus the CSS the Template generates, inlined.
Annotator marks never reach Preview or Export.

Preview ships without tables, figures and footnote blocks first; they are the last renderer work and
sit behind the Gate like everything else.

## Fonts and data files

Before GTK initialises, startup calls `FcConfigAppFontAddDir` on the fonts directory, which carries
the six Faces and the two Template families, Inter and Source Serif 4
([ADR 0007](adr/0007-quill-faces-renamed-and-private.md)).
Data files (fonts, the Style check lists, the `OFL` licences) are resolved from
one data directory: `$QUILL_DATA_DIR` if set, else the directory compiled in at build time
(`/usr/share/quill` for the package), else the repo root for a development build. Nothing is
downloaded at build time. Templates and the tagger model are compiled into the binary with
`include_str!` rather than resolved from the data directory: Templates still render without that
directory (`quill_engine::template`), and `harper-brill` owns the embedded model (ADR 0018).

## Command-line flags

The harness drives the app through flags applied before the first frame; the Gate names the states
and the determinism settings, this document names the flags:

- Judged state: `--text <file>`, `--theme light|dark` (the ground, and — given without `--palette` —
  the built-in table for it whatever the `palette` setting names, so a judged shot is the same on
  every machine), `--font duo|quattro|mono`, `--step <n>`,
  `--focus off|sentence|paragraph`, `--typewriter`, `--live` (turn Live on; absent under
  `--deterministic` it pins Live off, so every state judged before Live existed is shot with the
  markup written out), `--syntax off|on|nouns,verbs,adjectives,adverbs,conjunctions` (pin the whole
  `[syntax_highlight]` table: off, every Category on, or only the comma-separated Categories on;
  absent under `--deterministic` the table takes its defaults with the master off),
  `--style off|on|fillers,redundancies,cliches` (pin the whole `[style_check]` table the same way,
  by List), `--stats <names>|hidden` (pin the whole `[stats]` table: the comma-separated Statistics
  checked with the bar shown, or the bar hidden; there is no `off`, which means the master switch on
  the two flags above and would have to mean either of two different states here, so absent under
  `--deterministic` the table takes its defaults — the three cells every judged state carrying the
  chrome was frozen at), `--chrome on|off`, `--caret <offset>|end`,
  `--select <from>,<to>`, `--scroll <fraction>`, `--nocaret`, `--typing` (the chrome as it is
  inside the 500 ms after a keystroke: the title bar gone, the stats bar dimmed), `--menu
  view|document|stats|palette|outline` (that menu, the Palette, or the Palette on the Outline, open
  with its first row selected — the caret's section, on the Outline),
  `--library <dir>` (take the Library from the fixture tree at `<dir>`: the launch copies it to a
  folder of its own, stamps each file with the mtime the fixture's `manifest.json` names, and walks
  that copy as its one Location, so a judged shot of the Library is the same rows in the same order
  on every machine — a `--text` named inside the fixture is opened from the copy with it; the rest
  of the `[library]` table is pinned to its defaults for the launch, nothing Pinned and all four
  booleans off, so a writer who turned on hidden files or extensions does not change the shot),
  `--sidebar` (open with the Library beside the page), `--search <query>` (put `<query>` in the
  Library's search field and narrow the list to what it finds; refused without `--sidebar`, which is
  the pane the field stands in), `--preview split|full|pdf-split|pdf-full` (open with the Preview
  pane beside the Editor, or in place of it. The word names both of the pane's settings: `split` and
  `full` are `[preview] layout` with the pane showing the rendered sheet, and `pdf-split` and
  `pdf-full` are the same two layouts with the pane showing the pages Export writes. The rest of
  `[preview]` is pinned to its defaults for the launch, so a writer's remembered mode and zoom reach
  no judged shot), `--zoom <percent>` (draw the pane at that per cent of fit width, which is
  `[preview] zoom`, over the pinning above: the one key of the table a state names for itself, so
  that a state can stand two pages of the page column in one window. Refused outside the range the
  setting takes, and it wants `--preview`, which is what opens the pane),
  `--export-dialog pdf|html|markdown` (open that format's Export
  dialog over the page with its Options expander open, once the window has painted its first frame —
  a still cannot pull an expander, and a second surface over a toplevel the compositor has no frame
  of yet keeps the toplevel from ever mapping; `[export]` itself has no flag, and is pinned to its
  defaults under `--deterministic` with `paper` past its own default, because `auto` is the host's
  locale and a judged shot is the same on every machine), `--w <px> --h <px>`. Both `--typing` and
  `--menu` name a state the app is
  put in before the first frame, never one it is driven into after it.
- Harness: `--deterministic` (animations off, blink off, manual font rendering with pinned antialias,
  slight hinting, no subpixel, 96 dpi, hinted metrics, no client-side decorations; and Typewriter
  off unless `--typewriter` is given, so the writer's `settings.toml` reaches no judged shot),
  `--measure <out.jsonl>` (key capture in the capture phase, `GdkFrameTimings` presentation times,
  cold start against `QUILL_T0_NS`), `--settings <path>` (read and write settings in `<path>`, so a
  run drives a fixture — a `[shortcuts]` table, a theme — without touching the writer's file),
  `--palette <path>` (paint the grounds from the palette file at `<path>` for this launch, over the
  `palette` setting; with `--theme`, a palette previewed on a pinned ground).

Every flag has a matching setting or a harness-only effect; none creates state a writer cannot reach.

A launch carrying any of them is the harness's rather than a writer's, and that decides three things
about it. It runs non-unique, so a judged shot or a bench is served by the process that was launched
even when a writer's Quill is already open. It overrides the settings for that launch alone and
writes nothing back to `settings.toml` — the writer's own, which is the one thing `--settings` moves:
a launch that names a settings file of its own reads and writes that file, and the writer's is left
untouched either way. And it neither reads nor writes `state.toml`, so it opens at
the shape its flags name rather than at the window a writer left, the same command line is the same
window twice, and a bench at 1440×900 is not a writer resizing anything.

## Packaging

`PKGBUILD` builds the workspace with `cargo build --release --locked` from the working tree
(`cargo fetch` in `prepare`, so `makepkg` needs the network only there), `arch=('x86_64')`,
`license=('GPL-3.0-or-later' 'OFL-1.1' 'Apache-2.0' 'BSD-3-Clause' 'MIT' 'CC0-1.0')`,
`depends=('gtk4' 'enchant' 'hunspell-en_us' 'hicolor-icon-theme')`,
`makedepends=('cargo')`. It installs the
binary as `/usr/bin/quill`, data under `/usr/share/quill/` (the fonts, and the Style check lists
under `data/style/` with their `SOURCES.md`), the `.desktop` file and icon under the
application id, `fonts/OFL.txt` beside the fonts and under `/usr/share/licenses/quill/`,
`packaging/harper-brill-LICENSE` under that licence directory for the embedded model, and the
lists' four licence texts there too.

`hunspell-en_us` is a hard dependency, so a fresh install checks spelling out of the box, and
`en_US` is the tag a `C`, `POSIX` or empty locale resolves to. Quill links `libenchant-2` itself
(§ Workspace), so `enchant` is a build dependency of the workspace as well as a runtime one: `cargo
test -p quill-engine` and the Commit tier need `libenchant-2.so` on the machine. Other languages are
the writer's to install — `README.md` § Spell check in other languages. On Arch the "no dictionary"
state (§ Settings) is reached only by a non-English locale with no dictionary of its language or a
removed package; it is built in full anyway, because a Flatpak's non-English dictionaries live
behind the locale extension and dangle when the subset excludes them, and enchant reports them
absent exactly as it would here.

The Gate never reads the machine's dictionaries. `ref/spell/` holds a small `en_US` `.aff`/`.dic`
pair with every correctly spelled word of `ref/spell.md` and of the bench passages, and none of the
passage's misspellings; the harness (every `--deterministic` launch, whatever `--spell` says), the
bench and the engine tests point `ENCHANT_CONFIG_DIR` at a fresh temporary copy of that directory
per run. enchant searches the config directory's `hunspell/` before the system's, so the fixture
wins over an installed `hunspell-en_us`, and an Add during a keys run writes the copy, never the
checkout. A launch that forgets the variable reads whatever the machine has, and the `spell` states
go red on a word the real dictionary knows.

Flatpak comes later (the map's fog) and this design keeps it cheap: fonts are private, enchant and
its English dictionary are in `org.gnome.Platform`, data resolves through one directory, and nothing
runs at build time that needs the network.

**Licence rule.** Every dependency and data file is GPL-3.0-or-later compatible; no CC BY-SA data
ships in Quill, because it is compatible with GPL-3.0 only and breaks "or-later". The rule is about
what ships: `harper-brill`'s tagger model was *derived from* treebanks under those licences and
holds no line of any of them, which is a distinction nobody has adjudicated and the owner has
accepted knowingly — [ADR 0018](adr/0018-harper-brill-ships-as-is.md).

## Repo migration

One commit, once the Cargo workspace is standing beside the JavaScript app. The owner agreed at
[#52](https://github.com/danielbaldwin47/Quill/issues/52)'s creation that the workspace lands first,
so the move finds Rust already at the root rather than clearing the ground for it:

- `app/`, `tools/{serve,shoot,crop,latency,smoke}.mjs`, `bin/quill`, `package.json` and the Node
  lockfile move under `legacy/` unchanged; `legacy/bin/quill` still launches the JavaScript app from
  the checkout. `tools/blind.mjs`, `tools/thumb.mjs`, `tools/progress.mjs`, `tools/uinput-keys.py`
  and `tools/idle-check.py` stay at the root: the Gate uses them for the native app.
- `Cargo.toml` (workspace), `quill-engine/`, `quill/`, `tools/fontbuild.py` and `fonts/` are at the
  root by then. `tools/gate` is not: it arrives with the Gate tooling
  ([#19](https://github.com/danielbaldwin47/Quill/issues/19)).
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
ticket deletes `legacy/` and gives every judged state a `mac-native` crop as its opponent
([ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md)); a state that follows a
`docs/design.md` row takes one as soon as the Gate has the per-state key (#161).

## Port order

Ticket zero of the build effort is the Gate tooling ([#19](https://github.com/danielbaldwin47/Quill/issues/19));
no Piece can close without it. Then, each Piece a feature ticket closed by its Hand test in
`docs/agents/hand-tests.md`:

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
