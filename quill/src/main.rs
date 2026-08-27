//! Quill: the application, its windows, and one Document per window.
//!
//! A single-instance `GtkApplication` with `HANDLES_OPEN`, so a file opened
//! from a file manager or a second shell reaches the running instance and
//! becomes another window rather than another process. The Editor, Preview and
//! Stats belong to a window; the Library and settings belong to the
//! application.
//!
//! Nothing here styles anything yet: no Faces, no theme, no settings, no
//! command-line flags. Those are the next three Scaffold tickets, and each has
//! a place to land because this one put the window and the Editor in.

mod editor;
mod window;

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{gio, glib};

/// The application id, also the `.desktop` file's and the icon's name.
const APP_ID: &str = "io.github.danielbaldwin47.Quill";

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        // HANDLES_OPEN: the primary instance is handed the files, and the
        // second process exits. One Document per window, any number of windows.
        .flags(ApplicationFlags::HANDLES_OPEN)
        .build();

    // Launched with no file: an empty Editor, a Document with nothing in it.
    app.connect_activate(window::present_untitled);
    app.connect_open(|app, files, _hint| window::present_files(app, files));

    app.run()
}

/// Where a failure to open a file goes until there is a writer-facing place for
/// it. The Library spec decides what a writer sees; until then the window still
/// opens, empty, and the reason is on stderr rather than swallowed.
fn report(context: &str, err: &dyn std::error::Error) {
    eprintln!("quill: {context}: {err}");
}

/// The `gio::File`s an open request carried, as paths.
fn paths(files: &[gio::File]) -> Vec<std::path::PathBuf> {
    files.iter().filter_map(gio::File::path).collect()
}
