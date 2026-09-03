//! The rendered page beside the Editor.
//!
//! The engine lays a whole Document out in the Template ([`render`]) and this
//! is what a writer looks at: a scrollable sheet that snapshots the page's
//! Pango layouts, paints the Template's paper under them and its ink through
//! them, and centres the measure in whatever the pane has been dragged to. The
//! two panes carry different papers, which is the Design oracle's whole point
//! about the split (`ref/ia/mac-native/NOTES.md` § State 16): the Editor's
//! ground is where the source is typed, and the page is a distinctly darker —
//! or lighter — sheet beside it.
//!
//! It never selects text. There is no buffer here and no caret: a rendered
//! page is something to read, and what a writer copies they copy from the
//! source. The one thing a pointer does is open a link, and the one thing the
//! keyboard does is scroll, which is why Full can hold it at all.
//!
//! What it does not do yet is follow the writer: the page is laid out when the
//! pane opens, when the pane's width moves and when the Template does, and
//! ticket #270 is the idle refresh, the scroll sync and zoom. The block-to-
//! offset map ([`Preview::blocks`]) is here for it.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, pango};
use quill_engine::document::Document;
use quill_engine::render;
use quill_engine::settings::{Choice, EVEN, Settings};
use quill_engine::template;
use quill_engine::theme::{Colour, Scheme};

use crate::tags::pixels;
use crate::window::Window;

/// How wide the divider is to a pointer, as the Library's is
/// ([`crate::sidebar`]): a strip lying over the pane's left edge rather than
/// standing beside it, so the page is where it was and the strip has room to
/// be caught.
const GRAB: i32 = 6;

/// What the pointer becomes over the divider.
const RESIZE_CURSOR: &str = "col-resize";

/// The air between the pane's edge and the measure, in logical pixels.
///
/// The measure is what is left of the pane, and it is centred in it, so this
/// is the smaller half of what the oracle's Split shows either side of the
/// rendered column.
const MARGIN: f64 = 40.0;

/// The narrowest measure a page is laid out at, whatever the pane is dragged
/// to: a column narrower than this is not prose, and Pango wraps every word on
/// to its own line rather than saying so.
const NARROWEST_MEASURE: f64 = 160.0;

/// The air above and below the page, in logical pixels.
const PAD: f64 = 32.0;

/// The narrowest either half of the pair is dragged to, in logical pixels.
///
/// The Library's rule ([`quill_engine::settings::library_width`]) with both
/// ends the same, because both halves of this pair are pages: neither the
/// source nor the rendering is worth reading at less than this.
const NARROWEST: u32 = 240;

/// How much air the Well ground keeps around the ink it stands behind.
const WELL: f64 = 2.0;

/// How thick a thematic break's hairline is drawn.
const HAIRLINE: f64 = 1.0;

/// The width the Preview pane stands at in a pair `pair` pixels wide, given
/// the `wanted` width a drag or the state file asked for.
///
/// One pure function for all three askers, as the Library's is: a drag, a
/// launch reading the state file, and a window since made narrower. The one
/// value that is not a width is [`EVEN`], which is what a pane nobody has
/// dragged says, and it comes back as itself: an even Split is not 50 % of
/// anything until there is a pair to halve, and the pair halves itself.
#[must_use]
pub fn pane_width(wanted: u32, pair: u32) -> u32 {
    if wanted == EVEN {
        return EVEN;
    }
    wanted.clamp(NARROWEST, pair.saturating_sub(NARROWEST).max(NARROWEST))
}

/// Where a drag on the divider began: the pane's width then, and where the
/// pointer stood in the window's own pixels ([`Preview::dragged`]).
#[derive(Clone, Copy)]
struct Grab {
    /// The pane's width when the drag began, in logical pixels.
    width: i32,
    /// Where the pointer was across the window, in logical pixels.
    at: f64,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::render::Page;
    use quill_engine::template::Palette;

    use crate::window::Window;

