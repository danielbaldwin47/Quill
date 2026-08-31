//! The Editor: the text surface a Document is written on.
//!
//! One `GtkTextView` subclass (ADR 0004, `docs/adr/0004-gtktextview-editor.md`).
//! There is no mirror: the widget lays the text out once, so the web app's rule
//! that no per-token style may change glyph advance width stops being an
//! invariant to hold and becomes a property of the Faces. The `GtkTextBuffer`
//! owns the text the writer edits, which is where undo, IME preedit, clipboard,
//! selection and accessibility come from.
//!
//! The type and the page are here: the Face and the size, the leading that
//! belongs to that size, the 64-character measure centred in the window, and
//! the air above and below the column. Every number comes from
//! [`quill_engine::typography`] and none of it is decided here — this file is
//! the widget the numbers are set on. The tag table, the flattened
//! Markup × Focus runs, the hand-drawn caret and the dark palette land on this
//! type in the tickets that follow.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::time::Duration;

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use quill_engine::document::Document;
use quill_engine::settings::Face;
use quill_engine::theme::{Colours, Role, Scheme};
use quill_engine::typography;

use crate::caret;
use crate::flags;
use crate::tags;

/// The CSS class the Editor's type is named on.
const FACE_CLASS: &str = "quill-editor";

/// How far down the view `--caret` leaves the line the caret is on: the middle,
/// which is where a typewriter-scrolled Editor keeps it and the one place that
/// does not depend on how long the document is.
const CARET_LINE: f64 = 0.5;

/// How far below the baseline Pango reports the ink's own baseline is drawn,
/// in the pixels the widget lays out in.
///
/// [`caret::band_top`] hangs the band from the baseline, and the share it
/// hangs it by is the oracle's, measured in a browser. The two agree about the
/// type — the `caret` state and the oracle's are the same Face at the same
/// size, and the ink in both is 37 device pixels from ascender to descender —
/// and disagree about where under it the baseline falls: measured off the
/// judged shots at scale 2, the browser's is 15.25 logical pixels below the
/// top of the ink and Pango's is 14.75. Half a pixel, and it is the whole of
/// the difference between a band at 20 above the ascender and 15 below and the
/// 19 and 16 iA draws.
///
/// A pixel of granularity rather than a number about the type, most likely:
/// Pango hints the baseline onto a whole layout pixel and a browser at device
/// ratio 2 onto a whole device pixel, which is half of one. So it is a length
/// in pixels and not a share of the em — but it is *measured*, not derived,
/// and if the Type Piece ever moves the body size it is measured again. The
/// `caret` state is what says whether it is still true.
const BASELINE_DRIFT: f64 = 0.5;

/// The most rows of selection painted in one frame: `MAX_ROWS` in
/// `legacy/app/js/caret.js`.
///
/// A backstop and not the thing that keeps the paint bounded — the viewport
/// clip in [`Editor::selection`] does that, as it does in the oracle, which
/// clips before it counts. What is left for a cap is a window tall enough, or
/// a pitch small enough, that a screenful is still hundreds of rows.
const MAX_ROWS: usize = 400;

/// The scale the ladder is asked at for anything GTK lays the page out from.
///
/// A `GtkTextView`'s margins and its three leadings are logical pixels — GTK
/// applies the surface's scale factor itself, on the way to the glass — so the
/// pitch that feeds them is the ladder at scale 1. The surface's own scale is
/// for the caret alone, whose machine measures in device pixels because the
/// snap that keeps the bar's edges hard is a snap onto one of those.
const LAYOUT_SCALE: f64 = 1.0;

/// How far past the viewport the selection's rows are still built, in rows.
///
/// `M.pitch * 6` either side of the scroller in `drawSelection`. The slack is
/// what keeps a scroll from showing the seam: the rows a frame is about to
/// need are already drawn when it arrives.
const SELECTION_SLACK: f64 = 6.0;

/// Paper and ink: the light palette of `legacy/app/css/theme.css`.
///
/// Two constants rather than a table, because there is one theme until the
/// Dark & light ticket, and a table with one row in it says something that is
/// not true yet.
const PAPER: &str = "#f9f9f9";
pub(crate) const INK: &str = "#1c1c1c";

/// The weight ink is set at on paper: `--ink-weight: 415` in
/// `legacy/app/css/type.css`, a little heavier than Regular because a light
/// ground eats stems. The Faces are variable, and none of the three moves a
/// glyph's advance across the weight axis (`spike/gtk4-editor/RESULTS.txt`).
pub(crate) const INK_WEIGHT: u32 = 415;

/// The OpenType features prose is set without.
///
/// Ligatures, contextual alternates and kerning all make the text narrower
/// than the cell grid it is counted on, and the measure is counted in cells.
/// One list, because Pango and GTK's stylesheet spell a feature differently
/// and a page measured with kerning and drawn without it is measured wrong.
const FEATURES: [&str; 4] = ["liga", "clig", "calt", "kern"];

