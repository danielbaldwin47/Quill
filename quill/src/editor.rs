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

/// The CSS class the Editor's Face is named on.
const FACE_CLASS: &str = "quill-editor";

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