    /// The sheet the page is painted on.
    #[derive(Default)]
    pub struct Sheet {
        /// The page as it was last laid out, or nothing until it has been.
        pub page: RefCell<Option<Page>>,
        /// The Template's palette for the ground the Editor is on.
        pub palette: Cell<Option<Palette>>,
        /// How wide the page was laid out, in logical pixels.
        pub measure: Cell<f64>,
        /// Where the measure's left edge stands in the sheet.
        pub left: Cell<f64>,
        /// The sheet's width when the page was laid out, so an allocation
        /// that did not move it lays nothing out again.
        pub laid: Cell<i32>,
        /// Whether a fresh layout is already waiting on the main loop.
        pub pending: Cell<bool>,
        /// The window whose Document is drawn here.
        pub window: RefCell<Option<glib::WeakRef<Window>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sheet {
        const NAME: &'static str = "QuillPreviewSheet";
        type Type = super::Sheet;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Sheet {}

    impl WidgetImpl for Sheet {
        /// The whole of the paint: paper, the Well grounds, the ink, the link
        /// ink over it.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }

        /// A width the page was not laid out at is a page to lay out again.
        ///
        /// On the main loop rather than here: laying out sets the sheet's
        /// height, and a height set inside an allocation is an allocation
        /// inside an allocation. One is armed at a time, so a drag that
        /// reports twenty widths lays the page out once per frame at worst.
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            if width != self.laid.get() {
                self.obj().lay_out_soon();
            }
        }
    }
}

glib::wrapper! {
    /// The sheet the page is painted on.
    pub struct Sheet(ObjectSubclass<imp::Sheet>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Sheet {
    /// A sheet with no page on it.
    fn new() -> Self {
        glib::Object::builder().build()
    }

    /// The window whose Document this sheet draws.
    fn owner(&self) -> Option<Window> {
        self.imp()
            .window
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
    }

    /// Lays `document` out and keeps the page.
    ///
    /// The Template is read here rather than handed in, because the sheet is
    /// what knows how wide it is: the measure is the pane less its margins,
    /// and the measure is the one thing the render pass cannot be told twice.
    fn lay_out(&self, document: &Document, settings: &Settings, scheme: Scheme) {
        let width = self.width();
        if width <= 0 {
            // Not on the compositor yet: the first allocation asks again.
            return;
        }
        let template = template::built_in(settings.template.name.as_str())
            .or_else(|_| template::built_in(template::DEFAULT))
            .expect("the default Template is compiled in");
        let toggles = render::Toggles {
            center_headings: settings.template.center_headings,
            number_headings: settings.template.number_headings,
            indent_paragraphs: settings.template.indent_paragraphs,
        };
        let measure = (f64::from(width) - 2.0 * MARGIN).max(NARROWEST_MEASURE);
        let page = render::render(
            document,
            &template,
            toggles,
            measure,
            settings.preview.zoom,
            &self.pango_context(),
        );
        let height = page.height + 2.0 * PAD;
        let imp = self.imp();
        imp.palette.set(Some(*template.palette(scheme)));
        imp.left.set((f64::from(width) - measure) / 2.0);
        imp.measure.set(measure);
        imp.laid.set(width);
        imp.page.replace(Some(page));
        self.set_height_request(pixels(height));
        self.queue_draw();
    }

    /// Arms one fresh layout on the main loop.
    fn lay_out_soon(&self) {
        if self.imp().pending.replace(true) {
            return;
        }
        let sheet = self.clone();
        glib::idle_add_local_once(move || {
            sheet.imp().pending.set(false);
            if let Some(window) = sheet.owner() {
                window.refresh_preview();
            }
        });
    }

    /// Paints the page: paper, then block by block.
    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(palette) = imp.palette.get() else {
            return;
        };
        // The paper is the whole sheet and not the page's own height: a
        // Document shorter than the pane still reads as a sheet of paper, the
        // way the oracle's does.
        snapshot.append_color(
            &rgba(palette.paper),
            &graphene::Rect::new(0.0, 0.0, at(self.width()), at(self.height())),
        );
        let page = imp.page.borrow();
        let Some(page) = page.as_ref() else {
            return;
        };
        let left = imp.left.get();
        let measure = imp.measure.get();
        for block in &page.blocks {
            let top = PAD + block.top;
            if block.ground {
                snapshot.append_color(
                    &rgba(palette.code_ground),
                    &rect(
                        left - WELL,
                        top - WELL,
                        measure + 2.0 * WELL,
                        block.height + 2.0 * WELL,
                    ),
                );
            }
            if block.kind == render::Kind::Rule {
                snapshot.append_color(
                    &rgba(palette.muted),
                    &rect(
                        left,
                        top + (block.height - HAIRLINE) / 2.0,
                        measure,
                        HAIRLINE,
                    ),
                );
            }
            for placed in &block.layouts {
                snapshot.save();
                snapshot.translate(&point(left + placed.x, PAD + placed.y));
                for code in &placed.code {
                    for span in spans(&placed.layout, code) {
                        snapshot.append_color(&rgba(palette.code_ground), &span);
                    }
                }
                snapshot.append_layout(&placed.layout, &rgba(palette.ink));
                // The link's own ink is the same layout painted again through
                // a clip of its words: a Pango layout is one colour, and the
                // second pass lands exactly on the glyphs the first drew.
                for link in &placed.links {
                    for span in spans(&placed.layout, &link.at) {
                        snapshot.push_clip(&span);
                        snapshot.append_layout(&placed.layout, &rgba(palette.link));
                        snapshot.pop();
                    }
                }
                snapshot.restore();
            }
        }
    }

    /// The link whose words stand at `x`, `y` in the sheet's own pixels.
    fn link_at(&self, x: f64, y: f64) -> Option<String> {
        let imp = self.imp();
        let page = imp.page.borrow();
        let page = page.as_ref()?;
        let left = imp.left.get();
        let y = y - PAD;
        for block in &page.blocks {
            if y < block.top || y > block.top + block.height {
                continue;
            }
            for placed in &block.layouts {
                let (width, height) = placed.layout.pixel_size();
                let x = x - left - placed.x;
                let y = y - placed.y;
                if x < 0.0 || y < 0.0 || x > f64::from(width) || y > f64::from(height) {
                    continue;
                }
                let (inside, index, _) = placed.layout.xy_to_index(units(x), units(y));
                if !inside {
                    continue;
                }
                let index = usize::try_from(index).unwrap_or(usize::MAX);
                if let Some(link) = placed.links.iter().find(|link| link.at.contains(&index)) {
                    return Some(link.destination.clone());
                }
            }
        }
        None
    }

    /// The page's blocks as the sync rules read them: the key, the top and the
    /// height of each, in the sheet's own pixels.
    fn blocks(&self) -> Vec<(usize, f64, f64)> {
        let page = self.imp().page.borrow();
        page.as_ref().map_or_else(Vec::new, |page| {
            page.blocks
                .iter()
                .map(|block| (block.key, PAD + block.top, block.height))
                .collect()
        })
    }
}

/// The Preview pane: the sheet in its scroller, with the divider over its left
/// edge.
#[derive(Clone)]
pub struct Preview {
    /// The scroller and the divider: what stands right of the Editor
    /// ([`Preview::widget`]) and what is shown and hidden, since an overlay
    /// whose scroller is away would still take the divider's room.
    frame: gtk::Overlay,
    /// What scrolls the sheet, and what Full's arrow keys move.
    scroller: gtk::ScrolledWindow,
    /// The sheet the page is painted on.
    sheet: Sheet,
    /// The divider: a strip of [`GRAB`] pixels on the pane's left edge with
    /// nothing in it and nothing drawn, which a drag moves the pane's edge by
    /// and a double-click puts back to an even Split.
    divider: gtk::Box,
}

impl Default for Preview {
    fn default() -> Self {
        Self::new()
    }
}

impl Preview {
    /// Builds the pane, empty and hidden.
    #[must_use]
    pub fn new() -> Self {
        let sheet = Sheet::new();
        sheet.set_hexpand(true);
        sheet.set_vexpand(true);
        // Full holds the keyboard, and what it does with it is scroll.
        sheet.set_focusable(true);
        let scroller = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            // A rendered page wraps in the measure, so there is nothing to
            // scroll to sideways, exactly as the Editor's scroller has it.
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&sheet)
            .build();
        let divider = gtk::Box::new(gtk::Orientation::Vertical, 0);
        divider.set_width_request(GRAB);
        divider.set_halign(gtk::Align::Start);
        divider.set_cursor_from_name(Some(RESIZE_CURSOR));
        let frame = gtk::Overlay::new();
        frame.set_child(Some(&scroller));
        frame.add_overlay(&divider);
        // Hidden on the frame and not on the scroller, for the reason the
        // Library's is: a hidden pane inside a shown overlay would still
        // leave the divider's strip beside the page.
        frame.set_visible(false);
        Self {
            frame,
            scroller,
            sheet,
            divider,
        }
    }

    /// The widget to stand right of the Editor.
    #[must_use]
    pub fn widget(&self) -> &gtk::Widget {
        self.frame.upcast_ref()
    }

    /// Gives the pane its window — which is whose Document it draws, and what
    /// a drag on the divider resizes — and wires what a pointer and the
    /// keyboard do here.
    ///
    /// Split from [`Preview::new`] because a window's widgets are built before
    /// it has a session to build them from ([`crate::window::Window::new`]).
    pub fn attach(&self, window: &Window) {
        self.sheet.imp().window.replace(Some(window.downgrade()));
        self.watch_divider();
        self.watch_links();
        self.watch_keys();
    }

    /// Lays the Document out again and paints it.
    pub fn refresh(&self, document: &Document, settings: &Settings, scheme: Scheme) {
        self.sheet.lay_out(document, settings, scheme);
    }

    /// Whether the pane is showing.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.frame.is_visible()
    }

