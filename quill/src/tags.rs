//! The tag table: one `GtkTextTag` per look a span asks for, grown as it is.
//!
//! `docs/architecture.md` § Annotators and the keystroke path fixes the shape.
//! Overlapping tags override a property by priority rather than blending, so
//! the table holds one tag per distinct `(colour, alpha)` and one per
//! `(weight, slant)` rather than one per kind of mark, created on first use and
//! never removed. The table is GTK's own, keyed by name, so the Editor keeps no
//! second copy of it to fall out of step.
//!
//! Paragraph tags are the third row: `heading-1` to `heading-6` hang a
//! heading's `#` markers out into the left gutter, by exactly as far as the
//! layout will advance them, so the first word sits on the prose's edge and
//! `###### ` fills the seven-cell gutter [`typography::GUTTER`] was sized at.
//! **Nothing else hangs.** A bullet, an ordinal and a quote's `>` sit on the
//! body column and push their own words inward, and no rule is drawn beside a
//! quote: that is what the Design oracle draws
//! ([ADR 0016](../../docs/adr/0016-the-text-container-is-78-cells.md);
//! `ref/ia/mac-native/` state 14, `14-gutters` and `14-blocks`).
//!
//! The Parity oracle could hang nothing at all — `legacy/app/css/markup.css`
//! says why: a `<textarea>` takes no per-line horizontal shift, so the web app
//! bought the same calm with contrast instead of position — and with no oracle
//! for the rest, #102 read the marketing frames' `#` in the margin as a rule
//! for every marker and hung them all. The app running on the owner's Mac
//! hangs the heading and only the heading (#167).
//!
//! `ground-code-block` is the other paragraph tag, and it is one for the
//! opposite reason: a run's background stops with the last glyph on the line,
//! so only a paragraph's can run past both edges of the measure and read as a
//! well.

use std::ops::Range;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use quill_engine::annotate::{Ground, Ink, Look, Mark, Slant, Span, Weight};
use quill_engine::document::Document;
use quill_engine::settings::Face;
use quill_engine::theme::{Colour, Colours, Role, Scheme};
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
/// Every horizontal length this module sets — a hang, a well — is rounded here
/// and only here, so that two of them counted off the same measure cannot land
/// half a pixel apart.
fn pixels(length: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a marker is a handful of cells and a well a fraction of a type size; \
                  the measure itself is rounded the same way"
    )]
    let whole = length.round() as i32;
    whole
}

