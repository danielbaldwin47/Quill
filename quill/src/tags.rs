//! The tag table: one `GtkTextTag` per look a span asks for, grown as it is.
//!
//! `docs/architecture.md` § Annotators and the keystroke path fixes the shape.
//! Overlapping tags override a property by priority rather than blending, so
//! the table holds one tag per distinct `(colour, alpha)` and one per
//! `(weight, slant)` rather than one per kind of mark, created on first use and
//! never removed. The table is GTK's own, keyed by name, so the Editor keeps no
//! second copy of it to fall out of step.
//!
//! Paragraph tags are the third row and the one this ticket exists for:
//! `heading-1` to `heading-6` hang a heading's `#` markers out into the left
//! margin so the first word sits on the prose's edge. That is a step past the
//! Parity oracle, which could not do it — `legacy/app/css/markup.css` says why:
//! a `<textarea>` takes no per-line horizontal shift, so the web app bought the
//! same calm with contrast instead of position. ADR 0004 removed the mirror and
//! with it the constraint.

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use quill_engine::annotate::{self, Ground, Ink, Look, Mark, Slant, Span, Weight};
use quill_engine::document::Document;
use quill_engine::settings::Face;
use quill_engine::typography;

use crate::editor::{INK, INK_WEIGHT};

/// The marker grey: `--mark` of `legacy/app/css/theme.css`, 4.0:1 on paper.
///
/// A constant here beside the Editor's `PAPER` and `INK` for the same reason
/// they are constants: there is one theme until the Dark & light ticket moves
/// all three into the palette table.
const MARK: &str = "#7a7a7a";

/// The link colour: `--link` of `legacy/app/css/theme.css`, 4.6:1 on paper.
const LINK: &str = "#0b7cba";

/// The code ground: `--code-bg` of `legacy/app/css/theme.css`, which is 4.5%
/// black over the paper and lands on `#eeeeee`.
///
/// Flat rather than translucent because it is a ground, not a wash: nothing is
/// ever drawn under it.
const CODE_GROUND: &str = "#eeeeee";

/// How opaque a link's underline is: `color-mix(… var(--md-link) 35%,
/// transparent)` of `legacy/app/css/markup.css`.
///
/// A hairline the eye reads as belonging to the words above it rather than as a
/// second mark competing with them.
const UNDERLINE_ALPHA: u8 = 89;

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

/// How many cells of margin a heading of `level` hangs by.
///
/// Its `#`s and the one space after them: `# ` is two cells, `## ` is three.
/// A writer who types more than one space after the marker is left slightly
/// out; the cost of following the exact marker width is a paragraph tag per
/// width rather than per level, and iA Writer hangs by the level too.
fn marker_cells(level: u8) -> f64 {
    f64::from(level) + 1.0
}

/// The pixels a heading of `level` hangs by, in `face` at `size`.
///
/// The Faces are cut to one cell grid, so a marker's width is its cell count
/// times the advance [`typography::cell`] already measures the 64-character
/// measure from. Rounded once, here, so the hang and the measure are counted
/// off the same number and a heading cannot land half a pixel from the prose.
fn hang(face: Face, size: u32, level: u8) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a marker is a handful of cells; the measure itself is rounded the same way"
    )]
    let width = (typography::cell(face, size) * marker_cells(level)).round() as i32;
    width
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
fn ground(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "ground-code", |tag| {
        tag.set_background(Some(CODE_GROUND));
    })
}

/// The tag that underlines a link's words.
///
/// A decoration, layered over the runs rather than resolved into them:
/// underline and colour are different properties, so the overlap is safe
/// (`docs/architecture.md` § Annotators).
fn underline(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "decoration-underline", |tag| {
        tag.set_underline(pango::Underline::Single);
        tag.set_underline_rgba(Some(&shaded(LINK, UNDERLINE_ALPHA)));
    })
}

/// The tag that strikes struck text through.
fn struck(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "decoration-strike", |tag| {
        tag.set_strikethrough(true);
    })
}

/// The paragraph tag for a heading of `level`.
///
/// Made with nothing set on it, and hung by [`hang_headings`] instead: a
/// `GtkTextTag` applies only the properties whose `-set` flag is on, so a tag
/// that has not been hung yet changes no margin, and one that has is hung for
/// every line already carrying it.
fn heading(buffer: &gtk::TextBuffer, level: u8) -> gtk::TextTag {
    tag(buffer, &format!("heading-{level}"), |_| {})
}

/// The `(left margin, indent)` that hangs a heading of `level` against `side`.
///
/// The pair is the whole trick, so it is one function rather than two lines
/// inside a loop: the first row starts at the margin, and the indent gives the
/// marker back to every row under it, which must land on `side` exactly.
fn hung(face: Face, size: u32, level: u8, side: i32) -> (i32, i32) {
    let width = hang(face, size, level).min(side);
    (side - width, -width)
}

