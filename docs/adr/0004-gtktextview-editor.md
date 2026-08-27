# The Editor is a GtkTextView subclass

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

**The leading is split three ways.** `pixels-above-lines` is leading above a *paragraph*, and prose
wraps: `pixels-inside-wrap` carries the same leading between the rows of one paragraph, and
`pixels-below-lines` takes half of it, because GTK puts all the leading above a row where CSS splits
it.

**Markup × Focus is flattened before it reaches the buffer.** Overlapping `GtkTextTag`s resolve colour
by priority override rather than by blending, so the tiers are computed into non-overlapping runs
carrying one precomputed colour each: one tag per distinct colour, one per (weight, slant). Syntax
highlight and Style check will grow that tag table.

**Quattro's bold-italic widens.** 211 → 216 px per 19 characters at 20 px. Chromium measures the same
font identically, so this is the face, not the toolkit — but with no mirror it costs nothing
structural, and only the character grid is uneven.
