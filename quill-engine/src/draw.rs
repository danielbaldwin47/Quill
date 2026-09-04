//! Painting one page of paper, on whatever cairo context the sink hands over.
//!
//! [`crate::render`] lays a Document out and [`crate::paginate`] cuts it into
//! pages; this is the one place either of them is turned into ink. Both of
//! Export's page sinks draw through it (`docs/architecture.md` § Preview and
//! Export): the PDF writer ([`crate::pdf`]) on a `cairo::PdfSurface` of its
//! own, and Print on the context a `GtkPrintOperation` hands its `draw-page`
//! callback. The context is the caller's, scaled to points and otherwise
//! untouched, so a printer's own origin and margins are respected.
//!
//! The paper is the Template's **light** palette whatever ground the app is
//! wearing: a dark theme is a screen's comfort, and paper is white.
//!
//! The furniture — the header, the footer and the title page's lines — is laid
//! out here rather than by the paginator, which measures it and says where its
//! baseline sits: one short line per page is not worth a Pango layout carried
//! across a module boundary.

use std::ops::Range;

use crate::document::Document;
use crate::front_matter;
use crate::paginate::{Fragment, Frame, Furniture, Ground, Page, Role, Run, Wording};
use crate::render;
use crate::template::{Palette, Template};
use crate::theme::Colour;

/// How far the Well ground stands outside the text it is under, in points.
///
/// The Preview's own inset (`quill::preview`), in the unit a page is measured
/// in: a code block on paper is not a code block on screen with a different
/// gutter.
const WELL: f64 = 2.0;

/// A thematic break's rule, in points: the thinnest line a press reliably
/// holds, which is what a hairline means on paper.
const HAIRLINE: f64 = 0.5;

/// The weight the furniture is set at, as [`crate::render`] sets prose.
const BODY: u16 = 400;

/// Paints `page` of `rendered` onto `cr`, in points from the paper's corner.
///
/// `rendered` is the page [`crate::paginate::pages`] cut, and `frame` the one
/// it was cut under; a fragment naming a block or a layout that page does not
/// hold is passed over rather than panicking, so a stale pair draws less
/// rather than dying in a print callback.
///
/// The order is the order a press lays ink down: the paper, then each
/// fragment's Well ground, its rule or its lines, then the furniture. Nothing
/// is laid out but the furniture, and nothing outside the paper is drawn.
pub fn draw(
    cr: &cairo::Context,
    page: &Page,
    rendered: &render::Page,
    template: &Template,
    frame: &Frame,
) {
    let palette = template.light;
    ink(cr, palette.paper);
    cr.rectangle(0.0, 0.0, frame.paper.width, frame.paper.height);
    let _ = cr.fill();
    for fragment in &page.fragments {
        let Some(block) = rendered.blocks.get(fragment.block) else {
            continue;
        };
        wash(cr, palette.code_ground, &ground(fragment, frame));
        if block.kind == render::Kind::Rule {
            wash(cr, palette.muted, &[rule(fragment, frame)]);
        }
        for run in &fragment.runs {
            if let Some(placed) = block.layouts.get(run.placed) {
                lines(cr, placed, run, palette);
            }
        }
    }
    for line in &page.furniture {
        furniture(cr, line, template, frame, palette.muted);
    }
}

/// The [`Wording`] `document` prints: its name, and what its front matter says
/// the title page carries.
///
/// Here rather than in either sink because both of them print it: the PDF
/// writer and Print set the same header on the same paper.
#[must_use]
pub fn wording(document: &Document) -> Wording {
    let found = front_matter::read(document.text());
    Wording {
        name: document.name().into_owned(),
        title: found.title,
        author: found.author,
        date: found.date,
    }
}

/// A rectangle of paper, in points from its top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Patch {
    /// Its left edge.
    x: f64,
    /// Its top edge.
    y: f64,
    /// Its width.
    width: f64,
    /// Its height.
    height: f64,
}

/// The Well ground under `fragment`, or nothing when it stands on none.
///
/// A block cut across a page break is one ground per page, and the end of it
/// the page carries is the end that is padded: an opened ground runs off the
/// foot of its page and a closed one runs on from the head of its own, so the
/// two halves read as one well.
fn ground(fragment: &Fragment, frame: &Frame) -> Vec<Patch> {
    let (above, below) = match fragment.ground {
        Ground::None => return Vec::new(),
        Ground::Whole => (WELL, WELL),
        Ground::Opens => (WELL, 0.0),
        Ground::Continues => (0.0, 0.0),
        Ground::Closes => (0.0, WELL),
    };
    vec![Patch {
        x: frame.left - WELL,
        y: fragment.y - above,
        width: frame.measure + 2.0 * WELL,
        height: fragment.height + above + below,
    }]
}

/// The hairline a thematic break's fragment is room for, through its middle.
fn rule(fragment: &Fragment, frame: &Frame) -> Patch {
    Patch {
        x: frame.left,
        y: fragment.y + (fragment.height - HAIRLINE) / 2.0,
        width: frame.measure,
        height: HAIRLINE,
    }
}