/// Hangs the heading markers, given the margin the prose is set against.
///
/// Pango's `indent` only ever shifts to the right: a positive value moves the
/// first row of a paragraph, a negative one moves every row *after* the first.
/// So a marker cannot be hung by a negative indent alone. It is hung by a pair:
/// the paragraph's own left margin is set one marker to the left of the prose's
/// margin, and the indent gives that marker back to every wrapped row. The
/// first row starts at `side - hang` with the `#` in the margin, and every row
/// under it starts at `side`, flush with the prose.
///
/// Called whenever the page is laid out, because both halves of the pair move:
/// `side` with the width of the window, and the hang with the size of the type.
/// A window too narrow to give the marker its margin keeps what it has — the
/// tag's left margin cannot go below zero, and a heading that cannot hang is
/// worth less than a heading pushed off the left edge of the view.
pub fn hang_headings(buffer: &gtk::TextBuffer, face: Face, size: u32, side: i32) {
    for level in 1..=6u8 {
        let (margin, indent) = hung(face, size, level, side);
        let tag = heading(buffer, level);
        tag.set_left_margin(margin);
        tag.set_indent(indent);
    }
}

/// Draws `spans` on `buffer`, which must hold `document`'s text.
///
/// The whole Document at once, which is where it stays for now: the
/// architecture parses whole on open as a cold-start cost inside the 250 ms
/// budget, and the ticket that lands the keystroke path retags only the lines
/// whose runs changed.
pub fn apply(buffer: &gtk::TextBuffer, document: &Document, face: Face, spans: &[Span]) {
    buffer.remove_all_tags(&buffer.start_iter(), &buffer.end_iter());
    for run in annotate::flatten(spans) {
        let from = iter_at(buffer, document, run.at.start);
        let to = iter_at(buffer, document, run.at.end);
        let Look {
            ink,
            alpha,
            weight,
            slant,
            ground: on,
        } = run.look;
        buffer.apply_tag(&colour(buffer, hex(ink), alpha), &from, &to);
        buffer.apply_tag(&cut(buffer, face, weight, slant), &from, &to);
        if on == Ground::Code {
            buffer.apply_tag(&ground(buffer), &from, &to);
        }
        // The one decoration that reads off the run rather than off a mark: a
        // link's words are exactly the bytes that came out in the link colour,
        // whether they were bracketed or written bare as an autolink.
        if ink == Ink::Link {
            buffer.apply_tag(&underline(buffer), &from, &to);
        }
    }
    for span in spans {
        match span.mark {
            Mark::Heading(level) => hang_line(buffer, document, span, level),
            Mark::Strikethrough => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&struck(buffer), &from, &to);
            }
            _ => {}
        }
    }
}

/// Puts the paragraph tag of a heading of `level` on the line `span` is on.
///
/// The hang belongs to the line, not to the span: a paragraph property is read
/// off the tags at the start of the paragraph, and a heading's markers are not
/// inside its text.
fn hang_line(buffer: &gtk::TextBuffer, document: &Document, span: &Span, level: u8) {
    let mut start = buffer.start_iter();
    start.set_line(iter_at(buffer, document, span.at.start).line());
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    buffer.apply_tag(&heading(buffer, level), &start, &end);
}

/// The colour `ink` names.
///
/// The engine names a role because it cannot see a display; the three roles
/// meet their colours here, and the Dark & light ticket changes what they meet
/// without the engine hearing about it.
fn hex(ink: Ink) -> &'static str {
    match ink {
        Ink::Prose => INK,
        Ink::Marker => MARK,
        Ink::Link => LINK,
    }
}

