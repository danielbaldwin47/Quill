//! The tag table: one `GtkTextTag` per look a span asks for, grown as it is.
//!
//! `docs/architecture.md` § Annotators and the keystroke path fixes the shape.
//! Overlapping tags override a property by priority rather than blending, so
//! the table holds one tag per distinct `(colour, alpha)` and one per
//! `(weight, slant)` rather than one per kind of mark, created on first use and
//! never removed. The table is GTK's own, keyed by name, so the Editor keeps no
//! second copy of it to fall out of step.
//!
//! Paragraph tags are the third row, and they hang the same pair of properties
//! in opposite directions. `heading-1` to `heading-6` hang a heading's `#`
//! markers *out* into the left gutter, by exactly as far as the layout will
//! advance them, so the first word sits on the prose's edge and `###### ` fills
//! the seven-cell gutter [`typography::GUTTER`] was sized at. `hang:<run>`
//! hangs a list item's and a blockquote's wrapped rows *in*, by exactly as far
//! as that paragraph's own marker run advances: the `-`, the `123.` and the `>`
//! stay on the body column, and every row under them starts where the item's
//! first word did. A wrapped quote carries no second `>`, and no rule is drawn
//! beside one.
//!
//! That is what the Design oracle draws: `mac-native-19-{light,dark}-wrapped-markers`
//! and their `-h6` pair, measured in `ref/ia/mac-native/CAPTURE-2026-09-09.md`
//! § #241 and `NOTES.md` § State 14 — `- ` and `> ` anchor their continuations
//! two cells past the body column and `123. ` five, on both grounds and under
//! the widest heading alike. The rule is `docs/design.md` row What hangs, and
//! [ADR 0016](../../docs/adr/0016-the-text-container-is-78-cells.md) — whose
//! title is the older half of it — was narrowed to the same reading by #241.
//!
//! The Parity oracle could hang nothing at all — `legacy/app/css/markup.css`
//! says why: a `<textarea>` takes no per-line horizontal shift, so the web app
//! bought the same calm with contrast instead of position — and with no oracle
//! for the rest, #102 read the marketing frames' `#` in the margin as a rule
//! for every marker and hung them all. #167 read the running app and hung the
//! heading alone, which was the whole of what the captures then held; #241 shot
//! a wrapped item and found the other half of the rule.
//!
//! `ground-code-block` is the other paragraph tag, and it is one for the
//! opposite reason: a run's background stops with the last glyph on the line,
//! so only a paragraph's can run past both edges of the measure and read as a
//! well.
//!
//! **A run arrives with its colour already resolved.** The engine's flattening
//! takes the Markup mark, Focus tier and Syntax Category and answers with one
//! colour ([`quill_engine::annotate::paint_tagged_in`]), so this module reads no palette role
//! for text: it asks for the tag that draws that colour at that opacity, and
//! the roles a ground still owes it are the ones no run carries — the code
//! well and a link's rule. Nothing here holds a colour of its own, which is
//! why `grep -n '#[0-9a-f]\{6\}' quill/src` comes back empty.

use std::ops::Range;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use quill_engine::annotate::live::{self, Fold, Furniture, LiveLook};
use quill_engine::annotate::{self, Ink, Look, Mark, Slant, Span, Weight};
use quill_engine::document::Document;
use quill_engine::focus::{self, Focus, LineTiers, Tier};
use quill_engine::settings::Face;
use quill_engine::style::List;
use quill_engine::theme::{Colour, Colours, Role};
use quill_engine::typography;

use crate::editor::INK_WEIGHT;

/// The weight a heading or a strong is set at: `.md-h`, `.md-strong { font-
/// weight: 700 }` of `legacy/app/css/markup.css`.
///
/// Bold at body size, in the same Face and the same ink as the prose. The
/// level reads from the markers, so nothing about the text image jumps when a
/// `#` is typed or deleted — which is why no heading is ever set larger.
const BOLD: i32 = 700;

/// The weight the prose is set at, as GTK counts weights.
///
/// The Editor sets it in CSS on the whole widget; a run that is not bold has to
/// say so all the same, because the run before it may have been.
const REGULAR: i32 = INK_WEIGHT.cast_signed();

/// How much larger than the body a heading is set under Live, by level.
///
/// The fixed ladder the Preview spec names: H1 1.6, H2 1.4, H3 1.2, and H4 to
/// H6 at body size, bold as every heading already is. Fixed rather than
/// derived, because it is a design decision of the spec's own and not a
/// function of the type ladder: a heading is one and a half sentences of
/// title, and the three that carry a manuscript's structure are the three that
/// grow.
///
/// With Live off nothing reads it, which is what leaves every judged state the
/// page it was.
pub const LADDER: [f64; 6] = [1.6, 1.4, 1.2, 1.0, 1.0, 1.0];

/// How far the code ground runs past each edge of the measure, at `step` of
/// the type ladder.
///
/// The oracle's `box-shadow: -.7em 0 0 var(--code-bg), .7em 0 0 var(--code-bg)`
/// on `.line.l-code`, which is what makes a fenced block read as a well rather
/// than as a stripe the exact width of the prose. Ems rather than cells,
/// because that is what the oracle measured it in and the two are not the same
/// thing: a cell is 0.6em on these Faces.
fn well(step: u32) -> i32 {
    pixels(0.7 * typography::em(step))
}

/// `length` as whole pixels.
///
/// The app's one rounding to a whole pixel: every horizontal length this module
/// sets — a hang, a well — and every length the Library and the Palette lay out
/// come through here, so that two of them counted off the same measure cannot
/// land half a pixel apart.
pub(crate) fn pixels(length: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a marker is a handful of cells and a well a fraction of a type size; \
                  the measure itself is rounded the same way"
    )]
    let whole = length.round() as i32;
    whole
}

/// A length the typography counted, as the `i32` GTK takes it in.
///
/// [`typography::Column`] counts every edge and every hang in whole pixels
/// from the left of the view and [`typography::Leading`] counts the air around
/// a row the same way; GTK takes a margin and a paragraph's leading signed. A
/// container wider than `i32::MAX` is not a window anyone has.
fn signed(length: u32) -> i32 {
    i32::try_from(length).unwrap_or(i32::MAX)
}

/// The tag named `name`, made by `build` if this is the first ask for it.
fn tag(buffer: &gtk::TextBuffer, name: &str, build: impl FnOnce(&gtk::TextTag)) -> gtk::TextTag {
    let table = buffer.tag_table();
    if let Some(existing) = table.lookup(name) {
        return existing;
    }
    let tag = gtk::TextTag::builder().name(name).build();
    build(&tag);
    table.add(&tag);
    tag
}

/// The tag that draws text in `colour` at `alpha`.
///
/// `docs/architecture.md` § Annotators keys this row by `(colour, alpha)`, and
/// both halves are in the name: everything #87 draws is opaque, and #40 dims
/// what is out of focus by asking for the same colour at another alpha, which
/// is another tag rather than a blend with this one.
fn colour(buffer: &gtk::TextBuffer, colour: &str, alpha: u8) -> gtk::TextTag {
    tag(buffer, &format!("colour-{colour}-{alpha}"), |tag| {
        tag.set_foreground_rgba(Some(&shaded(colour, alpha)));
    })
}

/// `colour` at `alpha`, as GTK takes a colour.
///
/// A colour that will not parse comes out black, which is at least legible:
/// the three that reach here are constants of this module, so it cannot happen
/// without an edit to them.
fn shaded(colour: &str, alpha: u8) -> gdk::RGBA {
    let parsed = gdk::RGBA::parse(colour).unwrap_or(gdk::RGBA::BLACK);
    gdk::RGBA::new(
        parsed.red(),
        parsed.green(),
        parsed.blue(),
        f32::from(alpha) / f32::from(u8::MAX),
    )
}

/// The tag that sets text at `weight` and `slant`, in `face`.
///
/// The second key of that section. The slant is a family rather than a style:
/// each Italic is a Face of its own and its file declares itself roman
/// (ADR 0007), so asking for the Roman in an italic style would get a slanted
/// Roman — the wrong cut, and the one Face whose advances are not the Roman's.
fn cut(buffer: &gtk::TextBuffer, face: Face, weight: Weight, slant: Slant) -> gtk::TextTag {
    let weight = match weight {
        Weight::Regular => REGULAR,
        Weight::Bold => BOLD,
    };
    match slant {
        Slant::Upright => tag(buffer, &format!("weight-{weight}-upright"), |tag| {
            tag.set_weight(weight);
        }),
        Slant::Italic => {
            let italic = tag(buffer, &format!("weight-{weight}-italic"), |tag| {
                tag.set_weight(weight);
            });
            // Set every time rather than only on the first ask: the Face moves
            // under the tag when the writer changes it, and a tag that kept
            // the old family would set this Face's italics in the last one's.
            italic.set_family(Some(face.italic_family()));
            italic
        }
    }
}

/// The tag that draws the code ground, and nothing else.
///
/// The oracle pads an inline code span with a box-shadow rather than with
/// padding, precisely so that no glyph moves; a background is the same bargain
/// in Pango, and the ground reaching under the backticks is what makes it look
/// padded at all.
/// Set every time rather than only on the first ask, for the reason [`cut`]
/// re-sets the Italic family: the name is the tag's key, the ground moves under
/// it when the scheme changes, and a tag that kept the old ground would paint
/// the light well on the dark page. The colour-keyed tags [`colour`] makes need
/// none of this — their ground is in their name — and these three do because
/// there is one of each.
fn ground(buffer: &gtk::TextBuffer, colours: &Colours) -> gtk::TextTag {
    let ground = tag(buffer, "ground-code", |_| {});
    ground.set_background(Some(&code_well(colours)));
    ground
}

