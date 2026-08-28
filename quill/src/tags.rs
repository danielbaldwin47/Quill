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
//! margin so the first word sits on the prose's edge, and `list-1` to
//! `list-12` do the same for a bullet, a number or a quote's `>`, keyed by the
//! width of the whole run of markers the line opens with because that is what a
//! hang is — `- ` is two cells, `1. ` is three, `> - ` is four and a nested
//! item is wider again. That is a step past the Parity oracle, which
//! could not do it — `legacy/app/css/markup.css` says why: a `<textarea>` takes
//! no per-line horizontal shift, so the web app bought the same calm with
//! contrast instead of position. ADR 0004 removed the mirror and with it the
//! constraint.
//!
//! `ground-code-block` is the other paragraph tag, and it is one for the
//! opposite reason: a run's background stops with the last glyph on the line,
//! so only a paragraph's can run past both edges of the measure and read as a
//! well.

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
///
/// Every marker holds it, which is a step short of the oracle: `markup.css`
/// keeps line-head marks at the full grey and quiets inline punctuation to
/// `--md-mark-quiet`, 72% of it, lifting the caret's own line back to the full
/// grey. That ladder is what "the caret's line" means, so it belongs to #40
/// (Focus & typewriter) with the rest of it; `Look` already carries the alpha
/// key it will be spelled in.
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

/// The deepest list marker that is given a tag of its own.
///
/// `- ` is two cells and `1. ` three, and every level of nesting adds its
/// indentation, so the count is open-ended in a way a heading's never was.
/// Twelve is six levels of bullets or four of numbers; a marker past it hangs
/// by twelve instead of by its own width, so its text lands right of the prose
/// rather than on it. A list nested deeper than that has bigger troubles than a
/// cell of misalignment, and [`hung`] would have run out of margin to hang in
/// long before.
const LIST_CELLS: u8 = 12;

/// How far a tab advances a marker, in cells: CommonMark's tab stop.
const TAB_CELLS: usize = 4;

