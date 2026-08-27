# Can GTK4 text rendering reproduce the Editor's typography?

Research for [#3](https://github.com/danielbaldwin47/Quill/issues/3). Measured on this machine against
GTK 4.22.4, Pango 1.58.2, gtk4-rs 0.11.4 (2026-06-29, MSRV 1.92), GtkSourceView 5.20.0 (`extra/gtksourceview5`,
not currently installed here). Claims marked **[measured]** come from experiments run for this ticket; the
scripts are throwaway PyGObject and are not committed.

## Question

Can a GTK4 / `gtk4-rs` text widget reproduce what the JavaScript Editor does today — iA Writer
Duo/Quattro/Mono variable fonts, per-token Markup dimming and bold/italic **without changing glyph advance
width**, sentence and paragraph Focus dimming, Typewriter scrolling, a custom caret, light/dark themes — at
the current latency? Which of `GtkTextView`+Pango, GtkSourceView 5, a custom `GtkWidget` drawing Pango into a
GSK snapshot, or a non-GTK Rust stack (`cosmic-text`+`wgpu`, `iced`, `egui`) should the prototype try first?

## Answer in one line

Yes — `GtkTextView` with `GtkTextTag` clears every Piece, including the hard rule, and it does so on **one
surface**, which retires the textarea/mirror architecture entirely.

## Criteria

| # | Criterion | Comes from |
|---|---|---|
| C1 | Per-token bold/italic/dim **must not move a glyph** | `CLAUDE.md` hard rule, `app/js/markup.js` |
| C2 | Variable-font axes: body weight `wght=415`, three families | `app/css/type.css` `--ink-weight: 415` |
| C3 | Disable ligatures / kerning / contextual alternates | `app/css/type.css` grid comment |
| C4 | Overlapping tiers: Markup dim × Focus dim (bright / near / dim) over the same run | `app/js/focus.js` |
| C5 | Exact whole-pixel line pitch (`1.30·size + 10.4px`, rounded) | `--line-pitch` in `type.css` |
| C6 | Custom caret: `0.155em` wide, band 68.75 % above / 31.25 % below baseline, `+0.07em` nudge, glide 34–46 ms, custom blink idle | `app/js/caret.js` |
| C7 | Selection painted **beneath** the glyphs | `app/js/caret.js` header |
| C8 | Typewriter scrolling, retargetable ease | `app/js/focus.js` |
| C9 | Latency: no full re-render per keystroke | `BRIEF.md` standards |
| C10 | Free of charge: IME, a11y, Wayland, undo, clipboard, spell check | ADR 0001 (native Linux desktop app) |

| | GtkTextView + Pango | GtkSourceView 5 | Custom GtkWidget + Pango/GSK | cosmic-text + wgpu | iced | egui |
|---|---|---|---|---|---|---|
| C1 no advance shift | **yes [measured]** | same (subclass) | yes | yes | yes | yes |
| C2 `wght=415` | yes, via `font-desc` | same | yes | yes (0.19.0+) | via cosmic-text, not exposed | yes (`TextFormat.coords`) |
| C3 feature toggles | yes, `font-features` | same | yes | yes (`FontFeatures`) | not exposed | **no API** |
| C4 overlapping tiers | pre-merge required | same | pre-merge required | pre-merge required | **line-oriented `Highlighter` only** | pre-merge required |
| C5 exact pitch | **yes [measured]** | same | yes | yes (`Metrics.line_height` px) | `line_height` | `TextFormat.line_height` |
| C6 custom caret | yes, `snapshot_layer` | same | yes | hand-rolled | **no caret fields in `Style`** | no public knob |
| C7 selection beneath | **yes, default [measured]** | same | yes | yes (`Editor::draw`) | yes | yes |
| C8 typewriter | `scroll_to_iter` / vadjustment | same | own | own | own | own |
| C9 latency | **≤0.15 ms/keystroke [measured]** | + highlighter | own | own | own | immediate-mode redraw |
| C10 free platform | IME, AT-SPI, undo, clipboard, Wayland | + gutter/snippets | IME/a11y **you implement** | **none** | IME partial, a11y weakest | AccessKit yes, Linux IME reported broken |

## Per-approach findings

### 1. `GtkTextView` + `GtkTextTag` — recommended

**C1 is satisfied natively, and this is the headline result. [measured]** With iA Writer Duospace at 20 px in a
real `GtkTextView`, applying a `weight=700` tag, a `PangoStyle.ITALIC` tag, a `foreground-rgba` alpha tag, and
bold+dim together to the same word left `gtk_text_view_get_iter_location().x` **byte-identical** on every
following column against the untagged line:

```
col  plain   bold    ital     dim   bold+dim
  4      64      64      64      64      64
  9     144     144     144     144     144
 20     328     328     328     328     328
 48     784     784     784     784     784
```

Corroborated at the Pango layer: `iA Writer Duospace` and `iA Writer Mono S` measure identically at regular,
bold, italic and bold-italic. **`iA Writer Quattro S` italic does not** — 312 px vs 304 px for the same string,
the same defect `tools/fontgrid.py` already patches in the web app's `iAWriterQuattroV-Italic-grid.woff2`. The
patched font carries over unchanged; the OFL §3 renaming obligation carries over with it.

There is no mirror, so C1 is now a font property rather than an architectural tightrope: a single widget lays
out the text once.

**C2.** `GtkTextTag` has **no `font-variations` property** — confirmed by enumerating all 90 properties on
`Gtk.TextTag` here. But `GtkTextTag:font-desc` takes a whole `PangoFontDescription`, and
`pango_font_description_set_variations()` (Pango 1.42+) survives the round-trip intact: **[measured]** setting
`"iA Writer Quattro S 20"` + `wght=415` reads back `wght=415` and stringifies as
`iA Writer Quattro S 20 @wght=415`. Pango does apply arbitrary axis values — on Adwaita Sans (the only variable
family installed) `wght=100/400/700/900` produce 264/277/289/296 px for the same string. Note the `font`
*string* property did **not** parse `@wght=415` in this path; set `font-desc` with a real
`PangoFontDescription`, not the string shortcut. Pango has no API to query a font's axis ranges — values are
silently clamped to the `fvar` range, so validate with HarfBuzz if it matters.

The shipped fonts are `.woff2`; fontconfig cannot load them. They must be converted to `.ttf`/`.otf` and either
installed or loaded from the app's own directory.

**C3.** `GtkTextTag:font-features` exists (`gchararray`) and maps straight onto
`pango_attr_font_features_new()`. **[measured]** `"liga 0,clig 0,calt 0,kern 0"` changes DejaVu Serif's width
for `"office affluent VAV To"` from 239 px to 251 px, so the toggle is real. On the iA faces it is a no-op —
they carry nothing to disable — but the knob is there for fallback fonts.

**C4 is the one place the tag model is weaker than CSS classes.** Overlapping `GtkTextTag`s resolve
`foreground-rgba` by **priority override, not blending** (`gtktextattributes.c`: the highest-priority tag with
the property set wins outright; only `scale` multiplies). So a "markup dim" tag and a "focus dim" tag stacked on
one run do not compose. The fix is the same shape as `focus.js`'s `spansForLine()` already computes: flatten
Markup × Focus into non-overlapping runs with one **precomputed combined alpha each**, and apply one tag per
distinct (colour, alpha, weight, style) combination. The tag table is a small fixed set; only the ranges move.

Alpha itself works: **[measured]** `foreground-rgba` with `alpha=0.25` / `0.55` renders as three legible tiers
against the page in one `GtkTextView`.

**C5.** Contrary to the "factor-only" reading of `GtkTextTag:line-height` (a `gfloat` scale, GTK 4.6+), an exact
whole-pixel pitch **is** reachable from stock `GtkTextView`: **[measured]** with
`set_pixels_above_lines(10)` on a 35 px natural line height, `get_iter_location().y` for six consecutive lines
came back `10, 55, 100, 145, 190, 235` — a uniform **45 px** pitch, unchanged by bold/italic/dim tags. So
`pixels_above_lines = round(1.30·size + 10.4) − natural_height` gives the pitch exactly, and blank lines get
the same pitch for free (unlike the CSS `min-height` workaround in `page.css`). Pango's
`pango_attr_line_height_new_absolute()` (Pango 1.50) is not reachable through `GtkTextView`, and is not needed.

**C6.** `gtk_text_view_set_cursor_visible(FALSE)` hides the native caret. GTK exposes only
`GtkSettings:gtk-cursor-aspect-ratio` (default 0.04) and CSS `caret-color` — nowhere near a `0.155em` band with
an asymmetric baseline split. The sanctioned escape hatch is the `snapshot_layer` vfunc, called before and
after GTK draws its own text with `GTK_TEXT_VIEW_LAYER_BELOW_TEXT` / `ABOVE_TEXT`. **[measured]** subclassing
`GtkTextView` and overriding it painted a custom caret rect at the cursor's `get_iter_location()` in the
BELOW_TEXT layer. `TextViewImpl::snapshot_layer(&self, layer: TextViewLayer, snapshot: &Snapshot)` is present
in gtk4-rs 0.11.4, so this is a supported Rust subclass, not an FFI stunt.

Geometry inputs are all available: `get_iter_location()` gives the char cell (x, y, width, height),
`get_cursor_locations()` gives the strong/weak cursor rects. The 34–46 ms glide and the 480 ms blink idle need
their own driver — `GtkSettings:gtk-cursor-blink{,-time,-timeout}` is display-wide on/off/interval only, so use
`gtk_widget_add_tick_callback` on the frame clock, exactly as `caret.js` uses rAF.

**C7 is free, and this is the second architectural win.** GTK draws the selection as a background node beneath
the glyphs already. **[measured]** `textview text selection { background-color: #ffd75e; color: #101010; }`
renders dark glyphs on the highlight, legible — the *precise* failure that forced `caret.js` to draw its own
selection rectangles in the browser. `snapshot_layer(BELOW_TEXT)` remains available if the shape needs
customising (the `NL_TAIL` half-em for a selected newline, for instance).

**C8.** `gtk_text_view_scroll_to_iter()` with `yalign`/`within_margin`, or drive the
`GtkScrolledWindow` vadjustment directly from a tick callback for the retargetable ease `focus.js` uses. Both
are ordinary.

**C9.** The tag model is cheap. **[measured]** on a 38 KB / 641-line buffer: `set_text` 0.36 ms; applying 642
markup tags across the whole document 5.0 ms; a full-document Focus retag (clear five alpha tags, re-dim
everything, brighten one 300-char sentence) plus event drain **median 0.06 ms, p95 1.11 ms**; the same limited
to the visible range median 0.04 ms; a single-character insert plus drain **median 0.113 ms, max 0.150 ms**.
These measure buffer-side tag bookkeeping and event drain, not a full paint — but they bound the tag churn at
well under a frame, and `GtkTextView` caches a `PangoLayout` per line and re-lays out only lines that changed,
which is the same incremental discipline `core.js` implements by hand.

**C10, free:** `GtkIMContext` preedit, `gtk_text_buffer_set_enable_undo()` with
`begin/end_irreversible_action()`, clipboard, DnD, Wayland/HiDPI, and `GtkAccessibleText` (GTK 4.14+) —
`Gtk.TextView` implements it, so Orca works without effort. **[measured]** `Gtk.TextView.__mro__` includes
`AccessibleText`.

**Not free:** spell check (no in-tree GTK4 spell checker; `gspell` or Rust `spellbook` against
`/usr/share/hunspell`), and Wayland `text-input-v3` is **not** GTK4's default IM backend — the default is
`simple`, i.e. no input method, unless `GTK_IM_MODULE=wayland` is set.

**What it cannot do.** No `PangoAttrList` can be attached to a `GtkTextBuffer` range — tags are the only
vocabulary, so anything Pango can express but `GtkTextTag` cannot (absolute line-height attributes,
`PANGO_ATTR_FONT_SCALE`, per-run baseline shifts beyond `rise`) is out of reach without replacing the layout.
And see the Pango-wide limit below, which applies to every GTK option.

### 2. GtkSourceView 5

A direct `GtkTextView` subclass on the same `GtkTextBuffer`/`GtkTextTag` model, and `GtkSourceStyleScheme`
compiles down to ordinary `GtkTextTag`s — so it **lifts no ceiling** from §1 and inherits every limit. What it
adds (gutter, snippets, code completion, Vim mode, syntax engine, search/replace) is code-editor furniture the
Editor does not want; the Markup tokenizer in `markup.js` is already prose-specific and would be reimplemented
regardless. It also adds a runtime dependency (`extra/gtksourceview5` 5.20.0, not installed here) to the
PKGBUILD for no capability gain. **Skip it.** Revisit only if search/replace UI or printing becomes worth the
dependency.

### 3. Custom `GtkWidget` + Pango + GSK snapshot

Buys exactly two things over §1: `pango_attr_line_height_new_absolute()` (unnecessary — `pixels-above-lines`
already gives an exact pitch **[measured]**) and free-form `PangoAttrList` composition (unnecessary — the tiers
must be flattened to non-overlapping runs anyway, per C4). It costs the entire text-editing substrate:
cursor motion by grapheme/word/line, click and drag hit-testing, IME preedit placement, undo, and — most
expensively — a hand-written `GtkAccessibleText` implementation, which GTK's own docs frame as the thing you
write when you are building a terminal emulator. **Not first, and probably not ever.** It is the fallback if
one specific `GtkTextView` behaviour turns out to be immovable.

### 4. cosmic-text + wgpu / glyphon

Technically the closest non-GTK fit: `AttrsList::add_span` gives per-range attributes, `Weight` is a bare
`u16` newtype so `Weight(415)` is constructible, variable-font `wght` matching landed in 0.19.0 (PRs #486/#488,
2026-04), `FontFeatures::disable(KERNING / STANDARD_LIGATURES / CONTEXTUAL_ALTERNATES)` covers C3,
`Metrics { font_size, line_height }` are both **absolute pixels** so C5 is exact, and `Editor::draw` already
paints selection under the glyphs. Shaping is HarfRust; bidi and UAX #14 line breaking are present.

What it cannot do: **no IME/preedit** (issue #10, open since the repo's creation) and **no accessibility at
all** — both become the app's problem, and C10 is most of the reason ADR 0001 chose a native toolkit.
`glyphon` has **no subpixel positioning** (issue #3, closed as a design decision) and no concept of selection
or caret; hinting is a binary `Hinting::Disabled|Enabled` pending an upstream `fontations` PR (#279), with no
stem-darkening (#195) — which directly contradicts the `--ink-weight: 415` finding that iA's ink is optically
heavier on light backgrounds. `add_span` is last-write-wins on overlap, so C4 needs the same flattening.

### 5. iced

`text_editor::Content` is real, but the **only** per-token styling path into the editable widget is the
line-oriented `Highlighter` trait — feed a line, get styling back, re-feed everything after a change. That is a
syntax-highlighting shape, not "recompute three dim tiers as the caret moves." `rich_text` / `text::Span` is a
**display-only** widget and does not compose with `text_editor`. `text_editor::Style` has five fields —
`background`, `border`, `placeholder`, `value`, `selection` — and **no caret colour, width, shape or blink**,
so C6 is closed. `Edit` has no `Undo`/`Redo`. IME landed (PR #2777, 2025-02) but only over-the-spot, not
inline. Accessibility is the weakest of the three (tracking issue open 4½ years, no AccessKit). **Fails C4 and
C6 outright.**

### 6. egui

Better than its reputation as of 2026: `ab_glyph` was replaced by Skrifa + `vello_cpu` with hinting (0.34.0,
2026-03), HarfRust shaping for kerning and ligatures (0.35.0, 2026-06), and a real variable-axis API —
`TextFormat.coords: VariationCoords` takes `(b"wght", 415.0)` per span, satisfying C2 cleanly. `TextEdit::layouter`
returning a `LayoutJob` of per-range `TextFormat`s satisfies C1/C4-after-flattening, and it is the only stack
here with production AccessKit wired into `eframe`.

But: **no OpenType feature-toggle API** — the PR author says so explicitly — so C3 is unreachable even though
HarfRust supports it underneath. **No bidi** (issue #1016). Native Linux IME reported broken since 0.29
(issue #5544). No public caret knob. And immediate-mode redraw for a document you stare at all day is a latency
and battery argument in the wrong direction. **Fails C3.**

## Recommendation

**Build the prototype on a `GtkTextView` subclass.**

It is the only option that satisfies C1 through C10 with nothing marked "you implement it," and two of the web
app's most expensive workarounds simply evaporate: the `<textarea>`/`#mirror` pair collapses into one widget
(so the hard rule stops being an architectural constraint and becomes a font property), and the
hand-drawn selection rectangles in `caret.js` go away because GTK already paints selection beneath the glyphs.
The custom caret survives as the one genuinely custom piece, and `snapshot_layer` is the documented hook for it.

## What the prototype should try, in order

**First — the parity spike.** One `GtkTextView` subclass in `gtk4-rs` 0.11.4 that proves the four Pieces most
likely to fail, on `ref/sample.md`, in both themes:

1. Convert the three variable `.woff2` faces to `.ttf`, load them from the app directory, and set
   `GtkTextTag:font-desc` to a `PangoFontDescription` carrying `@wght=415`, plus
   `font-features = "liga 0,calt 0,kern 0"`.
2. Reproduce `markup.js`'s tokenizer output as a **flattened**, non-overlapping run list where each run carries
   one precomputed (colour, alpha, weight, style) tuple already combining Markup and Focus — one tag per
   distinct tuple, ranges recomputed on caret move. Assert the advance-width invariant by comparing
   `get_iter_location().x` at fixed columns across a tagged and an untagged copy of the same line, as a test.
3. Set the pitch with `pixels_above_lines = round(1.30·size + 10.4) − natural_height` and assert
   `get_iter_location().y` is an exact arithmetic progression.
4. Hide the native caret and draw the `caret.js` geometry in `snapshot_layer`, animated from
   `add_tick_callback`.

Then run the existing blind-judging Gate against the JavaScript oracle for **type**, **markup rendering**,
**cursor and caret**, and **focus and typewriter**.

**Second — only if the spike fails a Piece.** Keep the `GtkTextBuffer` and the tag model, and move just the
failing layer into a custom `GtkWidget` driving `PangoLayout` directly (§3), so the a11y/IME/undo substrate is
preserved for as long as possible. Do **not** move to cosmic-text or egui to fix a typography detail; their
gaps (IME, a11y, feature toggles, hinting) are larger than any gap found in GTK.

## Open risks

1. **Pango quantizes glyph advances to whole pixels.** **[measured]**, and the most consequential finding
   after C1. With `HINT_METRICS_OFF` + `HINT_STYLE_NONE` on a fresh `PangoCairo` font map, `iA Writer
   Quattro S` `'n'` measures 12.00 px at both 20 px and 20.5 px, and 24.00 px at 40.5 px (0.6 em predicts 12.3
   and 24.3); DejaVu Sans behaves the same. Consequences: the character cell moves in whole-pixel steps, so the
   Mod +/- size ladder is coarser than the browser's and the `--measure: 64ch` column width lands on different
   numbers; and the fair-comparison protocol in `BRIEF.md` cannot match a 27.65 px iA capture to the subpixel.
   This may well be *better* for grid crispness — but it must be judged, not assumed. **Measure it in the spike
   before anything else.**
2. **Quattro italic advances differ** (312 vs 304 px **[measured]**). The `fontgrid.py` patch and its OFL §3
   renaming obligation must be carried into the native build's font pipeline.
3. **Overlapping tags do not blend alpha.** Every Markup × Focus combination must be flattened to one tag per
   distinct tuple. If the combinatorics grow (Syntax highlight and Style check are still ahead), the tag table
   grows with them; watch the tag count.
4. **`wght=415` may be indistinguishable from 400 after quantization.** **[measured]** on Adwaita Sans,
   `wght=400` and `wght=415` produced identical widths — good for C1, but re-verify the *ink* difference is
   still visible on the real iA variable faces once converted.
5. **Wayland IME is not on by default** (`GTK_IM_MODULE=simple`); CJK preedit needs `GTK_IM_MODULE=wayland`,
   whose preedit styling is reported poor. Test before claiming IME as free.
6. **GSK renderer variance.** Hint mode negotiation differs between the cairo and the Vulkan/GL renderers
   (GTK 4.14+); the Vulkan renderer failed to open the DRM device on this machine and fell back. Pin the
   renderer for judging so screenshots are reproducible.
7. **No Rust toolchain installed here** (`cargo`/`rustc` absent; `gtk4` 4.22.4 and `pango` 1.58.2 present).
   Install `rustup` before the spike.
8. **Latency numbers above exclude paint.** They bound tag churn, not a frame. Port `tools/latency.mjs`'s
   methodology to the native app early so the Gate keeps a real budget.

## Sources

- [Gtk.TextTag](https://docs.gtk.org/gtk4/class.TextTag.html), [:font-features](https://docs.gtk.org/gtk4/property.TextTag.font-features.html), [:line-height](https://docs.gtk.org/gtk4/property.TextTag.line-height.html)
- [Gtk.TextView](https://docs.gtk.org/gtk4/class.TextView.html) (`snapshot_layer`, `cursor-visible`), [Gtk.AccessibleText](https://docs.gtk.org/gtk4/iface.AccessibleText.html), [Gtk.Settings:gtk-cursor-aspect-ratio](https://docs.gtk.org/gtk4/property.Settings.gtk-cursor-aspect-ratio.html)
- [gtk/gtktextattributes.c](https://github.com/GNOME/gtk/blob/main/gtk/gtktextattributes.c) (tag override semantics), [gtk/gtktexttag.c](https://github.com/GNOME/gtk/blob/main/gtk/gtktexttag.c)
- [Pango.FontDescription.set_variations](https://docs.gtk.org/Pango/method.FontDescription.set_variations.html), [Pango.AttrType](https://docs.gtk.org/Pango/enum.AttrType.html)
- [On fractional scales, fonts and hinting](https://blogs.gnome.org/gtk/2024/03/07/on-fractional-scales-fonts-and-hinting/), [Accessibility improvements in GTK 4.14](https://blog.gtk.org/2024/03/08/accessibility-improvements-in-gtk-4-14/), [An accessibility update](https://blogs.gnome.org/gtk/2025/05/12/an-accessibility-update/)
- [Input methods in GTK4 apps on Wayland](https://whynothugo.nl/journal/2026/06/22/input-methods-in-gtk4-apps-on-wayland/)
- [GtkSourceView 5 overview](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/overview.html), [style scheme reference](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/style-reference.html)
- [gtk4-rs TextViewImpl](https://docs.rs/gtk4/latest/gtk4/subclass/text_view/trait.TextViewImpl.html), [gtk4 crate](https://crates.io/crates/gtk4)
- cosmic-text: [AttrsList](https://docs.rs/cosmic-text/latest/cosmic_text/struct.AttrsList.html), [FontFeatures](https://docs.rs/cosmic-text/latest/cosmic_text/struct.FontFeatures.html), [Metrics](https://docs.rs/cosmic-text/latest/cosmic_text/struct.Metrics.html), [PR #486](https://github.com/pop-os/cosmic-text/pull/486), [IME issue #10](https://github.com/pop-os/cosmic-text/issues/10), [hinting issue #279](https://github.com/pop-os/cosmic-text/issues/279), [glyphon subpixel issue #3](https://github.com/grovesNL/glyphon/issues/3)
- iced: [text_editor::Style](https://docs.rs/iced/latest/iced/widget/text_editor/struct.Style.html), [Highlighter](https://docs.iced.rs/iced/advanced/text/trait.Highlighter.html), [IME PR #2777](https://github.com/iced-rs/iced/pull/2777)
- egui: [epaint CHANGELOG](https://github.com/emilk/egui/blob/main/crates/epaint/CHANGELOG.md), [font variations PR #7859](https://github.com/emilk/egui/pull/7859), [AccessKit PR #2294](https://github.com/emilk/egui/pull/2294), [bidi #1016](https://github.com/emilk/egui/issues/1016), [Linux IME #5544](https://github.com/emilk/egui/issues/5544)
- winit: [atomic IME PR #4263](https://github.com/rust-windowing/winit/pull/4263), [IME API issue #4412](https://github.com/rust-windowing/winit/issues/4412), [DnD PR #4571](https://github.com/rust-windowing/winit/pull/4571)
- [AccessKit](https://github.com/AccessKit/accesskit), [spellbook](https://github.com/helix-editor/spellbook), [ashpd](https://github.com/bilelmoussaoui/ashpd)
