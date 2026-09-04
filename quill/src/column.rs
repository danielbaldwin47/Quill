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
//! keystroke of a burst) and whenever the pane's width moves; scrolling only
//! repaints, and repaints the pages in view.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene};
use quill_engine::document::Document;
use quill_engine::draw;
use quill_engine::paginate;
use quill_engine::render;
use quill_engine::settings::{Choice, Settings};
use quill_engine::template;

use crate::tags::pixels;
use crate::window::Window;

/// The air either side of the page, in logical pixels.
///
/// What fit width fits: the page is scaled to the column's width less this
/// gutter twice, which leaves the paper's edge visible against the surround
/// rather than running it off the pane.
const GUTTER: f64 = 24.0;

/// The neutral the pages stand on, `#f7f7f7` in both themes.
///
/// One value for every Template and both grounds: the surround is not paper
/// and not the app's ground either, it is the light a sheet is read under.
const SURROUND: f32 = 0.968_627_5;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::paginate::Laid;
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
        /// Logical pixels to the point: what fit width worked out for the
        /// width the pages were laid out at.
        pub scale: Cell<f64>,
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
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
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

    /// The window whose Document this column draws.
    fn owner(&self) -> Option<Window> {
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
    pub(crate) fn lay_out(&self, document: &Document, settings: &Settings) {
        let width = self.width();
        if width <= 0 {
            // Not on the compositor yet: the first allocation asks again.
            return;
        }
        let template = template::named(settings.template.name.as_str());
        let paper = paginate::Geometry::of(&settings.export);
        // Its own context: `lay_out` puts the one it is handed into points,
        // and the Web sheet renders on the widget's shared context at the
        // screen's resolution (#293).
        let context = self.create_pango_context();
        let laid = paginate::lay_out(
            document,
            &template,
            render::Toggles::of(&settings.template),
            paper,
            f64::from(settings.export.text_size),
            &context,
        );
        // Fit width is the whole of the scale here; the zoom rows step over it
        // in #298.
        let scale = fit(f64::from(width), paper.width);
        let height = stack(laid.pages.len(), paper.height * scale, gap(paper, scale));
        let imp = self.imp();
        imp.scale.set(scale);
        imp.height.set(height);
        imp.laid_at.set(width);
        imp.template.replace(Some(template));
        imp.laid.replace(Some(laid));
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
            &rect(0.0, 0.0, width, f64::from(self.height())),
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
        let gap = gap(paper, scale);
        let left = ((width - page_width) / 2.0).max(0.0);
        let (from, to) = self.in_view();
        for (at, page) in laid.pages.iter().enumerate() {
            let top = page_top(at, page_height, gap);
            if top + page_height < from || top > to {
                continue;
            }
            // The drawer paints in points from the paper's corner, so the
            // context is put where the page stands and scaled to it; the node
            // is clipped to the page, so nothing the drawer does reaches the
            // surround.
            let cr = snapshot.append_cairo(&rect(left, top, page_width, page_height));
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
fn fit(width: f64, paper: f64) -> f64 {
    if paper <= 0.0 {
        return 0.0;
    }
    ((width - 2.0 * GUTTER) / paper).max(0.0)
}

/// The air between two pages, and above the first and below the last: one page
/// margin, in the column's own pixels.
///
/// The margin rather than a number of this module's own, so the air between
/// two pages reads as the air inside one and the column is one continuous
/// scroll rather than a stack of cards.
fn gap(paper: paginate::Geometry, scale: f64) -> f64 {
    paper.margin * scale
}

/// Where page `at` stands, in the column's own pixels: pages of `page` pixels
/// each with `gap` between them, the first `gap` down from the top.
fn page_top(at: usize, page: f64, gap: f64) -> f64 {
    gap + at as f64 * (page + gap)
}

/// How tall a column of `pages` pages stands: every page with its gap, and the
/// gap above the first.
fn stack(pages: usize, page: f64, gap: f64) -> f64 {
    gap + pages as f64 * (page + gap)
}

/// A rectangle in the column's own pixels, as `graphene` takes it.
///
/// The narrowing is where [`crate::preview`] puts it: a page is drawn in
/// `f32`, and nothing on it is near what an `f32` stops counting whole pixels
/// at.
fn rect(x: f64, y: f64, width: f64, height: f64) -> graphene::Rect {
    graphene::Rect::new(x as f32, y as f32, width as f32, height as f32)
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
        let (page, gap) = (height * scale, margin * scale);
        assert!(
            (page_top(0, page, gap) - gap).abs() < f64::EPSILON,
            "the first page stands one gap down"
        );
        assert!(
            (page_top(1, page, gap) - (gap + page + gap)).abs() < 1e-9,
            "the second page's top is the first's foot and one gap"
        );
        assert!(
            (stack(2, page, gap) - (page_top(1, page, gap) + page + gap)).abs() < 1e-9,
            "the column is as tall as the last page's foot and its gap"
        );
        assert!(
            (stack(0, page, gap) - gap).abs() < f64::EPSILON,
            "a column with no pages is the gap and nothing else"
        );
        assert!(
            fit(800.0, 0.0).abs() < f64::EPSILON,
            "a paper with no width scales to nothing rather than dividing by nought"
        );
        assert!(
            fit(10.0, width).abs() < f64::EPSILON,
            "a pane narrower than its own gutters scales to nothing (#298 scrolls it)"
        );
    }
}