/// How far the code ground runs past each edge of the measure, at `size`.
///
/// The oracle's `box-shadow: -.7em 0 0 var(--code-bg), .7em 0 0 var(--code-bg)`
/// on `.line.l-code`, which is what makes a fenced block read as a well rather
/// than as a stripe the exact width of the prose. Ems rather than cells,
/// because that is what the oracle measured it in and the two are not the same
/// thing: a cell is 0.6em on these Faces.
fn well(size: u32) -> i32 {
    pixels(0.7 * f64::from(size))
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

/// The pixels a marker of `cells` cells hangs by, in `face` at `size`.
///
/// The Faces are cut to one cell grid, so a marker's width is its cell count
/// times the advance [`typography::cell`] already measures the 64-character
/// measure from. Rounded once, here, so the hang and the measure are counted
/// off the same number and a heading cannot land half a pixel from the prose.
fn hang(face: Face, size: u32, cells: f64) -> i32 {
    pixels(typography::cell(face, size) * cells)
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

/// The tag that draws the ground under a whole code block.
///
/// A *paragraph* background rather than the run background [`ground`] gives a
/// code span, and that is the point: a run's background stops with its last
/// glyph, so a fenced block would come out as a ragged stack of lines the
/// length of the code on each. A paragraph background fills the line, which is
/// how the ground runs past both edges of the measure and reads as a well —
/// the same picture the oracle buys with a ±0.7em box-shadow on `.line.l-code`
/// because a `<textarea>` gives it no paragraph to paint.
fn code_ground(buffer: &gtk::TextBuffer) -> gtk::TextTag {
    tag(buffer, "ground-code-block", |tag| {
        tag.set_paragraph_background(Some(CODE_GROUND));
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

/// The paragraph tag for a list marker `cells` cells wide.
///
/// Keyed by the width rather than by the kind of list, because the width is the
/// whole of what a hang is: `- ` and `1.` are both two cells and hang alike,
/// while `- ` and `  - ` are the same bullet at two depths and must not.
fn list(buffer: &gtk::TextBuffer, cells: u8) -> gtk::TextTag {
    tag(buffer, &format!("list-{cells}"), |_| {})
}

/// The `(left margin, indent)` that hangs `cells` cells of marker against
/// `side`.
///
/// The pair is the whole trick, so it is one function rather than two lines
/// inside a loop: the first row starts at the margin, and the indent gives the
/// marker back to every row under it, which must land on `side` exactly.
fn hung(face: Face, size: u32, cells: f64, side: i32) -> (i32, i32) {
    let width = hang(face, size, cells).min(side);
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
pub fn hang_markers(buffer: &gtk::TextBuffer, face: Face, size: u32, side: i32) {
    let hang = |tag: &gtk::TextTag, cells: f64| {
        let (margin, indent) = hung(face, size, cells, side);
        tag.set_left_margin(margin);
        tag.set_indent(indent);
    };
    for level in 1..=6u8 {
        hang(&heading(buffer, level), marker_cells(level));
    }
    // The same pair, by width instead of by level. A list marker's width is not
    // its level: `1. ` is a cell wider than `- ` at the same depth, and a
    // nested item is wider again by its indentation, which is why the tag is
    // keyed by the count and not by the kind.
    for cells in 1..=LIST_CELLS {
        hang(&list(buffer, cells), f64::from(cells));
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
    let edge = well(size).min(side);
    let ground = code_ground(buffer);
    ground.set_left_margin(side - edge);
    ground.set_right_margin(side - edge);
    ground.set_indent(edge);
}

/// Draws `spans` on `buffer`, which must hold `document`'s text.
///
/// The whole Document at once, which is where it stays for now: the
/// architecture parses whole on open as a cold-start cost inside the 250 ms
/// budget, and the ticket that lands the keystroke path retags only the lines
/// whose runs changed.
/// Several tags land on the same bytes, which is safe here for one reason and
/// only one: no two of them set the same property. The colour, the cut, the
/// ground and the two decorations are five disjoint sets, so priority never
/// has to decide between them — and priority is what the flattening exists to
/// keep out of the colour, where they *would* collide.
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
    hang_lines(buffer, document, spans);
    for span in spans {
        match span.mark {
            Mark::Heading(level) => {
                paragraph(buffer, document, span, &heading(buffer, level));
            }
            Mark::CodeBlock => paragraph(buffer, document, span, &code_ground(buffer)),
            Mark::Strikethrough => {
                let from = iter_at(buffer, document, span.at.start);
                let to = iter_at(buffer, document, span.at.end);
                buffer.apply_tag(&struck(buffer), &from, &to);
            }
            _ => {}
        }
    }
}

/// Hangs every line that opens with markers, by the width of all of them.
///
/// One hang per line rather than one per marker, because a line can open with
/// more than one: `> - item` is a quote's marker and a bullet's, and its words
/// land on the prose's edge only when the line hangs by the pair. The width is
/// therefore read from the line's first byte to the end of the last line-head
/// marker on it, not from the marker's own span, and the spans arrive in the
/// order the bytes appear, so that last marker is the one that ends the run.
///
/// A heading keeps its own tag row: its span covers the whole heading rather
/// than its `#`s, so its hang is read off the level instead, in [`apply`].
fn hang_lines(buffer: &gtk::TextBuffer, document: &Document, spans: &[Span]) {
    let mut run: Option<&Span> = None;
    for span in spans {
        if !matches!(
            span.mark,
            Mark::QuoteMarker | Mark::BulletMarker | Mark::OrderedMarker
        ) {
            continue;
        }
        if let Some(last) = run
            && document.place(last.at.end).line != document.place(span.at.end).line
        {
            hang_line(buffer, document, last);
        }
        run = Some(span);
    }
    if let Some(last) = run {
        hang_line(buffer, document, last);
    }
}

/// Hangs the line `marker` is on by the width of the run `marker` ends.
fn hang_line(buffer: &gtk::TextBuffer, document: &Document, marker: &Span) {
    let cells = marker_width(head(document, marker.at.end));
    paragraph(buffer, document, marker, &list(buffer, cells));
}

/// The run of line-head markers that ends `end` bytes into `document`.
///
/// From the line's first byte rather than from the marker's own start, so that
/// the markers a line opens with add up: the run a quoted item's bullet ends is
/// `> - `, and four cells is what puts its words on the prose's edge.
fn head(document: &Document, end: usize) -> &str {
    &document.text()[end - document.place(end).index..end]
}

/// How many cells wide the marker run `marker` is.
///
/// Characters rather than bytes, because a cell is a character on the Faces'
/// grid; a marker is ASCII either way, but the count is what the tag is keyed
/// by and counting the wrong thing would key it wrong. Clamped to the widths
/// [`hang_markers`] made a tag for.
fn marker_width(marker: &str) -> u8 {
    let mut cells = 0usize;
    for character in marker.chars() {
        // A tab is not one cell: CommonMark advances it to the next stop, and
        // the engine lets a writer indent a nested item with one. Counting it
        // as a character would hang a tabbed item short of its own text.
        cells = if character == '\t' {
            (cells / TAB_CELLS + 1) * TAB_CELLS
        } else {
            cells + 1
        };
    }
    u8::try_from(cells)
        .unwrap_or(LIST_CELLS)
        .clamp(1, LIST_CELLS)
}

/// Puts a paragraph tag on every line `span` touches.
///
/// The property belongs to the line, not to the span: a paragraph property is
/// read off the tags at the start of the paragraph, and neither a heading's
/// markers nor a list's bullet is inside the text it governs. A heading and a
/// marker touch one line each; a fenced block touches all of its own, which is
/// what puts its ground under every row rather than only the first.
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
        for (level, width) in [(1u8, 24), (2, 36), (3, 48), (4, 60), (5, 72), (6, 84)] {
            assert_eq!(
                hang(Face::Duo, 20, marker_cells(level)),
                width,
                "a level {level} heading hangs by the wrong width"
            );
        }
    }

    #[test]
    fn a_list_marker_hangs_by_its_own_width_and_not_by_its_depth() {
        // The widths the engine measures off the line: `- ` is two cells,
        // `1. ` is three, and `  - ` is a nested bullet at four.
        for (cells, width) in [(2u8, 24), (3, 36), (4, 48)] {
            assert_eq!(
                hang(Face::Duo, 20, f64::from(cells)),
                width,
                "a {cells}-cell marker hangs by the wrong width"
            );
        }
        assert_eq!(
            hang(Face::Duo, 20, 2.0),
            hang(Face::Duo, 20, marker_cells(1)),
            "a bullet and a level-1 heading are both two cells, so they hang alike"
        );
    }

    #[test]
    fn a_quotes_marker_hangs_like_every_other_line_head_marker() {
        // The gap the blind critic named on both markup states: every other
        // marker hung and the `>` did not, so a quote was the one block whose
        // first line started two cells right of its own wrapped rows.
        let document = passage("quoted", "> There are things the sea keeps.\n");
        assert_eq!(
            hangs(&document),
            vec![2],
            "the quote's `> ` is two cells, and its words belong on the prose's edge"
        );
    }

    #[test]
    fn a_lines_hang_is_every_marker_it_opens_with_and_not_the_last_one_alone() {
        // `> - ` is a quote's marker and a bullet's. Hanging by the bullet
        // alone would leave the item's words two cells right of the prose.
        let document = passage("quoted_item", "> quoted\n\n> - quoted item\n");
        assert_eq!(
            hangs(&document),
            vec![2, 2, 4],
            "the bullet inside a quote hangs by both markers, not by its own"
        );
    }

    /// A Document holding `text`, the only way to build one outside the engine.
    fn passage(stem: &str, text: &str) -> Document {
        let path = std::env::temp_dir().join(format!("quill-tags-{stem}.md"));
        std::fs::write(&path, text).expect("writes to the temp directory");
        Document::open(&path).expect("reads the passage just written")
    }

    /// The hang, in cells, each line-head marker of `document` asks for.
    ///
    /// What [`hang_lines`] reads off the spans before it picks one tag per
    /// line; the tag itself needs a buffer, and a buffer needs a display.
    fn hangs(document: &Document) -> Vec<u8> {
        annotate::markup(document.text())
            .iter()
            .filter(|span| {
                matches!(
                    span.mark,
                    Mark::QuoteMarker | Mark::BulletMarker | Mark::OrderedMarker
                )
            })
            .map(|span| marker_width(head(document, span.at.end)))
            .collect()
    }

    #[test]
    fn a_list_items_words_land_on_the_prose_margin_whatever_its_marker_is() {
        let side = 240;
        for cells in 1..=LIST_CELLS {
            let (margin, indent) = hung(Face::Duo, 20, f64::from(cells), side);
            assert_eq!(
                margin - indent,
                side,
                "a {cells}-cell item's text must start at the same x as the prose"
            );
            assert!(
                margin < side,
                "and its marker must sit left of that, out in the margin"
            );
        }
    }

    #[test]
    fn the_hang_grows_with_the_type_it_is_set_in() {
        let small = hang(Face::Duo, 16, marker_cells(1));
        let large = hang(Face::Duo, 32, marker_cells(1));
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
                hang(face, 20, marker_cells(1)),
                hang(Face::Duo, 20, marker_cells(1)),
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
            let (margin, indent) = hung(Face::Duo, 20, marker_cells(level), side);
            assert_eq!(
                margin,
                side - hang(Face::Duo, 20, marker_cells(level)),
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
    fn the_code_ground_runs_past_both_edges_of_the_measure() {
        // The oracle's ±0.7em, which at 20 px is 14 px on each side.
        assert_eq!(well(20), 14);
        let side = 240;
        let edge = well(20).min(side);
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
    fn a_tab_indented_item_hangs_by_the_stop_it_reaches_not_by_one_cell() {
        assert_eq!(
            marker_width("-\t"),
            4,
            "`-` then a tab reaches the stop at four, which is where the word starts"
        );
        assert_eq!(marker_width("- "), 2, "and a space is still one cell");
        assert_eq!(
            marker_width("\t- "),
            6,
            "a tab-nested item hangs by the stop plus its own bullet"
        );
    }

    #[test]
    fn the_well_grows_with_the_type_it_is_set_in() {
        assert_eq!(
            well(40),
            well(20) * 2,
            "an em is the type's own size, so twice the size is twice the well"
        );
    }

    #[test]
    fn a_window_too_narrow_to_hang_the_marker_keeps_its_gutter() {
        let side = 4;
        assert!(
            hang(Face::Duo, 20, marker_cells(6)) > side,
            "this is the narrow case, or it proves nothing"
        );
        let (margin, indent) = hung(Face::Duo, 20, marker_cells(6), side);
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
