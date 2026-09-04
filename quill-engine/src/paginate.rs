//! Cutting a rendered page into the pages of paper Export prints.
//!
//! [`crate::render`] lays a whole Document out as one tall page; Export puts it
//! on paper. This module is what stands between: given that rendered page and a
//! [`Geometry`] — the paper, one margin, and the header, footer and title-page
//! switches — [`pages`] answers the ordered [`Page`]s, each a list of
//! [`Fragment`]s saying which of a block's layout lines fall on it and where
//! they stand, and the [`Furniture`] the page carries.
//!
//! It places and it measures; it draws nothing, and the layouts it cuts are
//! the render pass's own, made once at the [`Frame::measure`] and
//! [`Frame::zoom`] that [`frame`] answers for a geometry and a body size. The
//! ink and the paper are the drawer's.
//!
//! [`lay_out`] is the whole of that sequence in one call — the context set to
//! points, the frame, the render pass, the cut — because both page sinks make
//! it and two spellings of it are two pages ([`crate::pdf`] and Print).
//!
//! **Everything here is in points**, the unit a paper size is named in and the
//! unit a cairo PDF surface draws in, so a render pass at any other resolution
//! would be measured against the wrong paper: [`in_points`] is what a caller
//! sets its Pango context up with, and [`frame`]'s zoom is what puts the
//! Template's body at the writer's text size there.
//!
//! The geometry is Export's own rather than the Template's
//! (`docs/architecture.md` § Preview and Export); the rules a cut obeys live on
//! [`pages`].

use std::mem;
use std::ops::Range;

use crate::document::Document;
use crate::draw;
use crate::render;
use crate::settings::{Export, preview_zooms};
use crate::template::Template;

/// The resolution a page is measured at: one Pango pixel, one point.
const POINTS: f64 = 72.0;

/// The header's and the footer's size, as a multiple of the body size.
const FURNITURE: f64 = 0.8;

/// How far below the middle of the band it is centred in a furniture line's
/// baseline sits, in ems: half the cap height of the faces Quill ships, which
/// centres the header in the top margin without laying the text out.
const CAP: f64 = 0.36;

/// The slack two heights are compared with, so a block whose height is the
/// room it has to the last bit of a float is not moved to the next page.
const SLACK: f64 = 1e-6;

/// The paper a Document is printed on, in points.
///
/// Plain data the caller builds: the `[export]` table names the paper and the
/// margin, and Print's own page setup names them again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// The paper's width.
    pub width: f64,
    /// The paper's height.
    pub height: f64,
    /// The margin on all four sides. The header and the footer stand inside
    /// it, so it is the body band's edge too.
    pub margin: f64,
    /// Whether the Document's name is printed in the top margin.
    pub header: bool,
    /// Whether the page number is printed in the bottom margin.
    pub footer: bool,
    /// Whether an unnumbered title page stands before the body.
    pub title_page: bool,
}

impl Geometry {
    /// The page the `[export]` table asks for, in the points a page is laid
    /// out in.
    ///
    /// `auto` is resolved here rather than kept, so a writer who carries a
    /// laptop across an ocean exports on the paper their desktop now names
    /// ([`Export::paper_size`]).
    ///
    /// Every sink that reads the table lays its page out on this — Quick
    /// Export, the Export dialog, and Print before its own Page Setup has had
    /// a say — so an export and a print of one table are one page.
    #[must_use]
    pub fn of(export: &Export) -> Self {
        let (width, height) = export.paper_size();
        Self {
            width,
            height,
            margin: export.margin_points(),
            header: export.header,
            footer: export.footer,
            title_page: export.title_page,
        }
    }
}

/// What the furniture says: the Document's name, and the title page's three
/// strings as the front matter has them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Wording {
    /// The Document's name, which is what the header prints.
    pub name: String,
    /// The title page's title. The Document's name when the front matter names
    /// none.
    pub title: Option<String>,
    /// The title page's author.
    pub author: Option<String>,
    /// The title page's date.
    pub date: Option<String>,
}

impl Wording {
    /// The title page's rule: the front matter's `title`, and the Document's
    /// own name when it names none.
    ///
    /// What the title page prints and what the PDF file carries as its Title
    /// ([`crate::pdf::write`]), which are one string and not two.
    #[must_use]
    pub fn title_or_name(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.name.clone())
    }
}

/// Where the text stands on the paper, and what the render pass is called with.
///
/// [`frame`] is where it comes from; every measurement is in points, from the
/// paper's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The paper it was measured on.
    pub paper: Geometry,
    /// The text block's width: the smaller of the Template's measure and the
    /// paper less twice the margin. The measure the render pass is called with.
    pub measure: f64,
    /// The whole percentage the render pass is called with, which puts the
    /// Template's body at [`Frame::body`] on a context [`in_points`] has set up.
    pub zoom: u32,
    /// The body size.
    pub body: f64,
    /// The text block's left edge: the measure centred on the paper.
    pub left: f64,
    /// The body band's top edge, which is the margin: the header stands inside
    /// the margin rather than pushing the text down.
    pub top: f64,
    /// The body band's bottom edge, the footer likewise inside the margin.
    pub bottom: f64,
    /// The size the header and the footer are set at.
    pub furniture: f64,
    /// The size the title page's title is set at: the Template's heading 1.
    pub title: f64,
    /// The line pitch, which is what the title page's lines are spaced by.
    pub leading: f64,
}