/// How many frames `--scroll` holds the view where it was asked for.
const SCROLL_FRAMES: u32 = 8;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::settings::Face;

    use crate::caret;

    #[derive(Default)]
    pub struct Editor {
        /// The Face this Editor is set in.
        pub face: Cell<Face>,
        /// Which of the type ladder's fourteen steps the Editor is set at.
        pub step: Cell<u32>,
        /// The page as it was last laid out. Setting a margin queues another
        /// allocation, so an allocation that does not move the page must not
        /// set it again.
        pub laid_out: Cell<Option<super::Page>>,
        /// True while the buffer is being filled with a Document rather than
        /// written in. The fill is a delete and an insert like any other, and
        /// the engine's copy is already what they would produce, so the
        /// handlers watching for a keystroke have to be able to tell them
        /// apart from one.
        pub loading: Cell<bool>,
        /// The blink and the glide. The widget feeds it what happened and
        /// asks it where the bar is and how opaque; it decides neither.
        pub caret: Cell<caret::Caret>,
        /// The row band the bar spans, and the air the leading leaves over
        /// the ink inside it. Both are functions of the type, so both are
        /// kept with the type: placing the bar on the keystroke path must not
        /// cost a row measured again.
        pub pitch: Cell<u32>,
        /// How far the baseline sits below the top of the box
        /// `iter_location` answers with, which is what the bar's band is
        /// anchored to. See [`Editor::bar`], and [`caret::band_top`] for why
        /// the baseline and not the top of the box.
        pub baseline: Cell<f64>,
        /// The bar last handed to the machine, so that a relayout which moved
        /// nothing can be told from one that moved the row.
        pub bar: Cell<Option<caret::Bar>>,
        /// What last drove the widget, which is half of a move's kind.
        pub last: Cell<caret::Source>,
        /// The frame the buffer last changed in, on the frame clock's clock.
        /// A caret move in that same frame is the move that change made.
        pub edited: Cell<Option<i64>>,
        /// Whether the frame-clock callback is attached.
        pub ticking: Cell<bool>,
        /// Whether `--caret` is still owed the scroll that shows where it
        /// went. Set when the caret is placed and cleared by the first
        /// allocation that can resolve it. See [`Editor::reveal_caret`].
        pub reveal: Cell<bool>,
        /// The one-shot that brings the blink back when the quiet after a
        /// move or an edit runs out: no frame is asked for inside it, so
        /// something outside the frame clock has to ask for the one that ends
        /// it.
        pub resume: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Editor {
        const NAME: &'static str = "QuillEditor";
        type Type = super::Editor;
        type ParentType = gtk::TextView;
    }

    impl ObjectImpl for Editor {
        /// Takes the blink's one-shot off the main loop with the widget.
        ///
        /// It holds a weak reference, so left behind it would fire into
        /// nothing rather than into a freed Editor — but a source with
        /// nothing left to do does not belong on the loop, and an Editor is
        /// disposed of whenever a window closes.
        fn dispose(&self) {
            if let Some(resume) = self.resume.take() {
                resume.remove();
            }
        }
    }

    impl WidgetImpl for Editor {
        /// The measure is centred in the room there turned out to be, and the
        /// air below the column is a share of the window's height, so both are
        /// answered here rather than guessed before the window has a size.
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().lay_out(width, height);
            // The bar's row is the row the text wrapped to, and where the text
            // wrapped is not known until there is a width to wrap it in. A
            // caret placed before that — by `--caret`, or by the Document
            // being shown — was placed on a layout GTK had not laid out yet,
            // so it is placed again here, on the one it has. The machine drops
            // a placement that moves nothing, so the allocations that do not
            // move the page cost nothing either.
            self.obj().caret_settled();
            // And the scroll that shows the row it is on, which could not be
            // resolved against a layout that did not exist yet either (#148).
            self.obj().reveal_caret();
        }
    }

    impl TextViewImpl for Editor {
        /// The selection and the caret, and nothing else, around the text.
        ///
        /// The fill goes under the glyphs and the caret over them. Never both
        /// at once: the caret is out for as long as a selection stands
        /// ([ADR 0014](../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md)),
        /// so each layer is the other's idle frame.
        ///
        /// The fill has to be under: the ink of a held word is the ink of any
        /// other word, and a fill painted over it would tint it. The caret has
        /// to be over. A bar is [`caret::width`] of the em and a glyph's left
        /// side bearing can be less than that, so a bar standing on a boundary
        /// meets the ink of the letter after it; drawn under, the letter is
        /// rasterised straight through the bar and the two read as one mark,
        /// which is what #147 reported as the caret "sitting on" the glyph.
        /// Both the Parity oracle and iA Writer paint theirs over the ink —
        /// `legacy/app/css/caret.css` puts `#caret-layer` above `#mirror`.
        ///
        /// This costs the layer above the text, where a future Annotator's
        /// marks were going to go; they will have to sort against the caret
        /// rather than assume the layer to themselves. GTK's own caret is not
        /// here to be drawn over either way: `cursor-visible` is false for the
        /// widget's life.
        ///
        /// Both layers are snapshotted in buffer coordinates, which is what
        /// `iter_location` answers in, so nothing is translated on the way.
        fn snapshot_layer(&self, layer: gtk::TextViewLayer, snapshot: gtk::Snapshot) {
            if layer == gtk::TextViewLayer::BelowText {
                self.obj().draw_selection_fill(&snapshot);
            }
            if layer == gtk::TextViewLayer::AboveText {
                self.obj().draw_caret(&snapshot);
            }
        }
    }
}

