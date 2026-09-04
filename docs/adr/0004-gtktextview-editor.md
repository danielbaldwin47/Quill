# The Editor is a GtkTextView subclass

*Stands as of 2026-09-04, narrowed on the leading: the three-way split below is still how the air is
counted, but the view is no longer set to two of its parts. GTK 4.22 aborts in
`gtk_text_layout_get_iter_at_position` for a y in the band `pixels-below-lines` leaves under a line
holding invisible bytes, which under Live is every folded paragraph
([#278](https://github.com/danielbaldwin47/Quill/issues/278)), and GTK's own click, selection drag,
drop, `scroll_pages` and text-handle paths all reach it with no Quill frame to clamp in. So the band
is closed: `quill::editor`'s `restyle` hands GTK the whole paragraph gap as `pixels-above-lines` with
`pixels-below-lines` at zero, and moves the page's two margins to match — the top margin loses what
every box gained, `lay_out`'s bottom margin gains what the last paragraph no longer draws — so every
glyph row, the first row under the page top and the end-of-draft stop are on the pixels they were.
GTK paints a paragraph background over the line box rather than over the ink, so the code well would
otherwise rise by the lower half; `quill::tags`'s `well_edges` gives that half back to the well's
boundary rows and their two neighbours through the `well-head` and `well-foot` tags, and every judged
state re-shot byte-identical, `theme/light` and `theme/dark` included
([#279](https://github.com/danielbaldwin47/Quill/issues/279)). One band survives: the line above a
fenced block carries the lower half, so a code block that abuts a paragraph with no blank line
between is still exposed under Live. `press`'s own clamp is gone with the band.*

The native Editor is one `GtkTextView` subclass on GTK4 / Pango through `gtk4-rs`. A spike
([#10](https://github.com/danielbaldwin47/Quill/issues/10), branch `spike/gtk4-editor`) built the four
Pieces most likely to fail — type, markup, focus, caret — and judged them blind against the JavaScript
Parity oracle: 0.26–0.78 % of pixels differ across the whole frame, the caret lands within one logical
pixel of the browser's in both axes, and a whole-document retag costs 0.082 ms.

The decision retires two of the web app's architectural workarounds. There is no mirror: one widget
lays the text out once, so the hard rule that no per-token style may change glyph advance width stops
being an invariant to hold and becomes a property of the fonts. And GTK draws selection beneath the
glyphs already, so the hand-drawn selection rectangles go away. The custom caret survives as the one
genuinely custom piece, painted in `snapshot_layer` and driven from a frame-clock tick callback.
*Superseded on the selection by [ADR 0012](0012-editor-draws-its-own-selection.md): the Editor draws
it, and the caret is cut from the same band.*

## Considered options

`GtkSourceView 5` is a `GtkTextView` subclass on the same buffer and tag model, so it lifts no ceiling
and adds a runtime dependency for code-editor furniture the Editor does not want. A custom `GtkWidget`
driving `PangoLayout` buys nothing the tag model cannot already reach and costs the whole text-editing
substrate — cursor motion, hit-testing, IME preedit, undo, and a hand-written `GtkAccessibleText`.
`cosmic-text` + `wgpu` has no IME and no accessibility at all; `iced` has no caret geometry and only a
line-oriented highlighter; `egui` has no OpenType feature toggles. The reasoning is in
`docs/research/gtk4-typography.md` on `research/gtk4-typography`.

## Consequences

Five constraints the architecture spec has to carry.

**Fonts are bound file-to-family.** Each shipped Italic file names the Roman's family as well as its
own, so fontconfig returns the italic cut for a roman request and the whole document renders oblique.
The font pipeline assigns one unambiguous family per file through a `<match target="scan">` rule —
which is also the OFL §3 rename a public build owes.

**Font sizes are set in pixels, explicitly.** `FontDescription::from_string("family 20")` reads 20 as
points; every metric downstream is silently coarse until `set_absolute_size` is used.

**The leading is split three ways.** GTK puts all of `pixels-above-lines` above a row where CSS
splits a line box's leading half above the ink and half below — and it puts it above a *paragraph*,
not above a row, because prose wraps and a wrapped row is not a paragraph. So the air a row leaves
over is split by which of the two gaps GTK draws is doing the separating: `pixels-inside-wrap`
carries all of it, since it alone separates two rows of one paragraph, while the paragraph gap is
halved, since only the sum of its two halves separates two paragraphs. The three do not sum to the
air, and any split that made them sum to it would put one of the two gaps wrong. The spike measured
the result at a flat 36 px pitch across every row at 20 px.

The two halves no longer reach GTK as its two properties. The Editor hands it their sum as
`pixels-above-lines`, leaves `pixels-below-lines` at zero and moves the page's margins to match, and
gives the lower half back to the code well's boundary rows through tags — the status line above says
why, and `quill::tags`'s `well_edges` is where the four applications are.

**Markup × Focus is flattened before it reaches the buffer.** Overlapping `GtkTextTag`s resolve colour
by priority override rather than by blending, so the tiers are computed into non-overlapping runs
carrying one precomputed colour each: one tag per distinct colour, one per (weight, slant). Syntax
highlight and Style check will grow that tag table.

**Quattro's bold-italic widens.** 211 → 216 px per 19 characters at 20 px. Chromium measures the same
font identically, so this is the face, not the toolkit — but with no mirror it costs nothing
structural, and only the character grid is uneven.