/// Sets the Italic tags to `face`'s Italic, for text already tagged.
///
/// Called when the type changes, for the reason [`hang_headings`] is: the
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
    use super::*;

    // These test the arithmetic the hang is built on. Nothing here makes a
    // `GtkTextTag`: `tools/gate check` runs `cargo test` with no display
    // attached, and the tags themselves are judged from a shot instead.

    #[test]
    fn a_heading_hangs_by_its_markers_and_the_space_after_them() {
        // At 20 px every Face is on a 12 px cell (`typography` asserts it), so
        // `# ` is 24 px and each further `#` is another 12.
        for (level, width) in [(1, 24), (2, 36), (3, 48), (4, 60), (5, 72), (6, 84)] {
            assert_eq!(
                hang(Face::Duo, 20, level),
                width,
                "a level {level} heading hangs by the wrong width"
            );
        }
    }

    #[test]
    fn the_hang_grows_with_the_type_it_is_set_in() {
        let small = hang(Face::Duo, 16, 1);
        let large = hang(Face::Duo, 32, 1);
        assert!(
            small < large,
            "the marker is measured in cells, so it must move with the size: {small} then {large}"
        );
        assert_eq!(large, small * 2, "twice the size is twice the cell");
    }

    #[test]
    fn every_face_hangs_a_heading_by_the_same_width() {
        for face in [Face::Duo, Face::Quattro, Face::Mono] {
            assert_eq!(
                hang(face, 20, 1),
                hang(Face::Duo, 20, 1),
                "{face:?} is cut to the same cell grid as the others"
            );
        }
    }

    #[test]
    fn the_first_row_hangs_by_exactly_what_the_wrapped_rows_get_back() {
        // Pango puts the first row at the tag's own left margin and every row
        // after it at margin + |indent|, which must be the prose's margin.
        let side = 240;
        for level in 1..=6u8 {
            let (margin, indent) = hung(Face::Duo, 20, level, side);
            assert_eq!(
                margin,
                side - hang(Face::Duo, 20, level),
                "a level {level} heading starts one marker left of the prose"
            );
            assert_eq!(
                margin - indent,
                side,
                "a wrapped row of a level {level} heading must land on the prose margin"
            );
        }
    }

    #[test]
    fn a_window_too_narrow_to_hang_the_marker_keeps_its_gutter() {
        let side = 4;
        assert!(
            hang(Face::Duo, 20, 6) > side,
            "this is the narrow case, or it proves nothing"
        );
        let (margin, indent) = hung(Face::Duo, 20, 6, side);
        assert_eq!(margin, 0, "the left margin of a tag cannot go below zero");
        assert_eq!(
            margin - indent,
            side,
            "the rows still agree: what the first row gives up, the rest get back"
        );
    }

    #[test]
    fn a_colour_at_full_alpha_is_the_colour_itself() {
        let opaque = shaded(MARK, Look::OPAQUE);
        assert!(
            (opaque.alpha() - 1.0).abs() < f32::EPSILON,
            "the alpha half of the key has to reach the colour: {opaque:?}"
        );
        let dimmed = shaded(MARK, Look::OPAQUE / 2);
        assert!(
            dimmed.alpha() < opaque.alpha() && dimmed.red() == opaque.red(),
            "a dimmed row is the same colour, less of it: {dimmed:?}"
        );
    }

    #[test]
    fn the_three_inks_are_the_three_constants_and_no_two_are_alike() {
        assert_eq!(
            [hex(Ink::Prose), hex(Ink::Marker), hex(Ink::Link)],
            [INK, MARK, LINK]
        );
    }

    // The invariant the whole Markup rests on: a closing marker restyles the
    // text before it, and if any of that changed a glyph's advance the line
    // would jolt under the writer's hands as they typed the last asterisk.
    // Measured in the Faces themselves, at the two weights and the two cuts the
    // tag table asks for, on the passage the Piece is judged on. Pango's own
    // font map, because `cargo test` runs with no display and GTK's contexts
    // all come off one.
    #[test]
    fn no_face_moves_a_glyph_across_the_weights_and_cuts_the_tags_ask_for() {
        crate::fonts::load_private(&quill_engine::data::fonts())
            .expect("the Faces are in the checkout");
        let context = pangocairo::FontMap::default().create_context();
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
                if bold_italic_quattro(face, weight, slant) {
                    continue;
                }
                assert_eq!(
                    advance(&context, face, weight, slant, &passage),
                    prose,
                    "{face:?} at {weight} {slant:?} sets the passage to another width"
                );
            }
        }
    }

    #[test]
    fn the_quattro_italic_widens_its_narrow_glyphs_at_bold() {
        // The one combination the Faces get wrong, recorded rather than
        // asserted away: iA's Quattro Italic carries a `wght` advance delta on
        // its narrow glyphs — `t` goes 450 units to 600 — where its Roman and
        // the other two Italics carry none. So a Quattro run that goes bold
        // italic, which is emphasis inside a heading, moves the glyphs after
        // it. Issue #95 pins it in `tools/fontbuild.py`, beside the word space
        // that Face already needed pinning; the day it does, this test fails
        // and takes the carve-out above with it.
        crate::fonts::load_private(&quill_engine::data::fonts())
            .expect("the Faces are in the checkout");
        let context = pangocairo::FontMap::default().create_context();
        assert!(
            advance(&context, Face::Quattro, BOLD, Slant::Italic, "t")
                > advance(&context, Face::Quattro, REGULAR, Slant::Italic, "t"),
            "#95 is fixed: drop this test and the carve-out that names it"
        );
    }

    /// Whether this is the combination [`#95`](https://github.com/danielbaldwin47/Quill/issues/95)
    /// is about.
    fn bold_italic_quattro(face: Face, weight: i32, slant: Slant) -> bool {
        face == Face::Quattro && weight == BOLD && slant == Slant::Italic
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