    /// Shows or hides the pane.
    pub fn set_shown(&self, shown: bool) {
        self.frame.set_visible(shown);
    }

    /// Stands the pane at `width` logical pixels, or lets the pair divide
    /// itself where `width` is [`EVEN`].
    pub fn set_width(&self, width: u32) {
        if width == EVEN {
            self.frame.set_hexpand(true);
            self.frame.set_width_request(-1);
        } else {
            self.frame.set_hexpand(false);
            self.frame
                .set_width_request(i32::try_from(width).unwrap_or(i32::MAX));
        }
    }

    /// What the pane scrolls by: the sync ticket's handle on it (#270).
    #[must_use]
    pub fn vadjustment(&self) -> gtk::Adjustment {
        self.scroller.vadjustment()
    }

    /// The rendered page's blocks as vertical ranges — the key the Document's
    /// block index gave them, the top and the height — which is what a scroll
    /// sync rule is answered from (#270).
    #[must_use]
    pub fn blocks(&self) -> Vec<(usize, f64, f64)> {
        self.sheet.blocks()
    }

    /// The keyboard comes here: Full's arrows and Page keys scroll the page.
    pub fn grab_focus(&self) {
        self.sheet.grab_focus();
    }

    /// The divider's drag and its double-click, done as the Library's are
    /// (#260): one [`gtk::GestureClick`] for both, because a drag gesture
    /// beside a click gesture eats the press the double-click is counting.
    ///
    /// The pane is right of the divider, so a pointer travelling right makes
    /// it narrower — the one difference from the Library's, whose pane is left
    /// of its own edge.
    fn watch_divider(&self) {
        let grab = Rc::new(Cell::new(Grab { width: 0, at: 0.0 }));
        let clicks = gtk::GestureClick::new();
        clicks.set_button(gdk::BUTTON_PRIMARY);
        let pressed = self.clone();
        let taken = Rc::clone(&grab);
        clicks.connect_pressed(move |_, presses, x, _| {
            let Some(at) = pressed.pointer(x) else {
                return;
            };
            if presses >= 2 {
                // A double-click puts the pair back to an even Split, and
                // anchors the drag it also is, so the release leaves it there.
                taken.set(Grab { width: 0, at });
                pressed.resize(0);
                pressed.store_width();
                return;
            }
            taken.set(Grab {
                width: pressed.frame.width(),
                at,
            });
        });
        let moved = self.clone();
        let from = Rc::clone(&grab);
        clicks.connect_update(move |gesture, sequence| {
            if let Some((x, _)) = gesture.point(sequence) {
                moved.dragged(from.get(), x);
            }
        });
        let ended = self.clone();
        clicks.connect_released(move |_, _, _, _| ended.store_width());
        self.divider.add_controller(clicks);
    }