/// The tag that draws the ground under a whole code block.
///
/// A *paragraph* background rather than the run background [`ground`] gives a
/// code span, and that is the point: a run's background stops with its last
/// glyph, so a fenced block would come out as a ragged stack of lines the
/// length of the code on each. A paragraph background fills the line, which is
/// how the ground runs past both edges of the measure and reads as a well —
/// the same picture the oracle buys with a ±0.7em box-shadow on `.line.l-code`
/// because a `<textarea>` gives it no paragraph to paint.
fn code_ground(buffer: &gtk::TextBuffer, colours: &Colours) -> gtk::TextTag {
    let ground = tag(buffer, "ground-code-block", |_| {});
    ground.set_paragraph_background(Some(&code_well(colours)));
    ground
}

/// The tag that sets a row's air above to the upper half of a paragraph's gap.
///
/// The view is set to the whole gap above every paragraph and to nothing below
/// it, because a bottom band is where GTK aborts on a line holding invisible
/// bytes (#279; `Editor::restyle` says the rest). That keeps every glyph row
/// where it was and lifts the code well, which is a paragraph background and so
/// is painted over the whole line box, leading included. This tag and
/// [`well_foot`] hand the halves back around the well, four applications of the
/// two: see [`well_edges`].
///
/// Set every time rather than only on the first ask, for the reason [`cut`]
/// re-sets the Italic family — there is one of these and the type moves under
/// it.
fn well_head(buffer: &gtk::TextBuffer, leading: typography::Leading) -> gtk::TextTag {
    let head = tag(buffer, "well-head", |_| {});
    head.set_pixels_above_lines(signed(leading.above));
    head
}

/// The tag that draws the lower half of a paragraph's gap under a row.
///
/// The other half of [`well_head`], and the one the view no longer sets at all.
/// A row that carries it holds a band GTK could abort in — so it goes on a code
/// row and on the line above a code block, which are rows the fold leaves whole
/// (#279, [`well_edges`]).
fn well_foot(buffer: &gtk::TextBuffer, leading: typography::Leading) -> gtk::TextTag {
    let foot = tag(buffer, "well-foot", |_| {});
    foot.set_pixels_below_lines(signed(leading.below));
    foot
}

/// Sets both of the code well's leading tags to the air `leading` leaves.
///
/// What `Editor::restyle` calls, and it is called there rather than left to the
/// next [`draw`] because a type change restyles without retagging: the tags are
/// already on the rows they belong to, and only their values move with the
/// size.
pub fn well_leading(buffer: &gtk::TextBuffer, leading: typography::Leading) {
    well_head(buffer, leading);
    well_foot(buffer, leading);
}

/// The tag that underlines a link's destination.
///
/// A decoration, layered over the runs rather than resolved into them:
/// underline and colour are different properties, so the overlap is safe
/// (`docs/architecture.md` § Annotators).
///
/// [`Role::LinkRule`] rather than the link colour at an opacity, because the
/// Design oracle draws the hairline the same under a full-ink URL as under a
/// quieted one — it is its own ink, not a tint of the text above it (#198,
/// `ref/ia/mac-native/NOTES.md` § Found here: the link's ink and the code
/// ground).
fn underline(buffer: &gtk::TextBuffer, colours: &Colours) -> gtk::TextTag {
    let underline = tag(buffer, "decoration-underline", |tag| {
        tag.set_underline(pango::Underline::Single);
    });
    let rule = colours.colour(Role::LinkRule).to_hex();
    underline.set_underline_rgba(Some(&shaded(&rule, Look::OPAQUE)));
    underline
}

/// The tag that strikes struck text through.
fn struck(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "decoration-strike", |tag| {
        tag.set_strikethrough(true);
    })
}

/// What every Style check tag's name begins with, so that [`repaint`] can take
/// the three off a range without naming them one by one.
const STYLE_MARK: &str = "decoration-style-";

/// The tag that strikes a phrase `list` matched through.
///
/// Three tags, one per List, identical in every property. The split is for the
/// toggle and not for telling the Lists apart: a writer switching Clichés off
/// takes that tag off the page and leaves the other two where they are, and
/// the mark itself is one mark whichever List asked for it (#356).
///
/// A decoration, layered over the flattened runs like [`underline`] rather
/// than resolved into them, so a strike costs a run no colour, weight or
/// slant: `docs/architecture.md` § Annotators fixes that for decorations, and
/// it is why the same bytes can carry a Category's colour and this line at
/// once.
///
/// **Provisional** (#354, the capture that measures the mark against the
/// Design oracle). A Pango strikethrough and nothing else: with no colour of
/// its own the line takes the run's resolved colour, so it dims with Focus,
/// goes red over a red noun under Syntax highlight and is the body's ink
/// otherwise, at Pango's default thickness and position for the Face. What the
/// capture finds — a colour, a [`Role`] if the colour is not the ink's, a
/// thickness or a position — is one edit to this function and a
/// `docs/design.md` row; until then no row is written.
fn style_mark(buffer: &gtk::TextBuffer, list: List) -> gtk::TextTag {
    tag(buffer, &format!("{STYLE_MARK}{}", list.key()), |tag| {
        tag.set_strikethrough(true);
    })
}

/// Strikes every enabled List's spans through, over the bytes `at`.
///
/// After the colour runs and over them, because the mark is a decoration and
/// the colour is the run's: a retag, a recolour or a repaint of those bytes
/// changes what the words are drawn in and leaves the line across them alone.
/// The spans the store holds are every List's; [`crate::syntax::Syntax::paints`]
/// answers which of them are drawn, so a List switched off is a repaint and
/// never a re-match.
fn strike_lists(
    buffer: &gtk::TextBuffer,
    document: &Document,
    painting: Painting,
    at: &Range<usize>,
) {
    let syntax = painting.syntax.borrow();
    for (span, list) in syntax.lists_in(document, at) {
        if !syntax.paints(list) {
            continue;
        }
        let from = iter_at(buffer, document, span.start);
        let to = iter_at(buffer, document, span.end);
        buffer.apply_tag(&style_mark(buffer, list), &from, &to);
    }
}

/// The tag that closes a folded inline delimiter up.
///
/// `invisible` takes the bytes out of the layout altogether: they advance
/// nothing and the words on either side of them meet, which is what a folded
/// `**` or a backtick is meant to look like. It is the wrong fold for a
/// block-leading marker — see [`hidden`].
fn folded(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "live-folded", |tag| {
        tag.set_invisible(true);
    })
}

/// The tag that takes a marker's ink away and leaves its cells where they are,
/// by drawing it in `ground` — the colour it stands on.
///
/// The other fold, and the one a marker at the head of its block takes. An
/// invisible run advances zero, and a heading hangs by exactly what its `#`s
/// advance ([`hang_markers`]), so folding them invisibly would start the
/// heading's words one marker run inside the gutter. A marker drawn in its own
/// ground keeps the advance: the hang is handed back by the same glyphs, the
/// words stay on the body column, and the cells a bullet or a task box left
/// stay the width the Editor draws its furniture in (`Editor::draw_furniture`).
///
/// **The ground rather than transparent ink**, which is what this wanted:
/// `GtkTextTag`'s foreground reaches Pango as a `PangoColor`, which has no
/// alpha, so a foreground at alpha 0 is drawn at full strength — the fold was
/// built that way first and the `#`s came back on the page in the shot. The
/// price is that the colour has to be right: a marker inside a fenced block
/// stands on the well and not on the paper, which is why the caller reads the
/// ground under each span rather than passing the page's.
///
/// **The one place this module puts two tags with the same property on the
/// same bytes.** A folded marker already carries the colour its run resolved
/// to, and priority — the tag table's own order of addition — decides which
/// foreground is seen. So the fold is lifted to the top of the table each time
/// it is asked for: a colour first asked for after this tag was made would
/// otherwise outrank it and the marker would come back on the page.
fn hidden(buffer: &gtk::TextBuffer, ground: &str) -> gtk::TextTag {
    let hidden = tag(buffer, &format!("live-hidden-{ground}"), |tag| {
        tag.set_foreground_rgba(Some(&shaded(ground, Look::OPAQUE)));
    });
    hidden.set_priority(buffer.tag_table().size() - 1);
    hidden
}

/// The tag that sets a folded quotation's `>` in `ink`, the body's own.
///
/// The other place this module puts two tags with the same property on the same
/// bytes ([`hidden`] is the first, and says what priority does about it): the
/// flattening has already coloured this marker at [`Ink::Marker`], so the tag
/// is lifted to the top of the table each time it is asked for. A colour first
/// asked for after this tag was made would otherwise outrank it and the `>`
/// would stay the marker's grey on a palette that sets the two apart.
fn quoted(buffer: &gtk::TextBuffer, ink: &Colour) -> gtk::TextTag {
    let hex = ink.to_hex();
    let alpha = ink.opacity();
    let quoted = tag(buffer, &format!("live-quote-{hex}-{alpha}"), |tag| {
        tag.set_foreground_rgba(Some(&shaded(&hex, alpha)));
    });
    quoted.set_priority(buffer.tag_table().size() - 1);
    quoted
}

/// The whole line `at` stands on, its line break with it.
///
/// In GTK's own units rather than the Document's, because the byte a fence's
/// fold has to reach is the paragraph delimiter and no span the engine hands
/// over is about it: a line is taken off the page only when everything on it
/// *and* its delimiter are invisible.
fn whole_line(at: &gtk::TextIter) -> (gtk::TextIter, gtk::TextIter) {
    let mut from = *at;
    from.set_line_offset(0);
    let mut to = from;
    // False at the last line of the buffer, where it has already moved the
    // iterator to the end of that line, which is the answer wanted here.
    to.forward_line();
    (from, to)
}

