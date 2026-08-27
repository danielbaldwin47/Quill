# Preview and Export use native Templates, not iA `.iatemplate` bundles

Preview and Export apply a **Template**: a named typographic design (faces, sizes, rhythm, palettes)
declared as data in a TOML file, rendered by the one GTK4 / Pango + cairo renderer that
[#8](https://github.com/danielbaldwin47/Quill/issues/8) chose for Preview and PDF. Quill does not run
iA Writer's `.iatemplate` bundles. The decision and the Template model were settled in
[#14](https://github.com/danielbaldwin47/Quill/issues/14).

## Considered options

An `.iatemplate` is HTML + CSS + JavaScript with content injected by `innerHTML` on `data-` attributes,
so running one means a browser engine: WebKitGTK, measured at +252 MiB installed and +231 MiB resident
across two extra processes, and a second layout engine for Preview to disagree with PDF over. The
research found no middle path — a bundle with its JavaScript ignored still needs an HTML/CSS layout
engine. What compatibility buys is the dozen templates iA has ever published, one of which (GitHub)
switches the template settings off. Reasoning and measurements in `docs/research/preview-export.md`
on `research/preview-export`.

## Consequences

**Every Template property is expressible in both Pango and CSS.** One TOML file generates the Pango
styling for Preview and PDF and the stylesheet for HTML export, so the two cannot drift; a property
only one side can express does not belong in the model.

**A Template owns typography only.** Page size, margins, title page, header and footer are Export
settings styled by the active Template, matching the research finding that page geometry is the
app's job (WebKit ignores `@page`; iA reads margins from `Info.plist`).

**Five built-ins, and the current Template is one global setting.** Modern (Sans, Inter), Classic
(Serif, Source Serif 4) and Manuscript ×3 (Mono, Duo, Quattro). Per-Document association would need a
side store, which ADR 0002 rules out. User-authored Templates are out of this effort's scope, but
the built-ins are files a later user format can read unchanged from XDG config.