    /// Where the pointer is now in the window's own pixels, given where it is
    /// in the divider's: the divider travels with the edge it is, so an offset
    /// read off its own coordinates would count every pixel twice.
    fn pointer(&self, at: f64) -> Option<f64> {
        let window = self.sheet.owner()?;
        let point = self.divider.compute_point(&window, &point(at, 0.0))?;
        Some(f64::from(point.x()))
    }

    /// The pane's width part-way through a drag: what it was when the drag
    /// began, less how far the pointer has travelled since.
    fn dragged(&self, grab: Grab, at: f64) {
        let Some(now) = self.pointer(at) else {
            return;
        };
        self.resize(grab.width - pixels(now - grab.at));
    }

    /// Stands every window's pane at `width`
    /// ([`crate::window::Window::resize_preview`]).
    fn resize(&self, width: i32) {
        if let Some(window) = self.sheet.owner() {
            window.resize_preview(width);
        }
    }

    /// Writes the width the drag left the pane at to the state file.
    fn store_width(&self) {
        if let Some(session) = self.sheet.owner().and_then(|window| window.session()) {
            session.store_state();
        }
    }

    /// A click on a link's words opens the destination through the desktop's
    /// default handler.
    ///
    /// On the release rather than the press, so a click that began somewhere
    /// else and ended here opens nothing, and never on the second press of a
    /// double-click, which is a writer who missed.
    fn watch_links(&self) {
        let clicks = gtk::GestureClick::new();
        clicks.set_button(gdk::BUTTON_PRIMARY);
        let sheet = self.sheet.clone();
        clicks.connect_released(move |_, presses, x, y| {
            if presses != 1 {
                return;
            }
            let Some(destination) = sheet.link_at(x, y) else {
                return;
            };
            open(&destination, sheet.owner().as_ref());
        });
        self.sheet.add_controller(clicks);
    }

