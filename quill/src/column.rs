//! The pages beside the Editor: what Export writes, shown live.
//!
//! The Preview pane has two modes (`docs/architecture.md` § Preview and
//! Export). The Web mode is a sheet of the Document rendered in the Template
//! ([`crate::preview`]); this is the other one, a column of the pages Export
//! would write. The engine's paginator cuts the Document into pages at the
//! geometry `[export]` names ([`paginate::Geometry::of`]) and the engine's
//! drawer paints each of them ([`draw::draw`]) — the same two calls the PDF
//! writer and Print make, so the page in the pane and the page in the file are
//! one drawing.
//!
//! Three things separate it from the sheet:
//!
//! - It paints through cairo rather than a snapshot's layout nodes, because
//!   the drawer's one sink is a `cairo::Context`. Each visible page is one
//!   [`gtk::Snapshot::append_cairo`] node over that page's rectangle, scaled
//!   from points to the pane's pixels.
//! - It lays out on a Pango context of its own
//!   ([`gtk::prelude::WidgetExt::create_pango_context`]) and never the widget's
//!   shared one: [`paginate::lay_out`] puts the context it is handed into
//!   points, and the Editor and the Web sheet render on the screen's own
//!   resolution.
//! - The paper is the Template's light palette whatever the theme is wearing —
//!   the drawer's rule, not this module's — and the surround the pages stand
//!   on is one fixed neutral ([`SURROUND`]) for every Template, so a dark
//!   theme's Editor stands beside a light pane.
//!
//! It is laid out on the pane's own refresh path
//! ([`crate::window::Window::refresh_preview`], 200 ms after the last
//! keystroke of a burst) and whenever the pane's width or the zoom moves;
//! scrolling only repaints, and repaints the pages in view. What the pane's
//! scroller does not do it does itself: a page grown past the pane by the zoom
//! slides sideways under it ([`Column::pan_by`]), because the scroller is the
//! Web sheet's too and a sheet has nothing sideways to scroll.

use std::cell::Ref;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use quill_engine::document::Document;
use quill_engine::draw;
use quill_engine::paginate;
use quill_engine::render;
use quill_engine::settings::{Choice, Settings};
use quill_engine::sync;
use quill_engine::template;

use crate::preview;
use crate::preview::DialogOverride;
use crate::tags::pixels;
use crate::window::Window;

/// The air either side of the page, in logical pixels.
///
/// What fit width fits: the page is scaled to the column's width less this
/// gutter twice, which leaves the paper's edge visible against the surround
/// rather than running it off the pane.
const GUTTER: f64 = 24.0;

/// How far one notch of a sideways wheel slides a page too wide for the pane,
/// in logical pixels.
const PAN: f64 = 48.0;

/// The neutral the pages stand on, `#f7f7f7` in both themes.
///
/// One value for every Template and both grounds, pinned by the spec (#293
/// § The page column): the surround is not paper, it is the light a sheet is
/// read under.
///
/// It is the light theme's own paper to the byte
/// (`quill_engine::theme`'s light ground) — a coincidence, since one is a
/// theme's and the other is fixed against every theme, but one two judged
/// rules lean on: `pdf-full` and `dialog` in `tools/assert-state.mjs` read no
/// edge between the pane and the Editor's paper at the light palette, because
/// at that palette there is none to read.
const SURROUND: f32 = 0.968_627_5;

/// How the pages stand down the column, in the column's own pixels.
///
/// One value built per layout ([`Stack::of`]) rather than a page height and a
/// gap carried apart and a pitch worked out at each place that asks: a page's
/// top, the page a point is on, the page a top edge stands over and the
/// column's whole height are one stacking, and this is it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stack {
    /// How tall one page stands.
    page: f64,
    /// The air between two pages, and above the first and below the last: one
    /// page margin.
    ///
    /// The margin rather than a number of this module's own, so the air
    /// between two pages reads as the air inside one and the column is one
    /// continuous scroll rather than a stack of cards.
    gap: f64,
}

