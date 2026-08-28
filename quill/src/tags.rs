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

use gtk::prelude::*;
use quill_engine::annotate::{Mark, Span};
use quill_engine::document::Document;
use quill_engine::settings::Face;
use quill_engine::typography;

/// The marker grey: `--mark` of `legacy/app/css/theme.css`, 4.0:1 on paper.
///
/// A constant here beside the Editor's `PAPER` and `INK` for the same reason
/// they are constants: there is one theme until the Dark & light ticket moves
/// all three into the palette table.
const MARK: &str = "#7a7a7a";

/// The weight a heading is set at: `.md-h { font-weight: 700 }` of
/// `legacy/app/css/markup.css`.
///
/// Bold at body size, in the same Face and the same ink as the prose. The
/// level reads from the markers, so nothing about the text image jumps when a
/// `#` is typed or deleted — which is why no heading is ever set larger.
const HEADING_WEIGHT: i32 = 700;

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

/// The tag that draws text in `colour`.
///
/// `docs/architecture.md` § Annotators keys this row by `(colour, alpha)`.
/// Every mark #86 draws is opaque, so the alpha half of the key has one value
/// and the name says which; the Focus ticket brings the dimmed rows, and each
/// gets its own tag rather than blending with this one.
fn colour(buffer: &gtk::TextBuffer, colour: &str) -> gtk::TextTag {
    tag(buffer, &format!("colour-{colour}-opaque"), |tag| {
        tag.set_foreground(Some(colour));
    })
}

/// The tag that sets text at `weight`, upright.
///
/// The other half of that section's second key is slant, which has one value
/// until emphasis lands and is named here for the same reason.
fn weight(buffer: &gtk::TextBuffer, weight: i32) -> gtk::TextTag {
    tag(buffer, &format!("weight-{weight}-upright"), |tag| {
        tag.set_weight(weight);
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
pub fn apply(buffer: &gtk::TextBuffer, document: &Document, spans: &[Span]) {
    buffer.remove_all_tags(&buffer.start_iter(), &buffer.end_iter());
    for span in spans {
        let from = iter_at(buffer, document, span.at.start);
        let to = iter_at(buffer, document, span.at.end);
        match span.mark {
            Mark::Markup => buffer.apply_tag(&colour(buffer, MARK), &from, &to),
            Mark::Heading(level) => {
                buffer.apply_tag(&weight(buffer, HEADING_WEIGHT), &from, &to);
                // The hang belongs to the line, not to the span: a paragraph
                // property is read off the tags at the start of the paragraph,
                // and the markers the heading hangs by are not inside the
                // heading's text.
                let line = from.line();
                let mut start = buffer.start_iter();
                start.set_line(line);
                let mut end = start;
                if !end.ends_line() {
                    end.forward_to_line_end();
                }
                buffer.apply_tag(&heading(buffer, level), &start, &end);
            }
        }
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
}
