//! One Document per window.
//!
//! A window owns the Document it shows. That ownership is the structure the
//! rest of the file work hangs off: autosave, the reload when the file changes
//! underneath, the caret position remembered per Document and the engine's own
//! copy of the text all belong to the Document a window holds, not to a
//! `GtkTextBuffer` that has forgotten where its text came from.
//!
//! Windows belong to the application, so a file opened while Quill is running
//! joins the running instance instead of starting a second one, and closing one
//! window leaves the others alone.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use quill_engine::document::Document;

/// Until settings remember a size, every window opens at the shape the spike
/// was judged at.
const DEFAULT_WIDTH: i32 = 1100;
const DEFAULT_HEIGHT: i32 = 760;

mod imp {
    use std::cell::RefCell;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{ScrolledWindow, glib};
    use quill_engine::document::Document;

    use crate::editor::Editor;

    #[derive(Default)]
    pub struct Window {
        /// The one Document this window shows.
        pub document: RefCell<Document>,
        pub editor: Editor,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "QuillWindow";
        type Type = super::Window;
        type ParentType = gtk::ApplicationWindow;
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();
            window.set_default_size(super::DEFAULT_WIDTH, super::DEFAULT_HEIGHT);
            let scroller = ScrolledWindow::builder()
                .hexpand(true)
                .vexpand(true)
                // Prose wraps, so there is nothing to scroll to sideways.
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&self.editor)
                .build();
            window.set_child(Some(&scroller));
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
}

glib::wrapper! {
    /// A window, and the Document it shows.
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
                    gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl Window {
    /// A window of `app` showing `document`.
    fn new(app: &gtk::Application, document: Document) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();
        window.set_document(document);
        window.imp().editor.grab_focus();
        window
    }

    /// Takes `document` as this window's own and shows it.
    fn set_document(&self, document: Document) {
        self.imp().document.replace(document);
        let document = self.imp().document.borrow();
        self.set_title(Some(&document.title()));
        self.imp().editor.show_document(&document);
    }
}

/// Opens a window on a Document that is not on disk yet.
pub fn present_untitled(app: &gtk::Application) {
    Window::new(app, Document::untitled()).present();
}

/// Opens one window per file of an open request.
///
/// A file that cannot be read is reported and skipped, so the other files still
/// get their windows. Reporting is stderr until the Library spec decides what a
/// writer sees; the point today is that nothing fails silently.
pub fn present_files(app: &gtk::Application, files: &[gio::File]) {
    for file in files {
        let Some(path) = file.path() else {
            // A `gio::File` with no local path: a URI Quill cannot read as a
            // file. Saying so beats opening a window on nothing.
            eprintln!("quill: not a local file: {}", file.uri());
            continue;
        };
        match Document::open(&path) {
            Ok(document) => Window::new(app, document).present(),
            Err(err) => eprintln!("quill: cannot open {}: {err}", path.display()),
        }
    }
}