impl Stack {
    /// The stack pages cut on `paper` make when they are drawn at `scale`.
    fn of(paper: paginate::Geometry, scale: f64) -> Self {
        Self {
            page: paper.height * scale,
            gap: paper.margin * scale,
        }
    }

    /// One page and the air under it, which is what a page's top moves by.
    ///
    /// Nought or less is a column with no geometry — a pane that has not been
    /// laid out, or a scale of nothing — and every answer below is `None` or
    /// the offset it was handed.
    fn pitch(self) -> f64 {
        self.page + self.gap
    }

    /// Where page `at` stands: the pages in order, the first one gap down from
    /// the top of the column.
    fn top_of(self, at: usize) -> f64 {
        self.gap + at as f64 * self.pitch()
    }

    /// How tall a column of `pages` pages stands: every page with its gap, and
    /// the gap above the first.
    fn height(self, pages: usize) -> f64 {
        self.gap + pages as f64 * self.pitch()
    }

    /// The page standing at `y`, or nothing where `y` is in the air between two
    /// pages or off either end.
    fn index_at(self, y: f64) -> Option<usize> {
        if self.pitch() <= 0.0 || y < self.gap {
            return None;
        }
        let at = ((y - self.gap) / self.pitch()).floor();
        let at = usize::try_from(at as i64).ok()?;
        (y <= self.top_of(at) + self.page).then_some(at)
    }