/// A length the container measured, as the `i32` GTK sets a margin in.
///
/// [`typography::Column`] counts every edge and every hang in whole pixels
/// from the left of the view, and GTK takes a margin signed. A container wider
/// than `i32::MAX` is not a window anyone has.
fn margin(length: u32) -> i32 {
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
fn ground(buffer: &gtk::TextBuffer, scheme: Scheme) -> gtk::TextTag {
    let ground = tag(buffer, "ground-code", |_| {});
    ground.set_background(Some(&code_well(scheme)));
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
fn code_ground(buffer: &gtk::TextBuffer, scheme: Scheme) -> gtk::TextTag {
    let ground = tag(buffer, "ground-code-block", |_| {});
    ground.set_paragraph_background(Some(&code_well(scheme)));
    ground
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
/// `VERDICTS.md` § Found here: the link's ink and the code ground).
fn underline(buffer: &gtk::TextBuffer, scheme: Scheme) -> gtk::TextTag {
    let underline = tag(buffer, "decoration-underline", |tag| {
        tag.set_underline(pango::Underline::Single);
    });
    underline.set_underline_rgba(Some(&shaded(
        &Colours::of(scheme).colour(Role::LinkRule).to_hex(),
        u8::MAX,
    )));
    underline
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

/// The `(left margin, indent)` that hangs a marker `width` pixels wide against
/// `side`.
///
/// The pair is the whole trick, so it is one function rather than two lines
/// inside a loop: the first row starts at the margin, and the indent gives the
/// marker back to every row under it, which must land on `side` exactly.
fn hung(width: i32, side: i32) -> (i32, i32) {
    let width = width.min(side);
    (side - width, -width)
}

/// Hangs the heading markers into the gutter `column` gives them, by the
/// `advances` the caller measured off the layout.
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
/// Headings are the only markers hung: a bullet, an ordinal and a quote's `>`
/// take no tag here at all and so sit where the buffer puts them, on the body
/// column (ADR 0016).
///
/// Called whenever the page is laid out, because both halves of the pair move:
/// the container with the width of the window, and the marker run with the size
/// of the type. A window too narrow to give the marker its margin keeps what it
/// has — the tag's left margin cannot go below zero, and a heading that cannot
/// hang is worth less than a heading pushed off the left edge of the view.
pub fn hang_markers(
    buffer: &gtk::TextBuffer,
    step: u32,
    column: typography::Column,
    advances: [i32; 6],
    scheme: Scheme,
) {
    let side = margin(column.side);
    for (level, advance) in (1..=6u8).zip(advances) {
        let (left, indent) = hung(advance, side);
        let tag = heading(buffer, level);
        tag.set_left_margin(left);
        tag.set_indent(indent);
    }
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
    let ground = code_ground(buffer, scheme);
    ground.set_left_margin(side - edge);
    ground.set_right_margin(side - edge);
    ground.set_indent(edge);
}

/// Draws the whole of `document` on `buffer`, which must hold its text.
///
/// The whole Document at once is what opening one costs: the architecture
/// parses and draws whole on open as a cold-start cost inside the 250 ms
/// budget. A keystroke goes through [`retag`] instead.
pub fn apply(buffer: &gtk::TextBuffer, document: &Document, face: Face, scheme: Scheme) {
    buffer.remove_all_tags(&buffer.start_iter(), &buffer.end_iter());
    draw(buffer, document, face, scheme, &(0..document.text().len()));
}

/// Draws `lines` again, and leaves every other line of `buffer` alone.
///
/// The lines an [`Edit`] names, and no others. Every line below an edit
/// carries tags that are still right: GTK moves a tag with the text it is on,
/// so a run that only slid down the Document needs nothing done to it, and the
/// engine hands back only the lines whose runs came out looking different.
///
/// [`Edit`]: quill_engine::document::Edit
pub fn retag(
    buffer: &gtk::TextBuffer,
    document: &Document,
    face: Face,
    scheme: Scheme,
    lines: &Range<usize>,
) {
    if lines.is_empty() {
        return;
    }
    let at = document.line_bytes(lines.start).start..document.line_bytes(lines.end - 1).end;
    let from = iter_at(buffer, document, at.start);
    let to = iter_at(buffer, document, at.end);
    buffer.remove_all_tags(&from, &to);
    draw(buffer, document, face, scheme, &at);
}

/// Puts every tag the bytes `at` ask for on to `buffer`.
///
/// Several tags land on the same bytes, which is safe here for one reason and
/// only one: no two of them set the same property. The colour, the cut, the
/// ground and the two decorations are five disjoint sets, so priority never
/// has to decide between them — and priority is what the flattening exists to
/// keep out of the colour, where they *would* collide.
///
/// A run is clipped to `at` and a paragraph tag is not, because the two are
/// different kinds of thing: a run draws the bytes it covers, and the caller
/// has taken the tags off exactly those bytes; a paragraph property is read
/// off the whole line and belongs to every line its span touches, including
/// the ones outside `at` that still have it. Applying a tag they already carry
/// is what leaves them as they were.
fn draw(
    buffer: &gtk::TextBuffer,
    document: &Document,
    face: Face,
    scheme: Scheme,
    at: &Range<usize>,
) {
    for run in document.runs_in(at) {
        let from = iter_at(buffer, document, run.at.start.max(at.start));
        let to = iter_at(buffer, document, run.at.end.min(at.end));
        let Look {
            ink,
            alpha,
            weight,
            slant,
            ground: on,
        } = run.look;
        buffer.apply_tag(&colour(buffer, &hex(scheme, ink), alpha), &from, &to);
        buffer.apply_tag(&cut(buffer, face, weight, slant), &from, &to);
        if on == Ground::Code {
            buffer.apply_tag(&ground(buffer, scheme), &from, &to);
        }
    }
    for span in document.spans_in(at) {
        if span.at.end <= at.start {
            continue;
        }
        match span.mark {
            Mark::Heading(level) => {
                paragraph(buffer, document, span, &heading(buffer, level));
            }
            Mark::CodeBlock => paragraph(buffer, document, span, &code_ground(buffer, scheme)),
            Mark::Strikethrough => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&struck(buffer), &from, &to);
            }
            // The rule goes under the URL and nothing else: not the words that
            // stand for it, and not the brackets around them. The Design
            // oracle draws it that way (#198), and the mark says exactly which
            // bytes are the destination.
            Mark::Url => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&underline(buffer, scheme), &from, &to);
            }
            _ => {}
        }
    }
}

