//! Quill: the application, its windows, and one Document per window.
//!
//! A single-instance `GtkApplication` with `HANDLES_OPEN`, so a file opened
//! from a file manager or a second shell reaches the running instance and
//! becomes another window rather than another process. The Editor, Preview and
//! Stats belong to a window; the Library and settings belong to the
//! application.
//!
//! The settings are in: the writer's `settings.toml` is read once before the
//! first window and the state file is written once as the last one goes, so a
//! window comes back the size it was left and the Face a writer chose is the
//! Face they get. The command-line flags that override both for one launch are
//! the next Scaffold ticket's.

mod editor;
mod fonts;
mod session;
mod window;

use std::rc::Rc;

use gtk::gio::ApplicationFlags;
use gtk::glib;
use gtk::prelude::*;

use session::Session;

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

    // And before any window: what the writer chose, and what the last session
    // left. Both files are read here and nowhere else.
    let session = Session::open();

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        // HANDLES_OPEN: the primary instance is handed the files, and the
        // second process exits. One Document per window, any number of windows.
        .flags(ApplicationFlags::HANDLES_OPEN)
        .build();

    // Startup runs once, after GTK has a display and before any window: the
    // place for the stylesheet every Editor reads.
    let face = session.settings().face;
    app.connect_startup(move |_| editor::install_face(face));

    // Launched with no file: an empty Editor, a Document with nothing in it.
    // Each handler outlives this scope, so each holds the session it uses.
    let untitled = Rc::clone(&session);
    app.connect_activate(move |app| window::present_untitled(app, &untitled));
    let opened = Rc::clone(&session);
    app.connect_open(move |app, files, _hint| window::present_files(app, files, &opened));

    // Shutdown runs once, after the last window: the place state is written.
    app.connect_shutdown(move |app| {
        window::remember_open(app);
        session.store();
    });

    app.run()
}