/// Whether a fragment's block stands on the Template's Well ground, and which
/// end of that ground the fragment carries.
///
/// A code block cut across a page break is drawn as one ground per page, so
/// the drawer is told which page opens it and which closes it rather than
/// deriving it from the fragments around it. A quotation is [`Ground::None`]
/// on every page it runs over: the Template gives it an indent and no ground
/// ([`crate::render::Block::ground`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ground {
    /// The block does not stand on the Well ground.
    None,
    /// The whole block is here: the ground opens and closes on this page.
    Whole,
    /// The block starts here and runs on: the ground opens and is left open.
    Opens,
    /// Neither end of the block is here: the ground runs the fragment's height.
    Continues,
    /// The block ends here, having opened on an earlier page.
    Closes,
}

/// A run of one layout's lines, and where its first line stands.
///
/// The lines that follow keep the layout's own spacing, so a drawer walks the
/// layout's lines from [`Run::lines`] and offsets them all by the same amount.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The index into [`crate::render::Block::layouts`] of the layout it is a
    /// run of.
    pub placed: usize,
    /// The lines of that layout the run carries, as Pango counts them.
    pub lines: Range<usize>,
    /// The first line's left edge, from the paper's left edge: the text
    /// block's inset and the layout's own indent, together.
    pub x: f64,
    /// The first line's top edge, from the paper's top edge.
    pub y: f64,
}

/// As much of one rendered block as one page carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Fragment {
    /// The index into [`crate::render::Page::blocks`] of the block it is part
    /// of, which is also where its kind, its source range and its layouts are.
    pub block: usize,
    /// Its top edge, from the paper's top edge.
    pub y: f64,
    /// Its height.
    pub height: f64,
    /// The Well ground under it, and which end of it this is.
    pub ground: Ground,
    /// Its lines, in reading order. Empty for a thematic break, which is room
    /// for a hairline and no layout at all.
    pub runs: Vec<Run>,
}

/// What a furniture line is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// The Document's name, in the top margin.
    Header,
    /// The page number, in the bottom margin.
    Footer,
    /// The title page's title.
    Title,
    /// The title page's author.
    Author,
    /// The title page's date.
    Date,
}

/// One line of furniture: what it says, how big, and where its baseline sits.
///
/// Centred on the paper, which is where the text block is centred too. The
/// drawer lays the text out itself — one short line per page is not worth a
/// layout kept from here — and paints it in the ink [`Role`] names.
#[derive(Clone, Debug, PartialEq)]
pub struct Furniture {
    /// What it is.
    pub role: Role,
    /// What it says.
    pub text: String,
    /// The size it is set at.
    pub size: f64,
    /// Its baseline, from the paper's top edge.
    pub baseline: f64,
}

/// One page of paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The number it prints, and `None` for the title page: the body starts at
    /// 1 whether or not a title page stands before it.
    pub number: Option<u32>,
    /// The blocks, or the parts of them, that stand on it, in reading order.
    pub fragments: Vec<Fragment>,
    /// The header, the footer, or the title page's lines.
    pub furniture: Vec<Furniture>,
}

/// Sets `context` to lay out in points, which is what a page is measured in.
///
/// A Pango context that names no resolution answers with the screen's 96 dots
/// to the inch, and a page laid out in those against a paper named in points
/// would wrap a third too wide. Every caller of [`pages`] renders on a context
/// this has been called on.
pub fn in_points(context: &pango::Context) {
    pangocairo::functions::context_set_resolution(context, POINTS);
}

/// The frame `paper` puts a `template` body of `size` points in.
///
/// The measure is the smaller of the Template's own and what the paper leaves
/// between its margins, centred; the zoom is what draws the Template's base
/// size at `size` on a context [`in_points`] has set up, held to the
/// percentages the render pass takes ([`crate::settings::preview_zooms`]).
#[must_use]
pub fn frame(paper: Geometry, template: &Template, size: f64) -> Frame {
    let measure = (template.rhythm.measure * size).min(paper.width - 2.0 * paper.margin);
    Frame {
        paper,
        measure,
        zoom: zoom(template, size),
        body: size,
        left: (paper.width - measure) / 2.0,
        top: paper.margin,
        bottom: paper.height - paper.margin,
        furniture: size * FURNITURE,
        title: size * template.sizes.heading(1) / template.sizes.base,
        leading: size * template.rhythm.line_height,
    }
}

/// The whole percentage that draws `template`'s base size at `size` points on
/// a context [`in_points`] has set up.
///
/// The one rounding on this page: the render pass takes a whole percentage and
/// holds it to [`crate::settings::preview_zooms`], so a text size the ladder
/// cannot name lands on the nearest one it can.
fn zoom(template: &Template, size: f64) -> u32 {
    let zooms = preview_zooms();
    ((100.0 * size / template.sizes.base).round() as u32).clamp(*zooms.start(), *zooms.end())
}

