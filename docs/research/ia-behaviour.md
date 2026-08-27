# iA Writer: what the in-scope features actually do

Research for [#9](https://github.com/danielbaldwin47/Quill/issues/9). Every claim below comes from iA's own support pages (Mac and Windows tabs of the same article differ, so both are cited) or, where noted, from the cross-platform feature matrix. Written as behaviour: what the user sees, how it is switched on, what the options are, and where the documentation stops.

Feature names follow [CONTEXT.md](../../CONTEXT.md). iA's own UI names differ in places and those differences are called out, because they matter when reading their docs.

## Cross-platform reality check

iA ships genuinely different apps per platform, so "iA Writer does X" is usually a per-platform claim. From iA's feature matrix (<https://ia.net/writer/support/basics/features?platform=windows>):

| | Mac | Windows |
|---|---|---|
| Folding | — | yes |
| Dynamic Outline | — | yes |
| Writing Goals | — | yes |
| Wikilinks | yes | — |
| Global Document Search | yes | — |
| Blogging (Ghost/Medium/WordPress/Micropub/Micro.blog) | yes | — |
| Spelling engine | system (macOS) | bundled Hunspell |
| Syntax highlight languages | En De Fr It Es Ru | En |
| Style check languages | En De Es Fr | En De Fr |

Where the two platforms disagree, this document says so rather than picking a winner; the Quill spec has to choose.

## Library

**What the user sees (Mac).** Two panes to the left of the Editor: a grey **Organizer** and a **File List**. The Organizer holds four kinds of thing — **Locations**, **Favorites**, **Smart Folders**, **Hashtags** — and right-clicking any row opens a context menu. The File List shows the folders and files of whichever Location or folder is selected.

- Single click opens a document in the same window; double click opens it in a new window.
- Drag moves files to, from and within the Library, including between two iA Writer windows and to/from Finder; holding `option` while dragging duplicates instead of moving. `⌘`/`⇧` multi-select works.
- Right-click on a document gives delete, rename, duplicate and so on.
- An optional **Sort Bar** above the list (Name, Date, or Extension) and **Filter Bar** below it, both toggled from `View` → `Show Sort Bar` / `Show Filter Bar`. The Filter Bar matches documents in the *current folder* whose terms *start with* the typed string, and accepts the same advanced query syntax as Smart Folders.
- Title-bar back/forward buttons walk the File List history (`⌃⌘←` / `⌃⌘→`); click-and-hold on a button lists recently active documents.

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Show/hide Library | `⌃⌘S` | `Ctrl+E` |
| Show/hide Organizer | `View` → `Hide/Show Organizer` | n/a |
| Quick Search | `⇧⌘O` (`Go` → `Quick Search`) | **not available** |
| Show iCloud in Library | `⇧⌘I` | — |
| Open Favorite 1–9 | `⌃1`–`⌃9` | — |
| Show Smart Folder 1–9 | `⌃⌥1`–`⌃⌥9` | — |
| Cycle panes | `⌃tab` (Organizer → File List → Editor) | — |
| Add a Location | `Go` → `Add Location…`, or hover "Locations" and click the `+` | — |
| New folder | right-click in File List → `New Folder` | — |

Favorite and Smart Folder shortcuts are shown in the `Go` menu.

**Locations (external folders).** iCloud is the default Location on Mac; turn iCloud off and it becomes a folder labelled "On My Mac". **The default Location cannot be removed** — additional folders are added alongside it. Any local folder or cloud-provider folder can be added: iCloud, Dropbox, Google Drive, OneDrive and "Other" are all supported on Mac and Windows alike.

**Favorites.** Bookmarks to individual files *or* folders. Added by dragging from the File List into the Favorites section, or right-click → `Add to Favorites`. They are **per-device and not synced** — a Favorite added on the Mac must be re-added on the iPhone.

**Smart Folders.** Rule-driven dynamic folders; files enter them with no user action. Ships with exactly one, **Recents** — "the 25 most recently used files, across the whole Library". Unlike Favorites, Smart Folders **do** sync across iOS/iPadOS/macOS when iCloud is in use. When creating one you choose rules from:

- **Search** — a full query (syntax below)
- **Parent path** — the contents of one folder
- **Ancestor path** — that folder *and all subfolders*
- **Kind** — text, folder, or other
- **Date created**, **Date modified**

In paths, `/` means the default Location; `Location: /` filters to a named custom Location. The context menu in Library and Organizer copies paths.

**Search syntax (used by both Quick Search and the Filter Bar).** Scope is names of *all* files, but contents only of plain-text (`txt`, `text`) and Markdown (`md`, `markdown` et al.) files — so contents of anything else are invisible to search. Supported operators, from iA's table: prefix matching by default (`text machine` matches words *beginning* "text" and "machine"), `"quoted"` for exact/phrase, `-term` to exclude, `name:alice` to restrict to filenames, `^first` to anchor at the first word, `AND` / `OR` / `NOT`, `NEAR(time space)` (default within 10 words) and `NEAR(a "b" "c" 4)`, parentheses for grouping, `#tag` and `-#` for has-no-hashtag, and the task-specific `[ ]` (incomplete task) and `[x]` (completed task).

**Hashtags.** Written inline as `#word` (Twitter's spec — a dash makes the tag invalid). Every tag in the Library appears under the Hashtags section of the Organizer; clicking one filters the File List to documents containing it. Hashtags can be hidden from Preview/Export via a setting.

**Windows differences.** The Windows Library is much thinner: no Organizer with Favorites/Smart Folders/Hashtags, and **no Global Document Search or Quick Search at all** per iA's own feature matrix. Its Library options are just Settings → Library: *Sort By* (Name or Date), *Sort Order*, *Folders on Top*, *Show Extensions*, and *Show Excerpts* ("display an outline of your file in the Library list").

**Edge cases.** Content Blocks (embedded images, CSV tables, text files, code) are references to other files, so a Library move can break them — which is why iA offers a **Project Archive** export that bundles the document with its blocks. Wikilinks exist on Mac/iOS only, not Windows. Paths for Wikilinks and Content Blocks render as Shortest / Relative / Absolute per a Markdown setting.

**Sources.** <https://ia.net/writer/support/library/organize?platform=mac>, <https://ia.net/writer/support/library/navigation?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/features?platform=windows>

## Heading navigation

**This exists on Windows only.** iA's outline article is blunt: "Exclusively on iA Writer for Windows: Dynamic Outline and Folding", and the feature matrix marks Dynamic Outline and Folding as `—` for Mac and iOS.

**What the user sees.** The outline is not a separate panel. It lives *inside the Library file list*: "the active library element turns into an outline" — the row for the currently open document expands in place into its heading tree. Headings "indent[…] titles hierarchically", and clicking a title jumps the Editor to that point in the document. Alongside it, **Folding** collapses and expands a chapter (a heading and its body) with a single click in the Editor.

Windows also has Settings → Library → **Show Excerpts**, described as showing "an outline of your file in the Library list" — a separate, always-on per-file preview line, distinct from the Dynamic Outline.

**Toggles and shortcuts.** None documented. There is no menu item, no keyboard shortcut, and no on/off switch in either shortcuts page; the outline appears because the document is the active one in the Library, and the Library itself toggles with `Ctrl+E`.

**Options and edge cases.** iA documents no options at all — no heading-level depth limit, no filter, no "show only H1–H3". Behaviour for a document with no headings is unspecified. On Mac the substitute for jumping around a long document is Quick Search (`⇧⌘O`) and the File List history buttons, not an outline. A Preview-side table of contents is a *template* feature, not this feature.

**Sources.** <https://ia.net/writer/support/library/navigate-big-documents?platform=windows>, <https://ia.net/writer/support/basics/features?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=windows>

## Preview

**What the user sees.** A rendered pane showing the document as it will export or print. Two axes of choice: **layout** (Split, i.e. side-by-side with the Editor, or Full, where the Editor is hidden entirely) and **mode** (**Web**, rendering HTML, or **PDF**, rendering paginated output). The active **template** controls fonts, margins, line height and background.

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Show/hide Preview | `⌘R` | `Ctrl+R` |
| Menu | `View` → `Show Preview` | `View` → `Preview` |
| Other entry point | the `▶` arrow at the top right of the Toolbar | the Preview play button |
| Split vs Full | `View` → `Preview` → `Split \| Full`, or the right side of the Toolbar | dropdown / toolbar |
| Choose template | Toolbar (middle) or `View` → `Template` | Toolbar (middle of Preview) or `View` → `Templates` |
| Web inspector | `defaults write pro.writer.mac WebKitDeveloperExtras -bool true` in Terminal | `Ctrl+J` |

**Scroll sync.** Documented only on Windows: "By default, Preview will scroll in sync with the Editor. If you wish to decouple and scroll panes independently, you can disable this in `File` → `Settings` → `Editor` → `Synchronize Scroll`." Direction (editor-drives-preview only, or bidirectional) and granularity (block vs proportional) are **not** documented. The Mac pages never mention scroll sync — do not assume it behaves the same.

**Templates.** Five ship built in: **Modern (Sans)**, **Classic (Serif)**, **Manuscript (Mono)**, **Manuscript (Duo)**, **Manuscript (Quattro)**, plus **GitHub**. GitHub is a direct copy of GitHub's CSS aimed at technical writing and is the documented exception: it "does not support the Default Template Settings (such as Number heading, footer…)". Seven more are free downloads: Helvetica, Palatino, Academic MLA, Letter, Fountain (screenwriting), Chess.

Template-level formatting options (Settings → Templates) are: **Center Headings**, **Number Headings** (numbering starts at H2 on Windows), **Indent Paragraphs** (indentation instead of vertical space), **Invert Colors** (Preview in Night mode when the Editor is in Day mode and vice versa, Web mode only), plus Title Page / Header / Footer / Page Size for print and PDF.

**Custom templates.** A template is a bundle — `Example.iatemplate/Contents/{Info.plist, Resources/{document.html, title.html, header.html, footer.html, style.css}}` — plain HTML, CSS and JavaScript. `Info.plist` keys: `CFBundleName`, `CFBundleIdentifier`, `IATemplateDocumentFile` (required), and optional `IATemplateTitleFile`, `IATemplateHeaderFile`, `IATemplateFooterFile`, `IATemplateHeaderHeight` / `IATemplateFooterHeight` (CSS pixels, ≤400), `IATemplateDescription`. Installed by double-click, drag to the Dock icon, or Settings → Templates → `+`; on **Windows 1.2+** it is `File` → `Install Template` with a `.iatemplate.zip`. Installed bundles are **copied**, so edits to the original are not picked up.

Without writing a template, a document can override template CSS by embedding an HTML `<style>` block — iA's own example uses `@media print { body { font-size: 25pt; } }` to change PDF rendering only.

**Rendering behaviour and edge cases.**

- Equation rendering, Metadata, HTML Preview and PDF Preview are available on all platforms.
- **YAML metadata** at the top of a file is processed and hidden from Preview/Export when *Process Metadata* is on; `Author` is the one key iA uses itself, printed on Title Pages in Preview and PDF export.
- **Smart Punctuation** (Markdown → Processing) converts straight quotes and `--` in the *rendered* output only, leaving the file untouched — distinct from the Editor's Smart Quotes/Dashes which rewrite the file.
- **Single Return** changes paragraph rules so one newline ends a paragraph.
- Wikilinks, hashtags and autolinks each have independent output settings for HTML, "Other" (copy/export/publish) and Preview — a hashtag can render as source text, a `<span>`, a link, or be removed entirely.
- Syntax highlight and Style check marks never appear in Preview.
- Windows PDF mode does not always live-update: "you can click the refresh button at the bottom of the PDF preview to see these changes reflected."
- Windows Editor text size explicitly does not affect Preview.
- Windows has an **XSS Filtering** setting for embedded HTML.

**Sources.** <https://ia.net/writer/support/preview/modify-preview?platform=mac>, <https://ia.net/writer/support/preview/modify-preview?platform=windows>, <https://ia.net/writer/support/preview/templates?platform=mac>, <https://ia.net/writer/support/preview/custom-templates?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/features?platform=windows>

## Export

**What the user sees.** `File` → `Export…` (`⇧⌘E` on Mac; no shortcut documented on Windows) opens a format picker. iA's stated rule for all of it: **"Preview determines what Export and Print will look like"** — the template and typography currently selected in Preview are what the exported file gets.

**Formats.**

| Format | Mac | Windows | Notes from iA |
|---|---|---|---|
| PDF `.pdf` | yes | yes | "Template styling is applied and locked in" |
| MS Word `.docx` | yes | yes | for content that will be edited further in Word |
| HTML `.html` | yes | yes | for blogging platforms and CMSes |
| Markdown `.md` | yes | yes | "Preserves your original MD syntax" |
| **Project Archive `.zip`** | yes | **no** | document *plus* all embedded Content Blocks — "great for sharing an entire project or even for creating a duplicate/backup" |

**Print.** Mac has two: `File` → `Print` (`⌘P`) prints the formatted Preview, and `File` → `Print Plain Text…` (`⌥⌘P`) prints the raw Markdown. Page setup is `⇧⌘P` (`File` → `Page Setup…`) offering A4, Letter, Legal. Windows exposes A4 or Letter directly in the PDF export dialog and prints with `Ctrl+P`.

**Options.** PDF export shares its options with Print, set at Settings → Templates → *Printing & PDF Export*: include a **Title page**, **headers**, **footers** (page numbers). The default-template options — Center Headings, Number Headings, Indent Paragraphs — also apply, except under the GitHub template, which ignores them.

**Copy commands (Mac), which behave like miniature exports.** `⌘C` plain text, `⌥⌘C` formatted text, `⇧⌘C` as HTML, `⌃⌘C` as Markdown **including Content Blocks** (i.e. transclusions resolved). Windows has `Ctrl+Shift+C` for Copy HTML.

**Word round-trip.** `.docx` import (`File` → `Open…` or drag to the Dock on Mac; `File` → `Import` on Windows; `.doc` must be converted to `.docx` first) maps Word styles to Markdown and back: emphasis↔italics, strong↔bold, `~`↔strikethrough, `[]()`↔hyperlinks, `#`–`######`↔Heading 1–6, lists↔list styles, blockquote↔quote. Some conversions are one-way: Word underline imports (Markdown has no underline, so it is lost on export), inline code exports to a code character style, reference links flatten to hyperlinks, and image alt text becomes a bracketed caption.

**Sharing and publishing.** Mac (and iOS) can post drafts to **Ghost, Medium, WordPress, Micropub and Micro.blog**, choosing HTML or Markdown per account; Windows has none of this. Mac also gets the macOS system Share sheet on a text selection (`File` → `Share` or right-click → `Share`).

**Edge cases.** Syntax highlight colours and Style check strikethroughs never export. Hashtags can be stripped from output entirely (*Hide Hashtags* on Windows; the Markdown output settings on Mac). YAML metadata is hidden from export when *Process Metadata* is on, but `Author` reaches the title page. Content Blocks are resolved into HTML/PDF/Word output but only Project Archive preserves them *as* blocks. Smart Table `=(…)` calculations are evaluated for the rendered output, not in the file.

**Sources.** <https://ia.net/writer/support/preview/export-share-print?platform=mac>, <https://ia.net/writer/support/preview/export-share-print?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/features?platform=windows>, <https://ia.net/writer/support/editor/smart-automation?platform=mac>

## Syntax highlight

**What the user sees.** Words are recoloured in the Editor by part of speech: adjectives brown, nouns red, adverbs purple, verbs blue, conjunctions green. Nothing is dimmed — unselected parts of speech simply keep the normal text colour, so the effect is a coloured subset over ordinary prose rather than a focus dim. Only five parts of speech exist; there is no preposition or pronoun category. On Windows the feature is named **Syntax Control** in the UI (the docs use both names).

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Toggle whole feature | `⇧⌘D` | no documented shortcut |
| Menu | `Focus` → `Show Syntax` | `Focus` menu |
| Other entry points | Settings → Editor → Syntax; the Focus dropdown in the Editor's title bar | Focus menu only |

Individual parts of speech are checked/unchecked independently — Mac in Settings → Editor → Syntax, in the Focus menu, and in the title-bar Focus dropdown; Windows in the Focus menu, where "a check mark will appear" beside enabled ones. So the master toggle and the five per-category toggles are separate state: you can have Syntax highlight on with only adjectives and adverbs coloured.

**Options and edge cases.**

- Language-gated. Mac supports English, German, French, Spanish, Italian, Russian. Windows supports **English only**, and shows a warning strip at the bottom of the Editor for other content: "The language of the current file does not seem to be supported by Syntax Control."
- Editor-only. "the highlighted parts of speech will not be visible in Preview, exported or printed documents. This feature is visible in the Editor only." It never touches the file.
- Composes with Focus and with Style check — all three can be on at once; iA explicitly recommends Syntax highlight plus Focus Mode together, and Style check "can be activated concurrently".
- Language is detected per document ("the language of the current file"), not chosen per document by the user.

**Sources.** <https://ia.net/writer/support/editor/syntax-highlight?platform=mac>, <https://ia.net/writer/support/editor/syntax-highlight?platform=windows>, <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>

## Style check

**What the user sees.** Offending words and phrases are **struck through** — not underlined. The strikethrough is author-only: "only you see words being crossed out (the strikethrough mark won't appear in Preview or Export)". Nothing is deleted or rewritten: "no deletions occur unless you choose to do them." The docs describe no hover popover, no suggestion list and no replacement UI — the mark is the whole interaction; you delete the word yourself.

Three built-in categories, with iA's own examples:

- **Fillers** — *basically*, *pretty much*, *sort of*
- **Redundancies** — basic **fundamentals**, combine **together**, fall **down** (note only the redundant word is struck, not the whole phrase)
- **Clichés** — *against all odds*, *brass tacks*, *long and short of it*

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Toggle | `⌥⇧⌘D` | no documented shortcut |
| Menu | `Focus` → `Enable/Disable Style Check` | `Focus` menu |
| Other entry points | Navigation Bar; Settings → Editor → Style Check | Settings → Style Check |

As with Syntax highlight, each category (fillers, redundancies, clichés, custom patterns) toggles independently: "You can fully enable Style Check … or just some of them."

**Options and edge cases.**

- **Custom Patterns** — a user-editable rule list at Settings → Editor → Custom Patterns (Mac) / Settings → Style Check (Windows). Syntax: a bare pattern adds a rule (`custom ~~filler~~`); a leading `-` adds an exception that suppresses both custom and *built-in* matches (`-filler`); a pattern wrapped in slashes is a regex (`/reg(exp?|ular expression)/`).
- The regex engine is deliberately crippled for editing latency: character classes ASCII-only and non-repeating past 10 characters, repetition capped at 10, `.` and negated classes and all four lookaround forms **ignored**, lazy/greedy and `\w`/`\W` explicitly "undefined behavior". Case-insensitive matching is on by default (`i`); diacritic-insensitive (`d`) is off.
- Language support differs by platform: **Mac** English, French, Spanish, German; **Windows** English, French, German (no Spanish). iA's own pages disagree here — the Mac FAQ says "Style Check is available for En De Fr" while the Style Check article and the feature matrix both list Spanish for Mac. Treat En/De/Fr as the safe intersection.
- Entirely local: "your text stays private and on your device (not sent to an internet service for review) … there is no AI".
- The docs say nothing about suppressing Style check inside code blocks, quotes or markup — treat that as unspecified rather than known-good.

**Sources.** <https://ia.net/writer/support/editor/style-check?platform=mac>, <https://ia.net/writer/support/editor/style-check?platform=windows>, <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>

## Stats

Stats behave quite differently on the two platforms; the Mac model is the one worth copying.

**Mac — what the user sees.** Stats live in the **Toolbar at the bottom of the Editor**, which itself has three states set at `View` → `Toolbar`: *Fade In/Out*, *Always Show*, *Hide*. The same `View` menu chooses between two stats layouts:

- **Default** — the Toolbar carries the usual quick actions (bold, headings, tables…) and shows **exactly one** statistic in the right corner. Clicking the shown statistic expands the list of the others; clicking one pins it as the permanently displayed stat.
- **Stats Only** — the whole Toolbar is given over to statistics. Right-clicking the Toolbar and hovering `Stats Only >` gives checkboxes to show/hide individual statistics.

**Windows — what the user sees.** No always-on toolbar. A **stats bubble** appears when the pointer hovers near the bottom-right corner of the Editor. Clicking it gives the full list, lets you choose which single statistic the bubble displays, set a **Word Goal**, and pin the bubble to always show.

**Which numbers.** The docs do not give an exhaustive list. Mac is described as "character, sentence or task counts, and estimated reading time"; Windows as "character, word or sentence counts, estimated reading time, and word goal". So the confirmed set is: characters, words, sentences, tasks (Mac), reading time, plus a word goal on Windows. Paragraph count, line count, page count and speaking time are **not** documented. No reading-time formula or words-per-minute figure is published.

**Selection vs document.** Identical on both platforms and stated explicitly: with no selection the stats describe the whole document; with a selection they describe the selected range, updating live as the selection changes. Nothing is said about restoring the document figure on deselect beyond the "if no text is selected" rule, which implies it reverts.

**Edge cases the docs don't settle.** Whether markup characters, YAML metadata, footnotes or code blocks are counted is not documented. There is no word-goal feature on Mac. There is no keyboard shortcut for stats on either platform — the shortcuts pages list none.

**Sources.** <https://ia.net/writer/support/editor/stats?platform=mac>, <https://ia.net/writer/support/editor/stats?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>

## Spell check

The two platforms take opposite approaches: Mac delegates entirely to the OS, Windows ships its own dictionaries.

**Mac.** Spelling, autocorrect and grammar come from macOS: "iA Writer will automatically use the system settings defined in Language & Region." The app's only control is Settings → Editor → **Spelling and grammar**, which "Decide[s] how iA Writer checks your spelling and grammar, if at all" — i.e. the standard macOS Check Spelling While Typing / Check Grammar With Spelling / Correct Spelling Automatically triad, plus the system contextual-menu corrections, learned words and per-language dictionaries. Language follows the app language, which can be overridden per-app in System Settings → General → Languages & Region → Applications. iA warns that third-party checkers "(i.e. Antidote, Grammarly) may impair the functionality of system options" and to disable them when spell check misbehaves. Grammar checking therefore exists on Mac only because macOS provides it.

**Windows.** iA Writer bundles its own **hunspell** dictionaries, pre-installed for English, German, French, Spanish and Italian. The language is picked in Settings → Editor → **Spellcheck**, and users can install more by downloading OpenOffice/LibreOffice dictionary extensions from the web. No grammar checking is documented for Windows.

**Edge cases.** The docs do not state whether spell check is suppressed in code blocks, URLs, or markup, nor where the Windows custom-word list is stored, nor whether a document can carry its own language independent of the app setting (on Mac macOS supports per-text-view language selection, but iA does not document a per-document setting).

**Sources.** <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>

## Behaviour we already have, where iA does it differently

Only the details that differ from what our JavaScript app does today.

**Focus is one setting with three mutually exclusive scopes, not two independent toggles.** On Mac `Focus` → `Enable Focus Mode` (`⌘D`) turns on a mode whose *scope* is then chosen from **Sentence / Paragraph / Typewriter** (also at Settings → Editor → Focus scope). Typewriter is a scope *of* Focus Mode, and in that scope "the text will not be highlighted or dimmed" — you get typewriter scrolling instead of dimming, never both. Windows splits it: `Focus` → `Sentence`/`Paragraph` (`Ctrl+Shift+D` selects sentence mode), and "Typewriter Scrolling can be enabled independently of other Focus Mode or Syntax Control options." Our app should decide which model it copies; they are not compatible.

**Typewriter means vertically centred, and iA admits it fights editing.** "The cursor remains vertically centered in the Editor when typing or moving up or down in your document." There is no configurable offset. iA's own guidance is to turn Focus Mode off while editing because "A conflict between the area you will select to edit and Focus Mode's attempt to vertically center the cursor might result in the screen jumping vertically."

**Appearance.** Mac: `⌃⌘N` toggles dark/light, set at Settings → General → Appearance (Light/Dark only — no documented "follow system" option; the *dock icon* light/dark is a separate setting). Windows: `Alt+Shift+N` for Night mode. Windows additionally has an **Invert Colors** setting so Preview renders in Day mode while the Editor is in Night mode and vice versa — a per-pane theme split we do not have.

**Typography settings we should match.** Typeface is iA Writer **Mono / Duo / Quattro**. Line length limit is a three-way choice of **64, 72 or 80 characters** (Windows documents the default as 64). Text size is a slider on Mac (`⌘+` / `⌘-` / `⌘0` to reset), a stepper on Windows, and on Windows explicitly does not affect Preview. Mac also exposes indentation, tab key behaviour, tab width and wrapped-line/whitespace treatment under Editor → Advanced, a highlight colour (yellow/orange/pink/purple/blue/green) and completed-task appearance (strikethrough / faded / both).

**Chrome fades rather than hides.** Both the Title bar and the Toolbar on Mac have three states — Always Show, Fade In/Out, Hide — rather than a binary. Windows has *Auto Hide TitleBar*: "TitleBar hides when you start typing or scrolling within a document."

**Smart editing behaviours.** Smart Lists continue a list marker on Return and remove the empty marker and exit the list on a second Return (blockquotes behave the same). Smart Quotes and Smart Dashes rewrite characters *in the Editor*, whereas the separate Markdown → Processing → *Apply Smart Punctuation* changes only the Preview/PDF rendering and leaves the file bytes alone — two distinct settings that are easy to conflate. Bracket completion has two modes: "Insert and type-over matching brackets" and "Wrap selection in typed brackets".

**Sources.** <https://ia.net/writer/support/editor/focus-mode?platform=mac>, <https://ia.net/writer/support/editor/focus-mode?platform=windows>, <https://ia.net/writer/support/editor/smart-automation?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>
