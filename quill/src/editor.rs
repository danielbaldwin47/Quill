//! The Editor: the text surface a Document is written on.
//!
//! One `GtkTextView` subclass (ADR 0004, `docs/adr/0004-gtktextview-editor.md`).
//! There is no mirror: the widget lays the text out once, so the web app's rule
//! that no per-token style may change glyph advance width stops being an
//! invariant to hold and becomes a property of the Faces. The `GtkTextBuffer`
//! owns the text the writer edits, which is where undo, IME preedit, clipboard,
//! selection and accessibility come from.
//!
//! Today it shows a Document in the default Face and nothing more. The size,
//! the leading and the measure belong to the type and page tickets; the tag
//! table, the flattened Markup × Focus runs and the hand-drawn caret land on
//! this type.

use gtk::glib;
use gtk::prelude::*;
use quill_engine::document::Document;
use quill_engine::settings::Face;

use crate::flags::Caret;

/// The CSS class the Editor's Face is named on.
const FACE_CLASS: &str = "quill-editor";

/// How far down the view `--caret` leaves the line the caret is on: the middle,
/// which is where a typewriter-scrolled Editor keeps it and the one place that
/// does not depend on how long the document is.
const CARET_LINE: f64 = 0.5;

mod imp {
    use gtk::glib;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct Editor;

    #[glib::object_subclass]
    impl ObjectSubclass for Editor {
        const NAME: &'static str = "QuillEditor";
        type Type = super::Editor;
        type ParentType = gtk::TextView;
    }

    impl ObjectImpl for Editor {}
    impl WidgetImpl for Editor {}
    impl TextViewImpl for Editor {}
}

glib::wrapper! {
    /// The Editor widget.
    pub struct Editor(ObjectSubclass<imp::Editor>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

impl Editor {
    /// An Editor with an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        let editor: Self = glib::Object::new();
        // Prose wraps; the Editor never scrolls sideways. The measure, the
        // margins and the leading are the page and type tickets' work.
        editor.set_wrap_mode(gtk::WrapMode::WordChar);
        editor.add_css_class(FACE_CLASS);
        editor
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
    /// characters, so the two are converted here rather than at the flag. The
    /// view is scrolled to the caret because a caret the harness cannot see is
    /// not the state it asked for: a bench typing at the end of a draft has to
    /// be looking at the end of the draft.
    pub fn place_caret(&self, caret: Caret) {
        let buffer = self.buffer();
        let at = match caret {
            Caret::End => buffer.end_iter(),
            Caret::At(offset) => buffer.iter_at_offset(character_offset(&buffer, offset)),
        };
        buffer.place_cursor(&at);
        self.scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, CARET_LINE);
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

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// Names the Editor's Face on the display, once, before the first window.
///
/// A Face is asked for by family name and never by file: fontconfig already
/// holds the six that [`crate::fonts::load_private`] gave it, and the same line
/// still means something when they are missing. One `font-family` and nothing
/// else — the size and the leading are the type ticket's, and are the reason
/// the writer's `size` is read at launch but not yet applied: a size without
/// the leading that belongs to it is half a decision.
pub fn install_face(face: Face) {
    let Some(display) = gtk::gdk::Display::default() else {
        // No display: nothing to style, and nothing that will draw text.
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&format!(
        "textview.{FACE_CLASS} {{ font-family: \"{}\"; }}",
        face.family()
    ));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