/// A Document laid out on paper: everything a sink needs to draw it.
///
/// [`lay_out`] is where it comes from, and the four travel together because a
/// page cannot be drawn without all four: the fragments point into
/// [`Laid::rendered`], their coordinates are [`Laid::frame`]'s, and the
/// metadata a file carries is [`Laid::wording`]'s.
pub struct Laid {
    /// The frame the pages were cut under.
    pub frame: Frame,
    /// The Document as the render pass laid it out, which is what a page's
    /// fragments point into.
    pub rendered: render::Page,
    /// The pages themselves, in order.
    pub pages: Vec<Page>,
    /// What the furniture and the metadata say, read off the Document
    /// ([`crate::draw::wording`]).
    pub wording: Wording,
}

/// `document` on `paper`, laid out in `template` at `size` points and cut into
/// pages.
///
/// The whole of what a page sink does before it draws: `context` is set to lay
/// out in points ([`in_points`]), [`frame`] measures the text block on the
/// paper, the render pass lays the Document out at that measure, and [`pages`]
/// cuts what comes back. `toggles` is how the blocks are laid out
/// ([`crate::render::Toggles`]).
///
/// Both sinks call this and neither lays a page out itself — the PDF writer on
/// a surface's own Pango context ([`crate::pdf::write`]) and Print on the one
/// the print system hands its `begin-print` — so a print and an export of one
/// Document are the same pages.
#[must_use]
pub fn lay_out(
    document: &Document,
    template: &Template,
    toggles: render::Toggles,
    paper: Geometry,
    size: f64,
    context: &pango::Context,
) -> Laid {
    in_points(context);
    let frame = frame(paper, template, size);
    let rendered = render::render(
        document,
        template,
        toggles,
        frame.measure,
        frame.zoom,
        context,
    );
    let wording = draw::wording(document);
    let pages = pages(&rendered, &frame, &wording);
    Laid {
        frame,
        rendered,
        pages,
        wording,
    }
}

/// `rendered` cut into pages, under the geometry `frame` was measured for.
///
/// `rendered` is the render pass's answer at [`Frame::measure`] and
/// [`Frame::zoom`]; nothing is laid out again here.
///
/// The cut's rules:
///
/// - A heading is never the last thing on a page: it moves with the block after
///   it, and with the run of headings between them.
/// - A paragraph splits between lines with at least two on each side, or moves
///   whole when it cannot.
/// - A code block or a quotation splits at a line boundary. A code block
///   stands on the Template's Well ground, so its [`Fragment::ground`] says
///   which page opens that ground and which closes it; a quotation stands on
///   no ground and carries none over the cut.
/// - A rule never opens a page: it stays at the foot of the page before, inside
///   the bottom margin.
/// - A hard break never opens a page either: the line after one goes with the
///   line it broke.
/// - There is no page-break syntax.
///
/// A block too tall for a whole page is cut anyway rather than moved for ever,
/// and a Document with no blocks is one empty page.
///
/// One walk of the rendered page's blocks, one of each block's lines, and at
/// most one page turn between two of them: the pass is linear in the
/// Document's lines, and nothing here runs on the keystroke lane.
#[must_use]
pub fn pages(rendered: &render::Page, frame: &Frame, wording: &Wording) -> Vec<Page> {
    let table: Vec<Vec<Row>> = rendered.blocks.iter().map(rows).collect();
    let band = frame.bottom - frame.top;
    let mut pages = Vec::new();
    if frame.paper.title_page {
        pages.push(Page {
            number: None,
            fragments: Vec::new(),
            furniture: title_page(frame, wording),
        });
    }
    let mut fragments: Vec<Fragment> = Vec::new();
    let mut number = 1;
    let mut used = 0.0;
    let mut index = 0;
    let mut from = 0;
    while index < rendered.blocks.len() {
        let block = &rendered.blocks[index];
        let rows = &table[index];
        let fresh = fragments.is_empty();
        let gap = if fresh { 0.0 } else { space(rendered, index) };
        let room = band - used - gap;
        let head = rows.get(from).map_or(0.0, |row| row.top);
        let height = block.height - head;
        let y = frame.top + used + gap;
        match step(
            rendered,
            &table,
            index,
            from,
            reach(block, rows) - head,
            room,
            fresh,
        ) {
            Step::Whole => {
                fragments.push(fragment(index, block, rows, from..rows.len(), frame, y));
                used += gap + height;
                index += 1;
                from = 0;
            }
            Step::Split(cut) => {
                fragments.push(fragment(index, block, rows, from..cut, frame, y));
                from = cut;
                turn(
                    &mut pages,
                    &mut fragments,
                    &mut number,
                    &mut used,
                    frame,
                    wording,
                );
            }
            Step::Move => turn(
                &mut pages,
                &mut fragments,
                &mut number,
                &mut used,
                frame,
                wording,
            ),
        }
    }
    if !fragments.is_empty() || pages.iter().all(|page| page.number.is_none()) {
        turn(
            &mut pages,
            &mut fragments,
            &mut number,
            &mut used,
            frame,
            wording,
        );
    }
    pages
}