    /// The arrow and Page keys scroll the page, which is the whole of what the
    /// keyboard does here: Full has taken it from the Editor and has to give a
    /// reader some way down the page.
    fn watch_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        let scroller = self.scroller.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let adjustment = scroller.vadjustment();
            let step = adjustment.step_increment();
            let page = adjustment.page_increment();
            let to = match key {
                gdk::Key::Up => adjustment.value() - step,
                gdk::Key::Down => adjustment.value() + step,
                gdk::Key::Page_Up => adjustment.value() - page,
                gdk::Key::Page_Down | gdk::Key::space => adjustment.value() + page,
                gdk::Key::Home => adjustment.lower(),
                gdk::Key::End => adjustment.upper(),
                _ => return glib::Propagation::Proceed,
            };
            adjustment.set_value(to.clamp(
                adjustment.lower(),
                (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
            ));
            glib::Propagation::Stop
        });
        self.sheet.add_controller(keys);
    }
}

/// Opens `destination` through the desktop's default handler for it.
///
/// [`gtk::UriLauncher`] rather than the `gio` call the Settings window's
/// "Open settings.toml" uses: this is a URI from a Document and not a file
/// Quill wrote, so it goes through the portal, which is what asks the writer
/// before a strange scheme is handed to anything.
fn open(destination: &str, window: Option<&Window>) {
    gtk::UriLauncher::new(destination).launch(window, gio::Cancellable::NONE, |_| {});
}