/// The tag that draws the accent rule under a link's words.
///
/// Live's one piece of furniture that is a tag rather than a box painted in
/// the Editor's snapshot (#274), and it has to be. Every other piece stands in
/// the cells of a **block-leading** marker, and those cells are where GTK says
/// they are; a link's words stand after its folded `[`, and
/// `gtk_text_view_get_iter_location` answers for a byte as though nothing on
/// the line were invisible — measured on #274's own passage, where every
/// offset after a folded `](https://…)` came back at the end of the row. Pango
/// knows where the words really are, and an underline is what it is for.
///
/// The accent, which #263 § Live: the fold names for it: the colour the caret
/// is cut from, and the one a writer already reads as the app speaking rather
/// than the page. [`underline`]'s rule is the Design oracle's under a
/// *destination* and is a different colour on different bytes, so the two tags
/// never meet — and they must not, because both set `underline` and priority
/// would be left to decide.
fn link_rule(buffer: &gtk::TextBuffer, colours: &Colours) -> gtk::TextTag {
    let rule = tag(buffer, "live-link", |tag| {
        tag.set_underline(pango::Underline::Single);
    });
    let accent = colours.colour(Role::Accent).to_hex();
    rule.set_underline_rgba(Some(&shaded(&accent, Look::OPAQUE)));
    rule
}

/// The tag that sets a heading of `level` at its size on the Live ladder.
///
/// A character property and not a paragraph one, so it goes on the heading's
/// own bytes rather than on `heading-<level>`: that tag hangs every heading
/// whether Live is on or off, and a size on it would scale the page with Live
/// off. The levels the ladder leaves at body size take no tag at all, which is
/// why this answers `None` for them.
fn scaled(buffer: &gtk::TextBuffer, level: u8) -> Option<gtk::TextTag> {
    let scale = *LADDER.get(usize::from(level).saturating_sub(1))?;
    if scale == 1.0 {
        return None;
    }
    Some(tag(buffer, &format!("live-scale-{level}"), |tag| {
        tag.set_scale(scale);
    }))
}

/// The paragraph tag for a heading of `level`.
///
/// Made with nothing set on it, and hung by [`hang_markers`] instead: a
/// `GtkTextTag` applies only the properties whose `-set` flag is on, so a tag
/// that has not been hung yet changes no margin, and one that has is hung for
/// every line already carrying it.
fn heading(buffer: &gtk::TextBuffer, level: u8) -> gtk::TextTag {
    tag(buffer, &format!("heading-{level}"), |_| {})
}

/// Where a hung paragraph's rows begin.
///
/// A hang is the pair and neither half of it means anything alone: the margin
/// alone would move every row together, and the indent alone would move the
/// rows apart without saying from where. Both hangs here take a negative
/// indent, which Pango applies to every row *but* the first, so the first row
/// begins at [`left`](Self::left) and the rest [`indent`](Self::indent) further
/// in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Hang {
    /// The paragraph's own left margin, which is where its first row begins.
    left: i32,
    /// What Pango moves the rows under it by: negative, and its size is the
    /// marker run's.
    indent: i32,
}

impl Hang {
    /// Puts the pair on `tag`, which is the only way either half reaches GTK.
    fn on(self, tag: &gtk::TextTag) {
        tag.set_left_margin(self.left);
        tag.set_indent(self.indent);
    }
}

/// The [`Hang`] that hangs a marker `width` pixels wide out into the gutter
/// against `side`.
///
/// The first row starts at the margin with the marker in the gutter, and the
/// indent gives the marker back to every row under it, which must land on
/// `side` exactly.
fn hung(width: i32, side: i32) -> Hang {
    let width = width.min(side);
    Hang {
        left: side - width,
        indent: -width,
    }
}

/// What every marker-hang tag's name starts with, the marker run itself
/// following it.
const HANG: &str = "hang:";

/// The [`Hang`] that holds a marker's wrapped rows `width` pixels *inside*
/// `side`, taking at most `ceiling` of the measure.
///
/// [`hung`] read the other way round, and the two differ in the margin alone. A
/// heading puts its margin a marker to the left of the prose, so its `#`s begin
/// in the gutter and its wrapped rows come back to the body column; a list item
/// and a quote leave their margin on the body column, so the marker begins
/// there and the wrapped rows go in under the item's own first word.
///
/// The margin is set even though it is the one the view already has, so that a
/// paragraph that stops being a list item and keeps its tag is not a paragraph
/// whose margin nobody owns.
///
/// `ceiling` is what stops this hang eating the measure. A heading's is clamped
/// by the gutter, because a left margin cannot go below zero; this one has no
/// such floor — what it spends is the *measure*, and a marker run as wide as
/// the measure leaves its own wrapped rows nowhere to wrap in. See
/// [`indent_ceiling`] for what is held back, and why an item whose marker run
/// is wider than that keeps what the measure can still give it.
fn hung_under(width: i32, side: i32, ceiling: i32) -> Hang {
    Hang {
        left: side,
        indent: -width.clamp(0, ceiling),
    }
}

/// The most a marker's [`Hang`] may indent its wrapped rows by: `column`'s
/// measure less one cell.
///
/// The last cell is held back so that a wrapped row always has somewhere to put
/// a glyph, and an item whose marker run is wider than the measure carries on
/// in it — the same trade a window too narrow to hang a heading makes, where
/// the words go somewhere wrong rather than nowhere at all.
///
/// The cell is counted off the gutter rather than off the type, because
/// [`typography::Column`] holds the gutter at [`typography::GUTTER`] cells
/// exactly and this module has no Face to ask.
fn indent_ceiling(column: typography::Column) -> i32 {
    let cell = signed(column.gutter() / typography::GUTTER);
    (signed(column.width) - cell).max(0)
}

/// The paragraph tag that hangs the wrapped rows of a paragraph opened by the
/// marker run `run`.
///
/// Keyed by the run itself, so the table holds one tag per distinct marker —
/// `- `, `> `, `123. `, and a nested item's own indentation with it — made on
/// the first paragraph that asks for it and never removed, as every other tag
/// here is. The name carries the run so that [`hang_markers`] can measure it
/// again when the type moves under it, without this module keeping a second
/// copy of the runs to fall out of step.
///
/// `hang` is asked only when the tag is being made, because measuring a run is
/// a layout and a paragraph on the keystroke path usually asks for a tag that
/// already exists. It answers `None` before the page has been laid out at all:
/// there is no container to hang inside yet, and the layout that gives it one
/// hangs every tag in the table.
fn opened_by(
    buffer: &gtk::TextBuffer,
    run: &str,
    hang: impl FnOnce() -> Option<Hang>,
) -> gtk::TextTag {
    tag(buffer, &format!("{HANG}{run}"), |tag| {
        if let Some(hang) = hang() {
            hang.on(tag);
        }
    })
}

/// Hangs every marker in the table against the container `column`: the heading
/// markers out into its gutter, by the `advances` the caller measured off the
/// layout, and each list item's and quote's wrapped rows in under its own
/// words, by the run `measure` is asked for.
///
/// Pango's `indent` only ever shifts to the right: a positive value moves the
/// first row of a paragraph, a negative one moves every row *after* the first.
/// So a marker cannot be hung by a negative indent alone. It is hung by a pair:
/// the paragraph's own left margin is set one marker to the left of the prose's
/// margin, and the indent gives that marker back to every wrapped row. The
/// first row starts at `side - hang` with the `#` in the margin, and every row
/// under it starts at `side`, flush with the prose.
///
/// `advances[level - 1]` is how far that level's `#`s and their space actually
/// advance — [`Editor::marker_advance`](crate::editor::Editor::marker_advance)
/// says why it is measured rather than counted off the cell, and why counting
/// it was what lost round 10. Hanging by exactly it is what lands all six
/// levels' words on the one body column, which is the whole of what this pair
/// is for. [`typography::Column::hang`] is the same rule in the ladder's own
/// cell and is what sizes the gutter; it is not what the tag hangs by, because
/// the gutter is designed once and the type is laid out per launch.
///
/// A bullet, an ordinal and a quote's `>` are hung the other way, by
/// [`hung_under`]: their marker stays on the body column and their wrapped rows
/// come in under the item's first word. Their runs are not a ladder of six but
/// whatever the Document holds, so they are read back off the tags the buffer is
/// already carrying — [`opened_by`] named each one for its run — rather than
/// worked out from a Document this has not got. That is the same walk
/// [`set_face`] makes for the Italic tags and for the same reason: the Editor
/// does not re-derive its spans when the type changes under them.
///
/// Called whenever the page is laid out, because both halves of every pair
/// move: the container with the width of the window, and the marker run with
/// the size of the type. So every run is measured again here, which is a layout
/// per distinct marker in the table — the resize path, and not the keystroke
/// path, where a paragraph asks [`opened_by`] for a tag that already exists.
/// A window too narrow to give a heading its margin keeps what it has — the
/// tag's left margin cannot go below zero, and a heading that cannot hang is
/// worth less than a heading pushed off the left edge of the view; the mirror
/// of that guard, for a marker run wider than the measure, is
/// [`indent_ceiling`].
pub fn hang_markers(
    buffer: &gtk::TextBuffer,
    step: u32,
    column: typography::Column,
    advances: [i32; 6],
    measure: &dyn Measure,
    colours: &Colours,
) {
    let side = signed(column.side);
    for (level, advance) in (1..=6u8).zip(advances) {
        hung(advance, side).on(&heading(buffer, level));
    }
    let ceiling = indent_ceiling(column);
    buffer.tag_table().foreach(|tag| {
        let Some(name) = tag.name() else {
            return;
        };
        let Some(run) = name.as_str().strip_prefix(HANG) else {
            return;
        };
        hung_under(measure.advance(run), side, ceiling).on(tag);
    });
    // The well is the same pair read the other way round. A paragraph
    // background fills the line's own box, so the only way to put ground
    // outside the measure is to give the block a box wider than one: its
    // margins go a well past the prose on both sides, and the indent puts the
    // code itself back on the prose's edge. Every line of a fenced block is a
    // paragraph of its own, so every line is a first row and takes that indent.
    //
    // Two things follow, and neither is free. A wrapped tail of an over-long
    // code line sits a well to the left, because Pango's indent moves the first
    // row alone. And the box being wider is the box the code wraps in, so a
    // code line wraps a well later than prose would — Pango has no indent for
    // the right side. Both are the price of a ground that clears the measure at
    // all: the oracle pays none of it because a box-shadow is not layout, and a
    // `<textarea>` gave it no paragraph to paint. A well is under one cell and
    // a fifth, so the two rarely differ by a character.
    //
    // Deliberately not `min(side)`-ed away to nothing: a window too narrow to
    // give the well its margin is one the prose has no gutter in either.
    let edge = well(step).min(side);
    let ground = code_ground(buffer, colours);
    ground.set_left_margin(side - edge);
    ground.set_right_margin(side - edge);
    ground.set_indent(edge);
}