/// What becomes of the block being placed.
enum Step {
    /// All of what is left of it stands on this page.
    Whole,
    /// It is cut at this row: what is before stands here, what is after opens
    /// the next page.
    Split(usize),
    /// None of it stands here; the page turns and it is tried again.
    Move,
}

/// Which of the three the block at `index` takes, with `room` points left on
/// the page and `reach` points of block still to place.
fn step(
    rendered: &render::Page,
    table: &[Vec<Row>],
    index: usize,
    from: usize,
    reach: f64,
    room: f64,
    fresh: bool,
) -> Step {
    let block = &rendered.blocks[index];
    let rows = &table[index];
    if rows.is_empty() {
        // A rule: no layout, and it never opens a page, so it stands at the
        // foot of this one whether or not the room is there.
        return Step::Whole;
    }
    if heading(block) {
        // A heading moves with what follows it rather than splitting, and a
        // fresh page is the one it cannot be moved off.
        let stands = fresh || (reach <= room + SLACK && follows(rendered, table, index, room));
        return if stands { Step::Whole } else { Step::Move };
    }
    if reach <= room + SLACK {
        return Step::Whole;
    }
    let least = least(block.kind);
    if let Some(cut) = split(rows, from, room, least, least) {
        return Step::Split(cut);
    }
    if !fresh {
        return Step::Move;
    }
    // A fresh page has nowhere to move it to, so it is cut where it can be,
    // and at worst after its first line.
    Step::Split(split(rows, from, room, 1, 1).unwrap_or((from + 1).min(rows.len())))
}

/// How many lines of a block of `kind` must stand on each side of a cut.
///
/// The two-line rule is the paragraph's; a code block and a quotation are
/// listings, and split wherever the page ends.
fn least(kind: render::Kind) -> usize {
    match kind {
        render::Kind::Paragraph => 2,
        _ => 1,
    }
}

/// The row the block is cut at: the last one that fits in `room`, backed off
/// until `least` lines stay, `tail` lines go, and the cut is not a hard break.
///
/// `None` when no cut satisfies all three, which is a block that moves whole.
fn split(rows: &[Row], from: usize, room: f64, least: usize, tail: usize) -> Option<usize> {
    let head = rows.get(from).map_or(0.0, |row| row.top);
    let mut cut = from;
    while cut < rows.len() && rows[cut].bottom - head <= room + SLACK {
        cut += 1;
    }
    while cut > from {
        if cut - from >= least && rows.len() - cut >= tail && !rows[cut].hard {
            return Some(cut);
        }
        cut -= 1;
    }
    None
}

/// Whether what follows the heading at `index` fits under it in `room` points.
///
/// A run of headings is followed down to the first block that is not one, of
/// which [`opens`] is what has to fit: a heading may end a page only when
/// nothing at all follows it in the Document, where moving it would not help.
fn follows(rendered: &render::Page, table: &[Vec<Row>], index: usize, mut room: f64) -> bool {
    let mut at = index;
    loop {
        let block = &rendered.blocks[at];
        let opening = at == index || heading(block);
        let need = if opening {
            reach(block, &table[at])
        } else {
            opens(block, &table[at])
        };
        room -= need
            + if at == index {
                0.0
            } else {
                space(rendered, at)
            };
        if room < -SLACK {
            return false;
        }
        if !opening {
            return true;
        }
        at += 1;
        if at == rendered.blocks.len() {
            return true;
        }
    }
}

/// The least of `block` a page can open a heading's run with: what [`step`]
/// will accept there.
///
/// [`least`] lines of it, which is the smallest cut [`split`] answers, and the
/// whole block when it is shorter than a cut can leave on both sides of
/// itself — a two-line paragraph is one line on each side of a cut and the
/// cut wants two, so it moves whole and a heading over it moves with it.
fn opens(block: &render::Block, rows: &[Row]) -> f64 {
    let least = least(block.kind);
    let whole = reach(block, rows);
    if rows.len() < 2 * least {
        return whole;
    }
    rows.get(least - 1)
        .map_or(whole, |row| row.bottom)
        .min(whole)
}

/// How far down from its top edge `block`'s ink reaches: the bottom of its
/// last line, or its whole height when it has no line to measure.
///
/// What a fit is tested against, rather than [`crate::render::Block::height`]:
/// a layout's box is a whole number of pixels tall, and that last fraction of a
/// point spilling into the margin is not worth a page break.
fn reach(block: &render::Block, rows: &[Row]) -> f64 {
    rows.last().map_or(block.height, |row| row.bottom)
}

/// Whether `block` is a heading.
fn heading(block: &render::Block) -> bool {
    matches!(block.kind, render::Kind::Heading { .. })
}

/// The space the render pass left above the block at `index`.
fn space(rendered: &render::Page, index: usize) -> f64 {
    match index.checked_sub(1).and_then(|at| rendered.blocks.get(at)) {
        Some(before) => rendered.blocks[index].top - (before.top + before.height),
        None => 0.0,
    }
}

