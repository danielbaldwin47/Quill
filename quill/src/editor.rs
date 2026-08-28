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

use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use quill_engine::document::Document;
use quill_engine::settings::Face;
use quill_engine::typography;

use crate::flags::Caret;

/// The CSS class the Editor's type is named on.
const FACE_CLASS: &str = "quill-editor";

/// How far down the view `--caret` leaves the line the caret is on: the middle,
/// which is where a typewriter-scrolled Editor keeps it and the one place that
/// does not depend on how long the document is.
const CARET_LINE: f64 = 0.5;

/// Paper and ink: the light palette of `legacy/app/css/theme.css`.
///
/// Two constants rather than a table, because there is one theme until the
/// Dark & light ticket, and a table with one row in it says something that is
/// not true yet.
const PAPER: &str = "#f9f9f9";
const INK: &str = "#1c1c1c";

/// The weight ink is set at on paper: `--ink-weight: 415` in
/// `legacy/app/css/type.css`, a little heavier than Regular because a light
/// ground eats stems. The Faces are variable, and none of the three moves a
/// glyph's advance across the weight axis (`spike/gtk4-editor/RESULTS.txt`).
const INK_WEIGHT: u32 = 415;

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
    use std::cell::Cell;

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::settings::Face;

    #[derive(Default)]
    pub struct Editor {
        /// The Face this Editor is set in.
        pub face: Cell<Face>,
        /// The type size, in pixels.
        pub size: Cell<u32>,
        /// The page as it was last laid out. Setting a margin queues another
        /// allocation, so an allocation that does not move the page must not
        /// set it again.
        pub laid_out: Cell<Option<super::Page>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Editor {
        const NAME: &'static str = "QuillEditor";
        type Type = super::Editor;
        type ParentType = gtk::TextView;
    }

    impl ObjectImpl for Editor {}

    impl WidgetImpl for Editor {
        /// The measure is centred in the room there turned out to be, and the
        /// air below the column is a share of the window's height, so both are
        /// answered here rather than guessed before the window has a size.
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().lay_out(width, height);
        }
    }

    impl TextViewImpl for Editor {}
}

glib::wrapper! {
    /// The Editor widget.
    pub struct Editor(ObjectSubclass<imp::Editor>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
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
        editor
            .imp()
            .size
            .set(quill_engine::settings::default_size());
        editor
    }

    /// Sets the Editor in `face` at `size` and lays the page out again.
    ///
    /// Everything downstream of the size moves with it: a size without the
    /// leading that belongs to it is half a decision, and a leading without
    /// the measure that belongs to it is the other half.
    pub fn set_type(&self, face: Face, size: u32) {
        self.imp().face.set(face);
        self.imp().size.set(size);
        self.restyle();
    }

    /// The leading, the air above the column, and then the column.
    fn restyle(&self) {
        let pitch = typography::pitch(self.imp().size.get());
        let leading = typography::leading(pitch, self.row_height());
        self.set_pixels_above_lines(signed(leading.above));
        self.set_pixels_inside_wrap(signed(leading.inside_wrap));
        self.set_pixels_below_lines(signed(leading.below));
        self.set_top_margin(signed(typography::page_top(pitch)));
        self.lay_out(self.width(), self.height());
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
        let measure = typography::measure(self.imp().face.get(), self.imp().size.get());
        let page = Page {
            side: signed(typography::column(unsigned(width), measure).side),
            bottom: signed(typography::page_bottom(unsigned(height))),
        };
        if self.imp().laid_out.get() == Some(page) {
            return;
        }
        self.imp().laid_out.set(Some(page));
        self.set_left_margin(page.side);
        self.set_right_margin(page.side);
        self.set_bottom_margin(page.bottom);
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
        let layout = self.create_pango_layout(Some("Ag"));
        layout.set_font_description(Some(&body_font(
            self.imp().face.get(),
            self.imp().size.get(),
        )));
        let features = pango::AttrList::new();
        features.insert(pango::AttrFontFeatures::new(&pango_features()));
        layout.set_attributes(Some(&features));
        unsigned(layout.pixel_size().1)
    }

    /// Shows `document`, with the caret at its start.
    pub fn show_document(&self, document: &Document) {
        let buffer = self.buffer();
        buffer.set_text(document.text());
        buffer.place_cursor(&buffer.start_iter());
    }

    /// Puts the caret where `--caret` asked for it, and shows where it went.
    ///
    /// The harness names an offset in UTF-8 bytes, because that is what a
    /// Document is measured in everywhere else; a `GtkTextBuffer` counts in
    /// characters, so the two are converted here rather than at the flag.
    ///
    /// `reveal` scrolls the view to the caret, because a caret the harness
    /// cannot see is not the state it asked for: a bench typing at the end of
    /// a draft has to be looking at the end of the draft. It is false when
    /// `--scroll` has said where the view goes, since a state that names both
    /// means both, and a judged shot of a passage the opponent is not showing
    /// is not a comparison.
    pub fn place_caret(&self, caret: Caret, reveal: bool) {
        let buffer = self.buffer();
        let at = match caret {
            Caret::End => buffer.end_iter(),
            Caret::At(offset) => buffer.iter_at_offset(character_offset(&buffer, offset)),
        };
        buffer.place_cursor(&at);
        if reveal {
            self.scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, CARET_LINE);
        }
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

/// Where `bytes` UTF-8 bytes into `buffer` is, counted in the characters a
/// `GtkTextBuffer` addresses by.
///
/// An offset past the end lands at the end, and an offset inside a character
/// lands after that character: the flag names a place in a Document the harness
/// chose, and neither is worth refusing a launch over.
fn character_offset(buffer: &gtk::TextBuffer, bytes: u64) -> i32 {
    let bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    let characters = text
        .char_indices()
        .take_while(|(at, _)| *at < bytes)
        .count();
    i32::try_from(characters).unwrap_or(i32::MAX)
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
fn body_font(face: Face, size: u32) -> pango::FontDescription {
    let mut font = pango::FontDescription::new();
    font.set_family(face.family());
    font.set_style(pango::Style::Normal);
    font.set_weight(pango::Weight::Normal);
    font.set_variations(Some(&format!("wght={INK_WEIGHT}")));
    font.set_absolute_size(f64::from(size) * f64::from(pango::SCALE));
    font
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
pub fn install_type(face: Face, size: u32) {
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
        provider.load_from_string(&stylesheet(face, size));
    });
}

/// The stylesheet [`install_type`] loads.
///
/// The ink is named on the widget and on its text node both. A `GtkTextView`
/// draws untagged text in the colour of one of the two depending on how it
/// came by its attributes, and a page whose ink is the desktop theme's is not
/// this page at all: on a dark desktop it is white on the oracle's paper, and
/// there is nothing to read.
fn stylesheet(face: Face, size: u32) -> String {
    format!(
        "window {{ background-color: {PAPER}; }}\n\
         textview.{FACE_CLASS}, textview.{FACE_CLASS} text {{\n\
         \x20 background-color: {PAPER};\n\
         \x20 color: {INK};\n\
         \x20 font-family: \"{family}\";\n\
         \x20 font-size: {size}px;\n\
         \x20 font-style: normal;\n\
         \x20 font-weight: {INK_WEIGHT};\n\
         \x20 font-feature-settings: {features};\n\
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
