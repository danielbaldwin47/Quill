//! One window per Document.
//!
//! A window is a Document, an Editor and the chrome around them. Windows belong
//! to the application, so a file opened while Quill is running joins the
//! running instance instead of starting a second one, and closing one window
//! leaves the others alone.

use gtk::prelude::*;
use gtk::{ScrolledWindow, gio};
use quill_engine::document::Document;

use crate::editor::Editor;
use crate::{paths, report};

/// Until settings remember a size, every window opens at the shape the spike
/// was judged at.
const DEFAULT_WIDTH: i32 = 1100;
const DEFAULT_HEIGHT: i32 = 760;

/// Opens a window on a Document that is not on disk yet.
pub fn present_untitled(app: &gtk::Application) {
    present(app, &Document::untitled());
}

/// Opens one window per file of an open request.
///
/// A file that cannot be read is reported and skipped: the other files still
/// get their windows.
pub fn present_files(app: &gtk::Application, files: &[gio::File]) {
    let paths = paths(files);
    if paths.is_empty() {
        present_untitled(app);
        return;
    }
    for path in paths {
        match Document::open(&path) {
            Ok(document) => present(app, &document),
            Err(err) => report(&format!("cannot open {}", path.display()), &err),
        }
    }
}

fn present(app: &gtk::Application, document: &Document) {
    let editor = Editor::new();
    editor.show_document(document);

    let scroller = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        // Prose wraps, so there is nothing to scroll to sideways.
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&editor)
        .build();

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(document.title())
        .default_width(DEFAULT_WIDTH)
        .default_height(DEFAULT_HEIGHT)
        .child(&scroller)
        .build();

    window.present();
    editor.grab_focus();
}