/// Closes the page the fragments stand on and starts the next.
fn turn(
    pages: &mut Vec<Page>,
    fragments: &mut Vec<Fragment>,
    number: &mut u32,
    used: &mut f64,
    frame: &Frame,
    wording: &Wording,
) {
    pages.push(Page {
        number: Some(*number),
        fragments: mem::take(fragments),
        furniture: furniture(frame, wording, *number),
    });
    *number += 1;
    *used = 0.0;
}

/// The rows `span` of `rendered`, placed with their first line at `y`.
fn fragment(
    block: usize,
    rendered: &render::Block,
    rows: &[Row],
    span: Range<usize>,
    frame: &Frame,
    y: f64,
) -> Fragment {
    let head = rows.get(span.start).map_or(0.0, |row| row.top);
    let mut runs: Vec<Run> = Vec::new();
    for row in &rows[span.clone()] {
        match runs.last_mut() {
            Some(run) if run.placed == row.placed && run.lines.end == row.line => {
                run.lines.end = row.line + 1;
            }
            _ => runs.push(Run {
                placed: row.placed,
                lines: row.line..row.line + 1,
                x: frame.left + rendered.layouts[row.placed].x,
                y: y + row.top - head,
            }),
        }
    }
    let last = span.end.checked_sub(1).and_then(|at| rows.get(at));
    let height = match (rows.get(span.start), last) {
        (Some(first), Some(last)) => last.bottom - first.top,
        _ => rendered.height,
    };
    let ground = if rendered.ground {
        match (span.start == 0, span.end == rows.len()) {
            (true, true) => Ground::Whole,
            (true, false) => Ground::Opens,
            (false, true) => Ground::Closes,
            (false, false) => Ground::Continues,
        }
    } else {
        Ground::None
    };
    Fragment {
        block,
        y,
        height,
        ground,
        runs,
    }
}

/// The header and the footer of body page `number`.
fn furniture(frame: &Frame, wording: &Wording, number: u32) -> Vec<Furniture> {
    let mut lines = Vec::new();
    let middle = frame.furniture * CAP;
    if frame.paper.header && !wording.name.is_empty() {
        lines.push(Furniture {
            role: Role::Header,
            text: wording.name.clone(),
            size: frame.furniture,
            baseline: frame.paper.margin / 2.0 + middle,
        });
    }
    if frame.paper.footer {
        lines.push(Furniture {
            role: Role::Footer,
            text: number.to_string(),
            size: frame.furniture,
            baseline: frame.paper.height - frame.paper.margin / 2.0 + middle,
        });
    }
    lines
}

/// The title page's lines: the title on a baseline a third of the way down the
/// paper, then a line's space, then the author and the date under it.
fn title_page(frame: &Frame, wording: &Wording) -> Vec<Furniture> {
    let mut lines = Vec::new();
    let mut baseline = frame.paper.height / 3.0;
    let title = wording.title_or_name();
    if !title.is_empty() {
        lines.push(Furniture {
            role: Role::Title,
            text: title,
            size: frame.title,
            baseline,
        });
    }
    let mut under = false;
    for (role, said) in [
        (Role::Author, wording.author.as_ref()),
        (Role::Date, wording.date.as_ref()),
    ] {
        let Some(text) = said.filter(|text| !text.is_empty()) else {
            continue;
        };
        baseline += if under { 1.0 } else { 2.0 } * frame.leading;
        under = true;
        lines.push(Furniture {
            role,
            text: text.clone(),
            size: frame.body,
            baseline,
        });
    }
    lines
}

/// One line of one of a block's layouts, and where it stands under the block's
/// own top edge.
///
/// The one place a page cut is measured: a fragment is a range of these.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    /// The index into [`crate::render::Block::layouts`] of the layout it is a
    /// line of.
    placed: usize,
    /// Its line number inside that layout.
    line: usize,
    /// Its top edge, from the block's top.
    top: f64,
    /// Its bottom edge, from the block's top.
    bottom: f64,
    /// Whether it follows a hard break.
    ///
    /// Prose is the only place a `\n` in a layout's text is one: the render
    /// pass writes a soft break as a space and a hard break as a newline, and
    /// a code block's and a source block's newlines are the file's own.
    hard: bool,
}

/// Every line of every layout of `block`, in reading order.
fn rows(block: &render::Block) -> Vec<Row> {
    let prose = matches!(
        block.kind,
        render::Kind::Paragraph | render::Kind::Quote | render::Kind::List
    );
    let mut rows = Vec::new();
    for (placed, drawn) in block.layouts.iter().enumerate() {
        let text = drawn.layout.text();
        let bytes = text.as_bytes();
        let above = drawn.y - block.top;
        let mut iter = drawn.layout.iter();
        let mut line = 0;
        loop {
            let (top, bottom) = iter.line_yrange();
            let hard = prose
                && iter.line_readonly().is_some_and(|at| {
                    usize::try_from(at.start_index())
                        .ok()
                        .and_then(|start| bytes.get(start.wrapping_sub(1)))
                        == Some(&b'\n')
                });
            rows.push(Row {
                placed,
                line,
                top: above + back(top),
                bottom: above + back(bottom),
                hard,
            });
            line += 1;
            if !iter.next_line() {
                break;
            }
        }
    }
    rows
}