/// Measures how far a run of marker text advances in the type now set.
///
/// A capability rather than a width, because a marker run is not known until
/// the Document is walked: `- `, `> `, and every distinct ordinal a list
/// carries. [`Editor::marker_advance`](crate::editor::Editor::marker_advance)
/// says why a run is measured off a layout rather than counted off the cell,
/// and why counting it was what lost round 10.
pub trait Measure {
    /// How far `run` advances, in whole pixels.
    fn advance(&self, run: &str) -> i32;
}

/// Everything but the text that decides how a Document is drawn.
///
/// The Face and the ground were two arguments carried side by side through
/// every function here; Focus adds the tiers, which travel with them and are
/// read at the same moment, so the four are one value. What it is *not* is
/// state: it is read off the Editor at the top of each draw, because a tag is
/// drawn in the colours of the moment it is applied.
#[derive(Clone, Copy)]
pub struct Painting<'a> {
    /// Both Annotators' retained spans and their tables, read only over the
    /// range being drawn: the Categories that colour it and the List spans
    /// struck across it.
    pub syntax: &'a std::cell::RefCell<crate::syntax::Syntax>,
    /// The Face the Editor is set in.
    pub face: Face,
    /// The table every colour is read off: the ground's, as the Editor holds
    /// it. Nothing here asks which ground that is.
    pub colours: Colours,
    /// The air the type leaves around a row, as the Editor holds it. Carried
    /// rather than measured here so that [`well_edges`] costs no row measured
    /// again on the keystroke path.
    pub leading: typography::Leading,
    /// How much Focus leaves lit.
    pub focus: Focus,
    /// What Focus lights, for the caret where it now is. Empty with Focus off,
    /// and empty with Focus on when the caret lights nothing.
    pub tiers: &'a [LineTiers],
    /// Where the writer stands, with Live on, and `None` with Live off.
    ///
    /// Live's spans are a function of the caret and of nothing this module
    /// holds, so the place is carried and the spans are asked for at the draw,
    /// over exactly the bytes being drawn ([`live::spans_in`]): a fold is a
    /// judgement about the block the caret is not in, and the caret moves
    /// between two draws of the same bytes.
    pub live: Option<Writer>,
    /// The container the page was last laid out in, and `None` before a first
    /// allocation has given it one.
    ///
    /// Carried so that a marker run seen for the first time is hung on the
    /// keystroke that made it rather than at the next allocation: the tag is
    /// valued where it is made ([`opened_by`]), and [`hang_markers`] hangs it
    /// again whenever the container or the type moves.
    pub page: Option<typography::Column>,
    /// What measures a marker run, for the hang a list item and a quote take.
    pub measure: &'a dyn Measure,
}

/// Where the writer stands: the selection as the buffer holds it, empty for
/// the caret.
///
/// A pair rather than the `Range<usize>` [`live::spans_in`] takes, because
/// [`Painting`] is `Copy` and a range is not; [`Writer::at`] hands the range
/// back at the one place it is wanted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Writer {
    /// The first byte of the selection.
    pub start: usize,
    /// The byte after its last, equal to `start` for a caret.
    pub end: usize,
}

impl Writer {
    /// The writer's place as the engine takes it.
    fn at(self) -> Range<usize> {
        self.start..self.end
    }
}

/// Draws the whole of `document` on `buffer`, which must hold its text.
///
/// The whole Document at once is what opening one costs: the architecture
/// parses and draws whole on open as a cold-start cost inside the 250 ms
/// budget. A keystroke goes through [`retag`] instead.
pub fn apply(buffer: &gtk::TextBuffer, document: &Document, painting: Painting) {
    buffer.remove_all_tags(&buffer.start_iter(), &buffer.end_iter());
    draw(buffer, document, painting, &(0..document.text().len()));
}

/// Draws `lines` again, and leaves every other line of `buffer` alone.
///
/// The lines an [`Edit`] names, or the lines whose Focus tier changed under a
/// caret that moved ([`focus::changed`]), and no others. Every line below an
/// edit carries tags that are still right: GTK moves a tag with the text it is
/// on, so a run that only slid down the Document needs nothing done to it, and
/// the engine hands back only the lines whose runs came out looking different.
///
/// One line each side is drawn with them, because a code block's leading lives
/// on the lines *outside* it ([`well_edges`]): the neighbours carry a half of
/// the gap each, and a block is only in [`Document::spans_in`]'s answer for a
/// range that reaches the block itself. Without the widening, typing in the
/// paragraph above a block would take its `well-foot` off and drop the well.
/// [`Document::line_bytes`] clamps past the last line, so no line count is
/// needed here.
///
/// [`Edit`]: quill_engine::document::Edit
pub fn retag(
    buffer: &gtk::TextBuffer,
    document: &Document,
    painting: Painting,
    lines: &Range<usize>,
) {
    if lines.is_empty() {
        return;
    }
    let lines = lines.start.saturating_sub(1)..lines.end + 1;
    let at = document.line_bytes(lines.start).start..document.line_bytes(lines.end - 1).end;
    let from = iter_at(buffer, document, at.start);
    let to = iter_at(buffer, document, at.end);
    buffer.remove_all_tags(&from, &to);
    draw(buffer, document, painting, &at);
}

/// Takes the colour `was` off the bytes `at` and puts `now` on them instead.
///
/// One frame of a cross-fade. The old one comes off first so that exactly one
/// foreground tag is ever on those bytes: two would leave the tag table's own
/// order to decide which is seen, and that order is the order the tags happened
/// to be first asked for, which is neither the fade's nor anything a reader
/// could predict. Taking one off and putting one on is a decision this module
/// makes rather than one it delegates.
///
/// The interim colours are tags like any other: [`colour`] makes each one on
/// the first frame that asks for it and hands back the same tag every frame
/// after, in this fade and in every later one, so a fade in flight allocates
/// nothing. Nothing but the foreground moves — the cut, the code well and the
/// link's rule stay where the draw put them, because a tier changing is a
/// change of colour and of nothing else.
/// `at` is the buffer's own offsets ([`offsets_of`]) and not the Document's
/// bytes, because a fade is stepped from the frame clock, where there is a
/// widget and no Document: the two are mapped once when the fade is built, out
/// of the same [`iter_at`] every other byte range goes through.
pub fn recolour(buffer: &gtk::TextBuffer, at: &Range<i32>, was: Colour, now: Colour) {
    let from = buffer.iter_at_offset(at.start);
    let to = buffer.iter_at_offset(at.end);
    buffer.remove_tag(&colour(buffer, &was.to_hex(), was.opacity()), &from, &to);
    buffer.apply_tag(&colour(buffer, &now.to_hex(), now.opacity()), &from, &to);
}

/// Replaces foreground colours and Style check marks without touching
/// paragraph or Live properties.
///
/// Syntax answers and toggles change ink only. A structural retag would clear
/// the well's spacing on neighbouring lines and could leave half of a paired
/// gap outside the redraw. Here the existing cut, folds, scale and well tags
/// never come off. The protected-code on/off pixel pair in the Syntax Piece
/// pins that distinction, including the paragraph below an indented well.
///
/// The table walk is O(table tags * ranges), once per idle batch of at most
/// eight answers plus any settled fade ranges, or one whole-page toggle.
/// The painting walk is bounded to those ranges as in [`draw`]. Neither runs
/// on a keystroke; no scan of document paragraphs is used to find a range.
pub fn repaint(
    buffer: &gtk::TextBuffer,
    document: &Document,
    painting: Painting,
    lines: &[Range<usize>],
) {
    let ranges: Vec<_> = lines
        .iter()
        .filter(|lines| !lines.is_empty())
        .map(|lines| {
            document.line_bytes(lines.start).start
                ..document.line_bytes(lines.end.saturating_sub(1)).end
        })
        .collect();
    let offsets: Vec<_> = ranges
        .iter()
        .map(|at| {
            (
                iter_at(buffer, document, at.start),
                iter_at(buffer, document, at.end),
            )
        })
        .collect();
    // The Style check marks come off with the colours and go back on with
    // them: a List switched off answers the switch here, at once and without
    // the matcher running again, and the two tags a still-enabled List's
    // neighbours carry go back exactly where they were.
    buffer.tag_table().foreach(|tag| {
        if tag
            .name()
            .is_some_and(|name| name.starts_with("colour-") || name.starts_with(STYLE_MARK))
        {
            for (from, to) in &offsets {
                buffer.remove_tag(tag, from, to);
            }
        }
    });
    for at in ranges {
        let spans = document.spans_in(&at);
        for run in painted(document, painting, &spans, &at) {
            let from = iter_at(buffer, document, run.at.start);
            let to = iter_at(buffer, document, run.at.end);
            let ink = run.paint.colour;
            buffer.apply_tag(&colour(buffer, &ink.to_hex(), ink.opacity()), &from, &to);
        }
        strike_lists(buffer, document, painting, &at);
    }
}