    /// The physical page a top edge at `y` stands over, counting the title page
    /// as page 1 as [`paginate::page_words`] does, or nothing where there are
    /// no pages.
    ///
    /// [`Stack::index_at`] answers a point, and a point can land in the air
    /// between two pages or above the first; a top edge always has a page, so
    /// the air answers with the page coming into view under it, and an edge
    /// past the last page with the last.
    fn under(self, y: f64, count: usize) -> Option<usize> {
        if self.pitch() <= 0.0 || count == 0 {
            return None;
        }
        let at = ((y - self.gap) / self.pitch()).floor().max(0.0);
        let mut at = usize::try_from(at as i64).ok()?;
        if y > self.gap && self.index_at(y).is_none() {
            at += 1;
        }
        Some(at.min(count - 1) + 1)
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::paginate::Laid;
    use quill_engine::sync;
    use quill_engine::template::Template;

    use crate::window::Window;

    /// The column of pages.
    #[derive(Default)]
    pub struct Column {
        /// The Document as the paginator last cut it, or nothing until it has
        /// been: the frame, the rendered page the fragments point into, the
        /// pages themselves and what the furniture says.
        pub laid: RefCell<Option<Laid>>,
        /// The Template the pages were laid out in, which the drawer is handed
        /// with them.
        pub template: RefCell<Option<Template>>,
        /// Logical pixels to the point: fit width for the width the pages were
        /// laid out at, stepped by `[preview] zoom`.
        pub scale: Cell<f64>,
        /// The pages as the sync rules read them: a block's key is the
        /// Document's block index, as the sheet's rows are keyed, and its top
        /// and height are the column's own pixels
        /// ([`quill_engine::paginate::column_blocks`]).
        pub blocks: RefCell<Vec<sync::Block>>,
        /// How far the page is slid sideways under a pane too narrow to hold
        /// it, in the column's own pixels; nought whenever it fits.
        pub pan: Cell<f64>,
        /// How tall the whole column stands, in logical pixels — the
        /// scrollable height, kept here for the reason the sheet keeps its
        /// own: a column just laid out again is a height the scroller has not
        /// been allocated at yet.
        pub height: Cell<f64>,
        /// The column's width when the pages were laid out, so an allocation
        /// that did not move it lays nothing out again.
        pub laid_at: Cell<i32>,
        /// Whether a fresh layout is already waiting on the main loop.
        pub pending: Cell<bool>,
        /// What the pane scrolls by, which is what says which pages are in
        /// view.
        pub scroll: RefCell<Option<gtk::Adjustment>>,
        /// The window whose Document is drawn here.
        pub window: RefCell<Option<glib::WeakRef<Window>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Column {
        const NAME: &'static str = "QuillPreviewColumn";
        type Type = super::Column;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Column {}

    impl WidgetImpl for Column {
        /// The whole of the paint: the surround, then the pages in view.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }

        /// A width the pages were not laid out at is a column to lay out
        /// again, because fit width is a function of the width.
        ///
        /// On the main loop rather than here, as the sheet's is: laying out
        /// sets the column's height, and a height set inside an allocation is
        /// an allocation inside an allocation.
        ///
        /// The pan is held here and not there: a pane made wider has less to
        /// slide the moment it is allocated, and the paint that follows reads
        /// the pan without moving it ([`super::Column::hold_pan`]).
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().hold_pan();
            if width != self.laid_at.get() {
                self.obj().lay_out_soon();
            }
        }
    }
}

glib::wrapper! {
    /// The column of pages the Preview pane shows in PDF mode.
    pub struct Column(ObjectSubclass<imp::Column>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

impl Column {
    /// A column with no pages in it.
    pub(crate) fn new() -> Self {
        let column: Self = glib::Object::builder().build();
        column.set_hexpand(true);
        column.set_vexpand(true);
        // Full holds the keyboard, and what it does with it is scroll.
        column.set_focusable(true);
        column
    }

    /// Gives the column the window whose Document it draws.
    pub(crate) fn attach(&self, window: &Window) {
        self.imp().window.replace(Some(window.downgrade()));
    }

    /// Tells the column what the pane scrolls by, so it knows which pages are
    /// in view and repaints when they change.
    ///
    /// A viewport scrolls by moving its child's render node rather than asking
    /// for it again, so a column that paints the pages in view has to ask for
    /// the paint itself.
    pub(crate) fn watch_scroll(&self, adjustment: &gtk::Adjustment) {
        self.imp().scroll.replace(Some(adjustment.clone()));
        let column = self.clone();
        adjustment.connect_value_changed(move |_| column.queue_draw());
    }

    /// How tall the column stands in the pixels it was laid out in, which is
    /// what the pane's scroll rules are clamped to
    /// ([`crate::preview::Preview`]).
    ///
    /// Named apart from the widget's own `height`, which is what it has been
    /// allocated: a column just laid out again is a height the scroller has
    /// not caught up with.
    pub(crate) fn laid_height(&self) -> f64 {
        self.imp().height.get()
    }

    /// The pages' blocks as the scroll sync rules read them, keyed by the
    /// Document's block index exactly as the sheet's rows are, so the window's
    /// follow glue is the same glue in both modes
    /// ([`crate::preview::Preview::blocks`]).
    ///
    /// Borrowed rather than handed out, because an edit asks for them on every
    /// keystroke.
    pub(crate) fn rows(&self) -> Ref<'_, Vec<sync::Block>> {
        self.imp().blocks.borrow()
    }

    /// The link whose words stand at `x`, `y` in the column's own pixels.
    ///
    /// The sheet answers this from the render pass's blocks, which stand where
    /// it drew them; a page stands where the paginator's runs put them, so the
    /// point is taken back to the paper it landed on, in points, and the run's
    /// own lines are what carry the layout it is asked of.
    pub(crate) fn link_at(&self, x: f64, y: f64) -> Option<String> {
        let imp = self.imp();
        let scale = imp.scale.get();
        if scale <= 0.0 {
            return None;
        }
        let laid = imp.laid.borrow();
        let laid = laid.as_ref()?;
        let paper = laid.frame.paper;
        let stack = Stack::of(paper, scale);
        let at = stack.index_at(y)?;
        let page = laid.pages.get(at)?;
        let left = self.left(f64::from(self.width()), paper.width * scale);
        let point = ((x - left) / scale, (y - stack.top_of(at)) / scale);
        for fragment in &page.fragments {
            let Some(block) = laid.rendered.blocks.get(fragment.block) else {
                continue;
            };
            for run in &fragment.runs {
                let Some(placed) = block.layouts.get(run.placed) else {
                    continue;
                };
                if let Some(destination) = link_in(placed, run, point) {
                    return Some(destination);
                }
            }
        }
        None
    }

    /// What the stats bar says while the column stands at `offset`: the page
    /// under the pane's top edge, in the engine's words
    /// ([`paginate::page_words`]).
    ///
    /// The words are the engine's and the page is this widget's, because the
    /// column is what knows where a page stands: the paginator counts pages
    /// and the stacking here turns a scroll into one of them
    /// ([`Stack::under`]). Nothing until the pages have been laid out.
    pub(crate) fn page_words(&self, offset: f64) -> Option<String> {
        let imp = self.imp();
        let scale = imp.scale.get();
        if scale <= 0.0 {
            return None;
        }
        let laid = imp.laid.borrow();
        let laid = laid.as_ref()?;
        let stack = Stack::of(laid.frame.paper, scale);
        let at = stack.under(offset, laid.pages.len())?;
        paginate::page_words(&laid.pages, at)
    }

    /// Where a follow may leave the column, given the offset the sync rules
    /// answered with ([`held`]).
    ///
    /// The clamp is here rather than in the window's glue because it is the
    /// column that knows where a page stands: the sheet's rules answer in
    /// blocks, and a page is what the first of them has a header and a title
    /// page standing over.
    pub(crate) fn hold_follow(&self, target: f64) -> f64 {
        let imp = self.imp();
        let laid = imp.laid.borrow();
        let Some(laid) = laid.as_ref() else {
            return target;
        };
        let paper = laid.frame.paper;
        held(
            target,
            Stack::of(paper, imp.scale.get()),
            usize::from(paper.title_page),
        )
    }

    /// Slides a page too wide for the pane sideways by `notches` of a wheel, and
    /// says whether it moved.
    ///
    /// The column pans itself rather than the pane scrolling: the pane's
    /// scroller is the Web sheet's too, and a rendered page wraps in the
    /// measure, so there is nothing sideways in it to scroll
    /// ([`crate::preview::Preview::new`]).
    pub(crate) fn pan_by(&self, notches: f64) -> bool {
        let imp = self.imp();
        let laid = imp.laid.borrow();
        let Some(laid) = laid.as_ref() else {
            return false;
        };
        let page = laid.frame.paper.width * imp.scale.get();
        let was = imp.pan.get();
        let now = (was + notches * PAN).clamp(0.0, spare(f64::from(self.width()), page));
        if (now - was).abs() < f64::EPSILON {
            return false;
        }
        imp.pan.set(now);
        self.queue_draw();
        true
    }

    /// Where the page's left edge stands in a column `width` pixels wide.
    ///
    /// Centred in the pane while it fits, which is fit width and anything under
    /// it; wider than the pane, the gutter stays on the leading edge and the
    /// pan slides the paper under it.
    fn left(&self, width: f64, page: f64) -> f64 {
        let room = width - page;
        if room >= 0.0 {
            room / 2.0
        } else {
            GUTTER - self.imp().pan.get()
        }
    }

    /// Takes the pan back to what there is left to slide: a pane made wider, or
    /// a zoom stepped back down, takes the page back with it rather than
    /// leaving it off the edge.
    ///
    /// Run wherever the room moves — a fresh layout and an allocation — and
    /// never from the paint, which reads the pan and writes nothing: a widget
    /// that moves its own state while it is being drawn is a frame that draws
    /// something else.
    fn hold_pan(&self) {
        let imp = self.imp();
        let laid = imp.laid.borrow();
        let Some(laid) = laid.as_ref() else {
            return;
        };
        let page = laid.frame.paper.width * imp.scale.get();
        let room = spare(f64::from(self.width()), page);
        imp.pan.set(imp.pan.get().clamp(0.0, room));
    }

    /// The window whose Document this column draws.
    pub(crate) fn owner(&self) -> Option<Window> {
        self.imp()
            .window
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
    }

    /// Cuts `document` into pages at `[export]` and keeps them.
    ///
    /// The geometry is the saved table and nothing of the pane's: the paper,
    /// the margin, the text size and the three toggles are what Quick Export
    /// would write, which is the whole point of the mode. What the pane
    /// contributes is the scale, and fit width is a function of its width.
    pub(crate) fn lay_out(
        &self,
        document: &Document,
        settings: &Settings,
        over: Option<&DialogOverride>,
    ) {
        let width = self.width();
        if width <= 0 {
            // Not on the compositor yet: the first allocation asks again.
            return;
        }
        let template = template::named(settings.template.name.as_str());
        // What an open PDF dialog's Options are showing, and `[export]` and
        // `[template]` themselves whenever no dialog stands over the pane
        // ([`DialogOverride`]).
        let export = over.map_or(&settings.export, |over| &over.export);
        let toggles = over.map_or_else(
            || render::Toggles::of(&settings.template),
            |over| over.toggles,
        );
        let paper = paginate::Geometry::of(export);
        // Its own context: `lay_out` puts the one it is handed into points,
        // and the Web sheet renders on the widget's shared context at the
        // screen's resolution (#293).
        let context = self.create_pango_context();
        let laid = paginate::lay_out(
            document,
            &template,
            toggles,
            paper,
            f64::from(export.text_size),
            &context,
        );
        // Fit width is what the zoom's 100 % means here, and the three zoom
        // rows step over it: `[preview] zoom` is the one value both modes read.
        let scale = scale(f64::from(width), paper.width, settings.preview.zoom);
        let height = Stack::of(paper, scale).height(laid.pages.len());
        let blocks = rows(&laid, scale);
        let imp = self.imp();
        imp.scale.set(scale);
        imp.height.set(height);
        imp.laid_at.set(width);
        imp.template.replace(Some(template));
        imp.blocks.replace(blocks);
        imp.laid.replace(Some(laid));
        // A fresh scale is fresh room to slide in, and the pan is held to it
        // here rather than in the paint.
        self.hold_pan();
        self.set_height_request(pixels(height));
        self.queue_draw();
    }

    /// Arms one fresh layout on the main loop.
    fn lay_out_soon(&self) {
        if self.imp().pending.replace(true) {
            return;
        }
        let column = self.clone();
        glib::idle_add_local_once(move || {
            column.imp().pending.set(false);
            if let Some(window) = column.owner() {
                window.refresh_preview();
            }
        });
    }

    /// Paints the surround, then every page standing in it that is in view.
    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let width = f64::from(self.width());
        snapshot.append_color(
            &gdk::RGBA::new(SURROUND, SURROUND, SURROUND, 1.0),
            &preview::rect(0.0, 0.0, width, f64::from(self.height())),
        );
        let laid = imp.laid.borrow();
        let Some(laid) = laid.as_ref() else {
            return;
        };
        let template = imp.template.borrow();
        let Some(template) = template.as_ref() else {
            return;
        };
        let scale = imp.scale.get();
        if scale <= 0.0 {
            return;
        }
        let paper = laid.frame.paper;
        let (page_width, page_height) = (paper.width * scale, paper.height * scale);
        let stack = Stack::of(paper, scale);
        let left = self.left(width, page_width);
        let (from, to) = self.in_view();
        for (at, page) in laid.pages.iter().enumerate() {
            let top = stack.top_of(at);
            if top + page_height < from || top > to {
                continue;
            }
            // The drawer paints in points from the paper's corner, so the
            // context is put where the page stands and scaled to it; the node
            // is clipped to the page, so nothing the drawer does reaches the
            // surround.
            let cr = snapshot.append_cairo(&preview::rect(left, top, page_width, page_height));
            cr.translate(left, top);
            cr.scale(scale, scale);
            draw::draw(&cr, page, &laid.rendered, template, &laid.frame);
        }
    }

    /// The band of the column the pane is showing, in the column's own pixels.
    ///
    /// The pane's adjustment is the column's own vertical offset: the column
    /// is the only thing in the scroller that is visible in this mode, so it
    /// stands at the viewport's origin. A pane that has not been allocated yet
    /// has no page size, and the whole column is in view.
    fn in_view(&self) -> (f64, f64) {
        let scroll = self.imp().scroll.borrow();
        let Some(scroll) = scroll.as_ref().filter(|scroll| scroll.page_size() > 0.0) else {
            return (f64::MIN, f64::MAX);
        };
        (scroll.value(), scroll.value() + scroll.page_size())
    }
}

/// The scale a column `width` logical pixels wide fits a page `paper` points
/// wide at: fit width, which is what the zoom's 100 % means here.
///
/// At 100 % the page fits whatever the pane is: fit width is the width, and
/// there is nothing to slide sideways under it (#293 § Zoom). Sideways
/// scrolling arises above 100 % and nowhere else ([`spare`]).
fn fit(width: f64, paper: f64) -> f64 {
    if paper <= 0.0 {
        return 0.0;
    }
    ((width - 2.0 * GUTTER) / paper).max(0.0)
}

/// The scale a column `width` pixels wide draws a page `paper` points wide at,
/// `zoom` per cent of fit width.
///
/// The zoom is a step over fit width and not a second geometry: 100 % is the
/// page filling the pane less its gutters, and the range `[preview] zoom` is
/// clamped to ([`quill_engine::settings::preview_zooms`]) is what the Web sheet
/// steps over too.
fn scale(width: f64, paper: f64, zoom: u32) -> f64 {
    fit(width, paper) * f64::from(zoom) / 100.0
}

/// How much of a `page` pixels wide, with its gutters, a column `width` pixels
/// wide has not got the room for: what there is to pan.
fn spare(width: f64, page: f64) -> f64 {
    (page + 2.0 * GUTTER - width).max(0.0)
}

/// The pages as the sync rules read them, in the column's own pixels.
///
/// The engine walks the fragments ([`paginate::column_blocks`], which is handed
/// the gap in the points the paper is measured in); what this adds is the air
/// above the first page, since the column stands its first page one gap down
/// ([`Stack::top_of`]) and the engine counts from the first page's top edge.
fn rows(laid: &paginate::Laid, scale: f64) -> Vec<sync::Block> {
    let paper = laid.frame.paper;
    let stack = Stack::of(paper, scale);
    paginate::column_blocks(laid, paper.margin, scale)
        .into_iter()
        .map(|block| sync::Block::new(block.key, block.top + stack.gap, block.height))
        .collect()
}

/// `target`, taken back to the top of page `first` where a follow would leave
/// the column standing on that page.
///
/// The sync rules answer in blocks and put a block's own top at the pane's top
/// edge ([`quill_engine::sync`]), and the first body block's top is where the
/// text band starts rather than where the page does. Followed to the letter,
/// every keystroke on the first body page scrolls that page's header — and, on
/// a Document with a title page, the title page over it — off the top. So a
/// target landing anywhere on page `first` stands the column at that page's own
/// top instead, and every page after it follows the sheet's rule unchanged
/// (#293 § Refresh and follow).
fn held(target: f64, stack: Stack, first: usize) -> f64 {
    let top = stack.top_of(first);
    if target > top && target < top + stack.page {
        top
    } else {
        target
    }
}

/// The destination of the link at `at` — a point on the paper, in points — in
/// the lines `run` carries of `placed`, or nothing where the point is on none
/// of them.
///
/// The drawer's walk read backwards ([`draw::draw`]): the layout stands with
/// its first line's top edge at [`paginate::Run::y`] and its left edge at
/// [`paginate::Run::x`], so a point inside one of the run's lines is a point in
/// the layout's own coordinates, and Pango says which byte it lands on.
fn link_in(placed: &render::Placed, run: &paginate::Run, at: (f64, f64)) -> Option<String> {
    let mut iter = placed.layout.iter();
    let mut line = 0;
    let mut origin = None;
    loop {
        if run.lines.contains(&line) {
            let (top, bottom) = iter.line_yrange();
            let origin = *origin.get_or_insert(run.y - render::back(top));
            if at.1 >= origin + render::back(top) && at.1 < origin + render::back(bottom) {
                let (inside, index, _) = placed
                    .layout
                    .xy_to_index(render::units(at.0 - run.x), render::units(at.1 - origin));
                if !inside {
                    return None;
                }
                let index = usize::try_from(index).unwrap_or(usize::MAX);
                return placed
                    .links
                    .iter()
                    .find(|link| link.at.contains(&index))
                    .map(|link| link.destination.clone());
            }
        }
        line += 1;
        if !iter.next_line() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fit width and the stack, which is the whole of the column's geometry:
    /// A4 is 595.28 pt wide, so an 800 px pane fits it at (800 − 48) / 595.28,
    /// and the pages stand a margin apart down the column.
    #[test]
    fn fit_width_scales_the_paper_to_the_pane_and_the_pages_stand_a_margin_apart() {
        let (width, height, margin) = (595.28, 841.89, 72.0);
        let scale = fit(800.0, width);
        assert!(
            (scale - (800.0 - 2.0 * GUTTER) / width).abs() < f64::EPSILON,
            "fit width is the pane less its gutters over the paper, not {scale}"
        );
        assert!(
            (width * scale - (800.0 - 2.0 * GUTTER)).abs() < 1e-9,
            "the page fills the pane less its gutters"
        );
        let stack = Stack {
            page: height * scale,
            gap: margin * scale,
        };
        assert!(
            (stack.top_of(0) - stack.gap).abs() < f64::EPSILON,
            "the first page stands one gap down"
        );
        assert!(
            (stack.top_of(1) - (stack.gap + stack.page + stack.gap)).abs() < 1e-9,
            "the second page's top is the first's foot and one gap"
        );
        assert!(
            (stack.height(2) - (stack.top_of(1) + stack.page + stack.gap)).abs() < 1e-9,
            "the column is as tall as the last page's foot and its gap"
        );
        assert!(
            (stack.height(0) - stack.gap).abs() < f64::EPSILON,
            "a column with no pages is the gap and nothing else"
        );
        assert!(
            fit(800.0, 0.0).abs() < f64::EPSILON,
            "a paper with no width scales to nothing rather than dividing by nought"
        );
        // Fit width is 100 % whatever the pane is (#293 § Zoom): a narrow pane
        // draws a small page, and there is nothing to slide under it.
        for pane in [240.0, 100.0, 60.0] {
            assert!(
                spare(pane, width * fit(pane, width)).abs() < 1e-9,
                "a page at fit width fits a {pane} px pane whole"
            );
        }
    }

    /// The zoom rows step over fit width, 100 % being fit width itself, and a
    /// page grown past the pane leaves that much to pan.
    #[test]
    fn the_zoom_steps_over_fit_width_and_what_will_not_fit_is_what_pans() {
        let width = 595.28;
        let fit = fit(800.0, width);
        assert!(
            (scale(800.0, width, 100) - fit).abs() < f64::EPSILON,
            "100 % is fit width"
        );
        assert!(
            (scale(800.0, width, 200) - 2.0 * fit).abs() < 1e-9,
            "200 % is twice it"
        );
        assert!(
            (scale(800.0, width, 50) - fit / 2.0).abs() < 1e-9,
            "50 % is half of it"
        );
        assert!(
            spare(800.0, width * fit).abs() < f64::EPSILON,
            "a page at fit width has nothing to pan"
        );
        let grown = width * scale(800.0, width, 200);
        assert!(
            (spare(800.0, grown) - (grown + 2.0 * GUTTER - 800.0)).abs() < 1e-9,
            "a page past the pane pans by what it hangs over, its gutters included"
        );
    }

    /// A point in the column is on the page it stands on, and the air over a
    /// page and between two of them is on neither: which is what a click has to
    /// answer before a link on the paper can.
    #[test]
    fn a_point_is_on_the_page_it_stands_on_and_the_air_is_on_none_of_them() {
        let stack = Stack {
            page: 800.0,
            gap: 40.0,
        };
        let (first, second) = (stack.top_of(0), stack.top_of(1));
        assert_eq!(
            stack.index_at(first + 1.0),
            Some(0),
            "a point on the first page is the first page's"
        );
        assert_eq!(
            stack.index_at(first + stack.page + 1.0),
            None,
            "a point in the air between two pages is on neither"
        );
        assert_eq!(
            stack.index_at(second + 1.0),
            Some(1),
            "and a point past that air is the second page's"
        );
        assert_eq!(
            stack.index_at(stack.gap / 2.0),
            None,
            "the air above the first page is on no page either"
        );
    }

    /// A follow that would leave the column standing part way down the first
    /// body page stands it at that page's top instead, so the header over the
    /// caret's block — and the title page over that — stay in view while the
    /// caret is on page one (#293 § Refresh and follow).
    #[test]
    fn a_follow_on_to_the_first_body_page_stands_the_column_at_that_pages_top() {
        let stack = Stack {
            page: 800.0,
            gap: 40.0,
        };
        // No title page: the first body page is the column's own first.
        let first = stack.top_of(0);
        assert!(
            (held(first + 120.0, stack, 0) - first).abs() < f64::EPSILON,
            "a target part way down page one is taken back to its top"
        );
        assert!(
            (held(0.0, stack, 0) - 0.0).abs() < f64::EPSILON,
            "a target above the page is left where it is: nothing is scrolled off"
        );
        let second = stack.top_of(1);
        assert!(
            (held(second + 120.0, stack, 0) - (second + 120.0)).abs() < f64::EPSILON,
            "a target on page two follows the sheet's rule unchanged"
        );
        // With a title page the body opens on the column's second page, and it
        // is that page the follow is held to.
        let body = stack.top_of(1);
        assert!(
            (held(body + 300.0, stack, 1) - body).abs() < f64::EPSILON,
            "a target part way down the first body page is taken back to its top"
        );
        assert!(
            (held(stack.top_of(0) + 10.0, stack, 1) - (stack.top_of(0) + 10.0)).abs()
                < f64::EPSILON,
            "and a target on the title page over it is left where it is"
        );
    }

    /// The top edge always stands over a page, whatever the scroll: the air
    /// above the first page and the air between two of them read as the page
    /// coming into view, and an edge past the end as the last page (#299).
    #[test]
    fn the_top_edge_stands_over_a_page_wherever_the_column_is_scrolled() {
        let stack = Stack {
            page: 800.0,
            gap: 40.0,
        };
        let second = stack.top_of(1);
        assert_eq!(stack.under(0.0, 3), Some(1), "the column's head");
        assert_eq!(
            stack.under(stack.top_of(0) + 1.0, 3),
            Some(1),
            "a point on the first page"
        );
        assert_eq!(
            stack.under(second - stack.gap / 2.0, 3),
            Some(2),
            "the air between two pages is the one coming into view"
        );
        assert_eq!(
            stack.under(second + 1.0, 3),
            Some(2),
            "and the page itself is that page"
        );
        assert_eq!(
            stack.under(stack.top_of(9), 3),
            Some(3),
            "past the last page is the last page"
        );
        assert_eq!(stack.under(0.0, 0), None, "no pages, no page");
    }
}