/// `units` of Pango's, as the points a page is measured in.
///
/// The drawer works in the same points off the same layouts, so it reads them
/// back through this rather than through a second copy of it.
pub(crate) fn back(units: i32) -> f64 {
    f64::from(units) / f64::from(pango::SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Toggles;
    use crate::settings::Paper;
    use crate::template;

    /// The paper a page is cut on here: A4 at the `[export]` defaults, read
    /// off the table rather than written out again.
    fn a4() -> Geometry {
        Geometry {
            width: Paper::A4.size().0,
            ..Geometry::of(&Export::default())
        }
    }

    /// The body size every page here is set at: the `[export]` default.
    fn size() -> f64 {
        f64::from(Export::default().text_size)
    }

    /// Long enough to wrap to four lines and more at the Template's measure,
    /// which is what a paragraph must do to be split at all.
    const PROSE: &str = "The keeper wrote every evening, in a hand that the salt \
        air had not yet reached, about the weather and the ships and the way the \
        light swung out over the water and came back with nothing on it; and when \
        the winter storms came in from the south-west he wrote about those too, \
        which is how we know what the winter of that year was like on the rock, \
        and how long the boats were kept away from it.";

    /// Long enough to wrap to two lines and no longer, which is a paragraph
    /// that cannot be split at all: a cut leaves two lines on each side of it
    /// and there are only two to leave.
    const PAIR: &str = "The keeper wrote every evening about the weather and \
        the ships, and about the way the light swung out over the water.";

    /// Nine lines of code, which is enough for a block to run across three
    /// pages and be neither opened nor closed on the middle one.
    const CODE: &str = "```rust\n\
        let one = 1;\nlet two = 2;\nlet three = 3;\nlet four = 4;\nlet five = 5;\n\
        let six = 6;\nlet seven = 7;\nlet eight = 8;\nlet nine = 9;\n```";

    /// A Pango context with no display behind it, laying out in points, which
    /// is what makes a page's geometry testable in the engine at all.
    fn context() -> pango::Context {
        use pango::prelude::FontMapExt;

        let context = pangocairo::FontMap::default().create_context();
        in_points(&context);
        context
    }

    /// The default Template, which is the one every page here is set in.
    fn modern() -> Template {
        template::built_in("modern").expect("a built-in Template")
    }

    /// A frame on A4-wide paper whose body band is `room` points tall, with no
    /// furniture.
    fn paper(room: f64) -> Frame {
        switched(room, false, false, false)
    }

    /// The same, with the header, footer and title page as asked for.
    fn switched(room: f64, header: bool, footer: bool, title_page: bool) -> Frame {
        let a4 = a4();
        frame(
            Geometry {
                height: room + 2.0 * a4.margin,
                header,
                footer,
                title_page,
                ..a4
            },
            &modern(),
            size(),
        )
    }

    /// `text` rendered for `frame`.
    ///
    /// Every frame here is the same width, so one render serves however many
    /// rooms a test measures a cut against.
    fn rendered(text: &str, frame: &Frame) -> render::Page {
        let mut document = Document::untitled();
        document.reload(text.to_string());
        render::render(
            &document,
            &modern(),
            Toggles::default(),
            frame.measure,
            frame.zoom,
            &context(),
        )
    }

    /// The block each fragment carries and the lines of it: a page's shape.
    fn carried(page: &Page) -> Vec<(usize, Range<usize>)> {
        page.fragments
            .iter()
            .map(|fragment| {
                let lines = match (fragment.runs.first(), fragment.runs.last()) {
                    (Some(first), Some(last)) => first.lines.start..last.lines.end,
                    _ => 0..0,
                };
                (fragment.block, lines)
            })
            .collect()
    }

    /// What furniture `page` carries, in the order it answers it.
    fn roles(page: &Page) -> Vec<Role> {
        page.furniture.iter().map(|line| line.role).collect()
    }

    /// The `[export]` table becomes a page geometry: the paper's own size in
    /// points, the margin converted from millimetres, and the three switches
    /// carried straight through.
    #[test]
    fn the_export_table_becomes_the_page_geometry() {
        let mut export = Export::default();
        export.paper = Paper::A4;
        export.footer = true;
        let paper = Geometry::of(&export);
        assert_eq!((paper.width, paper.height), Paper::A4.size());
        assert_eq!(paper.margin, export.margin_points());
        assert_eq!(
            (paper.header, paper.footer, paper.title_page),
            (false, true, false)
        );
        assert_eq!(
            Geometry::of(&Export::default()).width,
            Paper::default().size().0,
            "auto is resolved to a size, never left as auto"
        );
    }

    #[test]
    fn a_one_block_document_is_one_page() {
        let frame = paper(600.0);
        let rendered = rendered("A paragraph.", &frame);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].number, Some(1));
        assert_eq!(carried(&pages[0]), vec![(0, 0..1)]);
        assert_eq!(pages[0].fragments[0].ground, Ground::None);
        assert_eq!(pages[0].fragments[0].y, frame.top);
        assert_eq!(pages[0].fragments[0].runs[0].x, frame.left);
        assert!(pages[0].furniture.is_empty());
    }

    #[test]
    fn a_paragraph_splits_with_two_lines_on_each_side_of_the_cut() {
        let rendered = rendered(PROSE, &paper(10_000.0));
        let rows = rows(&rendered.blocks[0]);
        assert!(
            rows.len() >= 4,
            "the fixture wraps to four lines or more, not {}",
            rows.len()
        );
        // Room for every line but the last, so that the cut that fits would
        // leave one line over and has to be backed off.
        let frame = paper(rows[rows.len() - 2].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(pages.len(), 2);
        assert_eq!(carried(&pages[0]), vec![(0, 0..rows.len() - 2)]);
        assert_eq!(carried(&pages[1]), vec![(0, rows.len() - 2..rows.len())]);
        assert_eq!(pages[1].fragments[0].y, frame.top);
    }

    #[test]
    fn a_paragraph_with_room_for_one_line_moves_whole() {
        let tall = paper(10_000.0);
        let text = format!(
            "{PROSE}\n\nA second paragraph, shorter than the first one, but longer than a single line of it."
        );
        let rendered = rendered(&text, &tall);
        let first = rows(&rendered.blocks[0]);
        let rows = rows(&rendered.blocks[1]);
        assert!(rows.len() >= 2, "the second paragraph wraps");
        // Room for the first paragraph, the space under it and one line more.
        let frame = paper(rendered.blocks[0].height + space(&rendered, 1) + rows[0].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..first.len())]);
        assert_eq!(carried(&pages[1]), vec![(1, 0..rows.len())]);
    }

    #[test]
    fn a_heading_that_would_end_a_page_moves_with_the_block_after_it() {
        let tall = paper(10_000.0);
        let text = format!("A first paragraph.\n\n## A heading\n\n{PROSE}");
        let rendered = rendered(&text, &tall);
        let rows = rows(&rendered.blocks[2]);
        // Room for the paragraph, the heading, and half a line of the
        // paragraph under it: the heading fits, and nothing may follow it.
        let heading = &rendered.blocks[1];
        let frame =
            paper(heading.top + heading.height + space(&rendered, 2) + rows[0].bottom / 2.0);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..1)]);
        assert_eq!(pages[1].fragments[0].block, 1);
        assert_eq!(pages[1].fragments[1].block, 2);
    }

    /// One line of the paragraph under it is not enough to hold a heading on a
    /// page: the cut the paragraph would then have to take leaves two lines on
    /// each side of it, so what stands under the heading is nothing at all.
    #[test]
    fn a_heading_moves_when_only_one_line_of_the_paragraph_under_it_would_fit() {
        let tall = paper(10_000.0);
        let text = format!("A first paragraph.\n\n## A heading\n\n{PROSE}");
        let rendered = rendered(&text, &tall);
        let rows = rows(&rendered.blocks[2]);
        assert!(
            rows.len() >= 4,
            "the fixture wraps to four lines or more, not {}",
            rows.len()
        );
        let heading = &rendered.blocks[1];
        // Room for the paragraph, the heading, and one whole line of the
        // paragraph under it: one line short of the cut it would take.
        let frame = paper(heading.top + heading.height + space(&rendered, 2) + rows[0].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..1)]);
        assert_eq!(pages[1].fragments[0].block, 1);
        assert_eq!(pages[1].fragments[1].block, 2);
    }

    /// A paragraph too short to be cut at all moves whole, so the heading over
    /// it needs the room for the whole of it: the two of them turn the page
    /// together.
    #[test]
    fn a_heading_and_a_paragraph_that_cannot_be_cut_move_together() {
        let tall = paper(10_000.0);
        let text = format!("A first paragraph.\n\n## A heading\n\n{PAIR}");
        let rendered = rendered(&text, &tall);
        let rows = rows(&rendered.blocks[2]);
        assert_eq!(rows.len(), 2, "the fixture wraps to two lines");
        let heading = &rendered.blocks[1];
        // Room for the paragraph, the heading, and one of the two lines under
        // it, which is a line more than the pair can be split at.
        let frame = paper(heading.top + heading.height + space(&rendered, 2) + rows[0].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..1)]);
        assert_eq!(carried(&pages[1]), vec![(1, 0..1), (2, 0..2)]);
    }

    #[test]
    fn a_code_block_carries_its_ground_across_the_break() {
        let rendered = rendered(CODE, &paper(10_000.0));
        let rows = rows(&rendered.blocks[0]);
        assert_eq!(rows.len(), 9);
        let frame = paper(rows[2].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(pages.len(), 3);
        assert_eq!(carried(&pages[0]), vec![(0, 0..3)]);
        assert_eq!(carried(&pages[1]), vec![(0, 3..6)]);
        assert_eq!(carried(&pages[2]), vec![(0, 6..9)]);
        assert_eq!(
            pages
                .iter()
                .map(|page| page.fragments[0].ground)
                .collect::<Vec<_>>(),
            vec![Ground::Opens, Ground::Continues, Ground::Closes]
        );
    }

    #[test]
    fn a_rule_stays_at_the_foot_of_the_page_before_rather_than_opening_one() {
        let tall = paper(10_000.0);
        let rendered = rendered("A paragraph.\n\n---\n\nAnother paragraph.", &tall);
        assert_eq!(rendered.blocks[1].kind, render::Kind::Rule);
        // Room for the first paragraph and nothing else.
        let frame = paper(rendered.blocks[0].height);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..1), (1, 0..0)]);
        assert_eq!(carried(&pages[1]), vec![(2, 0..1)]);
    }

    #[test]
    fn a_hard_break_never_opens_a_page() {
        let tall = paper(10_000.0);
        let text = format!("{PROSE}\n\none  \ntwo  \nthree  \nfour");
        let rendered = rendered(&text, &tall);
        let first = rows(&rendered.blocks[0]);
        let rows = rows(&rendered.blocks[1]);
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter().map(|row| row.hard).collect::<Vec<_>>(),
            vec![false, true, true, true]
        );
        // Room for the first paragraph and two of the four broken lines, which
        // is a cut the two-line rule would allow and the hard break does not.
        let frame = paper(rendered.blocks[0].height + space(&rendered, 1) + rows[1].bottom);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(carried(&pages[0]), vec![(0, 0..first.len())]);
        assert_eq!(carried(&pages[1]), vec![(1, 0..4)]);
    }

    #[test]
    fn a_title_page_is_unnumbered_and_the_body_starts_at_one() {
        let frame = switched(600.0, true, true, true);
        let rendered = rendered("A paragraph.", &frame);
        let wording = Wording {
            name: "The Lighthouse".to_string(),
            title: None,
            author: Some("A Keeper".to_string()),
            date: Some("1902".to_string()),
        };
        let pages = pages(&rendered, &frame, &wording);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].number, None);
        assert!(pages[0].fragments.is_empty());
        assert_eq!(
            roles(&pages[0]),
            vec![Role::Title, Role::Author, Role::Date]
        );
        // The title falls back to the Document's name, at the Template's
        // heading 1, on a baseline a third of the way down the paper.
        assert_eq!(pages[0].furniture[0].text, "The Lighthouse");
        assert_eq!(pages[0].furniture[0].size, frame.title);
        assert_eq!(pages[0].furniture[0].baseline, frame.paper.height / 3.0);
        assert_eq!(pages[0].furniture[1].size, frame.body);
        assert_eq!(pages[1].number, Some(1));
        assert_eq!(roles(&pages[1]), vec![Role::Header, Role::Footer]);
        assert_eq!(pages[1].furniture[0].text, "The Lighthouse");
        assert_eq!(pages[1].furniture[1].text, "1");
    }

    #[test]
    fn a_footer_on_a_one_page_document_says_one() {
        let frame = switched(600.0, false, true, false);
        let rendered = rendered("A paragraph.", &frame);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(pages.len(), 1);
        assert_eq!(roles(&pages[0]), vec![Role::Footer]);
        assert_eq!(pages[0].furniture[0].text, "1");
        assert_eq!(pages[0].furniture[0].size, frame.body * FURNITURE);
        assert!(pages[0].furniture[0].baseline > frame.bottom);
        assert!(pages[0].furniture[0].baseline < frame.paper.height);
    }

    #[test]
    fn a_document_with_no_blocks_is_one_page() {
        let frame = paper(600.0);
        let rendered = rendered("", &frame);
        let pages = pages(&rendered, &frame, &Wording::default());
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].number, Some(1));
        assert!(pages[0].fragments.is_empty());
    }

    #[test]
    fn the_measure_is_the_smaller_of_the_templates_and_what_the_margins_leave() {
        let template = modern();
        let wide = paper(600.0);
        assert_eq!(wide.measure, template.rhythm.measure * size());
        assert_eq!(wide.left, (a4().width - wide.measure) / 2.0);
        let margin = a4().margin;
        let narrow = frame(
            Geometry {
                width: 300.0,
                height: 500.0,
                ..a4()
            },
            &template,
            size(),
        );
        assert_eq!(narrow.measure, 300.0 - 2.0 * margin);
        // The measure the margins leave, centred, is the margin again — to the
        // last bit of the halving, which is not the same arithmetic.
        assert!((narrow.left - margin).abs() < SLACK, "{}", narrow.left);
        assert_eq!(narrow.top, margin);
        assert_eq!(narrow.bottom, 500.0 - margin);
    }

    #[test]
    fn the_zoom_puts_the_templates_body_at_the_text_size() {
        let template = modern();
        assert_eq!(paper(600.0).zoom, 75);
        assert_eq!(render::resolution(&context()), POINTS);
        for size in [9.0, 12.0, 18.0] {
            let frame = switched(600.0, false, false, false);
            let frame = self::frame(frame.paper, &template, size);
            let drawn = template.sizes.base * render::scale(frame.zoom, POINTS);
            assert!(
                (drawn - size).abs() < 0.1,
                "a {size} pt body is drawn at {drawn} pt at {}%",
                frame.zoom
            );
        }
    }
}