fn painted(
    document: &Document,
    painting: Painting,
    spans: &[Span],
    at: &Range<usize>,
) -> Vec<annotate::Painted> {
    let syntax = painting.syntax.borrow();
    let tagged = syntax.spans_in(document, at);
    annotate::paint_tagged_in(
        spans,
        &tagged,
        syntax.categories(),
        at,
        painting.tiers,
        painting.focus,
        &painting.colours,
    )
}

/// The offsets `buffer` counts the bytes `at` of `document` in.
///
/// The one crossing between the two ways of naming a place in the text, taken
/// where there is still a Document to take it from. Everything else in this
/// module works in the Document's bytes; a cross-fade cannot, because the frame
/// clock hands its callback a widget and nothing else.
pub fn offsets_of(buffer: &gtk::TextBuffer, document: &Document, at: &Range<usize>) -> Range<i32> {
    iter_at(buffer, document, at.start).offset()..iter_at(buffer, document, at.end).offset()
}

/// Puts every tag the bytes `at` ask for on to `buffer`.
///
/// Several tags land on the same bytes, which is safe here for one reason and
/// only one: no two of them set the same property to different values. The
/// colour, the cut, the ground and the decorations are disjoint sets, so
/// priority never has to decide between them — and priority is what the
/// flattening exists to keep out of the colour, where they *would* collide.
///
/// The one pair that meets is a `~~` strike and a Style check mark over the
/// same phrase: both set `strikethrough` and both set it `true`, so whichever
/// priority picks draws the same line, and the two are told apart on the page
/// by the colour under it — Markup's ink for the `~~` run, the body's for the
/// prose (#356).
///
/// A run is clipped to `at` and a paragraph tag is not, because the two are
/// different kinds of thing: a run draws the bytes it covers, and the caller
/// has taken the tags off exactly those bytes; a paragraph property is read
/// off the whole line and belongs to every line its span touches, including
/// the ones outside `at` that still have it. Applying a tag they already carry
/// is what leaves them as they were.
fn draw(buffer: &gtk::TextBuffer, document: &Document, painting: Painting, at: &Range<usize>) {
    let Painting {
        face,
        colours,
        leading,
        focus,
        tiers,
        live,
        page,
        measure,
        ..
    } = painting;
    // The flattening resolves the Markup mark and the Focus tier into one
    // colour, so the ink is read here rather than off the run's role: with
    // Focus on, most of the page is drawn in a colour no role names.
    let spans = document.spans_in(at);
    for run in painted(document, painting, &spans, at) {
        let from = iter_at(buffer, document, run.at.start);
        let to = iter_at(buffer, document, run.at.end);
        let ink = run.paint.colour;
        buffer.apply_tag(&colour(buffer, &ink.to_hex(), ink.opacity()), &from, &to);
        buffer.apply_tag(
            &cut(buffer, face, run.paint.weight, run.paint.slant),
            &from,
            &to,
        );
        if run.paint.ground == annotate::Ground::Code {
            buffer.apply_tag(&ground(buffer, &colours), &from, &to);
        }
    }
    strike_lists(buffer, document, painting, at);
    for span in &spans {
        if span.at.end <= at.start {
            continue;
        }
        // A paragraph property is not cut at a tier boundary, so it takes the
        // one tier the block it belongs to is in.
        let tier = focus::tier_in(tiers, focus, &span.at);
        match span.mark {
            Mark::Heading(level) => {
                paragraph(buffer, document, &span.at, &heading(buffer, level));
            }
            // Out of focus a code block keeps its glyphs and loses its well —
            // `legacy/app/css/focus.css:42-43` sets the background transparent
            // — so that the dim is one flat grey rather than a stack of lit
            // panels. The same rule the flattening applies to a code span's
            // own ground ([`quill_engine::annotate::Paint`]).
            //
            // Lifted across the whole block rather than left to the lines the
            // caller cleared, because a paragraph tag was put on rows outside
            // them: see [`unparagraph`].
            Mark::CodeBlock => {
                let well = code_ground(buffer, &colours);
                match tier {
                    Tier::Bright => paragraph(buffer, document, &span.at, &well),
                    Tier::Dim => unparagraph(buffer, document, &span.at, &well),
                }
            }
            Mark::Strikethrough => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&struck(buffer), &from, &to);
            }
            // The rule goes under the URL and nothing else: not the words that
            // stand for it, and not the brackets around them. The Design
            // oracle draws it that way (#198), and the mark says exactly which
            // bytes are the destination.
            //
            // Out of focus it goes, as the well does:
            // `legacy/app/css/focus.css:41` takes the rule's colour to
            // `transparent` on a dimmed URL, so a dim link is grey words and
            // nothing under them.
            Mark::Url if tier == Tier::Bright => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&underline(buffer, &colours), &from, &to);
            }
            _ => {}
        }
    }
    for run in marker_runs(document, &spans, at) {
        let text = &document.text()[run.clone()];
        let tag = opened_by(buffer, text, || {
            page.map(|column| {
                hung_under(
                    measure.advance(text),
                    signed(column.side),
                    indent_ceiling(column),
                )
            })
        });
        paragraph(buffer, document, &run, &tag);
    }
    if let Some(writer) = live {
        fold(buffer, document, painting, &spans, writer, at);
    }
    // Last of the three, because it asks the buffer which of a block's rows
    // are folded away and the fold above is what folds them. Unconditional on
    // the tier: dimming a block lifts its well and must not move a row.
    for span in &spans {
        if span.at.end > at.start && matches!(span.mark, Mark::CodeBlock) {
            well_edges(buffer, document, span, leading);
        }
    }
}

/// Gives the well of the code block `span` back the leading its rows lost.
///
/// The view holds the whole gap between two paragraphs above them and nothing
/// below (`Editor::restyle`, #279), which leaves every glyph row where it was
/// and lifts the well: GTK paints a paragraph background over the whole line
/// box, leading included, so both of its edges rise by the half that moved.
/// Four applications of two tags put them back — the lower half under the line
/// above the block and under the block's last standing row, the upper half
/// alone above the block's first standing row and above the line after the
/// block. Each of those two gaps still sums to the air the type leaves, so no
/// glyph moves and the well's rectangle is the one it was.
///
/// Which rows stand is asked of the buffer rather than read off `span`. Under
/// Live a fence is folded over its whole line and its delimiter, and GTK gives
/// such a line no height, no leading and no background at all, so a tag on it
/// would be inert; and the block the caret has just entered has had its fold
/// taken off by the retag, which only the buffer knows. The two neighbours are
/// counted from the block's own first and last line for the same reason: a
/// folded fence between them and the block carries nothing.
///
/// The two applications above the block are made together or not at all, and so
/// are the two below it: each pair takes a half off one row and puts it on the
/// other, and half a pair would move every row under it rather than only the
/// well's edge. A block on the buffer's first line therefore takes neither of
/// the upper pair, and its well's top edge sits the lower half higher — the one
/// place the edge is not where it was, and no judged state has one.
fn well_edges(
    buffer: &gtk::TextBuffer,
    document: &Document,
    span: &Span,
    leading: typography::Leading,
) {
    let (block_start, block_end) = paragraph_lines(buffer, document, &span.at);
    let (first, last) = (block_start.line(), block_end.line());
    let folded = folded(buffer);
    let standing = |line: i32| !line_start(buffer, line).has_tag(&folded);
    let head = (first..=last).find(|line| standing(*line));
    let foot = (first..=last).rev().find(|line| standing(*line));
    let (Some(head), Some(foot)) = (head, foot) else {
        return;
    };
    let above = well_head(buffer, leading);
    let below = well_foot(buffer, leading);
    if first > 0 {
        apply_line(buffer, first - 1, &below);
        apply_line(buffer, head, &above);
    }
    if last + 1 < buffer.line_count() {
        apply_line(buffer, foot, &below);
        apply_line(buffer, last + 1, &above);
    }
}

/// Puts `tag` on the whole of `line`, its line break with it.
///
/// The break is in the range because the neighbours a well's leading falls on
/// are usually blank lines — Markdown's own separator — and a blank line's text
/// is empty: a range that stopped at the line's end would be empty too, and
/// `gtk_text_buffer_apply_tag` over an empty range applies nothing. A paragraph
/// property is read off the start of the paragraph either way, and the range
/// ends where the next line begins, so nothing on it is covered.
fn apply_line(buffer: &gtk::TextBuffer, line: i32, tag: &gtk::TextTag) {
    let (from, to) = whole_line(&line_start(buffer, line));
    buffer.apply_tag(tag, &from, &to);
}

/// The first byte of `line`, in GTK's own units.
fn line_start(buffer: &gtk::TextBuffer, line: i32) -> gtk::TextIter {
    let mut at = buffer.start_iter();
    at.set_line(line);
    at
}