glib::wrapper! {
    /// The Editor widget.
    pub struct Editor(ObjectSubclass<imp::Editor>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

/// The selection, as the boxes that draw it.
///
/// Built by [`Editor::selection`] and drawn by [`Editor::draw_selection`], in
/// device pixels like everything else the caret's layer holds.
struct Selection {
    /// One fill per display row the selection covers, top to bottom.
    ///
    /// The whole of it. A selection carries no bars at either end and no
    /// caret: see [ADR 0014](../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md).
    rows: Vec<caret::Bar>,
}

impl Editor {
    /// An Editor with an empty buffer, at the default type.
    ///
    /// The Face and size a launch is actually running are handed to
    /// [`Editor::set_type`] as the window is built; these are what the widget
    /// is until then, so that it is never laid out on a size of zero.
    #[must_use]
    pub fn new() -> Self {
        let editor: Self = glib::Object::new();
        // Prose wraps, and it wraps inside a word when a word is longer than
        // the measure, because the measure is the promise and a word that
        // breaks the column breaks the page.
        editor.set_wrap_mode(gtk::WrapMode::WordChar);
        editor.add_css_class(FACE_CLASS);
        // GTK's caret is a one-pixel line at the font's height on a system
        // timer, and ours is the whole point of the Piece. It is switched off
        // here, once and for the widget's life, rather than per launch: there
        // is no state in which both are wanted, and `--nocaret` asks for
        // neither.
        editor.set_cursor_visible(false);
        editor
            .imp()
            .step
            .set(quill_engine::settings::default_step());
        editor.watch_caret();
        editor
    }

    /// Puts the four things the caret's machine is fed onto the widget.
    ///
    /// GTK says that the insert mark moved but never what moved it, so the
    /// two controllers are here to answer that and nothing else. Neither
    /// binds a chord and neither claims an event: they run in the capture
    /// phase, before GTK's own handling turns the press into a caret move, and
    /// then let it through. They are the exception named by #119's rule that
    /// only `quill::chrome` binds keys.
    fn watch_caret(&self) {
        let clicks = gtk::GestureClick::new();
        clicks.set_propagation_phase(gtk::PropagationPhase::Capture);
        clicks.connect_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _, _| editor.imp().last.set(caret::Source::Pointer),
        ));
        self.add_controller(clicks);

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, _, _| {
                editor.imp().last.set(caret::Source::Key);
                glib::Propagation::Proceed
            },
        ));
        self.add_controller(keys);

        let buffer = self.buffer();
        buffer.connect_insert_text(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _| editor.caret_edit_began(),
        ));
        buffer.connect_delete_range(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _| editor.caret_edit_began(),
        ));
        // After, and it is the only handler here that waits for another. The
        // window's own `changed` splices the Document and retags the lines the
        // edit touched, and a retag can put a run in another Face — so a
        // column read before it is a column measured on advances the writer
        // never sees. The rest of these read the text alone and do not care.
        let watcher = self.downgrade();
        buffer.connect_closure(
            "changed",
            true,
            glib::closure_local!(move |_: gtk::TextBuffer| {
                if let Some(editor) = watcher.upgrade() {
                    editor.caret_edited();
                }
            }),
        );
        buffer.connect_mark_set(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |buffer, _, mark| {
                let insert = mark == &buffer.get_insert();
                if insert {
                    editor.caret_moved();
                }
                // The other end moves on its own through a shift-drag and a
                // shift-arrow, and it is half of what there is to draw.
                if insert || mark == &buffer.selection_bound() {
                    editor.caret_selected();
                }
            },
        ));
    }

    /// Sets the Editor in `face` at `step` of the type ladder and lays the
    /// page out again.
    ///
    /// Everything downstream of the size moves with it: a size without the
    /// leading that belongs to it is half a decision, and a leading without
    /// the measure that belongs to it is the other half. A step names all
    /// three at once, which is the point of a ladder measured off the app
    /// rather than a curve fitted to it.
    pub fn set_type(&self, face: Face, step: u32) {
        self.imp().face.set(face);
        self.imp().step.set(step);
        self.restyle();
    }

    /// The leading, the air above the column, and then the column.
    fn restyle(&self) {
        // The Italic is a Face of its own, so the tags that ask for it have to
        // be moved to the new one; the rest of the type is CSS the widget
        // picks up on its own.
        tags::set_face(&self.buffer(), self.imp().face.get());
        let pitch = typography::pitch(self.imp().step.get(), LAYOUT_SCALE);
        let leading = typography::leading(pitch, self.row_height());
        // The caret's band and its unit, kept with the type: a bar placed on
        // the keystroke path must not cost a row measured all over again.
        self.imp().pitch.set(pitch);
        self.imp().baseline.set(self.row_baseline());
        self.tell_caret(|caret| caret.resize(self.em()));
        self.set_pixels_above_lines(signed(leading.above));
        self.set_pixels_inside_wrap(signed(leading.inside_wrap));
        self.set_pixels_below_lines(signed(leading.below));
        self.set_top_margin(signed(typography::page_top(pitch)));
        // The cell a heading's markers hang by moves with the size, so the
        // page is laid out from scratch rather than compared with the last
        // one: the column can be the same width at two sizes, and the hang
        // never is.
        self.imp().laid_out.set(None);
        self.lay_out(self.width(), self.height());
        // The bar is as wide and as tall as the type, so it is cut again with
        // it rather than left at the last size until the caret next moves.
        self.caret_settled();
    }

    /// Centres the measure in a view this wide and leaves the page its air.
    ///
    /// A view with no size yet is not laid out at all: `size_allocate` asks
    /// again the moment there is a size, and a column centred in nothing would
    /// only have to be undone.
    fn lay_out(&self, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let cell = typography::cell(self.imp().face.get(), self.imp().step.get());
        let page = Page {
            side: signed(typography::column(unsigned(width), cell).side),
            bottom: signed(typography::page_bottom(unsigned(height))),
        };
        if self.imp().laid_out.get() == Some(page) {
            return;
        }
        self.imp().laid_out.set(Some(page));
        self.set_left_margin(page.side);
        self.set_right_margin(page.side);
        self.set_bottom_margin(page.bottom);
        // The heading and list markers hang off this margin, so they are
        // re-hung with it: both halves of the pair move, `side` with the window
        // and the marker's width with the type.
        tags::hang_markers(
            &self.buffer(),
            self.imp().face.get(),
            self.imp().step.get(),
            page.side,
        );
    }

    /// How tall one row of ink is, in the Face and size now set.
    ///
    /// Laid out on a description built here rather than read off the widget:
    /// the stylesheet that will draw this same text is applied when GTK next
    /// recomputes style, which is after this is asked, so a layout that took
    /// the widget's word for it would be measuring the desktop theme's font.
    /// The description and the stylesheet are two spellings of one type —
    /// Pango and GTK's CSS name a feature and a weight differently — and both
    /// are spelled from [`FEATURES`] and [`INK_WEIGHT`], because a row
    /// measured with kerning and drawn without it is the wrong row, and the
    /// leading is built on this number.
    fn row_height(&self) -> u32 {
        unsigned(self.body_layout().pixel_size().1)
    }

    /// How far the baseline sits below the top of one row of body type, in the
    /// pixels the widget lays out in.
    ///
    /// The anchor [`caret::band_top`] takes, measured off the same layout the
    /// row's height is, because a baseline measured on one layout and a row
    /// measured on another are two rows.
    fn row_baseline(&self) -> f64 {
        f64::from(self.body_layout().baseline()) / f64::from(pango::SCALE)
    }

    /// One row of body type, laid out to be measured.
    fn body_layout(&self) -> pango::Layout {
        let layout = self.create_pango_layout(Some("Ag"));
        layout.set_font_description(Some(&body_font(
            self.imp().face.get(),
            typography::em(self.imp().step.get()),
        )));
        let features = pango::AttrList::new();
        features.insert(pango::AttrFontFeatures::new(&pango_features()));
        layout.set_attributes(Some(&features));
        layout
    }

    /// Shows `document`, marked up, with the caret at its start.
    ///
    /// The Markup is derived and drawn in the same breath as the text, because
    /// a frame showing the prose before its structure is a flash of the wrong
    /// page. Whole-Document on open is the architecture's cold-start cost; the
    /// keystroke path retags by the block.
    ///
    /// Filling the buffer is itself an edit as far as GTK is concerned, and
    /// the Document being shown is already the Document those edits would
    /// produce, so [`Editor::loading`] is true across it and the handlers on
    /// the buffer stand down.
    pub fn show_document(&self, document: &Document) {
        let buffer = self.buffer();
        self.imp().loading.set(true);
        buffer.set_text(document.text());
        self.imp().loading.set(false);
        tags::apply(&buffer, document, self.imp().face.get());
        buffer.place_cursor(&buffer.start_iter());
    }

    /// Whether the buffer is being filled rather than written in.
    pub(crate) fn loading(&self) -> bool {
        self.imp().loading.get()
    }

    /// Draws the lines an edit changed, and no others.
    ///
    /// Called from the buffer's `changed`, after the text has moved and after
    /// the engine's copy has been spliced to match it, because a tag is put on
    /// by the line and the byte index within it and both of those have to be
    /// the ones the writer can now see.
    pub fn retag(&self, document: &Document, lines: &Range<usize>) {
        tags::retag(&self.buffer(), document, self.imp().face.get(), lines);
    }

    /// The frame the widget is in, in the microseconds the machine counts.
    ///
    /// The frame clock's own time and never a system clock, so that the blink
    /// and the glide are functions of the frame they will be seen in. A widget
    /// with no surface yet has no clock; before the first frame everything
    /// happens at zero, which is one instant, which is what "before the first
    /// frame" means to the machine.
    fn now(&self) -> i64 {
        self.frame_clock().map_or(0, |clock| clock.frame_time())
    }

    /// Tells the caret's machine what happened, and keeps what it made of it.
    ///
    /// The machine is `Copy` and lives in a `Cell`, so every event is a read,
    /// a change and a write back; this is that, once, so that the four places
    /// that feed it say only what they are feeding it.
    fn tell_caret(&self, said: impl FnOnce(&mut caret::Caret)) {
        let mut caret = self.imp().caret.get();
        said(&mut caret);
        self.imp().caret.set(caret);
    }

    /// One em in the device pixels the machine measures its gates in.
    fn em(&self) -> f64 {
        typography::em(self.imp().step.get()) * self.scale()
    }

    /// The surface's scale factor, which is the machine's unit.
    ///
    /// Everything in a [`caret::Bar`] is device pixels, because the snap that
    /// keeps the bar's edges hard is a snap onto one of those. Never zero: a
    /// widget with no surface yet is told it is at scale 1.
    fn scale(&self) -> f64 {
        f64::from(self.scale_factor().max(1))
    }

    /// The bar at the caret now, in device pixels.
    ///
    /// The column comes from `iter_location`, which answers in the buffer
    /// coordinates the layer is snapshotted in, and is the advance boundary
    /// itself: the bar stands on the boundary between two cells and is offset
    /// from it by nothing. See [ADR 0013](../../docs/adr/0013-caret-on-the-advance-boundary.md)
    /// for the 0.07 em that used to be added here and what measuring iA Writer
    /// itself said about it.
    ///
    /// The band is the pitch, not the glyph, and it hangs from the row's
    /// baseline: `iter_location` gives the top of the box, the baseline is the
    /// same distance below it on every row, and [`caret::band_top`] carries
    /// the share of the pitch that goes above it. Anchoring to the box instead
    /// is what `caret.js`'s header warns against in as many words: a band on
    /// the box is centred on the font's em box rather than on the ink, and
    /// reads as top-heavy against the letters.
    ///
    /// The distance holds on every row, wrapped or not, because of the
    /// three-way leading split of ADR 0004: `pixels-inside-wrap` carries all
    /// the air between two rows of a paragraph and `pixels-above-lines` plus
    /// `pixels-below-lines` carry all of it between two paragraphs, so no row
    /// has more air inside its own box than any other.
    ///
    /// `None` before the type has been set, when there is no band to speak of.
    fn bar(&self) -> Option<caret::Bar> {
        let pitch = self.imp().pitch.get();
        if pitch == 0 {
            return None;
        }
        let buffer = self.buffer();
        let row = self.iter_location(&buffer.iter_at_mark(&buffer.get_insert()));
        let step = self.imp().step.get();
        let scale = self.scale();
        let (y, h) = self.band(f64::from(row.y()));
        Some(caret::Bar {
            x: f64::from(row.x()) * scale,
            y,
            w: f64::from(caret::width(caret_em_px(step))) * scale,
            h,
        })
    }

    /// The band of the row whose box starts at `top`, in device pixels: the y
    /// every mark on that row takes, and the height they all take.
    ///
    /// One function because the registration is the whole of the answer. The
    /// caret and the selection's fill rows are boxes that have to agree to the
    /// pixel — the `selection` state is judged on exactly that — and two sites
    /// computing one band from the same three numbers are two chances to
    /// disagree. `caret.js` has the same rule and keeps it the same way: one
    /// `dy` at `:185`, read by the fill and the caret alike.
    ///
    /// The caller has already checked the pitch: a zero pitch here is a band
    /// of no height, which draws nothing rather than wrongly.
    /// The top is snapped here rather than left to the machine, which snaps
    /// the caret's own on the way in ([`caret::Caret::moved`]). The fill rows
    /// never go through the machine, and a band left on a
    /// fraction of a device pixel is rasterised a row taller than it was cut,
    /// with a pale row standing in for the fraction — which is exactly the
    /// disagreement this function exists to prevent, one end of the band at a
    /// time.
    fn band(&self, top: f64) -> (f64, f64) {
        let pitch = f64::from(self.imp().pitch.get());
        let scale = self.scale();
        (
            caret::snap(
                caret::band_top(top + self.imp().baseline.get() + BASELINE_DRIFT, pitch) * scale,
            ),
            pitch * scale,
        )
    }

    /// The selection as the boxes that draw it, in device pixels.
    ///
    /// `None` when there is nothing selected, or before the type has been set.
    ///
    /// The walk is display lines rather than logical ones, because a wrapped
    /// paragraph is as many bands as it has rows on the glass. Each row is
    /// measured from the box of the character it starts at and the box of the
    /// character it ends at, and never from an iterator sitting on a wrap:
    /// that one offset is both the end of one row and the start of the next,
    /// and which of the two `iter_location` answers for is not ours to decide.
    /// A character's own box is on one row and only one.
    ///
    /// Only what can be seen is built. `drawSelection` clips to the viewport
    /// with six rows of slack either side and counts nothing outside it toward
    /// its cap, and this does the same at both ends: the walk begins at the
    /// first row inside the band rather than at the selection's own start, and
    /// stops at the first row past it. A select-all is then a screenful of
    /// work wherever the view is sitting, instead of a rectangle for every row
    /// of the document — and it is drawn, which a walk that spent its cap
    /// above the fold would not be.
    ///
    /// Rows are the whole of what is built: neither end is bracketed by a bar
    /// and the caret is out while the selection stands (ADR 0014). So a
    /// selection clipped at one end draws exactly what a selection clipped at
    /// neither does, and the walk owes nothing to where it stopped.
    fn selection(&self) -> Option<Selection> {
        let (start, end) = self.buffer().selection_bounds()?;
        let pitch = f64::from(self.imp().pitch.get());
        if pitch == 0.0 {
            return None;
        }
        let tail = caret::tail(caret_em_px(self.imp().step.get()));
        let scale = self.scale();
        let view = self.visible_rect();
        let top = f64::from(view.y()) - pitch * SELECTION_SLACK;
        let bottom = f64::from(view.y() + view.height()) + pitch * SELECTION_SLACK;
        let mut rows: Vec<caret::Bar> = Vec::new();
        let mut at = start;
        if let Some(seen) = self.iter_at_location(0, top.max(0.0) as i32)
            && seen > at
        {
            at = seen;
        }
        if at >= end {
            // The whole of it is above the band. Nothing to build, and no end
            // to bracket: the walk below would read the row `end` sits on as
            // the selection's last and bar it, which is a mark on a row the
            // writer is not looking at.
            return None;
        }
        // `at <= end`, not `<`: a selection that ends where a row begins ends
        // *on* that row, and its closing bar belongs at that row's left edge
        // rather than out at the end of the row above. Selecting through a
        // line's newline and no further is exactly this, and it is what the
        // oracle draws — `rows.set(0, { l: lr.left, r: lr.left })` when a line
        // contributes no rectangles, with `lastEdge` taken from it. The row is
        // empty, so it costs a zero-width fill that [`draw_box`] drops.
        while at <= end && rows.len() < MAX_ROWS {
            let mut stop = at;
            // The iterator is the answer here, never the boolean. Neither of
            // GTK's two display-line moves reports whether it moved: each
            // reports whether the place it arrived at is something other than
            // the buffer's end iterator, and the last row of a Document ends
            // exactly there. Read as "it did not move", that false put `stop`
            // back at the row's start, and the row then measured no width and
            // never satisfied `stop >= end` — so the Document's last row drew
            // no fill and the walk left no bar on it, whatever the selection
            // really ended at. Every row but the last one was painted (#146).
            //
            // A move that has nowhere to go leaves the iterator alone, which
            // is the empty row and the row a selection starts at the end of,
            // and both of those want `stop == at` anyway.
            self.forward_display_line_end(&mut stop);
            stop = stop.max(at).min(end);
            let box_of_first = self.iter_location(&at);
            if f64::from(box_of_first.y()) > bottom {
                break;
            }
            let left = f64::from(box_of_first.x());
            let mut right = left;
            if stop > at {
                let mut last = stop;
                last.backward_char();
                let glyph = self.iter_location(&last);
                right = f64::from(glyph.x() + glyph.width());
            }
            // A selected newline has no advance to fill, so it is drawn as a
            // stub past the last glyph. `ends_line` is what tells it from a
            // wrap: a wrap is inside one line and never ends it.
            if stop < end && stop.ends_line() {
                right += tail;
            }
            let (y, h) = self.band(f64::from(box_of_first.y()));
            let x = caret::snap(left * scale);
            let row = caret::Bar {
                x,
                y,
                w: (caret::snap(right * scale) - x).max(0.0),
                h,
            };
            rows.push(row);
            if stop >= end {
                break;
            }
            // The same false, for the same reason: stepping onto a last row
            // that is empty — a Document ending in a newline — arrives at the
            // end iterator and is reported as a failure to step. `next <= at`
            // is the honest stop, and it is the one a move with nowhere to go
            // leaves behind.
            let mut next = at;
            self.forward_display_line(&mut next);
            if next <= at {
                break;
            }
            at = next;
        }
        if rows.is_empty() {
            return None;
        }
        // The fill and nothing else. Where the oracle sets a bar at each end
        // (`setEdge` in `legacy/app/js/caret.js`) and the port inherited both,
        // iA Writer for Mac — measured off the owner's captures of the running
        // app — draws no bar at either end of a selection and no caret while
        // one stands. ADR 0014 carries the measurements and what they cost.
        Some(Selection { rows })
    }

    /// A change to the buffer is starting.
    ///
    /// Stamped where the change starts rather than where it ends, because the
    /// mark moves inside the change and GTK does not promise which of
    /// `mark-set` and `changed` a listener hears first. What the caret path
    /// needs is only that the stamp is down before the move arrives.
    ///
    /// Filling the buffer with a Document is a delete and an insert like any
    /// other and is not a writer's edit, so this stands down for it with the
    /// rest of the handlers watching this buffer.
    fn caret_edit_began(&self) {
        if self.loading() {
            return;
        }
        self.imp().edited.set(Some(self.now()));
    }

    /// The insert mark moved: tell the machine where to, and draw it there.
    fn caret_moved(&self) {
        let Some(bar) = self.bar() else {
            return;
        };
        let now = self.now();
        let kind = caret::kind(self.imp().last.get(), self.imp().edited.get() == Some(now));
        self.place_bar(bar, kind, now);
        self.keep_in_band(bar);
    }

    /// Keeps the caret's row inside the scroll band as the writer moves it:
    /// `scroll-padding: 10vh 0 28vh` in `legacy/app/css/page.css`.
    ///
    /// Only for a move a key made, which is the oracle's own rule rather than
    /// a narrowing of it. The band is `scroll-padding` on the scroller, and
    /// `scroll-padding` is spent by the browser's caret-into-view — which runs
    /// on typing and on cursor keys, and not on a click, whose target the hand
    /// could already see. So a click low on the page does not jump it, and the
    /// first key pressed afterwards brings the row into the band.
    ///
    /// [`caret::Source::App`] is out for a second reason: a launch flag, a
    /// restored position or a Command is put where it was asked for rather
    /// than travelled to. A judged state naming both `--caret` and `--scroll`
    /// means both, and a view that chased the caret would shoot a different
    /// passage from the one it was asked for.
    ///
    /// The row comes from the bar the machine was just handed rather than a
    /// second `iter_location`, because this is on the keystroke path; the bar
    /// is device pixels and the adjustment is not, so it comes back through
    /// the scale. The target is not clamped by the engine, and does not need
    /// to be: a `GtkAdjustment` holds itself inside its own ends.
    fn keep_in_band(&self, bar: caret::Bar) {
        if self.imp().last.get() != caret::Source::Key {
            return;
        }
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere for the band to move to.
            return;
        };
        let scale = self.scale();
        if let Some(target) = typography::band_target(
            bar.y / scale,
            bar.h / scale,
            adjustment.value(),
            adjustment.page_size(),
        ) {
            adjustment.set_value(target);
        }
    }

    /// The bar is where it was, but the page under it moved.
    ///
    /// A relayout or a change of type puts the same caret at a new place on
    /// the glass, and nothing travelled to get there — the row did. So it is
    /// put there, whatever the hand was last doing: a window resized just
    /// after a word jump must not glide the caret across the new column.
    fn caret_settled(&self) {
        let Some(bar) = self.bar() else {
            return;
        };
        // A relayout that did not move the bar is not news. The machine drops
        // a repeated position but holds the blink on for it either way, which
        // is right for the keystroke that sends the same position twice and
        // wrong here: a window being dragged is allocated on every frame, and
        // the caret would never blink again while a hand was on its edge.
        if self.imp().bar.get() == Some(bar) {
            return;
        }
        self.place_bar(bar, caret::Move::FollowsEdit, self.now());
    }

    /// Hands the machine the bar the layout says is there now.
    fn place_bar(&self, bar: caret::Bar, kind: caret::Move, now: i64) {
        self.imp().bar.set(Some(bar));
        self.tell_caret(|caret| caret.moved(bar, kind, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// The buffer changed: the blink is held, and the bar goes to the column
    /// the edit left the caret at.
    ///
    /// The move is taken from here rather than left to `mark-set`, which is
    /// emitted for a mark something *moved* — a click, a cursor key,
    /// `place_cursor` — and not for the insert mark being carried along by an
    /// insertion at its own position. Typing is the second kind and nothing
    /// else is, so a caret that waited for `mark-set` sat wherever it had last
    /// been put while the words went out from under it. The blink was held,
    /// the machine was told, and the bar did not move: the one path the judged
    /// states could not show, because every one of them is a still.
    ///
    /// [`caret::Caret::edited`] comes first because it is what puts the move
    /// about to be made inside the edit-snap window, so the bar is put at the
    /// new column rather than travelling to it, and because the frames the
    /// placement asks for are asked for on what the machine knows by then.
    fn caret_edited(&self) {
        if self.loading() {
            return;
        }
        let now = self.now();
        self.tell_caret(|caret| caret.edited(now));
        self.caret_moved();
        // Typing over a selection replaces it, and the marks it collapses can
        // land on the same offsets they were already on — which is a `changed`
        // and no `mark-set` at all, the same gap the placement above closes.
        self.caret_selected();
    }

    /// The buffer's selection opened, changed or collapsed.
    ///
    /// Whether there is one at all is the whole of what the machine is told:
    /// the caret goes out while it stands, and the shape of what is held is
    /// read from the buffer again at every frame it is drawn in, because a
    /// reflow moves the bands without touching either mark.
    fn caret_selected(&self) {
        let now = self.now();
        let held = self.buffer().has_selection();
        self.tell_caret(|caret| caret.selected(held, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// Asks for frames while the machine wants them, and for the one frame
    /// that ends the quiet when it does not.
    ///
    /// One of the two places [`caret::Caret::wants_tick`] is read and the tick
    /// source stands or falls by it; the other is the callback's own tail in
    /// [`Editor::start_tick`], which keeps the source while it stays true and
    /// drops it when it does not. Nothing else decides, so a `--deterministic`
    /// window, whose caret is frozen on, asks the frame clock for nothing at
    /// all while it is idle.
    fn ask_for_frames(&self) {
        let caret = self.imp().caret.get();
        if caret.wants_tick() {
            self.start_tick();
        } else if let Some(when) = caret.resumes_at() {
            self.wake_at(when);
        }
    }

    /// Attaches the frame-clock callback, if it is not already attached.
    ///
    /// Every frame it is attached for is a frame in which the blink or the
    /// glide has moved, which is why it may redraw on all of them — and why
    /// it is not attached at all while a hand is typing, when the caret is
    /// simply put where the glyph is and held on.
    fn start_tick(&self) {
        if self.imp().ticking.replace(true) {
            return;
        }
        self.add_tick_callback(|editor, clock| {
            editor.tell_caret(|caret| caret.tick(clock.frame_time()));
            editor.queue_draw();
            let caret = editor.imp().caret.get();
            if caret.wants_tick() {
                return glib::ControlFlow::Continue;
            }
            editor.imp().ticking.set(false);
            if let Some(when) = caret.resumes_at() {
                editor.wake_at(when);
            }
            glib::ControlFlow::Break
        });
    }

    /// Asks for one frame at `when`, on the frame clock's clock.
    ///
    /// The quiet after a move or an edit is the one stretch in which the
    /// caret has something coming and wants no frames until it comes, so it
    /// is the one place a timer belongs — as it does in the oracle, whose
    /// `setTimeout` covers the same 480 ms. All this one does is put the tick
    /// source back; whether the quiet is really over is settled by the frame
    /// times the callback is then handed, so a timer that fires early costs a
    /// frame rather than a wrong blink, and a window with no frames at all
    /// cannot be woken into a loop.
    fn wake_at(&self, when: i64) {
        if self.imp().resume.borrow().is_some() {
            return;
        }
        let left = u64::try_from(when - self.now()).unwrap_or(0);
        let id = glib::timeout_add_local_once(
            Duration::from_micros(left),
            glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move || {
                    editor.imp().resume.take();
                    editor.start_tick();
                },
            ),
        );
        self.imp().resume.replace(Some(id));
    }

    /// Paints the bar the machine says is there, and nothing else.
    ///
    /// Back into the widget's own pixels on the way out. The x the machine
    /// hands back is on a whole device pixel, which on a scale-2 output is
    /// every half of a logical one — which is the point of having snapped it
    /// there rather than here.
    fn draw_caret(&self, snapshot: &gtk::Snapshot) {
        let now = self.now();
        let caret = self.imp().caret.get();
        let alpha = caret.alpha(now);
        if alpha <= 0.0 {
            return;
        }
        draw_box(
            snapshot,
            &paint(Role::Accent, alpha),
            caret.rect(now),
            self.scale(),
        );
    }

    /// Paints the selection, under the glyphs: the fill, which is all of it.
    ///
    /// One [`Editor::selection`] walk per frame, on the one layer. The walk is
    /// bounded by the visible band and `MAX_ROWS`, and with no selection open
    /// it is a `selection_bounds()?` and a return — which is every frame the
    /// writer is only typing.
    fn draw_selection_fill(&self, snapshot: &gtk::Snapshot) {
        let Some(selection) = self.selection() else {
            return;
        };
        let scale = self.scale();
        let fill = selection_fill(self.imp().caret.get().focused());
        for row in &selection.rows {
            draw_box(snapshot, &fill, *row, scale);
        }
    }

    /// The mode `--deterministic` and `--nocaret` asked for.
    ///
    /// A whole machine rather than a mode set on the one there is, because
    /// this is read before the first frame and a machine that has been in
    /// another mode has been keeping the wrong kind of state.
    pub fn set_mode(&self, mode: caret::Mode) {
        self.imp().caret.set(caret::Caret::new(mode));
        self.tell_caret(|caret| caret.resize(self.em()));
    }

    /// The window this Editor is in became active, or stopped being.
    ///
    /// The one place the ghost is decided. The caret's machine drops to its
    /// ghost alpha with the blink stopped, and the selection's fill swaps to
    /// the idle band, because a window that has lost focus should
    /// say where the writer was without shouting it. Both return on focus.
    ///
    /// The machine's own flag is the whole of the state: the selection is
    /// painted here rather than by GTK, so there is nothing left for a CSS
    /// class to reach — [`Editor::draw_selection`] asks
    /// [`caret::Caret::focused`] the same question the caret's alpha does.
    pub fn set_active(&self, active: bool) {
        let now = self.now();
        self.tell_caret(|caret| caret.focus(active, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// The app is about to place the caret itself, with no hand behind it.
    ///
    /// [`caret::Source`] is sticky: a controller sets it and nothing clears
    /// it, so once the writer has pressed a key every later placement would
    /// read as the writer's own. The places that move the caret without a hand
    /// — the launch flags, and later a restored position or a Command — say so
    /// here, which is what lets [`Editor::keep_in_band`] trust the answer
    /// rather than only being right until the first keystroke.
    fn placing(&self) {
        self.imp().last.set(caret::Source::App);
    }

    /// Selects `from` to `to`, in UTF-8 bytes, as `--select` asked.
    ///
    /// The insert mark goes to `to` and the bound to `from`, which is where a
    /// drag or a Shift+arrow leaves them: the selection is GTK's own from
    /// here, painted beneath the glyphs, and the caret is at the end the hand
    /// was moving.
    pub fn select(&self, document: &Document, from: u64, to: u64) {
        self.placing();
        let buffer = self.buffer();
        let bound = tags::iter_at(&buffer, document, byte_offset(from));
        let insert = tags::iter_at(&buffer, document, byte_offset(to));
        buffer.select_range(&insert, &bound);
    }

    /// Puts the caret where `--caret` asked for it, and shows where it went.
    ///
    /// The harness names an offset in UTF-8 bytes, because that is what a
    /// Document is measured in everywhere else, and it is reached through the
    /// Document's own line table by [`tags::iter_at`] — the one byte-to-iter
    /// mapping the app has.
    ///
    /// `reveal` scrolls the view to the caret, because a caret the harness
    /// cannot see is not the state it asked for: a bench typing at the end of
    /// a draft has to be looking at the end of the draft. It is false when
    /// `--scroll` has said where the view goes, since a state that names both
    /// means both, and a judged shot of a passage the opponent is not showing
    /// is not a comparison.
    pub fn place_caret(&self, document: &Document, caret: flags::Caret, reveal: bool) {
        self.placing();
        let buffer = self.buffer();
        let at = match caret {
            flags::Caret::End => buffer.end_iter(),
            flags::Caret::At(offset) => tags::iter_at(&buffer, document, byte_offset(offset)),
        };
        buffer.place_cursor(&at);
        if reveal {
            self.imp().reveal.set(true);
            self.reveal_caret();
        }
    }

    /// Scrolls the view to the caret, but never before the page is laid out.
    ///
    /// GTK resolves a `scroll_to_mark` against the layout the view has, and
    /// until the first `size_allocate` it has none: the request is queued as
    /// a pending scroll and flushed when the view is next validated. On a
    /// Document shorter than the viewport that flush never happens — the
    /// scroll it asks for is a scroll there is no room to make — and the
    /// validation it is holding up is the one that draws the text, so the
    /// window comes up as bare paper with neither glyph nor bar on it
    /// (#148). A long Document was never hit by it because there the scroll
    /// does move, which flushes the queue and lets the validation through.
    ///
    /// So the reveal waits for a size the way [`Editor::lay_out`] does, and
    /// `size_allocate` asks again the moment there is one. One request from
    /// there is enough, where [`Editor::scroll_to`] holds its own across
    /// [`SCROLL_FRAMES`]: measured on a Document of three paragraphs,
    /// `--caret 0`, `--caret 10` and `--caret end` each leave the window on
    /// the same ink `--scroll 0` leaves it on, and `ref/sample.md` does not
    /// move.
    fn reveal_caret(&self) {
        if !self.imp().reveal.get() || self.imp().laid_out.get().is_none() {
            return;
        }
        self.imp().reveal.set(false);
        self.scroll_to_mark(&self.buffer().get_insert(), 0.0, true, 0.0, CARET_LINE);
    }

    /// Scrolls the Document to `fraction` of its length, 0 at the top.
    ///
    /// The fraction is a fraction of the scroll, not of the text: at 0 the
    /// view is at the top of the page, showing the air above the first row,
    /// which is where the oracle puts it and is not where the first row is.
    ///
    /// Held for [`SCROLL_FRAMES`] rather than set once. A view GTK has not
    /// laid out yet cannot say how far it can go, and GTK makes a scroll of
    /// its own on the way there — the one that keeps the caret on screen when
    /// the Editor takes focus, resolved while the layout is validated and so
    /// after this is read. Holding the view for a few frames outlasts it, and
    /// is over long before the harness, which waits for a still window, takes
    /// its shot.
    pub fn scroll_to(&self, fraction: f64) {
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere to scroll to.
            return;
        };
        let fraction = fraction.clamp(0.0, 1.0);
        let left = Cell::new(SCROLL_FRAMES);
        self.add_tick_callback(move |_, _| {
            let room = adjustment.upper() - adjustment.lower() - adjustment.page_size();
            adjustment.set_value(adjustment.lower() + room.max(0.0) * fraction);
            left.set(left.get().saturating_sub(1));
            if left.get() == 0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
}

/// The margins a page was laid out with, in the pixels GTK takes.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Page {
    /// `left-margin` and `right-margin`, which are the same: the column is
    /// centred.
    side: i32,
    /// `bottom-margin`: the air below the last row of the Document.
    bottom: i32,
}

/// A byte offset a flag named, as the Document counts them.
fn byte_offset(bytes: u64) -> usize {
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

/// A length in device pixels, in the widget's own pixels, as `graphene` takes
/// it.
///
/// The narrowing is the one place it belongs: a snapshot is drawn in `f32`,
/// and a caret's four lengths are three digits at most, so nothing here is
/// near what an `f32` stops counting exactly.
fn logical(device: f64, scale: f64) -> f32 {
    (device / scale) as f32
}

/// A channel of a `quill_engine::theme::Colour` as `gdk` takes it: 0 to 1
/// either way, so the narrowing loses nothing a screen could show.
fn channel(value: f64) -> f32 {
    value as f32
}

/// The colour the selection's fill is painted in, which is the whole of the
/// selection's paint since ADR 0014 took the end bars off it.
///
/// A window that is not active keeps saying what is held, in the paler of the
/// two bands `legacy/app/css/theme.css` sets — the fill is the only mark left
/// to say it with, now that the free caret is out for as long as a selection
/// stands and no bar brackets either end.
///
/// A function of the one flag so that it can be checked without a window:
/// `unfocused` is shot with nothing selected, so no judged state carries the
/// idle band and the swap is only ever true here.
fn selection_fill(focused: bool) -> gdk::RGBA {
    if focused {
        paint(Role::Selection, 1.0)
    } else {
        paint(Role::SelectionIdle, 1.0)
    }
}

/// One role's colour on the light ground, at `alpha` of the alpha the table
/// gives it.
///
/// Read from the engine's colour table rather than written out here beside
/// [`PAPER`] and [`INK`], because the table already carries every one of them,
/// on both grounds, at exactly the values `legacy/app/css/theme.css` sets them
/// to. A second copy of a number the critic reads is a second thing to keep
/// true.
///
/// The light ground is not a choice yet: there is one theme until the Dark &
/// light ticket, and [`PAPER`] and [`INK`] are constants for the same reason.
/// The two the selection needs — [`Role::Selection`] and
/// [`Role::SelectionIdle`] — do differ between the grounds, unlike the accent,
/// so this is one of the sites that ticket has to reach.
fn paint(role: Role, alpha: f64) -> gdk::RGBA {
    let colour = Colours::of(Scheme::Light).colour(role);
    gdk::RGBA::new(
        channel(colour.red),
        channel(colour.green),
        channel(colour.blue),
        channel(colour.alpha * alpha),
    )
}

/// Draws one box of the caret's layer, back in the widget's own pixels.
///
/// A box with no area is not drawn: an empty row of a selection is a real
/// place in the text — the end of a line whose newline is not held — and it
/// has nothing to paint.
fn draw_box(snapshot: &gtk::Snapshot, colour: &gdk::RGBA, bar: caret::Bar, scale: f64) {
    if bar.w <= 0.0 || bar.h <= 0.0 {
        return;
    }
    snapshot.append_color(
        colour,
        &graphene::Rect::new(
            logical(bar.x, scale),
            logical(bar.y, scale),
            logical(bar.w, scale),
            logical(bar.h, scale),
        ),
    );
}

/// A count of pixels as a GTK widget takes it.
fn signed(count: u32) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// A GTK widget's own measurement, as the typography takes it. A negative
/// dimension is a widget with no size, which is no room at all.
fn unsigned(pixels: i32) -> u32 {
    u32::try_from(pixels).unwrap_or(0)
}

/// The features as CSS names them, for the stylesheet.
fn css_features() -> String {
    FEATURES
        .map(|feature| format!("\"{feature}\" 0"))
        .join(", ")
}

/// The features as Pango names them, for a layout being measured.
fn pango_features() -> String {
    FEATURES.map(|feature| format!("{feature}=0")).join(",")
}

/// The description body text is laid out with.
///
/// A Face is asked for by family name and never by file: fontconfig already
/// holds the six that [`crate::fonts::load_private`] gave it. The Roman is
/// named explicitly upright and at Regular before the weight axis is set,
/// because a description that leaves either open is one a missing Face can be
/// resolved into obliquely (ADR 0004, ADR 0007). Sizes are absolute pixels;
/// points appear nowhere.
///
/// `em` is the ladder's own value in logical pixels and is fractional — the
/// default step's is 21.33 — so it is handed to Pango as it stands. Rounding
/// it to the whole pixel the type used to be named in would put the page a
/// third of a pixel off the Design oracle at the default and further at other
/// steps, which is the difference the oracles are frozen at.
fn body_font(face: Face, em: f64) -> pango::FontDescription {
    let mut font = pango::FontDescription::new();
    font.set_family(face.family());
    font.set_style(pango::Style::Normal);
    font.set_weight(pango::Weight::Normal);
    font.set_variations(Some(&format!("wght={INK_WEIGHT}")));
    font.set_absolute_size(em * f64::from(pango::SCALE));
    font
}

/// The em at `step` of the type ladder, rounded to whole logical pixels.
///
/// The type itself is set at the fractional em ([`body_font`]); this is the
/// caret's unit alone, because [`caret::width`] and [`caret::tail`] are still
/// scaled off a whole-pixel size. The ladder carries a caret width per step
/// ([`typography::caret_width`]) and reading it there is
/// [#169](https://github.com/danielbaldwin47/Quill/issues/169)'s, with the
/// blink.
fn caret_em_px(step: u32) -> u32 {
    typography::em(step).round() as u32
}

thread_local! {
    /// The one stylesheet the type is named in, held so that a size step
    /// reloads it rather than stacking another provider on the display behind
    /// it. GTK is one thread, so one place is enough.
    static TYPE_STYLE: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Names the type on the display: the Face, the size, the paper and the ink.
///
/// The base font goes through CSS rather than through the buffer's tags so
/// that a blank line, which carries no tag, sits on exactly the same metrics
/// as a written one — a page whose empty lines are a different height is not a
/// page. It is installed once, before the first window, and reloaded whenever
/// the writer steps the size.
pub fn install_type(face: Face, step: u32) {
    let Some(display) = gtk::gdk::Display::default() else {
        // No display: nothing to style, and nothing that will draw text.
        return;
    };
    TYPE_STYLE.with_borrow_mut(|held| {
        let provider = held.get_or_insert_with(|| {
            let provider = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            provider
        });
        provider.load_from_string(&stylesheet(face, typography::em(step)));
    });
}

/// The stylesheet [`install_type`] loads.
///
/// The ink is named on the widget and on its text node both. A `GtkTextView`
/// draws untagged text in the colour of one of the two depending on how it
/// came by its attributes, and a page whose ink is the desktop theme's is not
/// this page at all: on a dark desktop it is white on the oracle's paper, and
/// there is nothing to read.
///
/// The selection rule is here to take GTK's ground away and nothing else. The
/// band belongs to [`Editor::selection`], which cuts it off the same band the
/// caret is cut from so the two register exactly; a ground painted here as
/// well would be a second, differently rounded rectangle under it. The
/// `color` stays, for the same reason the ink is named twice above: clearing
/// the ground alone leaves the selected glyphs to the desktop theme.
fn stylesheet(face: Face, em: f64) -> String {
    format!(
        "window {{ background-color: {PAPER}; }}\n\
         textview.{FACE_CLASS}, textview.{FACE_CLASS} text {{\n\
         \x20 background-color: {PAPER};\n\
         \x20 color: {INK};\n\
         \x20 font-family: \"{family}\";\n\
         \x20 font-size: {em}px;\n\
         \x20 font-style: normal;\n\
         \x20 font-weight: {INK_WEIGHT};\n\
         \x20 font-feature-settings: {features};\n\
         }}\n\
         textview.{FACE_CLASS} text selection {{\n\
         \x20 background-color: transparent;\n\
         \x20 color: {INK};\n\
         }}\n",
        family = face.family(),
        features = css_features()
    )
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder's ems are fractional, and the type is set at them.
    ///
    /// Both places the size is named — GTK's CSS and Pango's description —
    /// take the fraction, and the Editor rounded it to a whole pixel only
    /// while `--size` counted in pixels. At the default step that rounding was
    /// 21.33 to 21, which is a page a third of a pixel narrower per em than
    /// the one the oracles are frozen at.
    #[test]
    fn the_type_is_set_at_the_ladders_fractional_em() {
        let em = typography::em(quill_engine::settings::default_step());
        assert!((em - 21.33).abs() < 0.001, "the default step's em is {em}");
        assert!(
            stylesheet(Face::Duo, em).contains("font-size: 21.33px"),
            "the stylesheet named the type at a whole pixel"
        );
        let described = f64::from(body_font(Face::Duo, em).size());
        let wanted = em * f64::from(pango::SCALE);
        assert!(
            (described - wanted).abs() <= 1.0,
            "Pango was given {described} rather than {wanted}"
        );
    }

    /// GTK paints no selection ground at all, and still paints the glyphs in
    /// the page's own ink.
    ///
    /// The band is ours now, drawn in the same pass and off the same band as
    /// the bar (§ the snapshot), because that is the only way the fill, the
    /// two end bars and the caret can be guaranteed to register. What is left
    /// for the stylesheet is to get GTK out of the way without letting it take
    /// the ink with it: a `selection` rule that only clears the ground would
    /// leave `color` to the desktop theme's selected-text colour, which on a
    /// dark desktop is white on our paper.
    #[test]
    fn the_stylesheet_leaves_the_selection_ground_to_us_and_keeps_the_ink() {
        let css = stylesheet(
            Face::Duo,
            typography::em(quill_engine::settings::default_step()),
        );
        let rule = css
            .split_once(&format!("textview.{FACE_CLASS} text selection"))
            .expect("no selection rule at all")
            .1;
        let rule = rule.split_once('}').expect("unclosed selection rule").0;
        assert!(
            rule.contains("background-color: transparent"),
            "GTK is still painting a selection ground under ours:\n{css}"
        );
        assert!(
            rule.contains(&format!("color: {INK}")),
            "selected glyphs are not in the page's ink:\n{css}"
        );
        assert_eq!(
            css.matches("selection").count(),
            1,
            "the idle swap is the fill's now, not the stylesheet's:\n{css}"
        );
    }

    /// The band swaps when the window goes.
    ///
    /// Held here because no shot can hold it, which is why the stylesheet used
    /// to hold it: `caret/unfocused` is shot with `select: null`, so the one
    /// judged state that is not active is also the one state with no band to
    /// be idle. The swap moved from the stylesheet to the paint when the
    /// selection became ours, and the check moves with it.
    #[test]
    fn the_selection_goes_idle_with_the_window() {
        let fill = selection_fill(true);
        let idle = selection_fill(false);
        assert_eq!(
            fill,
            paint(Role::Selection, 1.0),
            "an active window is the oracle's --selection"
        );
        assert_ne!(fill, idle, "the band did not go idle");
        assert_eq!(idle, paint(Role::SelectionIdle, 1.0));
    }
}