/// Draws the lines `run` names of `placed`, with the first of them at
/// [`Run::y`].
///
/// One walk of the layout's lines, whatever the run's span: Pango has no
/// random access to a line's position, and a heading or a paragraph is a
/// handful of lines.
fn lines(cr: &cairo::Context, placed: &render::Placed, run: &Run, palette: Palette) {
    let mut iter = placed.layout.iter();
    let mut at = 0;
    let mut origin = None;
    loop {
        if run.lines.contains(&at) {
            let (top, bottom) = iter.line_yrange();
            let origin = *origin.get_or_insert(run.y - back(top));
            if let Some(line) = iter.line_readonly() {
                let band = Patch {
                    x: run.x,
                    y: origin + back(top),
                    width: 0.0,
                    height: back(bottom - top),
                };
                for at in &placed.code {
                    wash(cr, palette.code_ground, &edges(&line, at, band));
                }
                let (x, y) = (
                    run.x + back(line_x(&mut iter)),
                    origin + back(iter.baseline()),
                );
                ink(cr, palette.ink);
                cr.move_to(x, y);
                pangocairo::functions::show_layout_line(cr, &line);
                // A Pango layout is one colour, so a link's own ink is the
                // same line drawn again through a clip of its words: the
                // second pass lands exactly on the glyphs the first drew.
                for link in &placed.links {
                    for patch in edges(&line, &link.at, band) {
                        let _ = cr.save();
                        cr.rectangle(patch.x, patch.y, patch.width, patch.height);
                        cr.clip();
                        ink(cr, palette.link);
                        cr.move_to(x, y);
                        pangocairo::functions::show_layout_line(cr, &line);
                        let _ = cr.restore();
                    }
                }
            }
        }
        at += 1;
        if !iter.next_line() {
            break;
        }
    }
}

/// The patches the bytes `at` of `line` are drawn in, one per visual run they
/// fall in, in `band`'s row of the paper.
///
/// Empty when the range touches none of the line, which is every line of a
/// layout but the one or two an inline span runs across.
fn edges(line: &pango::LayoutLine, at: &Range<usize>, band: Patch) -> Vec<Patch> {
    let start = line.start_index();
    let end = start.saturating_add(line.length());
    let (from, to) = (index(at.start).max(start), index(at.end).min(end));
    if from >= to {
        return Vec::new();
    }
    line.x_ranges(from, to)
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Patch {
            x: band.x + back(pair[0]),
            width: back(pair[1] - pair[0]),
            ..band
        })
        .collect()
}

/// Lays `line` out and paints it in `colour`, centred on the paper with its
/// baseline where the paginator put it.
fn furniture(
    cr: &cairo::Context,
    line: &Furniture,
    template: &Template,
    frame: &Frame,
    colour: Colour,
) {
    let (face, weight) = match line.role {
        Role::Title => (&template.faces.heading, template.headings.weight),
        Role::Header | Role::Footer | Role::Author | Role::Date => (&template.faces.body, BODY),
    };
    let mut font = pango::FontDescription::new();
    font.set_family(&face.family);
    font.set_weight(render::weight(weight));
    font.set_absolute_size(line.size * f64::from(pango::SCALE));
    let laid = pangocairo::functions::create_layout(cr);
    laid.set_font_description(Some(&font));
    laid.set_text(&line.text);
    let (_, logical) = laid.extents();
    ink(cr, colour);
    cr.move_to(
        (frame.paper.width - back(logical.width())) / 2.0,
        line.baseline - back(laid.baseline()),
    );
    pangocairo::functions::show_layout(cr, &laid);
}

/// Fills `patches` with `colour`, in one path.
fn wash(cr: &cairo::Context, colour: Colour, patches: &[Patch]) {
    if patches.is_empty() {
        return;
    }
    ink(cr, colour);
    for patch in patches {
        cr.rectangle(patch.x, patch.y, patch.width, patch.height);
    }
    let _ = cr.fill();
}

/// Sets `cr`'s source to `colour`.
fn ink(cr: &cairo::Context, colour: Colour) {
    cr.set_source_rgba(colour.red, colour.green, colour.blue, colour.alpha);
}

/// The left edge of the line `iter` is on, inside its layout: what a centred
/// heading's alignment comes to, which [`Run::x`] cannot carry because it is
/// one number for a run of lines.
fn line_x(iter: &mut pango::LayoutIter) -> i32 {
    let (_, logical) = iter.line_extents();
    logical.x()
}

/// `units` of Pango's, as the points a page is measured in.
fn back(units: i32) -> f64 {
    f64::from(units) / f64::from(pango::SCALE)
}

/// A byte offset as Pango counts them: a range past what an `i32` holds is a
/// Document no layout of it could carry, and clamping it there selects nothing
/// rather than wrapping to something.
fn index(at: usize) -> i32 {
    i32::try_from(at).unwrap_or(i32::MAX)
}