/// Puts Live's fold and Live's ladder on the bytes `at`, for a writer at
/// `writer`.
///
/// The third loop, and it is its own function because it is its own Annotator:
/// the Live spans are asked for over the blocks these bytes touch and mapped
/// one to one on to a tag, exactly as the marks above are. A furnished
/// marker's cells are folded and left empty here, and what stands in them is
/// painted in the Editor's snapshot (`Editor::draw_furniture`); the one
/// exception is a link's accent rule, which is a tag because only Pango knows
/// where a link's words are ([`link_rule`]).
fn fold(
    buffer: &gtk::TextBuffer,
    document: &Document,
    painting: Painting,
    spans: &[Span],
    writer: Writer,
    at: &Range<usize>,
) {
    let colours = &painting.colours;
    let paper = colours.colour(Role::Paper).to_hex();
    let well = code_well(colours);
    // The ground a folded marker stands on, which is what it is drawn in: the
    // well under a fence or an info string, and the page under everything else.
    let ground = |at: &Range<usize>| {
        let code = spans.iter().any(|span| {
            matches!(span.mark, Mark::CodeBlock | Mark::Code)
                && span.at.start <= at.start
                && at.end <= span.at.end
        });
        if code { well.clone() } else { paper.clone() }
    };
    for span in live::spans_in(document, &writer.at(), at) {
        let from = iter_at(buffer, document, span.at.start);
        let to = iter_at(buffer, document, span.at.end);
        match span.look {
            LiveLook::Folded(Fold::Hanging) => {
                buffer.apply_tag(&hidden(buffer, &ground(&span.at)), &from, &to);
            }
            LiveLook::Folded(Fold::Inline) => {
                buffer.apply_tag(&folded(buffer), &from, &to);
            }
            LiveLook::Scaled(level) => {
                if let Some(scaled) = scaled(buffer, level) {
                    buffer.apply_tag(&scaled, &from, &to);
                }
            }
            // A link's words are the writer's own and are not folded: what
            // stands over them is the accent rule, and it is the one piece of
            // furniture drawn from here rather than in the Editor's snapshot
            // ([`link_rule`] says why).
            LiveLook::Furniture(Furniture::Link { .. }) => {
                buffer.apply_tag(&link_rule(buffer, colours), &from, &to);
            }
            // A fence takes its whole row with it, its line break included:
            // nothing hangs in a fence's cells and nothing stands in them, so
            // folding it to its own ground would leave a blank row of Well
            // where #263's story 28 asks for a code block without its fences.
            // A line whose every byte and whose paragraph delimiter are
            // invisible is a line GTK gives no height at all, which is what
            // takes the row off the page; the code rows keep their Well,
            // because the ground is a paragraph property of their own lines.
            LiveLook::Furniture(Furniture::Fence) => {
                let (from, to) = whole_line(&from);
                buffer.apply_tag(&folded(buffer), &from, &to);
            }
            // Every other piece of furniture stands where its marker did, so
            // the marker goes off the page and its cells stay the width they
            // were ([`hidden`]); the Editor paints what stands in them.
            LiveLook::Furniture(furniture) => {
                if furniture.folds() {
                    buffer.apply_tag(&hidden(buffer, &ground(&span.at)), &from, &to);
                }
            }
            // A quotation's `>` is the one marker Live leaves on the page, and
            // this is what keeps it from reading as dimmed: the body's own ink
            // over the marker ink the Markup Annotator rested it at, at the
            // tier the block it stands in is lit to, so a quote out of focus
            // still dims with the prose around it.
            LiveLook::Quote => {
                let tier = focus::tier_in(painting.tiers, painting.focus, &span.at);
                let ink = annotate::colour(Ink::Prose, tier, colours);
                buffer.apply_tag(&quoted(buffer, &ink), &from, &to);
            }
        }
    }
}

/// Puts a paragraph tag on every line the bytes `at` touch.
///
/// The property belongs to the line, not to the span: a paragraph property is
/// read off the tags at the start of the paragraph, and a heading's markers are
/// not inside the text they govern. A heading touches one line; a fenced block
/// touches all of its own, which is what puts its ground under every row rather
/// than only the first.
///
/// A byte range rather than a [`Span`] because a marker's hang is asked for by
/// the run its line opens with, which is a range of two spans' making as often
/// as one's.
fn paragraph(buffer: &gtk::TextBuffer, document: &Document, at: &Range<usize>, tag: &gtk::TextTag) {
    let (start, end) = paragraph_lines(buffer, document, at);
    buffer.apply_tag(tag, &start, &end);
}

/// The byte the line holding `at` opens at.
fn line_opens_at(document: &Document, at: usize) -> usize {
    document.text()[..at]
        .rfind('\n')
        .map_or(0, |newline| newline + 1)
}

/// The marker run of every line in `spans` that opens with one: from the line's
/// own first byte to the end of the last marker standing on it.
///
/// One run per line, and never more, which is the whole reason this is gathered
/// rather than applied a span at a time. A line can carry two markers — `> - a`
/// is a quote's and a bullet's — and hanging both would be two tags setting one
/// property, left to the tag table's order to choose between. The furthest of
/// them is the one that puts the wrapped rows under the item's first word.
///
/// It reaches back to the line's first byte because that is where the hang is
/// measured from: the engine already cuts a list marker's span that way, "so
/// that its width is the whole distance from the line's start to the item's
/// first word" ([`Mark::BulletMarker`]). A quote's `>` is one per line and
/// opens its own, so for a quote the run and the mark's own span are the same
/// bytes — and a wrapped quote is given the indent and nothing else, no second
/// `>` of its own, which is what the Design oracle draws.
///
/// **A nested item and a line carrying two markers are unmeasured.** No capture
/// holds either, so neither is the oracle's rule: extending the lone item's
/// rule to them is the owner's decision, recorded in `docs/design.md` row What
/// hangs, and a capture may still overrule it.
///
/// Spans arrive in the Document's order, so the line being gathered is always
/// the last one and no lookup is needed.
fn marker_runs(document: &Document, spans: &[Span], at: &Range<usize>) -> Vec<Range<usize>> {
    let mut runs: Vec<Range<usize>> = Vec::new();
    for span in spans {
        if span.at.end <= at.start
            || !matches!(
                span.mark,
                Mark::BulletMarker | Mark::OrderedMarker | Mark::QuoteMarker
            )
        {
            continue;
        }
        let start = line_opens_at(document, span.at.start);
        match runs.last_mut() {
            Some(last) if last.start == start => last.end = last.end.max(span.at.end),
            _ => runs.push(start..span.at.end),
        }
    }
    runs
}

/// Takes a paragraph tag off every line `span` touches.
///
/// The other half of [`paragraph`], and it exists because the two are not
/// symmetric anywhere else: the caller takes the tags off the lines it is about
/// to redraw, and a paragraph tag was put on lines outside them. A code block
/// the caret has just left is dim on all of its rows, and only some of them are
/// in the retag — so the well has to be lifted from the block rather than left
/// to the lines, or the rows nobody redrew keep a fragment of it.
fn unparagraph(
    buffer: &gtk::TextBuffer,
    document: &Document,
    at: &Range<usize>,
    tag: &gtk::TextTag,
) {
    let (start, end) = paragraph_lines(buffer, document, at);
    buffer.remove_tag(tag, &start, &end);
}

/// The whole lines the bytes `at` touch, as the pair of iterators both halves
/// use.
fn paragraph_lines(
    buffer: &gtk::TextBuffer,
    document: &Document,
    at: &Range<usize>,
) -> (gtk::TextIter, gtk::TextIter) {
    let mut start = buffer.start_iter();
    start.set_line(iter_at(buffer, document, at.start).line());
    let mut end = buffer.start_iter();
    end.set_line(iter_at(buffer, document, at.end).line());
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    (start, end)
}

/// The code ground in `colours`, flattened onto that ground's paper.
///
/// [`Role::CodeBg`] is a wash — 4.5 % black over the light paper, 6 % white
/// over the dark — and a `GtkTextTag` background is a ground rather than a
/// wash: nothing is ever drawn under it. So it is composited here, once,
/// rather than handed to GTK translucent to be blended against whatever the
/// widget happens to have behind the line.
fn code_well(colours: &Colours) -> String {
    let wash = colours.colour(Role::CodeBg);
    Colour::over(wash, colours.colour(Role::Paper), wash.alpha).to_hex()
}

/// Sets the Italic tags to `face`'s Italic, for text already tagged.
///
/// Called when the type changes, for the reason [`hang_markers`] is: the
/// Editor does not re-derive its spans when the writer picks another Face, so
/// the tags the buffer is already carrying have to be moved to it.
pub fn set_face(buffer: &gtk::TextBuffer, face: Face) {
    for weight in [Weight::Regular, Weight::Bold] {
        cut(buffer, face, weight, Slant::Italic);
    }
}

/// The iterator at `offset` bytes into `document`, which `buffer` holds.
///
/// Reached by line and byte index within the line, never by counting
/// characters from the top of the buffer: that is O(document) per lookup
/// against one lookup per span, and `docs/architecture.md` § Text model forbids
/// it for exactly that reason. The app's one byte-to-iter mapping lives here
/// because the tag table is its only heavy user; `--caret` shares it so that
/// there is not a second one to drift.
pub fn iter_at(buffer: &gtk::TextBuffer, document: &Document, offset: usize) -> gtk::TextIter {
    let place = document.place(offset);
    let mut iter = buffer.start_iter();
    iter.set_line(gtk_index(place.line));
    iter.set_line_index(gtk_index(place.index));
    iter
}

/// The byte `at` names, in the offsets the engine counts in.
///
/// `GtkTextIter` already counts bytes within a line, which is the half of the
/// mapping GTK gives away for nothing; the Document's line table gives the
/// other half. This is [`iter_at`] read backwards, and it must be called while
/// the two still hold the same text. It lives beside it so that the app has one
/// mapping in each direction and no second copy to drift: the splice reads a
/// keystroke's offsets with it, and Focus reads where the caret now is.
pub fn offset_of(document: &Document, at: &gtk::TextIter) -> usize {
    let line = usize::try_from(at.line()).unwrap_or(0);
    let index = usize::try_from(at.line_index()).unwrap_or(0);
    document.line_bytes(line).start + index
}

