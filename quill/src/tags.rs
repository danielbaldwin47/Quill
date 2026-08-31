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

use std::collections::HashSet;
use std::ops::Range;

use gtk::gdk;
use gtk::pango;
use gtk::prelude::*;
use quill_engine::annotate::{Ground, Ink, Look, Mark, Slant, Span, Weight};
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
/// Every marker holds it, which is a step short of the oracle, and short of it
/// in a wider way than this comment used to say. `markup.css` quiets more than
/// inline punctuation: a heading's `#`s (`.md-hmark`), a quote's `>`
/// (`.md-quote-mark`), fences, code marks and URLs all take `--md-quiet`,
/// resting at `--md-mark-quiet` — 72% of the grey over the page, `#9e9e9e` —
/// while bullets, task boxes and a fence's info string keep the full grey and a
/// thematic break drops again to `--md-hair`, 34% of it. Only the lift of the
/// caret's own line back to the full grey is #40's (`markup.css:39`,
/// `.line.md-here { --md-quiet: var(--mark) }`). The resting ladder is this
/// Piece's, and it wants its own ticket rather than a line here: it is three
/// greys across a dozen marks, and the blind critic reads the oracle's own
/// two-tone as inconsistency as often as it reads ours as too loud.
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

/// The pixels a marker of `cells` cells hangs by, in `face` at `step` of the
/// type ladder.
///
/// The Faces are cut to one cell grid, so a marker's width is its cell count
/// times the advance [`typography::cell`] already measures the 64-character
/// measure from. Rounded once, here, so the hang and the measure are counted
/// off the same number and a heading cannot land half a pixel from the prose.
fn hang(face: Face, step: u32, cells: f64) -> i32 {
    pixels(typography::cell(face, step) * cells)
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
fn hung(face: Face, step: u32, cells: f64, side: i32) -> (i32, i32) {
    let width = hang(face, step, cells).min(side);
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
pub fn hang_markers(buffer: &gtk::TextBuffer, face: Face, step: u32, side: i32) {
    let hang = |tag: &gtk::TextTag, cells: f64| {
        let (margin, indent) = hung(face, step, cells, side);
        tag.set_left_margin(margin);
        tag.set_indent(indent);
    };
    for level in 1..=6u8 {
        hang(&heading(buffer, level), marker_cells(level));
    }
    // The same pair, by width instead of by level, for every other marker a
    // line can open with. A marker's width is not its level or its kind: `1. `
    // is a cell wider than `- ` at the same depth, `> - ` is a quote's marker
    // and a bullet's together, and a nested item is wider again by its
    // indentation, which is why the tag is keyed by the count.
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
    let edge = well(step).min(side);
    let ground = code_ground(buffer);
    ground.set_left_margin(side - edge);
    ground.set_right_margin(side - edge);
    ground.set_indent(edge);
}

/// Draws the whole of `document` on `buffer`, which must hold its text.
///
/// The whole Document at once is what opening one costs: the architecture
/// parses and draws whole on open as a cold-start cost inside the 250 ms
/// budget. A keystroke goes through [`retag`] instead.
pub fn apply(buffer: &gtk::TextBuffer, document: &Document, face: Face) {
    buffer.remove_all_tags(&buffer.start_iter(), &buffer.end_iter());
    draw(buffer, document, face, &(0..document.text().len()));
}

/// Draws `lines` again, and leaves every other line of `buffer` alone.
///
/// The lines an [`Edit`] names, and no others. Every line below an edit
/// carries tags that are still right: GTK moves a tag with the text it is on,
/// so a run that only slid down the Document needs nothing done to it, and the
/// engine hands back only the lines whose runs came out looking different.
///
/// [`Edit`]: quill_engine::document::Edit
pub fn retag(buffer: &gtk::TextBuffer, document: &Document, face: Face, lines: &Range<usize>) {
    if lines.is_empty() {
        return;
    }
    let at = document.line_bytes(lines.start).start..document.line_bytes(lines.end - 1).end;
    let from = iter_at(buffer, document, at.start);
    let to = iter_at(buffer, document, at.end);
    buffer.remove_all_tags(&from, &to);
    draw(buffer, document, face, &at);
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
fn draw(buffer: &gtk::TextBuffer, document: &Document, face: Face, at: &Range<usize>) {
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
    hang_lines(buffer, document, at);
    for span in document.spans_in(at) {
        if span.at.end <= at.start {
            continue;
        }
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
/// therefore read from the line's first byte to the end of its *first*
/// line-head marker — which takes in a nested item's indentation — and then
/// each later marker on the line by its own width alone. What the writer set
/// between two markers is their prose and hangs nothing, the same rule that
/// leaves the padding of `>      Beans` where they typed it (#102).
///
/// A heading keeps its own tag row: its span covers the whole heading rather
/// than its `#`s, so its hang is read off the level instead, in [`draw`]. Its
/// line is left alone here, and that is what keeps [`draw`]'s one safety
/// true — a `> # Title` hung twice would be two tags setting the same two
/// properties on one line, with tag priority left to decide between them. The
/// price is that such a line hangs by its `#` alone and its words sit a quote
/// marker right of the prose; hanging a heading by the run in front of it wants
/// the two rows folded into one, which is more than a judged round should take.
///
/// A marker ending at or before `at` starts is on a line the caller left alone,
/// and its hang is already on that line: [`Document::spans_in`] reaches back to
/// the block's first byte, so the skip is the same one [`draw`] makes.
fn hang_lines(buffer: &gtk::TextBuffer, document: &Document, at: &Range<usize>) {
    for (_, marker, cells) in hangs(document, document.spans_in(at)) {
        if marker.at.end <= at.start {
            continue;
        }
        paragraph(buffer, document, marker, &list(buffer, cells));
    }
}

/// Which line takes a hang, the marker that places it, and how wide it is.
///
/// The choice, with no buffer in it, because the choice is the whole of what
/// [`hang_lines`] decides: one hang per line, the run of markers the line opens
/// with is what measures it, and a heading's line is not here at all.
fn hangs<'a>(document: &Document, spans: &'a [Span]) -> Vec<(usize, &'a Span, u8)> {
    let headings: HashSet<usize> = spans
        .iter()
        .filter(|span| matches!(span.mark, Mark::Heading(_)))
        .map(|span| document.place(span.at.start).line)
        .collect();
    let mut hangs: Vec<(usize, &Span, usize)> = Vec::new();
    for span in spans.iter().filter(|span| line_head(span.mark)) {
        let line = document.place(span.at.end).line;
        if headings.contains(&line) {
            continue;
        }
        // The spans arrive in the order the bytes appear, so a marker on the
        // line already taken is a later one on it, and it adds its own width
        // and only that: whatever stands before it that is not a marker is the
        // writer's, and the first marker's reach already has the indentation.
        if let Some(last) = hangs.last_mut().filter(|(taken, _, _)| *taken == line) {
            last.1 = span;
            last.2 += cells_in(&document.text()[span.at.clone()]);
        } else {
            hangs.push((line, span, cells_in(head(document, span.at.end))));
        }
    }
    hangs
        .into_iter()
        .map(|(line, marker, cells)| (line, marker, marker_width(cells)))
        .collect()
}

/// Whether `mark` is a marker a line can open with.
fn line_head(mark: Mark) -> bool {
    matches!(
        mark,
        Mark::QuoteMarker | Mark::BulletMarker | Mark::OrderedMarker
    )
}

/// The run of line-head markers that ends `end` bytes into `document`.
///
/// From the line's first byte rather than from the marker's own start, so that
/// the markers a line opens with add up: the run a quoted item's bullet ends is
/// `> - `, and four cells is what puts its words on the prose's edge.
///
/// `end` is a span's end and so a byte the engine named: inside the text and on
/// a character boundary. [`Document::place`] would forgive neither, but a span
/// that was outside the text it came from is a bug worth the panic.
fn head(document: &Document, end: usize) -> &str {
    &document.text()[end - document.place(end).index..end]
}

/// How many cells the marker run `marker` advances.
///
/// Characters rather than bytes, because a cell is a character on the Faces'
/// grid; a marker is ASCII either way, but the count is what the tag is keyed
/// by and counting the wrong thing would key it wrong. Unclamped, because a
/// line's hang is a sum of these and it is the sum a tag is made for.
fn cells_in(marker: &str) -> usize {
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
    cells
}

/// A line's `cells` as one of the widths [`hang_markers`] made a tag for.
fn marker_width(cells: usize) -> u8 {
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

    /// The judged step: the ladder's default, whose em is 21.33 logical
    /// pixels and whose cell is therefore 12.798.
    const STEP: u32 = 5;

    #[test]
    fn a_heading_hangs_by_its_markers_and_the_space_after_them() {
        // Every Face is on a 0.6 em cell (`typography` asserts it), so at the
        // default step `# ` is two cells of 12.798 px and each further `#` is
        // another one.
        for (level, width) in [(1u8, 26), (2, 38), (3, 51), (4, 64), (5, 77), (6, 90)] {
            assert_eq!(
                hang(Face::Duo, STEP, marker_cells(level)),
                width,
                "a level {level} heading hangs by the wrong width"
            );
        }
    }

    #[test]
    fn a_list_marker_hangs_by_its_own_width_and_not_by_its_depth() {
        // The widths the engine measures off the line: `- ` is two cells,
        // `1. ` is three, and `  - ` is a nested bullet at four.
        for (cells, width) in [(2u8, 26), (3, 38), (4, 51)] {
            assert_eq!(
                hang(Face::Duo, STEP, f64::from(cells)),
                width,
                "a {cells}-cell marker hangs by the wrong width"
            );
        }
        assert_eq!(
            hang(Face::Duo, STEP, 2.0),
            hang(Face::Duo, STEP, marker_cells(1)),
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
            widths(&document),
            vec![2],
            "the quote's `> ` is two cells, and its words belong on the prose's edge"
        );
    }

    #[test]
    fn a_lines_hang_is_every_marker_it_opens_with_and_not_the_last_one_alone() {
        // `> - ` is a quote's marker and a bullet's. Hanging by the bullet
        // alone would leave the item's words two cells right of the prose, and
        // hanging the line twice would leave tag priority to choose.
        let document = passage("quoted_item", "> quoted\n\n> - quoted item\n");
        assert_eq!(
            widths(&document),
            vec![2, 4],
            "the bullet inside a quote hangs by both markers, not by its own"
        );
    }

    #[test]
    fn a_line_hangs_by_its_own_marker_however_the_writer_padded_it() {
        // #102's Reproduce passage. Its first line hung by twelve cells and
        // its last swung the `>` seven into the margin, because a marker span
        // had run past the marker. The owner's decision on the padding: `>`
        // and one space is the whole of a quote's marker, and `>      Beans`
        // is a word the writer set five cells in, not a wider hang.
        let document = passage(
            "padded_quote",
            "> Why hello there \n> Beans\n> Beans\n>      Beans\n",
        );
        assert_eq!(
            widths(&document),
            vec![2, 2, 2, 2],
            "no line hangs by more than the run of markers it opens with"
        );
    }

    #[test]
    fn a_space_the_writer_left_between_two_markers_hangs_nothing() {
        // #102 one line further in. The markers are `> ` and `> `, four cells;
        // the spaces at bytes 2 and 5 are the writer's, and are painted in
        // their ink. Measuring from the line's first byte to the last marker's
        // end hangs them too, and the line takes a cell it has no marker for.
        let document = passage("spaced_markers", ">  >  Beans\n");
        assert_eq!(
            widths(&document),
            vec![4],
            "a line hangs by the markers it opens with and by the indentation \
             before them, never by what the writer set between them"
        );
    }

    #[test]
    fn a_quote_line_ending_in_a_space_hangs_exactly_as_one_that_does_not() {
        let spaced = passage("spaced_quote", "> one \n> two \n> three \n");
        let plain = passage("plain_quote", "> one\n> two\n> three\n");
        assert_eq!(
            widths(&spaced),
            widths(&plain),
            "#102: the space joined its line to the next, and the next took \
             the first one's measurement"
        );
        assert_eq!(
            lines(&spaced),
            lines(&plain),
            "and every line of the quote hangs, not every other one"
        );
    }

    #[test]
    fn a_quoted_heading_is_hung_once_and_by_its_heading_tag() {
        // Two paragraph tags on one line would both set the left margin and
        // the indent, and tag priority rather than the code would pick the
        // hang. The heading's own row wins the line, so the quote's marker
        // asks for nothing on it.
        let document = passage("quoted_heading", "> ### Title\n");
        assert_eq!(
            lines(&document),
            Vec::<usize>::new(),
            "a heading's line is hung in `draw`, not here"
        );
        let document = passage("plain_heading", "### Title\n\n> quoted\n");
        assert_eq!(
            lines(&document),
            vec![2],
            "and a quote that is not on a heading's line still hangs"
        );
    }

    /// The lines [`hangs`] gives a hang to, in order.
    fn lines(document: &Document) -> Vec<usize> {
        hangs(document, document.spans_in(&(0..document.text().len())))
            .into_iter()
            .map(|(line, _, _)| line)
            .collect()
    }

    /// A Document holding `text`, the only way to build one outside the engine.
    ///
    /// Named for the process as well as the passage: this repo is worked in
    /// several worktrees at once, and two of them testing together would
    /// otherwise write and read one file.
    fn passage(stem: &str, text: &str) -> Document {
        let path =
            std::env::temp_dir().join(format!("quill-tags-{stem}-{}.md", std::process::id()));
        std::fs::write(&path, text).expect("writes to the temp directory");
        Document::open(&path).expect("reads the passage just written")
    }

    /// The hang, in cells, each hung line of `document` takes.
    ///
    /// What [`hang_lines`] reads off [`hangs`] before it asks for the tag; the
    /// tag itself needs a buffer, and a buffer needs a display.
    fn widths(document: &Document) -> Vec<u8> {
        hangs(document, document.spans_in(&(0..document.text().len())))
            .into_iter()
            .map(|(_, _, cells)| cells)
            .collect()
    }

    #[test]
    fn a_list_items_words_land_on_the_prose_margin_whatever_its_marker_is() {
        let side = 240;
        for cells in 1..=LIST_CELLS {
            let (margin, indent) = hung(Face::Duo, STEP, f64::from(cells), side);
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
        // Every rung of the ladder, because the ladder is measurement rather
        // than a multiple: the em from step to step grows by anything from
        // 0.75 px to 6.17, and only the direction is a rule.
        for step in quill_engine::settings::type_steps().skip(1) {
            let small = hang(Face::Duo, step - 1, marker_cells(1));
            let large = hang(Face::Duo, step, marker_cells(1));
            assert!(
                small < large,
                "the marker is measured in cells, so it must move with the type: \
                 step {} hangs by {small} and step {step} by {large}",
                step - 1
            );
        }
    }

    #[test]
    fn every_face_hangs_a_heading_by_the_same_width() {
        for face in [Face::Duo, Face::Quattro, Face::Mono] {
            assert_eq!(
                hang(face, STEP, marker_cells(1)),
                hang(Face::Duo, STEP, marker_cells(1)),
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
            let (margin, indent) = hung(Face::Duo, STEP, marker_cells(level), side);
            assert_eq!(
                margin,
                side - hang(Face::Duo, STEP, marker_cells(level)),
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
    fn a_tab_indented_item_hangs_by_the_stop_it_reaches_not_by_one_cell() {
        assert_eq!(
            cells_in("-\t"),
            4,
            "`-` then a tab reaches the stop at four, which is where the word starts"
        );
        assert_eq!(cells_in("- "), 2, "and a space is still one cell");
        assert_eq!(
            cells_in("\t- "),
            6,
            "a tab-nested item hangs by the stop plus its own bullet"
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
        let side = 4;
        assert!(
            hang(Face::Duo, STEP, marker_cells(6)) > side,
            "this is the narrow case, or it proves nothing"
        );
        let (margin, indent) = hung(Face::Duo, STEP, marker_cells(6), side);
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