/// Puts a paragraph tag on every line `span` touches.
///
/// The property belongs to the line, not to the span: a paragraph property is
/// read off the tags at the start of the paragraph, and a heading's markers are
/// not inside the text they govern. A heading touches one line; a fenced block
/// touches all of its own, which is what puts its ground under every row rather
/// than only the first.
fn paragraph(buffer: &gtk::TextBuffer, document: &Document, span: &Span, tag: &gtk::TextTag) {
    let mut start = buffer.start_iter();
    start.set_line(iter_at(buffer, document, span.at.start).line());
    let mut end = buffer.start_iter();
    end.set_line(iter_at(buffer, document, span.at.end).line());
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    buffer.apply_tag(tag, &start, &end);
}

/// The colour `ink` names on `scheme`, as GTK parses one.
///
/// The engine names a role because it cannot see a display; the three roles
/// meet their colours here, and that is the whole of what a ground is to
/// marked-up text — the same three roles read off the other row of the table.
/// Nothing here holds a colour of its own, which is why
/// `grep -n '#[0-9a-f]\{6\}' quill/src` comes back empty.
///
/// Every marker holds the body's own ink, and that is the whole ladder. #198
/// shot the Design oracle at every mark kind on both grounds and found no
/// resting marker grey: `markup.css`'s three tiers — `--md-mark-quiet` at 72 %
/// for a heading's `#`s, a quote's `>`, fences, code marks and URLs, the full
/// grey for bullets, task boxes and a fence's info string, `--md-hair` at 34 %
/// for a thematic break — have no counterpart in the app the port is measured
/// against, which rests all three at the ink. The ladder was the Parity
/// oracle's, and ADR 0015 is why it was measured before it was ported.
///
/// So [`Ink::Marker`] and [`Ink::Prose`] answer with the same hex on both
/// built-in grounds, and the two stay separate roles for the writer whose
/// `palette` file sets them apart. What is quieter than the prose is a link's
/// plumbing, which is [`Ink::Link`].
fn hex(scheme: Scheme, ink: Ink) -> String {
    let role = match ink {
        Ink::Prose => Role::Ink,
        Ink::Marker => Role::Mark,
        Ink::Link => Role::Link,
    };
    Colours::of(scheme).colour(role).to_hex()
}

/// The code ground on `scheme`, flattened onto that ground's paper.
///
/// [`Role::CodeBg`] is a wash — 4.5 % black over the light paper, 6 % white
/// over the dark — and a `GtkTextTag` background is a ground rather than a
/// wash: nothing is ever drawn under it. So it is composited here, once,
/// rather than handed to GTK translucent to be blended against whatever the
/// widget happens to have behind the line.
fn code_well(scheme: Scheme) -> String {
    let colours = Colours::of(scheme);
    let wash = colours.colour(Role::CodeBg);
    Colour::over(wash, colours.colour(Role::Paper), wash.alpha).to_hex()
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
        let side = margin(container(Face::Duo, STEP).side);
        for hinted in [true, false] {
            for (level, advance) in (1..=6u8).zip(advances(hinted)) {
                let (left, indent) = hung(advance, side);
                assert_eq!(
                    left + advance,
                    side,
                    "hinted {hinted}: a level {level} heading's words start off the body column"
                );
                assert_eq!(
                    left - indent,
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
        let (left, indent) = hung(width, side);
        assert_eq!(left, 0, "the left margin of a tag cannot go below zero");
        assert_eq!(
            left - indent,
            side,
            "the rows still agree: what the first row gives up, the rest get back"
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
    /// what is checked here is that this module reaches the right three roles
    /// and reads them off the ground it was handed.
    #[test]
    fn the_three_inks_are_three_roles_of_the_ground() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = Colours::of(scheme);
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
            let well = code_well(scheme);
            let colours = Colours::of(scheme);
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
        assert_ne!(code_well(Scheme::Light), code_well(Scheme::Dark));
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