/// `count` as the `i32` GTK counts lines and byte indices in.
///
/// A Document long enough to overflow this is 2 GB of prose; saturating rather
/// than wrapping means the worst an impossible Document does is style its last
/// line twice.
fn gtk_index(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use quill_engine::annotate::Ink;
    use quill_engine::theme::Scheme;

    use super::*;

    // These test the arithmetic the hang is built on. Nothing here makes a
    // `GtkTextTag`: `tools/gate check` runs `cargo test` with no display
    // attached, and the tags themselves are judged from a shot instead.

    /// The colour `ink` is drawn in on `scheme` with Focus off, as GTK parses
    /// one: what a run carries by the time it reaches this module.
    ///
    /// Through the flattening rather than a table of its own, because that is
    /// where the module now reads a colour from, and a second mapping here
    /// would be one that could disagree with the drawn page.
    fn hex(scheme: Scheme, ink: Ink) -> String {
        annotate::colour(
            ink,
            Tier::Bright,
            &crate::ground::Ground::of(scheme).colours,
        )
        .to_hex()
    }

    /// The judged step: the ladder's default, whose em is 21.33 logical
    /// pixels and whose cell is therefore 12.798.
    const STEP: u32 = 5;

    /// The window the judged states are shot in, in logical pixels.
    const VIEW: u32 = 1440;

    /// The container `face` at `step` lays out in that window.
    fn container(face: Face, step: u32) -> typography::Column {
        typography::column(VIEW, typography::cell(face, step))
    }

    /// The marker runs a level 1 to 6 heading opens with, as the layout may
    /// advance them: `hinted` to a whole cell each, or off the true cell.
    ///
    /// Both are real, which is the reason [`hung`] is handed a width rather
    /// than working one out. A `--deterministic` launch pins metric hinting on
    /// and a writer's own launch takes the desktop's answer, so at the default
    /// step the same six runs advance 13 px a cell in a judged shot and 12.798
    /// in the app — and `Editor::marker_advance` measures which it is.
    fn advances(hinted: bool) -> [i32; 6] {
        let cell = typography::cell(Face::Duo, STEP);
        std::array::from_fn(|level| {
            let cells = level as f64 + 2.0;
            pixels(if hinted {
                cell.round() * cells
            } else {
                cell * cells
            })
        })
    }

    #[test]
    fn the_first_row_hangs_by_exactly_what_the_wrapped_rows_get_back() {
        // Pango puts the first row at the tag's own left margin and every row
        // after it at margin + |indent|, which must be the prose's margin —
        // and puts the first row's own words at margin + the advance, which
        // must be the same. Round 10 was lost on the second of those: the
        // hang was `level + 1` of a cell the glyphs were not being advanced
        // by, so `# ` and `## ` started their words on one column and the
        // deeper four on another, two device pixels over. Whatever the layout
        // advances, the pair has to give back exactly what it took, so both
        // ladders are held to it here.
        let side = signed(container(Face::Duo, STEP).side);
        for hinted in [true, false] {
            for (level, advance) in (1..=6u8).zip(advances(hinted)) {
                let hang = hung(advance, side);
                assert_eq!(
                    hang.left + advance,
                    side,
                    "hinted {hinted}: a level {level} heading's words start off the body column"
                );
                assert_eq!(
                    wrapped(hang),
                    side,
                    "hinted {hinted}: a wrapped row of a level {level} heading must land on the prose margin"
                );
            }
        }
    }

    #[test]
    fn the_code_ground_runs_past_both_edges_of_the_measure() {
        // The oracle's ±0.7em, which at the default step's 21.33 px em is 15
        // px on each side.
        assert_eq!(well(STEP), 15);
        let side = 240;
        let edge = well(STEP).min(side);
        assert!(
            edge > 0 && side - edge < side,
            "the block's box has to start left of the prose for its ground to"
        );
        assert_eq!(
            (side - edge) + edge,
            side,
            "and the indent has to put the code itself back on the prose's edge"
        );
    }

    #[test]
    fn the_well_grows_with_the_type_it_is_set_in() {
        // Never back down the ladder, and plainly bigger across it. Not
        // strictly bigger at every rung: a well is under one cell and a fifth,
        // and two adjacent ems of the ladder's small end — 15.25 and 16.17 —
        // round to the same whole pixel.
        let ladder = quill_engine::settings::type_steps();
        for step in ladder.clone().skip(1) {
            assert!(
                well(step - 1) <= well(step),
                "a well is 0.7 of an em, so it must never shrink as the type grows: \
                 step {} wells by {} and step {step} by {}",
                step - 1,
                well(step - 1),
                well(step)
            );
        }
        assert!(
            well(*ladder.end()) > 4 * well(*ladder.start()),
            "the ladder's top em is more than four times its bottom one"
        );
    }

    #[test]
    fn a_window_too_narrow_to_hang_the_marker_keeps_its_gutter() {
        // The container barely asks for this — [`typography::column`] holds
        // the gutter at seven cells and gives up measure instead, so a marker
        // run has the whole gutter to hang in. The clamp is the guard for the
        // window that is narrower than the gutter itself, and it must leave
        // the two rows agreeing even there. The heading's words go off the
        // column when it bites, which is the right way round: a window that
        // narrow has no column left to speak of.
        let side = 4;
        let width = advances(true)[5];
        assert!(
            width > side,
            "this is the narrow case, or it proves nothing"
        );
        let hang = hung(width, side);
        assert_eq!(
            hang.left, 0,
            "the left margin of a tag cannot go below zero"
        );
        assert_eq!(
            wrapped(hang),
            side,
            "the rows still agree: what the first row gives up, the rest get back"
        );
    }

    /// The runs a bullet, a three-digit ordinal and a quote open with, and how
    /// many cells past the body column each anchors its own wrapped rows.
    ///
    /// The Design oracle's ladder for a wrapped item, measured off
    /// `mac-native-19-{light,dark}-wrapped-markers` and their `-h6` pair:
    /// bullet and quote continuations start 2 × 25.6 device pixels past the
    /// body and the ordered item's 5 × 25.6, on both grounds and under the
    /// widest heading alike (`ref/ia/mac-native/CAPTURE-2026-09-09.md` § #241).
    const RUNS: [(&str, f64); 3] = [("- ", 2.0), ("123. ", 5.0), ("> ", 2.0)];

    /// [`RUNS`]' advances, as the layout may advance them, the two ways
    /// [`advances`] gives the heading ladder's.
    fn run_advances(hinted: bool) -> [i32; 3] {
        let cell = typography::cell(Face::Duo, STEP);
        RUNS.map(|(_, cells)| {
            pixels(if hinted {
                cell.round() * cells
            } else {
                cell * cells
            })
        })
    }

    /// Where the rows under a [`Hang`]'s first begin.
    ///
    /// The reading of a negative indent, which Pango applies to every row but
    /// the first — and the one thing every hang here has to get right, so it is
    /// written once and every assertion below goes through it.
    const fn wrapped(hang: Hang) -> i32 {
        hang.left - hang.indent
    }

    /// A Document holding `text`.
    fn document(text: &str) -> Document {
        let mut document = Document::untitled();
        document.insert(0, text);
        document
    }

    #[test]
    fn a_markers_wrapped_rows_start_under_the_items_own_first_word() {
        // The heading's pair read the other way up. A heading takes its marker
        // out of the gutter so that its words land on the body column; a list
        // item and a quote leave their marker *on* that column and take the
        // run's own advance off every row under it, so the item's second row
        // starts where its first word did and not where its marker did. Held
        // to both ladders for the reason the heading's is: what the first row
        // keeps, the rest give up, exactly.
        let column = container(Face::Duo, STEP);
        let side = signed(column.side);
        let ceiling = indent_ceiling(column);
        for hinted in [true, false] {
            for ((run, _), advance) in RUNS.iter().zip(run_advances(hinted)) {
                let hang = hung_under(advance, side, ceiling);
                assert_eq!(
                    hang.left, side,
                    "hinted {hinted}: `{run}` must open on the body column"
                );
                assert_eq!(
                    wrapped(hang),
                    side + advance,
                    "hinted {hinted}: `{run}`'s wrapped rows start under its own first word"
                );
            }
        }
    }

    #[test]
    fn an_ordinal_hangs_its_own_five_cells_where_a_bullet_hangs_two() {
        // The hang is the paragraph's own marker run and not a constant, which
        // is the whole reason the oracle's ordered item was captured with a
        // three-digit marker: `1. ` and `123. ` are the same kind of mark and
        // not the same width, so a hang counted per kind would put the two
        // items' wrapped rows on one column.
        let column = container(Face::Duo, STEP);
        let side = signed(column.side);
        let ceiling = indent_ceiling(column);
        let cell = typography::cell(Face::Duo, STEP);
        let [bullet, ordinal, quote] =
            run_advances(false).map(|advance| wrapped(hung_under(advance, side, ceiling)) - side);
        assert_eq!(bullet, pixels(2.0 * cell), "`- ` hangs its own two cells");
        assert_eq!(quote, pixels(2.0 * cell), "`> ` hangs its own two cells");
        assert_eq!(
            ordinal,
            pixels(5.0 * cell),
            "`123. ` hangs its own five cells"
        );
    }

    #[test]
    fn a_marker_run_wider_than_the_measure_leaves_the_wrapped_rows_a_cell() {
        // The guard on the other edge from
        // [`a_window_too_narrow_to_hang_the_marker_keeps_its_gutter`]. A
        // heading spends the gutter and cannot spend past the view's left
        // edge; a marker spends the measure, and a run as wide as the measure
        // would leave its own wrapped rows nowhere to wrap in. So the last
        // cell is held back and a deeply indented item carries on in it, which
        // is the same trade the narrow window makes: the words go somewhere
        // wrong rather than nowhere at all.
        let column = container(Face::Duo, STEP);
        let side = signed(column.side);
        let ceiling = indent_ceiling(column);
        let width = signed(column.width);
        assert!(ceiling < width, "a cell of the measure is held back");
        let hang = hung_under(width * 2, side, ceiling);
        assert_eq!(hang.left, side, "the marker still opens on the body column");
        assert_eq!(
            wrapped(hang),
            side + ceiling,
            "the wrapped rows stop at what the measure can still give them"
        );
    }

    #[test]
    fn a_wrapped_quote_is_given_the_indent_and_no_second_marker() {
        // The oracle's third row: a wrapped quote carries no `>` of its own on
        // the rows below the first. Nothing here could draw one — the hang is a
        // paragraph property and the only run in the line is the `> ` the
        // writer typed — and the run gathered is exactly that.
        //
        // The second line is why the runs are gathered rather than applied a
        // mark at a time: `> - ` is a quote's marker and a bullet's, and
        // hanging both would be two tags setting one property. One run a line,
        // reaching to the further of them, puts the wrapped rows under the
        // item's first word either way.
        let text = "> A quoted paragraph long enough to wrap.\n\n> - A quoted item\n";
        let document = document(text);
        let at = 0..document.text().len();
        let spans = document.spans_in(&at);
        let runs: Vec<&str> = marker_runs(&document, &spans, &at)
            .into_iter()
            .map(|run| &document.text()[run])
            .collect();
        assert_eq!(
            runs,
            ["> ", "> - "],
            "one run a line, reaching to the last marker standing on it"
        );
    }

    #[test]
    fn a_colour_at_full_alpha_is_the_colour_itself() {
        let mark = hex(Scheme::Light, Ink::Marker);
        let opaque = shaded(&mark, Look::OPAQUE);
        assert!(
            (opaque.alpha() - 1.0).abs() < f32::EPSILON,
            "the alpha half of the key has to reach the colour: {opaque:?}"
        );
        let dimmed = shaded(&mark, Look::OPAQUE / 2);
        assert!(
            dimmed.alpha() < opaque.alpha() && dimmed.red() == opaque.red(),
            "a dimmed row is the same colour, less of it: {dimmed:?}"
        );
    }

    /// The three inks are three roles of the table, on whichever ground is
    /// asked for.
    ///
    /// Two grounds rather than one: an ink that came out the same on both would
    /// be a role read off the wrong row. What is *not* asserted here any more
    /// is that the marker differs from the prose — the Design oracle rests
    /// every mark kind at the body's ink (#198), so on the built-in grounds the
    /// two are one colour and the mapping is what keeps them separable. The
    /// values themselves are the engine's to assert (`theme.rs` § `ORACLE`);
    /// what is checked here is that a run reaches this module carrying the
    /// right three roles, read off the ground it was handed.
    #[test]
    fn the_three_inks_are_three_roles_of_the_ground() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = crate::ground::Ground::of(scheme).colours;
            assert_eq!(
                [
                    hex(scheme, Ink::Prose),
                    hex(scheme, Ink::Marker),
                    hex(scheme, Ink::Link)
                ],
                [
                    colours.colour(Role::Ink).to_hex(),
                    colours.colour(Role::Mark).to_hex(),
                    colours.colour(Role::Link).to_hex()
                ],
                "{scheme:?}"
            );
        }
        assert_ne!(
            hex(Scheme::Light, Ink::Prose),
            hex(Scheme::Dark, Ink::Prose),
            "the two grounds are not the same ink"
        );
        for scheme in [Scheme::Light, Scheme::Dark] {
            assert_eq!(
                hex(scheme, Ink::Prose),
                hex(scheme, Ink::Marker),
                "{scheme:?} rests its markers at the prose's ink"
            );
            assert_ne!(
                hex(scheme, Ink::Prose),
                hex(scheme, Ink::Link),
                "{scheme:?} quiets a link's plumbing below the prose"
            );
        }
    }

    /// The code well is opaque by the time GTK sees it, and it is a different
    /// opaque colour on each ground.
    ///
    /// [`Role::CodeBg`] is a wash and a `GtkTextTag` background is not, so the
    /// flattening in [`code_well`] is the whole of the conversion: a colour
    /// still carrying an alpha here would be a well GTK blends against
    /// whatever is behind the line rather than against the page.
    ///
    /// The two greys are the engine's to pin — `theme.rs`'s flattening test
    /// states them, which is where a hand that edits the wash fails — so what
    /// is asserted here is the part that is this module's: that a wash goes in
    /// and something that is neither the wash nor the page comes out.
    #[test]
    fn the_code_well_is_flattened_onto_the_ground_it_is_drawn_on() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = crate::ground::Ground::of(scheme).colours;
            let well = code_well(&colours);
            let wash = colours.colour(Role::CodeBg);
            assert!(wash.alpha < 1.0, "{scheme:?} code ground is not a wash");
            assert!(
                gdk::RGBA::parse(&well).is_ok_and(|parsed| parsed.alpha() == 1.0),
                "{scheme:?} well reaches GTK still translucent: {well}"
            );
            assert_ne!(
                well,
                colours.colour(Role::Paper).to_hex(),
                "{scheme:?} well is the page: nothing would read as code"
            );
        }
        assert_ne!(
            code_well(&crate::ground::Ground::of(Scheme::Light).colours),
            code_well(&crate::ground::Ground::of(Scheme::Dark).colours)
        );
    }

    // The invariant the whole Markup rests on: a closing marker restyles the
    // text before it, and if any of that changed a glyph's advance the line
    // would jolt under the writer's hands as they typed the last asterisk.
    // Measured in the Faces themselves, at the two weights and the two cuts the
    // tag table asks for, on the passage the Piece is judged on.
    #[test]
    fn no_face_moves_a_glyph_across_the_weights_and_cuts_the_tags_ask_for() {
        let context = faces();
        let passage = std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo");
        for face in [Face::Duo, Face::Quattro, Face::Mono] {
            let prose = advance(&context, face, REGULAR, Slant::Upright, &passage);
            assert!(prose > 0, "{face:?} measured nothing at all");
            for (weight, slant) in [
                (BOLD, Slant::Upright),
                (REGULAR, Slant::Italic),
                (BOLD, Slant::Italic),
            ] {
                assert_eq!(
                    advance(&context, face, weight, slant, &passage),
                    prose,
                    "{face:?} at {weight} {slant:?} sets the passage to another width"
                );
            }
        }
    }

    // The passage above is prose, and prose is not where iA left most of the
    // advance varying. Five of the six Faces carried a `wght` delta on the
    // advance of some glyph, and `tools/fontbuild.py` zeroes every one; #95
    // asked for the pin having found the one pair prose reaches, Quattro
    // Italic's `t` and `f`, and its comment records the other four Faces. The
    // rest of what moved is punctuation and marks the passage happens not to
    // use, so a glyph of each is measured here — a sample, not the whole set:
    // the dieresis composites and the Greek `.case` glyphs have no plain
    // character to type. Across the weights only, one cut at a time: what an
    // italic sets a glyph to is the Face's business, and the invariant is that
    // setting a run bold leaves the run the width it was.
    #[test]
    fn no_face_moves_the_varied_glyphs_a_writer_types_when_a_run_goes_bold() {
        let context = faces();
        for face in [Face::Duo, Face::Quattro, Face::Mono] {
            for slant in [Slant::Upright, Slant::Italic] {
                let ink = advance(&context, face, REGULAR, slant, MOVERS);
                assert!(ink > 0, "{face:?} {slant:?} measured nothing at all");
                assert_eq!(
                    advance(&context, face, BOLD, slant, MOVERS),
                    ink,
                    "{face:?} {slant:?} sets these glyphs wider at bold"
                );
            }
        }
    }

    /// A glyph iA varied on `wght` from each of the five Faces that varied one,
    /// and every one of them a writer can type: Quattro Italic's narrow
    /// letters, Mono's `j` and per-cent signs, the ellipsis, the exclamation
    /// mark, the dieresis, and the two Cyrillic letters Quattro and Mono move.
    const MOVERS: &str = "jf t %‰©…!¨ юј ťțţ";

    /// The Faces loaded into a Pango context of their own. `cargo test` runs
    /// with no display and GTK's contexts all come off one, so the measuring
    /// tests take Pango's own font map instead.
    fn faces() -> pango::Context {
        crate::fonts::load_private(&quill_engine::data::fonts())
            .expect("the Faces are in the checkout");
        pangocairo::FontMap::default().create_context()
    }

    /// The width of `passage` set in `face` at `weight` and `slant`, in Pango
    /// units, from the same description the Editor builds.
    fn advance(
        context: &pango::Context,
        face: Face,
        weight: i32,
        slant: Slant,
        passage: &str,
    ) -> i32 {
        let mut font = pango::FontDescription::new();
        font.set_family(match slant {
            Slant::Upright => face.family(),
            Slant::Italic => face.italic_family(),
        });
        font.set_style(pango::Style::Normal);
        font.set_weight(pango::Weight::Normal);
        font.set_variations(Some(&format!("wght={weight}")));
        font.set_absolute_size(f64::from(SIZE) * f64::from(pango::SCALE));
        let layout = pango::Layout::new(context);
        layout.set_font_description(Some(&font));
        layout.set_text(passage);
        layout.size().0
    }

    /// The size the measurements are taken at, which is the one the arithmetic
    /// above is written for.
    const SIZE: u32 = 20;
}
