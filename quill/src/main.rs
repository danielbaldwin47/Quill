//! Quill: the application, its windows, and one Document per window.
//!
//! A single-instance `GtkApplication` with `HANDLES_OPEN`, so a file opened
//! from a file manager or a second shell reaches the running instance and
//! becomes another window rather than another process. The Editor, Preview and
//! Stats belong to a window; the Library and settings belong to the
//! application.
//!
//! The Faces are in: a Document is set in Quill Duo from the first launch. The
//! settings, the command-line flags and the theme are the next Scaffold
//! tickets, and each has a place to land because the window and the Editor are
//! already here.

mod editor;
mod fonts;
mod window;

use gtk::gio::ApplicationFlags;
use gtk::glib;
use gtk::prelude::*;

/// The application id, also the `.desktop` file's and the icon's name.
const APP_ID: &str = "io.github.danielbaldwin47.Quill";

fn main() -> glib::ExitCode {
    // Before anything GTK: Pango builds its font map from the current
    // fontconfig the first time it lays text out, and the Faces have to be in
    // it by then. A writer whose Faces are missing gets a line on stderr and a
    // working editor in whatever fontconfig does have, not a dead launch.
    if let Err(err) = fonts::load_private(&quill_engine::data::fonts()) {
        eprintln!("quill: {err}");
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        // HANDLES_OPEN: the primary instance is handed the files, and the
        // second process exits. One Document per window, any number of windows.
        .flags(ApplicationFlags::HANDLES_OPEN)
        .build();

    // Startup runs once, after GTK has a display and before any window: the
    // place for the stylesheet every Editor reads.
    app.connect_startup(|_| editor::install_face());

    // Launched with no file: an empty Editor, a Document with nothing in it.
    app.connect_activate(window::present_untitled);
    app.connect_open(|app, files, _hint| window::present_files(app, files));

    app.run()
}
