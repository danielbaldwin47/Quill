# Preview render and PDF export: WebKitGTK vs a native GTK/Pango renderer

Research for [#8](https://github.com/danielbaldwin47/Quill/issues/8). Measured 2026-08-27 on Arch (kernel 7.1.9, 20 cores), against `webkitgtk 2.52.6`, `gtk4 1:4.22.4`, `pango 1.x` as packaged in `extra`.

## Question

Preview shows the rendered Document beside or instead of the Editor; Export writes PDF, HTML and a Markdown copy. Should Preview be a WebKitGTK 6 web view rendering an HTML/CSS template (closest to iA Writer's `.iatemplate` bundles, PDF via its print path), or a native GTK4/Pango renderer (no browser engine, PDF via cairo or `typst`)? Can Preview and PDF share one renderer?

## Options

| | (a) WebKitGTK 6 | (b) Native GTK4/Pango | (c) Hybrid (native Preview, headless PDF) |
|---|---|---|---|
| Preview fidelity to HTML/CSS templates | exact — it is a browser | must be re-implemented; tables and floated images are the hard part | native, same gap as (b) |
| Runs iA `.iatemplate` bundles (HTML+CSS+JS) | yes | no, not at all | no |
| PDF route | `WebKitPrintOperation` → Skia/PDF | `cairo::PdfSurface` + Pango, or `GtkPrintOperation` ACTION_EXPORT | second engine (WebKit or `typst`) just for PDF |
| Preview and PDF share a renderer | yes (both HTML) | yes (both Pango/cairo) | **no** — two renderers, two sets of bugs |
| Arch install cost over a plain GTK4 app | **+252.3 MiB**, 21 extra packages | 0 (cairo, pango already in the gtk4 closure) | +252 MiB or +432-crate `typst` tree |
| Flatpak cost | **0** — `org.gnome.Platform` already ships it | 0 | 0 |
| Runtime cost | +231 MiB PSS, 2 extra processes | ~0 | as (a)/(b) for the export step |
| Preview refresh (25-page doc) | 26.9 ms mean | ~1 ms/page laid out (20 pages in 20.1 ms) | as (b) |
| Work to build | small | large: a block layout engine | largest |

## Findings

### Dependency weight on Arch (measured with `pacman`/`pactree`)

- `pacman -Si webkitgtk-6.0` → version `2.52.6-1`, **Download Size 36.18 MiB, Installed Size 130.81 MiB**.
- Full dependency closure (`pactree -su`): `gtk4` pulls 210 packages / 935.6 MiB; `webkitgtk-6.0` pulls 231 packages / 1187.9 MiB. **The marginal cost of adding WebKitGTK to a GTK4 app is 252.3 MiB across 21 packages.**
- The 21 extras, by size: `webkitgtk-6.0` 130.8, `perl` 70.3, `openjpeg2` 13.4, `aom` 9.2, `rav1e` 7.5, `db5.3` 6.5, `svt-av1` 5.4, `tpm2-tss` 3.6, `libyuv` 1.5, `libsecret` 1.2, `libavif` 0.9, `libxslt` 0.8, `libmanette` 0.4, `enchant` 0.3, `libevdev` 0.2, `woff2` 0.2, `hidapi` 0.1, `xdg-dbus-proxy` 0.06, `hyphen` 0.04, `harfbuzz-icu`, `ttf-font` (MiB).
- `perl` (70 MiB, the second-largest item) arrives only because `webkitgtk` depends on `hyphen`, which depends on `perl` (`pactree -r perl` → `perl → hyphen → webkit2gtk-4.1`). Three AV1 encoders (`aom`, `rav1e`, `svt-av1`) and `libmanette` (gamepads) come in for video and Gamepad API support a writing app will never use.
- For scale: the current Quill Arch package is 384 KB.

### Flatpak: WebKitGTK is free there

`org.gnome.Platform` already contains it. GNOME's `gnome-build-meta` composes the runtime from `elements/sdk-platform.bst`, whose `depends:` list includes both `sdk/webkit2gtk-4.1.bst` and `sdk/webkitgtk-6.0.bst` (https://gitlab.gnome.org/GNOME/gnome-build-meta/-/raw/master/elements/sdk-platform.bst). `elements/sdk.bst` is `sdk-platform.bst` plus dev tools, so the Platform — not just the SDK — ships the engine. **On the Flatpak route WebKitGTK costs nothing; the 252 MiB is an AUR/`pacman`-only cost.**

### Rust bindings

- `webkit6` 0.6.1 (11 Mar 2026), from GNOME World: https://gitlab.gnome.org/World/Rust/webkit6-rs. Feature flags run `v2_42` … `v2_52`, so it covers the 2.52.6 in `extra`. It is gir-generated on the gtk-rs stack (`gtk`, `gdk`, `glib`, `gio`, `soup` from the workspace).
- `webkit6::PrintOperation` exposes `new()`, `print()`, `run_dialog()`, `set_print_settings()`, `set_page_setup()`, `connect_finished()`, `connect_failed()` (https://docs.rs/webkit6/latest/webkit6/struct.PrintOperation.html). `print()` runs without a dialog. docs.rs reports the crate is 15.83% documented.

### Headless PDF through WebKit works — measured

Measured against `webkit2gtk-4.1` 2.52.6 (the GTK3 ABI of the same 2.52.6 engine; `webkitgtk-6.0` is not installed on this machine and installing it costs 252 MiB). A GTK `OffscreenWindow` + `WebView`, `WebKitPrintOperation` with `GtkPrintSettings` `output-uri=file:///…pdf`, `output-file-format=pdf`, printer `Print to File`:

| step | time |
|---|---|
| import + GTK/WebKit init | 73–81 ms |
| WebView construct + show | 46–48 ms |
| load 42 KB / 25-page HTML → `LoadEvent.FINISHED` | 279–311 ms from process start |
| `print()` → `finished` signal | 192–222 ms |
| **process start → PDF on disk** | **672–734 ms** |

Output: 25 pages, `Producer: Skia/PDF m145`, all fonts embedded and subset (`AAAAAA+LiberationSans`, `DAAAAA+JetBrainsMonoNF-Regular`, …), text extracts cleanly with `pdftotext`. So PDF text is selectable and searchable.

Two gotchas found by measurement:

1. **WebKitGTK needs a GL context.** The first run died with `ERROR **: GDK is not able to create a GL context: The current backend does not support OpenGL` in an offscreen window. It only worked with `WEBKIT_DISABLE_COMPOSITING_MODE=1` (or `LIBGL_ALWAYS_SOFTWARE=1`, which cost 570 ms extra and logged `Failed to create GBM buffer`). A headless export path must set that env var, and the Gate's screenshot harness (#7) inherits the same constraint.
2. **CSS `@page` is ignored; `GtkPageSetup` wins.** A page declaring `@page { size: A5; margin: 25mm 20mm }` still produced 612×792 pt letter pages. Setting `Gtk.PageSetup.set_paper_size(PaperSize.new('iso_a5'))` + `set_*_margin(20, Unit.MM)` produced 420×595 pt A5. Page geometry is Quill's job, not the template's — which matches iA, where margins come from `IATemplateHeaderHeight`/`IATemplateFooterHeight` in `Info.plist`, not from CSS.

CSS fragmentation *is* honoured: `h1 { break-before: page }` on two headings produced 3 pages.

### Runtime footprint — measured (PSS, `/proc/*/smaps_rollup`)

| process tree | PSS |
|---|---|
| `python3` + GTK3, one label | 27.2 MiB |
| `python3` + GTK4, one label | 49.0 MiB |
| + `WebView` constructed, nothing loaded | 85.9 MiB (`python3` 79.5 + `WebKitNetworkProcess` 6.2) |
| + 42 KB preview page loaded | **257.9 MiB** (`python3` 121.5 + `WebKitNetworkProcess` 40.1 + `WebKitWebProcess` 96.3) |

A single WebView spawns **two extra processes** (network + web content) and adds 230.7 MiB PSS over the same process without a WebView at all (172 MiB of that after the page loads). RSS across the tree at print time was 811 MiB (shared pages double-counted; PSS is the honest number).

### Preview refresh cost — measured

Warm `WebView`, `load_html()` of the same 42 KB / 25-page document, 20 iterations, timed to `LoadEvent.FINISHED`: **mean 26.9 ms, median 25.9, min 25.4, max 42.6**. That is fine for a debounced preview but ~5× over Quill's ≤5 ms mean keystroke budget, so a WebKit Preview can never be driven per-keystroke; it needs debouncing plus scroll-position restoration on every reload.

### Native PDF through Pango + cairo — measured

`cairo.PDFSurface` + `PangoCairo.show_layout`, naive paragraph-at-a-time pagination of 16,160 characters in iA Writer Quattro S 11pt:

- import Pango/PangoCairo/cairo: **48.9 ms**
- layout + render 16,160 chars → **20 pages: 20.1 ms**
- total process → PDF: **69.0 ms** (vs 672–734 ms for WebKit)
- file: **42,533 bytes** (vs 2,974,932 bytes from WebKit — 70× larger)
- fonts: `LCLNBQ+iAWriterQuattroS-Bold`, TrueType, **embedded and subset**; `pdftotext` extracts the text
- process PSS: **20.2 MiB**

cairo's PDF surface docs (https://www.cairographics.org/manual/cairo-PDF-Surfaces.html) confirm PDF 1.4–1.7 output, `cairo_pdf_surface_set_metadata` for title/author/dates and `cairo_pdf_surface_add_outline` for a bookmark outline (i.e. a PDF table of contents from the heading tree, which #2 lists as a Preview/Export feature). Font subsetting is not documented there but is demonstrated by the measurement above.

`GtkPrintOperation` gives the same cairo path plus real printing: `gtk_print_operation_set_export_filename` "Sets up the `GtkPrintOperation` to generate a file instead of showing the print dialog… Currently, PDF is the only supported format" (https://docs.gtk.org/gtk4/method.PrintOperation.set_export_filename.html), used with `GTK_PRINT_OPERATION_ACTION_EXPORT`. The `draw-page` callback hands you a cairo context per page — the same context type the Preview widget can draw into.

### What a native renderer can and cannot do

`GtkTextTag` (https://docs.gtk.org/gtk4/class.TextTag.html) has every property a prose renderer needs: `family`, `size`, `weight`, `style`, `foreground`, `background`, `paragraph-background`, `pixels-above-lines`, `pixels-below-lines`, `pixels-inside-wrap`, `left-margin`, `right-margin`, `indent`, `justification`, `wrap-mode`, `underline`, `strikethrough`, `rise`, `letter-spacing`, `line-height`, `font-features`. Pango handles line breaking, bidi, font fallback and shaping; `gtk_snapshot_append_layout()` renders a `PangoLayout` straight into a widget's snapshot, and `gtk_snapshot_append_cairo()` gives a cairo context inside the same widget — **so one layout pass can feed both the Preview widget and the PDF surface.**

What it does not give you, from iA's own reference output (`ref/ia/templates/Markup.html`, the HTML iA produces for its markup sample): `<table>` with `colgroup`, per-column alignment and `colspan`; `<figure>`/`<figcaption>` with images; footnote and citation blocks with back-references; task-list checkboxes; code blocks with a full-width background; horizontal rules. Each is buildable — tables are a column-width solve plus per-cell `PangoLayout`s; images via `gtk_text_buffer_insert_paintable` or a custom widget; checkboxes and rules are drawing — but tables are genuinely the expensive one, and page-breaking a table across pages in the PDF path is more work again. `gtk-markdown` 0.2.0 (crates.io, May 2026) is "a GTK4 widget that renders **a subset of** Markdown as native GTK widgets", which is a fair signal of where the ceiling sits for off-the-shelf help.

### Other PDF routes, rejected

- **`typst` as a library** — 0.15.1 (17 Jul 2026), Apache-2.0, so GPL-3 compatible one-way. `typst` has 13 required deps, `typst-pdf` has 21 (incl. `krilla`, `image`, `typst-assets`); the workspace `Cargo.lock` has **432 packages**. Embedding means implementing the `World` trait — `library()`, `book()`, `main()`, `source()`, `file()`, `font()`, `today()` — i.e. supplying a font book and a virtual filesystem. Fatal objection: **Typst's input is Typst markup, not Markdown**, so Export would need a Markdown→Typst translation layer, and the PDF would then be laid out by an engine that shares nothing with Preview. Preview and PDF would drift, which is exactly what the Gate blind-judges.
- **`printpdf`** — 0.12.6 (17 Aug 2026), MIT, actively maintained, but it is a PDF *writer*: you position text yourself. No line breaking, no pagination. You would be writing the layout engine anyway, and without Pango's shaping.
- **`genpdf`** — 0.2.0, last released **17 Jun 2021**. Abandoned. Do not use.
- **`blitz` / `stylo`** (0.3.0-beta.2, Aug 2026, MIT/Apache) — a pure-Rust HTML/CSS renderer on Servo's Stylo. Interesting as a future "HTML fidelity without 252 MiB" answer, but beta, and its PDF story is not there. Revisit, do not build on.

## Recommendation

**Preview: a native GTK4/Pango renderer. PDF: cairo `PdfSurface` driven by the same layout pass, exposed through `GtkPrintOperation` so Print and Export to PDF are one code path. HTML export: the Markdown parser (#4) plus Quill's template CSS, no engine involved.** Preview and PDF share one renderer; there is no hybrid.

Why, in order of weight:

1. **The Editor is already Pango.** A WebKit Preview would put a second text stack — second shaper, second font rasterizer, second colour and theme pipeline — beside the Editor, in an app whose whole premise is that its typography survives blind judging against iA Writer. "Preview and Editor set the same passage differently" is a Piece-level failure that a native renderer cannot have by construction.
2. **252.3 MiB and two extra processes for a secondary view.** On Arch that is the entire cost of shipping a preview pane, including 70 MiB of Perl, three AV1 encoders and a gamepad library. Runtime is +231 MiB PSS and a 26.9 ms refresh that cannot be driven per-keystroke.
3. **The native PDF path is better on every measured axis** — 69 ms vs 672 ms, 42 KB vs 2.9 MB, both with subset-embedded fonts and extractable text — and it is the same cairo context the Preview widget draws into, so what you see is what you print, by construction rather than by luck.
4. **`GtkPrintOperation` gets printing free.** WebKit's route ends at a file; GTK's ends at a file *or* a printer, and CUPS/portal printing is a feature #2 has not scoped but iA has.

**This recommendation is conditional on one open decision in #2's fog: "whether iA's template format is supported or Quill ships its own."** iA `.iatemplate` bundles are HTML + CSS + JavaScript with `data-document` injection. If Quill must run third-party `.iatemplate` bundles, the native path is impossible and the answer flips to WebKitGTK for both Preview and PDF — the 252 MiB becomes the price of the feature, and it is free on Flatpak. **Recommend deciding this first, and deciding against `.iatemplate` compatibility:** Quill ships its own Preview design, matched to the Editor, and Export's HTML uses the same stylesheet.

## Risks

- **Tables are the real work.** Column-width solving, `colspan`, per-column alignment, and breaking a table across PDF pages. If the spike (#10) cannot get a table to survive blind judging, that is the signal to reconsider. Mitigation: build the block layout engine table-last and ship Preview without tables behind the same gate the other Pieces use.
- **Everything measured on WebKit's side used `webkit2gtk-4.1` (GTK3 ABI), not `webkitgtk-6.0`,** because installing the GTK4 build costs 252 MiB on this machine. Same upstream 2.52.6 and the same `WebKitPrintOperation`, but GTK4 has no `GtkOffscreenWindow`, so **headless printing in GTK4 needs re-verifying** against an unmapped or unrealized window. Cheap to check in the spike; do it before relying on any of this if the decision flips to WebKit.
- **The GL-context requirement is a live hazard** for any headless path (export from CLI, the Gate's harness). `WEBKIT_DISABLE_COMPOSITING_MODE=1` was needed even under a working Wayland session.
- **Page-break quality is on us.** WebKit gives widow/orphan control and `break-inside: avoid` free; a cairo paginator gets whatever we implement. Budget for keeping a heading with its following paragraph and not orphaning a single line.
- **Footnotes, citations and a heading outline** are Export features iA has (`ref/ia/templates/Markup.html`) that a native paginator must place itself. `cairo_pdf_surface_add_outline` covers the outline; footnote placement at page bottom is real layout work, and CommonMark footnotes depend on the parser choice in #4.
- **Preview refresh policy** must be specified either way: debounce, and restore scroll position and the caret's corresponding block after a re-render.

## Method

Numbers above are from this machine, not from documentation: `pacman -Si` / `pactree -su` for sizes; `python3-gobject` driving `Gtk.OffscreenWindow` + `WebKit2.WebView` + `WebKit2.PrintOperation` for the WebKit timings, PSS from `/proc/*/smaps_rollup`, PDF inspected with `pdfinfo`/`pdffonts`/`pdftotext`; `cairo.PDFSurface` + `PangoCairo` for the native timings. The test document is iA's own `ref/ia/templates/Markup.html` (its full markup sample) repeated 20× inside `GitHub.iatemplate`'s `document.html` and `github.css`, 42,413 bytes, 25 printed pages.