/// The rectangles the bytes `at` of `layout` are drawn in, one per line they
/// run across, in the layout's own pixels.
///
/// What the Well ground is painted behind and what the link ink is clipped to.
fn spans(layout: &pango::Layout, at: &Range<usize>) -> Vec<graphene::Rect> {
    let (from, to) = (index(at.start), index(at.end));
    let mut spans = Vec::new();
    let mut iter = layout.iter();
    loop {
        if let Some(line) = iter.line_readonly() {
            let start = line.start_index();
            let end = start.saturating_add(line.length());
            let (from, to) = (from.max(start), to.min(end));
            if from < to {
                let (top, bottom) = iter.line_yrange();
                for edges in line.x_ranges(from, to).as_chunks::<2>().0 {
                    spans.push(rect(
                        back(edges[0]),
                        back(top),
                        back(edges[1] - edges[0]),
                        back(bottom - top),
                    ));
                }
            }
        }
        if !iter.next_line() {
            break;
        }
    }
    spans
}

/// A byte offset as Pango counts them: a range past what an `i32` holds is a
/// Document no layout of it could carry, and clamping it there selects
/// nothing rather than wrapping to something.
fn index(at: usize) -> i32 {
    i32::try_from(at).unwrap_or(i32::MAX)
}

/// A length in Pango units, as `xy_to_index` takes it.
fn units(value: f64) -> i32 {
    (value * f64::from(pango::SCALE)) as i32
}

/// A length in Pango units, back in the layout's own pixels.
fn back(units: i32) -> f64 {
    f64::from(units) / f64::from(pango::SCALE)
}

/// A rectangle in the sheet's own pixels, as `graphene` takes it.
///
/// The narrowing is where it belongs: a page is drawn in `f32`, and nothing on
/// it is near what an `f32` stops counting whole pixels at.
fn rect(x: f64, y: f64, width: f64, height: f64) -> graphene::Rect {
    graphene::Rect::new(x as f32, y as f32, width as f32, height as f32)
}

/// A length in the sheet's own pixels, as `graphene` takes it.
fn at(value: i32) -> f32 {
    value as f32
}

/// A point in the sheet's own pixels, as `graphene` takes it.
fn point(x: f64, y: f64) -> graphene::Point {
    graphene::Point::new(x as f32, y as f32)
}

/// A Template's colour as `gdk` takes it: 0 to 1 either way, so the narrowing
/// loses nothing a screen could show ([`crate::editor`] says the same of the
/// Editor's).
fn rgba(colour: Colour) -> gdk::RGBA {
    gdk::RGBA::new(
        colour.red as f32,
        colour.green as f32,
        colour.blue as f32,
        colour.alpha as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pair's rule, which is the Library's with both ends the same: a
    /// width nobody dragged stays the one value that is not a width.
    #[test]
    fn a_pane_nobody_dragged_divides_the_pair_and_a_dragged_one_leaves_a_page() {
        assert_eq!(pane_width(EVEN, 1440), EVEN, "an even Split is not a width");
        assert_eq!(pane_width(700, 1440), 700, "what fits is what was asked");
        assert_eq!(
            pane_width(12, 1440),
            NARROWEST,
            "a pane too narrow to read is stood at the narrowest"
        );
        assert_eq!(
            pane_width(1400, 1440),
            1440 - NARROWEST,
            "a pane that would leave the Editor nothing stops"
        );
        assert_eq!(
            pane_width(700, 300),
            NARROWEST,
            "a pair with room for neither still stands the pane at the narrowest"
        );
    }
}
