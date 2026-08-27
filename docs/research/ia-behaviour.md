# iA Writer: what the in-scope features actually do

Research for [#9](https://github.com/danielbaldwin47/Quill/issues/9). Behaviour, not marketing: what the user sees, how it is switched on, what the options are, and where the documentation stops.

Sources are iA's own support pages, release notes and ia.net/topics posts. Two things to know about them before reading:

- **There are no per-platform URL paths.** `ia.net/writer/support/mac/...` 404s. The same article serves different text per platform via `?platform=mac` / `?platform=windows`, and the Mac and Windows texts frequently disagree. Both are cited throughout.
- **The docs lag the product.** iA Writer 8 for Apple platforms (17 June 2026) merged the document outline into Quick Search, but the support pages still say outlines are Windows-exclusive. Where the blog and the support docs conflict, both are given and the conflict is flagged.

Feature names follow [CONTEXT.md](../../CONTEXT.md). iA's own UI names differ in places; those differences are called out because they matter when reading their docs.

## Cross-platform reality check

iA ships genuinely different apps per platform, so "iA Writer does X" is almost always a per-platform claim. From iA's feature matrix (<https://ia.net/writer/support/basics/features>):

| | Mac | Windows |
|---|---|---|
| Folding | — | yes |
| Dynamic Outline | — | yes |
| Writing Goals | — | yes |
| Wikilinks | yes | — |
| Global Document Search | yes | — |
| Blogging (Ghost/Medium/WordPress/Micropub/Micro.blog) | yes | — |
| Project Archive export | yes | — |
| Print Plain Text | yes | — |
| Smart Tables | calculation | formatting only |
| Spelling engine | system (macOS) | bundled Hunspell |
| Syntax highlight languages | En De Fr It Es Ru | En |
| Style check languages | En De Es Fr | En De Fr |

Equal on both: HTML Preview, PDF Preview, equation rendering, metadata, `.docx` import/export, PDF export, iA fonts, custom templates, Content Blocks, all cloud providers.

Where the two platforms disagree this document says so rather than picking a winner; the Quill spec has to choose.

## Library

**What the user sees (Mac).** Two panels left of the Editor: a grey **Organizer** and a **File List**. With Preview open it is a three-panel app. The Organizer holds exactly four sections in this order — **Locations, Favorites, Smart Folders, Hashtags** — and right-clicking any row opens a context menu. Which sections appear is a preference (Settings → Library → Organizer), and hiding a section hides it from the `Go` menu too.

Around the File List sit two optional bars: a **Sort Bar** above and a **Filter Bar** below, both from `View` → `Show Sort Bar` / `Show Filter Bar`. Each file row can show a text excerpt of its contents.

- Single click opens a document in the same window; double click opens it in a new window.
- Drag moves files to, from and within the Library, across multiple iA Writer windows and to/from Finder; `option`-drag duplicates instead of moving. `⌘`/`⇧` multi-select works.
- Right-click gives delete, rename, duplicate, and (since 5.4) share, export, publish, print and Copy Markdown.
- Deletion is **Move to Trash**; holding `⌥` deletes immediately and, from the Library context menu, without a confirmation.
- Sort Bar sorts by **Name, Date, or Extension** (later builds added date-created). Sort order is Newest/Oldest or A–Z/Z–A.
- Since 7.2 the File List is a **tree view**: tapping a folder expands its contents inline without navigating into it, and you can create files and folders inside an expanded subfolder from the context menu. Before 7.2 it was a single column.
- Title-bar back/forward buttons walk the document history (`⌃⌘←` / `⌃⌘→`); click-and-hold shows the full recent list, "just like in a web browser or Finder". From 6.0, `⌘`-swipe left/right in the document does the same. History covers files opened from the Library, from wikilinks, and from Quick Search alike.

**What the user sees (Windows).** The same two-part structure but far thinner. A `＋ Add to Library` button sits at the bottom left and a ⚙️ at the top opens Settings. No Organizer sections are documented — no Smart Folders, no Hashtags panel — and there is no Filter Bar. Clicking a document opens it in the Editor; there is no new-window behaviour. Right-click gives rename, delete, duplicate, favorite, show in Explorer, and change sort order. Renaming happens in place. Deleted files go to the trash, with no hard-delete modifier. Files with Authorship annotations get an icon in the list.

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Show/hide Library | `⌃⌘S` | `Ctrl+E` |
| Show/hide Organizer | `View` → `Hide/Show Organizer` (no shortcut) | — |
| Cycle panes | `⌃tab` (Organizer → File List → Editor) | — |
| Quick Search | `⇧⌘O` (`Go` → `Quick Search`) | **not available** |
| Show iCloud in Library | `⇧⌘I` | — |
| Open Favorite 1–9 | `⌃1`–`⌃9` | — |
| Show Smart Folder 1–9 | `⌃⌥1`–`⌃⌥9` | — |
| Sort/Filter bars | `View` → `Show Sort Bar` / `Show Filter Bar` | Sort Bar always present |
| Add a Location | `Go` → `Add Location…`, the `+` that appears on hovering "Locations", or drag a folder from Finder | `＋ Add to Library`, or drag from File Explorer |
| New folder | right-click in File List → `New Folder` (7.2 also added a toolbar button) | right-click → `New Folder` |
| Settings | `⌘,` | `File` → `Settings`, or the ⚙️ |

Favorite and Smart Folder shortcuts are auto-assigned and shown in the `Go` menu. The Mac Library toggle used to be `⌘E` and was changed to `⌃⌘S`; Windows still uses `Ctrl+E`. macOS lets you bind unbound menu items yourself (System Settings → Keyboard → Shortcuts → App Shortcuts), which is iA's documented workaround for the missing Organizer shortcut.

**Locations (external folders).** On Mac iCloud is the default Location; turn iCloud off and it becomes "On My Mac". **The default Location cannot be removed** — extra folders are added alongside it, and "basically any folder you can locate in Finder" qualifies, which is how Dropbox, Google Drive and OneDrive are supported (sync folders, not API integrations). Locations can be renamed for display. Windows can add a **single file** as well as a folder, and Locations are removed by right-click → `Remove from Library`. Windows notes that "An internet connection is required to access or synchronize files stored in cloud locations", and warns that old versions of iCloud for Windows are buggy.

**Favorites.** Bookmarks to files *or* folders. Added by dragging into the Favorites section or right-click → `Add to Favorites`; renamable in the Organizer. **They are per-device and do not sync** — stated twice in iA's docs, with a note that syncing is planned. `⌃1`–`⌃9` opens the first nine. Windows has Favorites via the context menu but no shortcut and no Organizer section.

**Smart Folders (Mac/iOS only).** "Normal folders are static; Smart folders are dynamic… New files can find their way into a Smart Folder with no user interaction at all." One ships by default: **Recents**, "the 25 most recently used files, across the whole Library". Creating one asks for a name, whether files must match **all** rules or **any**, then rules from six criteria — **Search, Parent path, Ancestor path, Kind (text/folder/other), Date created, Date modified** — then whether to limit results to 25 items or be limitless, then a per-folder sort order. Rules are edited by double-clicking the folder in the Organizer, "as in Mail".

Path semantics: **Parent path** is one folder's contents; **Ancestor path** is that folder *and all subfolders*. `/` means the default Location, `Location: /` filters custom Locations, and the context menu copies paths. Unlike Favorites, **Smart Folders sync across devices over iCloud**.

**Search syntax** (shared by Quick Search, the Filter Bar and Smart Folder Search rules). Scope is names of *all* files, but contents only of plain-text (`txt`, `text`) and Markdown (`md`, `markdown` et al.) — so a PDF, `.docx` or image in the Library is findable by filename only, forever. Bare terms are **prefix matches**, not substring matches; quoting forces exact/phrase.

| Query | Matches |
|---|---|
| `.png` | extension begins `.png` |
| `".m"` | extension is exactly `m` |
| `text machine` | name or text has words *beginning* "text" and "machine" |
| `-microsoft word` | excludes "microsoft", includes "word" |
| `#priority` | has a hashtag beginning `#priority` |
| `-#` | has **no** hashtag |
| `[ ]` / `[x]` | contains an incomplete / completed task item |
| `^first` | first word of name or text is "first…" |
| `name:alice`, `name:^"note"`, `text:"book"` | field-scoped; `^` anchors |
| `time AND space`, `time OR "space"`, `time NOT space`, `(a AND b) OR c` | boolean |
| `NEAR(time space)` | within 10 words |
| `NEAR(time "space" "travel" 4)` | within 3 terms |

"Search works like a search engine. It's optimized to be fast with a large number of files."

**Hashtags.** `#word` inline, Twitter's spec — `#-Alice` and `#2 Alice` are invalid, and `#Alice-Rabbit` registers only "Alice". Every tag appears under the Organizer's Hashtags section and in the `Go` menu; clicking one filters the File List. Autocomplete suggests tags as you type. `⌘`-click or `⌘⏎` on a tag shows it in Quick Search. Tags can be hidden from Preview and export. On Windows tags render in the Editor and can be hidden from output, but there is no tag panel and no tag filtering.

**Search scope, summarised.**

| Tool | Platform | Scope |
|---|---|---|
| Quick Search `⇧⌘O` | Mac, iOS | Whole Library (documents, wikilinks, tags); + current document from v8 |
| Filter Bar | Mac | **Current folder only**, prefix matching, same advanced syntax |
| Smart Folder Search rule | Mac, iOS | Whole Library, narrowable with a path rule |
| Find `⌘F` / `Ctrl+F` | both | Current document |
| Dynamic Outline | Windows | Current document's headings |

Inside Quick Search results, `⏎` opens in the current window and `⇧⏎` in a new one (this pair was swapped in 5.3 — older release notes have it backwards). Arrow keys move through results, and fuzzy Library path matching is supported.

**Edge cases.**

- **Indexing is expensive.** "Any folders added as locations to the Library must be indexed, which is CPU intensive. As such, we discourage adding extremely large folders such as entire system partitions." `node_modules` is excluded explicitly.
- **The Library is an index that can go stale.** If files exist in Finder but not the Library, iA's own fix is to quit, delete `~/Library/Containers/pro.writer.mac/Data/Library/Application Support/iA Writer/index.db`, and relaunch to rebuild. The Windows equivalents are `library.json` and `user_settings.json` in `%AppData%\iA Writer`.
- **The iCloud Location is filtered to text files** (iA "Stopped showing non-text files in iCloud Library"), while added Locations are not — an asymmetry worth not copying blindly.
- Recovery is delegated: `File` → `Revert to…` for iCloud versions; icloud.com Recently Deleted; Dropbox/Drive/OneDrive keep 30 days of versions.
- Handoff between devices matches Locations **by name**, so differently-named Locations silently break it.
- Content Blocks reference other files, so moving files can break them; Mac's Project Archive export exists to bundle a document with its blocks. Block paths render as Shortest / Relative / Absolute per setting, and from 6.0 a bare filename resolves to "the nearest matching file". On Windows the path needs a leading `/` and the target must be in the same folder or a subfolder.

**Sources.** <https://ia.net/writer/support/library/organize?platform=mac>, <https://ia.net/writer/support/library/organize?platform=windows>, <https://ia.net/writer/support/library/navigation?platform=mac>, <https://ia.net/writer/support/library/navigation?platform=windows>, <https://ia.net/writer/support/library/cloud-storage?platform=mac>, <https://ia.net/writer/support/library/cloud-storage?platform=windows>, <https://ia.net/writer/support/library/content-blocks?platform=mac>, <https://ia.net/writer/support/help/trouble-shooting?platform=mac>, <https://ia.net/writer/support/help/trouble-shooting?platform=windows>, <https://ia.net/writer/support/help/version-history/release-notes-mac>, <https://ia.net/writer/support/help/version-history/release-notes-windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/topics/a-new-document-library>, <https://ia.net/topics/faster-filing-with-tree-view-a-step-forward-for-large-writing-projects>

## Heading navigation

Three different things get called "outline" in iA Writer. Keeping them apart is the main finding here.

### 1. Windows: the Dynamic Outline, inside the File List

Not a pane. The currently selected file's **row in the Library expands in place** into its heading tree:

- "the active library element turns into an outline"
- "it indents titles hierarchically"
- "you can click on the title to go directly to that spot in the document"
- and, from the Windows Navigate page where it is called **Focused Outline**: "Just like Focus Mode, only the section of the document you are editing is highlighted in the Outline, so you always know exactly where you are."

Headings only — no tasks, links or images. **No menu item and no keyboard shortcut are documented.** The only documented on/off control is `Settings` → `Library` → **Show Excerpts**, described on Windows as "display an outline of your file in the Library list" — ambiguous, because the identically named Mac setting means text excerpts. Prerequisite: the Library must be open (`Ctrl+E`) and the file selected; hide the Library and the outline is gone with it.

**Folding** is the companion, and it lives in the Editor, not the Library: "click on a `#` heading to hide its contents. Click again to expand it." No shortcut. iA on its absence elsewhere: "The only complaint we have gotten is that it is not out on all other platforms. The answer to this is 'patience, please…'"

### 2. Mac/iOS from iA Writer 8: the outline is inside Quick Search

Announced 17 June 2026 and **not yet reflected in the support pages**, which still say outlines are Windows-exclusive:

- "Document Outline is now built directly into Search."
- "Click the looking glass to reveal the structure of your document" — an empty query shows the current document's headings.
- "Start typing to search: … results from both the document and across your entire library appear in a tidy list."
- "Use the arrow and return keys to jump to any heading, filename, or text search result."
- Shortcut unchanged: `⇧⌘O`.

It is navigation only, deliberately: "Structuring a document requires a different mindset than navigating. We think it deserves its own, separate solution." You cannot reorder headings from it. This was pre-announced in 7.2's tree-view post as the next step after tree view.

### 3. Both platforms: `{{TOC}}`, a rendered table of contents

A Markdown token, not a navigation UI. "Just add `{{TOC}}` wherever you want the table of content to appear and iA Writer generates it from the Headlines you use in your text." One entry per heading, no documented depth cap, and clicking an entry in Preview jumps to that section. There is a TOC button in the format toolbar. It shows as literal `{{TOC}}` in the Editor.

**Edge cases.** Behaviour with no headings is nowhere documented (the Windows how-to conditions the outline on "a file with multiple headings", implying nothing appears). Focus Mode dismisses the Preview and Library panes on Mac, which would take the Library-hosted outline with it on Windows. Heading levels run H1–H6 (`⌘1–6` / `Ctrl+1–6`).

**Sources.** <https://ia.net/writer/support/library/navigate-big-documents>, <https://ia.net/writer/support/library/navigation?platform=windows>, <https://ia.net/writer/support/basics/markdown-guide>, <https://ia.net/writer/support/basics/features>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/how-to/stay-organized>, <https://ia.net/topics/search-to-navigate>, <https://ia.net/topics/new-file-library-for-windows>, <https://ia.net/topics/faster-filing-with-tree-view-a-step-forward-for-large-writing-projects>

## Preview

**What the user sees.** A rendered pane showing the document as it will export or print. Two independent axes:

- **Layout** — **Split** (half-screen beside the Editor, "often used by writers who want to keep track of the formatting as they go") or **Full** (the Editor is hidden entirely, "an ideal reading environment").
- **Mode** — **Web**, an HTML rendering with continuous scroll and editor-like padding, or **PDF**, which shows "each full page of the document, complete with headers, footers and the location of each page break".

The Web/PDF split is the most important structural fact: Preview is two renderers, and only PDF mode honours title page, headers, footers and page size. PDF Preview arrived in 5.5.

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Show/hide Preview | `⌘R`, `View` → `Show Preview`, or the ▶ at the top right of the Toolbar | `Ctrl+R`, `View` → `Preview`, or the play button |
| Split vs Full | `View` → `Preview` → `Split \| Full`, or the Toolbar (right) | toolbar control (no menu path documented) |
| Choose template | Toolbar (middle) or `View` → `Template` | Toolbar (middle) or `View` → `Templates` |
| Reload template (authoring) | `⇧⌘R` | — |
| Web inspector | `defaults write pro.writer.mac WebKitDeveloperExtras -bool true` | `Ctrl+J` (Chromium) |

There is **no shortcut for switching templates** on either platform.

**Refresh.** Mac's PDF Preview "automatically refreshes as you edit the text". Windows' does not: "If you make changes, you can click the refresh button at the bottom of the PDF preview to see these changes reflected."

**Scroll sync.** On by default. Only Windows exposes a toggle — `File` → `Settings` → `Editor` → **Synchronize Scroll**, "Toggle to keep Preview and Editor aligned or scroll them independently"; the Mac Settings page has no equivalent entry. The one precise statement about the algorithm is from the Windows 1.3 notes and describes **element anchoring, not proportional scrolling**: "even if your text contains large elements like tables or images, the top element in the Editor always matches the top of the Preview." Whether sync is bidirectional, and whether editing preserves position, is never stated.

**Colour.** `⌃⌘N` / `Alt+Shift+N` toggles dark. Separately, **Invert Colors** (Settings → Templates, Web mode only) makes Preview use Night mode when the Editor is in Day mode and vice versa — a deliberate per-pane theme split.

**Templates.** Six ship built in — **Modern (Sans)**, **Classic (Serif)**, **Manuscript (Mono)**, **Manuscript (Duo)**, **Manuscript (Quattro)**, **GitHub** (the Mac page says "5 built-in templates" and then lists six; the Windows page says six). **GitHub is a documented exception**: "Crafted by directly replicating GitHub's CSS, this template does not support the Default Template Settings (such as Number heading, footer…)" — choosing it silently disables those settings.

Free downloads: Helvetica, Palatino, **Academic MLA** (listed on Mac only), Letter, Fountain (screenwriting), Chess.

Template-level settings: **Center Headings**, **Number Headings** (from H2 on Windows), **Indent Paragraphs**, **Invert Colors**, plus Title Page / Header / Footer / Page Size for print and PDF, and on Windows a **Quotes Style** and **Hide Hashtags**.

**Custom templates.** A bundle of plain web files:

```
Example.iatemplate/Contents/
    Info.plist
    Resources/{document.html, title.html, header.html, footer.html, style.css}
```

`Info.plist` keys: `CFBundleName`, `CFBundleIdentifier` (required); `IATemplateDocumentFile` (required); optional `IATemplateTitleFile`, `IATemplateHeaderFile`, `IATemplateFooterFile`, `IATemplateHeaderHeight` / `IATemplateFooterHeight` (CSS pixels, ≤400), `IATemplateDescription`, `IATemplateAuthor`, `IATemplateAuthorURL`, `IATemplateSuportsSmartTables` (iA's own typo, one `p`; defaults YES), `IATemplateSupportsMath` (TeX → MathML, defaults YES), `IATemplateTitleUsesHeaderAndFooterHeight`.

Content arrives by **`innerHTML` replacement on `data-` attributes**, not by templating placeholders: `<span data-date></span>` becomes `<span data-date>June 22, 2016</span>` at load. Available: `data-document`, `data-title` (**from the file name**), `data-author` (from preferences), `data-date` (format string in the attribute value — UTS #35 on Apple, Microsoft format standards on Windows), and header/footer-only `data-page-number` and `data-page-count`. Title pages deliberately have no page number "to keep numbering the same whether or not you include the title page during export". iA "dispatches an `ia-writer-change` event to elements when it updates them with new content" — the hook anything dynamic needs.

`<html>` carries environment classes iA sets and will overwrite: `night-mode`, `ios`, `mac`, plus iOS Dynamic Type size classes. Authoring rules: avoid vertical margins on the document page (iA adjusts `<html>` padding to match the Editor in Web mode, and print margins come from header/footer heights), and set `color` and `background-color` on `<html>` because macOS tints the Preview toolbar to match.

**Title page, header and footer render only in print and PDF** — never in Web Preview or HTML export.

Installing: Mac takes a bundle by double-click, drag-to-Dock, or Settings → Templates → `+`, and **copies it**, so edits to the original are ignored (right-click → Show in Finder to edit the installed copy). Windows 1.2+ takes only a `.iatemplate.zip` via `File` → `Install Template`. iOS takes bundles or one-template ZIPs.

Without writing a template, a document can override template CSS inline — iA's own example is `<style>@media print { body { font-size: 25pt; } }</style>`.

**Markdown rendering.** The converter is MultiMarkdown "(with a few additions)".

| Feature | Syntax | Mac | Windows |
|---|---|---|---|
| Table of contents | `{{TOC}}` | yes | yes |
| Reference footnotes | `[^1]` … `[^1]: text` | yes | yes |
| **Inline footnotes** | `[^the note itself]` | **yes** | **no** |
| **Citations** | `[p. 23][#Doe:2006]` | **yes** | **no** |
| Cross-references | `[My Header][]`, custom label `# Header [Label]` | yes | **different syntax**: `[text][Header]` only |
| Tables | pipes, `:--` alignment, extra `\|` merges cells | yes | yes |
| Smart Tables | `=(…)` | **calculation** (math.js, cell refs `A0`-style, metadata vars, unit conversion) | **formatting only** (`Format` → `Table` → `Reformat`) |
| Math | `$…$`, `$$…$$`, KaTeX, converted to MathML | yes | yes |
| Task lists | `- [ ]` / `- [x]` | yes | yes |
| Page break | `+++` on its own line | yes | yes |
| Super/subscript, highlight | `^2^`, `~z` | yes | yes |
| `//` line comments | Apple only | yes | HTML comments only |
| Mermaid / diagrams | — | **no** | **no** |

Mermaid is not a product feature; the community solves it with a custom template that bundles mermaid.js, which works only because templates are arbitrary HTML/CSS/JS with a change event.

**Front matter is a variable system, not just metadata.** A `---`-delimited block at the top of the file, referenced inline as `[%key]`: "The metadata will be automatically substituted for Export and in Preview." Three tiers with defined precedence — **Content Block metadata > document metadata > global metadata** (global set in Markdown settings). `Author` is special-cased: "the only default metadata iA Writer uses, and it will be included on Title Pages in Preview and PDF export if present" — this is what feeds `data-author`. Windows can turn the whole mechanism off with Settings → Advanced → **Process Metadata**; Mac has no documented equivalent.

**Other rendering edge cases.**

- **Smart Dashes break tables**: "If you find your table does not render correctly in Preview, please ensure Smart Dashes are turned off in `Edit` → `Substitutions`" — the substitution eats the `---` separator row.
- **Smart Punctuation is render-only.** Markdown → Processing → *Apply Smart Punctuation* changes quotes and dashes in Preview and PDF only; the Editor's Smart Quotes/Dashes rewrite the file itself. iA flags the distinction explicitly.
- **Single Return** relaxes Markdown's two-return paragraph rule.
- Wikilinks, hashtags and autolinks have **independent output settings for HTML, "Other" (copy/export/publish) and Preview** on Mac — a hashtag can be source text, a `<span>`, a link, or removed entirely, differently per channel. Windows has only *Hide Hashtags*.
- Markdown-syntax local images "must be in a folder added as a Library location. This gives iA Writer permission to use the file" — a sandbox constraint, so a relative path outside the Library will not render. Spaces must be percent-encoded, unlike in Content Blocks.
- Syntax highlight colours and Style check strikethroughs never appear in Preview.
- Windows Editor text size does not affect Preview, and Windows has an **XSS Filtering** setting for embedded HTML.

**Sources.** <https://ia.net/writer/support/preview/modify-preview?platform=mac>, <https://ia.net/writer/support/preview/modify-preview?platform=windows>, <https://ia.net/writer/support/preview/templates?platform=mac>, <https://ia.net/writer/support/preview/templates?platform=windows>, <https://ia.net/writer/support/preview/custom-templates>, <https://ia.net/writer/support/basics/markdown-guide?platform=mac>, <https://ia.net/writer/support/basics/markdown-guide?platform=windows>, <https://ia.net/writer/support/editor/metadata>, <https://ia.net/writer/support/editor/smart-automation?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/topics/new-pdf-preview-better-web-publishing-improved-editing>, <https://ia.net/topics/syntax-control-individual-scroll-snippets>, <https://github.com/iainc/iA-Writer-Templates>

## Export

**The governing rule**, stated identically on both platforms: **"Preview determines what Export and Print will look like."** The template currently selected in Preview is the export styling — there is no per-export template picker. And "PDF documents exported via `File` → `Export…` use the same options as `File` → `Print…`".

**Formats.**

| Format | Mac | Windows | iA's description |
|---|---|---|---|
| PDF `.pdf` | yes | yes | "Template styling is applied and locked in" |
| MS Word `.docx` | yes | yes | for content edited further in Word |
| HTML `.html` | yes | yes | for blogging platforms and CMSes |
| Markdown `.md` | yes | yes | "Preserves your original MD syntax" |
| **Project Archive `.zip`** | yes | **no** | document *plus* embedded Content Blocks |
| **ePub** | **no** | **no** | offered nowhere, on any platform |

There is no ePub export and therefore no chapter splitting; the nearest book workflow is Content Blocks compiling chapters into one document, plus Project Archive on Mac.

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Export | `File` → `Export…`, `⇧⌘E` | `File` → `Export` — **no shortcut at all** |
| Print (formatted) | `File` → `Print`, `⌘P` | `Ctrl+P` |
| **Print Plain Text** | `File` → `Print Plain Text…`, `⌥⌘P` | **not available** |
| Page size | `File` → `Page Setup…`, `⇧⌘P` — A4, Letter, **Legal** | chosen in the PDF export dialog — **A4 or Letter only** |
| Copy as HTML | `⇧⌘C` | `Ctrl+Shift+C` |
| Copy formatted | `⌥⌘C` | — |
| Copy as Markdown (**resolves Content Blocks**) | `⌃⌘C` | — |
| Import Word | `File` → `Open…`, or drop on the Dock icon | `File` → `Import` |
| Publish to blog | `File` → `Publish → New Draft on…`, or Library right-click | **not available** |

**Options.** Settings → Templates → *Printing & PDF Export*: include a **Title page**, **headers**, **footers**. The default-template options (Center Headings, Number Headings, Indent Paragraphs) apply too — except under the GitHub template, which ignores them.

**Which formats the template touches.** PDF and print, definitively. HTML export is never documented as inlining template CSS. Markdown and `.docx` are explicitly unstyled: for Word, "Styling elements (fonts, colors, sizes) require adjustment within MS Word itself."

**Word round-trip.** `.docx` import maps Word styles to Markdown and back (`.doc` must be converted first). `↔` both ways, `→` export only, `←` import only:

emphasis ↔ italics; strong ↔ bold; `~` ↔ strikethrough; `[]()` ↔ hyperlinks; `#`–`######` ↔ Heading 1–6; lists ↔ list styles; `>` ↔ quote style; **underline ← Word only** (Markdown has none, so it is lost on export); inline code → code character style; reference links → flattened hyperlinks; **image alt text → a literal `"[…]"` string**.

The omissions matter: **tables, footnotes, math, task lists, TOC, citations and actual images are absent from iA's conversion table**, even though changelogs show real work on math in Word and Content Blocks are promised in HTML/PDF/Markdown/Word output. **Change tracking and Word comments are not supported in either direction** and are never mentioned; what *is* documented is the opposite — Windows "Improved: Html comments removal when exporting to Word", i.e. `<!-- -->` is stripped rather than converted. iA's own review mechanism is **Authorship** (per-author text attribution), not redlining. Word export is "Compatible with Microsoft Word 2007 or later, Apple Pages, and Google Docs" and "keeps headings on the same page as the following paragraph".

**Sharing and publishing (Mac/iOS only).** Drafts post to **Medium, WordPress, Ghost, Micro.blog and Micropub**; accounts live in Preferences → Accounts, and per-account Options choose **HTML or Markdown** for the posted draft. Publishing creates a *draft* and opens it in the browser rather than going live. Mac also gets the macOS Share sheet on a selection. "iA Writer for Windows doesn't currently offer direct publishing to blogs."

**Edge cases.** Syntax highlight and Style check marks never export. Hashtags can be stripped entirely. YAML metadata is substituted then hidden, but `Author` reaches the title page. Content Blocks are resolved into HTML/PDF/Word output; only Project Archive preserves them *as* blocks, and what Markdown export does with them is not documented (`⌃⌘C` "Copy Markdown (includes Content Blocks)" hints at flattening). Smart Table `=(…)` calculations are evaluated for the rendered output, not written back to the file.

**Sources.** <https://ia.net/writer/support/preview/export-share-print?platform=mac>, <https://ia.net/writer/support/preview/export-share-print?platform=windows>, <https://ia.net/writer/support/preview/blog?platform=mac>, <https://ia.net/writer/support/preview/blog?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/features>, <https://ia.net/writer/support/help/version-history/release-notes-mac>, <https://ia.net/writer/support/help/version-history/release-notes-windows>

## Syntax highlight

**What the user sees.** Words are recoloured in the Editor by part of speech: adjectives brown, nouns red, adverbs purple, verbs blue, conjunctions green. Nothing is dimmed — unselected parts of speech keep the normal text colour, so the effect is a coloured subset over ordinary prose, not a focus dim. Only five parts of speech exist; there is no preposition or pronoun category. On Windows the UI name is **Syntax Control** (the docs use both names).

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Toggle whole feature | `⇧⌘D` | no documented shortcut |
| Menu | `Focus` → `Show Syntax` | `Focus` menu |
| Other entry points | Settings → Editor → Syntax; the Focus dropdown in the Editor's title bar | Focus menu only |

Individual parts of speech are checked independently — Mac in Settings → Editor → Syntax, the Focus menu, and the title-bar dropdown; Windows in the Focus menu, where "a check mark will appear". The master toggle and the five category toggles are separate state: Syntax highlight can be on with only adjectives and adverbs coloured.

**Options and edge cases.**

- Language-gated. Mac: English, German, French, Spanish, Italian, Russian. Windows: **English only**, with a warning strip at the bottom of the Editor for anything else — "The language of the current file does not seem to be supported by Syntax Control."
- Language is detected per document ("the language of the current file"), not chosen by the user.
- Editor-only: "the highlighted parts of speech will not be visible in Preview, exported or printed documents." It never touches the file.
- Composes with Focus and Style check — all three can be on at once, and iA recommends pairing it with Focus Mode.

**Sources.** <https://ia.net/writer/support/editor/syntax-highlight?platform=mac>, <https://ia.net/writer/support/editor/syntax-highlight?platform=windows>, <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>

## Style check

**What the user sees.** Offending words and phrases are **struck through**, not underlined. The mark is author-only: "only you see words being crossed out (the strikethrough mark won't appear in Preview or Export)". Nothing is rewritten: "no deletions occur unless you choose to do them." The docs describe no hover popover, no suggestion list and no replacement UI — the mark is the whole interaction; you delete the word yourself.

Three built-in categories, with iA's own examples:

- **Fillers** — *basically*, *pretty much*, *sort of*
- **Redundancies** — basic **fundamentals**, combine **together**, fall **down** (only the redundant word is struck, not the phrase)
- **Clichés** — *against all odds*, *brass tacks*, *long and short of it*

**Toggles and shortcuts.**

| | Mac | Windows |
|---|---|---|
| Toggle | `⌥⇧⌘D` | no documented shortcut |
| Menu | `Focus` → `Enable/Disable Style Check` | `Focus` menu |
| Other entry points | Navigation Bar; Settings → Editor → Style Check | Settings → Style Check |

Each category (fillers, redundancies, clichés, custom patterns) toggles independently: "You can fully enable Style Check … or just some of them."

**Options and edge cases.**

- **Custom Patterns** — a user rule list at Settings → Editor → Custom Patterns (Mac) / Settings → Style Check (Windows). A bare pattern adds a rule (`custom ~~filler~~`); a leading `-` adds an exception that suppresses custom *and built-in* matches (`-filler`); slashes make it a regex (`/reg(exp?|ular expression)/`).
- The regex engine is deliberately limited for editing latency: character classes ASCII-only and non-repeating past 10 characters, repetition capped at 10, `.` and negated classes and all four lookaround forms **ignored**, lazy/greedy and `\w`/`\W` "undefined behavior". Case-insensitive is on by default (`i`); diacritic-insensitive (`d`) is off.
- Languages: **Mac** English, French, Spanish, German; **Windows** English, French, German. iA's pages disagree — the Mac FAQ says "En De Fr" while the Style Check article and feature matrix list Spanish for Mac. En/De/Fr is the safe intersection.
- Entirely local: "your text stays private and on your device (not sent to an internet service for review) … there is no AI".
- Behaviour inside code blocks, quotes and markup is not documented.

**Sources.** <https://ia.net/writer/support/editor/style-check?platform=mac>, <https://ia.net/writer/support/editor/style-check?platform=windows>, <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>, <https://ia.net/writer/support/basics/faq?platform=mac>

## Stats

The two platforms differ enough that the Mac model and the Windows model are separate designs.

**Mac — what the user sees.** Stats live in the **Toolbar at the bottom of the Editor**, which has three states set at `View` → `Toolbar`: *Fade In/Out*, *Always Show*, *Hide*. The same `View` menu picks the layout:

- **Default** — the Toolbar carries the usual quick actions (bold, headings, tables…) and shows **exactly one** statistic in the right corner. Clicking it expands the list of the others; clicking one pins it.
- **Stats Only** — the whole Toolbar becomes statistics. Right-clicking it and hovering `Stats Only >` gives checkboxes to show or hide individual statistics.

**Windows — what the user sees.** No always-on toolbar. A **stats bubble** appears on hovering near the bottom-right corner of the Editor. Clicking it shows the full list, lets you choose which single statistic the bubble displays, set a **Word Goal**, and pin the bubble open.

**Which numbers.** No exhaustive list is published. Mac: "character, sentence or task counts, and estimated reading time". Windows: "character, word or sentence counts, estimated reading time, and word goal". Confirmed set: characters, words, sentences, tasks (Mac), reading time, word goal (Windows). Paragraph, line and page counts and speaking time are **not** documented, and no reading-time formula or words-per-minute figure is published anywhere.

**Selection vs document.** Identical on both and stated explicitly: no selection means whole-document stats; a selection means stats for that range, updating live. With multiple documents compiled through Content Blocks, "Stats will show characters, words and reading time of all documents combined" — the master document's figures include its transclusions.

**Edge cases.** Whether markup characters, YAML metadata, footnotes or code blocks count is undocumented. There is no word goal on Mac. **No keyboard shortcut on either platform** — neither shortcuts page lists one.

**Sources.** <https://ia.net/writer/support/editor/stats?platform=mac>, <https://ia.net/writer/support/editor/stats?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/topics/faster-filing-with-tree-view-a-step-forward-for-large-writing-projects>

## Spell check

Opposite designs: Mac delegates entirely to the OS, Windows ships its own dictionaries.

**Mac.** Spelling, autocorrect and grammar come from macOS: "iA Writer will automatically use the system settings defined in Language & Region." The app's only control is Settings → Editor → **Spelling and grammar**, which "Decide[s] how iA Writer checks your spelling and grammar, if at all" — i.e. the macOS Check Spelling While Typing / Check Grammar With Spelling / Correct Spelling Automatically triad, plus system contextual-menu corrections, learned words and per-language dictionaries. Language follows the app language, overridable per-app in System Settings → General → Languages & Region → Applications. iA warns that third-party checkers "(i.e. Antidote, Grammarly) may impair the functionality of system options" and to disable them if spell check misbehaves. **Grammar checking exists on Mac only because macOS provides it.**

**Windows.** iA bundles **hunspell** dictionaries, pre-installed for English (US), German, French, Spanish and Italian. Language is chosen at Settings → Editor → **Spellcheck**, and more can be added by downloading OpenOffice/LibreOffice dictionary extensions. **No grammar checking is documented for Windows.**

**Edge cases.** Nothing documents whether spell check is suppressed in code blocks, URLs or markup, where the Windows custom-word list is stored, or whether a document can carry a language independent of the app setting.

**Sources.** <https://ia.net/writer/support/basics/localisation?platform=mac>, <https://ia.net/writer/support/basics/localisation?platform=windows>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/features>

## Behaviour we already have, where iA does it differently

Only details that differ from what our JavaScript app does today.

**Focus is one setting with three mutually exclusive scopes, not two independent toggles.** On Mac `Focus` → `Enable Focus Mode` (`⌘D`) turns on a mode whose *scope* is then Sentence / Paragraph / **Typewriter** (also Settings → Editor → Focus scope). Typewriter is a scope *of* Focus, and in it "the text will not be highlighted or dimmed" — you get typewriter scrolling instead of dimming, never both. Windows splits them: `Focus` → `Sentence`/`Paragraph` (`Ctrl+Shift+D` picks sentence), and "Typewriter Scrolling can be enabled independently of other Focus Mode or Syntax Control options." The two models are incompatible; Quill has to choose.

**Typewriter means vertically centred, and iA admits it fights editing.** "The cursor remains vertically centered in the Editor when typing or moving up or down in your document." No configurable offset. iA's own advice is to turn Focus off while editing because "A conflict between the area you will select to edit and Focus Mode's attempt to vertically center the cursor might result in the screen jumping vertically."

**Focus dismisses panes.** On Mac, "Focus Mode now dismisses Preview and Library panes" — entering Focus is also a chrome action, not just a text-rendering change.

**Appearance.** Mac `⌃⌘N` toggles dark/light (Settings → General → Appearance, Light/Dark only — no documented follow-system option; the dock icon's light/dark is a *separate* setting). Windows `Alt+Shift+N`. Windows additionally has **Invert Colors**, giving Preview the opposite theme to the Editor — a per-pane split we do not have.

**Typography.** Typeface is iA Writer **Mono / Duo / Quattro**. Line length limit is a three-way choice of **64, 72 or 80 characters** (default 64). Text size is a slider on Mac (`⌘+` / `⌘-` / `⌘0`), a stepper on Windows, and on Windows explicitly does not affect Preview. Mac also exposes indentation, tab key behaviour, tab width and wrapped-line/whitespace treatment under Editor → Advanced, a highlight colour (yellow/orange/pink/purple/blue/green), and completed-task appearance (strikethrough / faded / both).

**Chrome fades rather than hides.** Title bar and Toolbar on Mac each have three states — Always Show, Fade In/Out, Hide — not a binary. Windows has *Auto Hide TitleBar*: "TitleBar hides when you start typing or scrolling within a document."

**Smart editing.** Smart Lists continue a list marker on Return and remove the empty marker and exit the list on a second Return (blockquotes too). Smart Quotes and Smart Dashes rewrite characters *in the file*, while Markdown → Processing → *Apply Smart Punctuation* changes only the render — two settings that are easy to conflate, and Smart Dashes breaks Markdown tables. Bracket completion has two modes: "Insert and type-over matching brackets" and "Wrap selection in typed brackets".

**Sources.** <https://ia.net/writer/support/editor/focus-mode?platform=mac>, <https://ia.net/writer/support/editor/focus-mode?platform=windows>, <https://ia.net/writer/support/editor/smart-automation?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=mac>, <https://ia.net/writer/support/basics/settings?platform=windows>, <https://ia.net/writer/support/basics/shortcuts?platform=mac>, <https://ia.net/writer/support/basics/shortcuts?platform=windows>, <https://ia.net/writer/support/help/version-history/release-notes-mac>

## What the documentation does not answer

Open questions, so nobody re-reads the same pages hoping for an answer:

1. Whether Preview scroll sync is bidirectional, and whether editing preserves scroll position.
2. Whether HTML export inlines the active template's CSS or emits bare content HTML.
3. How footnotes, tables, math and real images actually land in `.docx` — absent from iA's conversion table despite changelog evidence they work.
4. What Markdown export does with Content Blocks — flattened inline, or left as path references.
5. Whether the Windows "Show Excerpts" setting really is the Dynamic Outline toggle, or a doc error carried over from the Mac wording of the same label.
6. What the outline shows for a document with no headings.
7. Whether markup characters, metadata, footnotes or code blocks are counted in Stats, and what words-per-minute reading time assumes.
8. Whether spell check and Style check are suppressed inside code blocks and markup.
9. On Mac 8.x, whether `⇧⌘P` is Page Setup (shortcuts page) or the new Command Palette (blog), and whether the Filter Bar still exists after search absorbed the outline.
