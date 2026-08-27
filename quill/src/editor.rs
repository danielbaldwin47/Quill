//! The Editor: the text surface a Document is written on.
//!
//! One `GtkTextView` subclass (ADR 0004, `docs/adr/0004-gtktextview-editor.md`).
//! There is no mirror: the widget lays the text out once, so the web app's rule
//! that no per-token style may change glyph advance width stops being an
//! invariant to hold and becomes a property of the Faces. The `GtkTextBuffer`
//! owns the text the writer edits, which is where undo, IME preedit, clipboard,
//! selection and accessibility come from.
//!
//! Today it shows a Document and nothing more. The tag table, the flattened
//! Markup × Focus runs and the hand-drawn caret land on this type.

use gtk::glib;
use gtk::prelude::*;
use quill_engine::document::Document;

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
