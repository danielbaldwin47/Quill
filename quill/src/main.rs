//! Quill: the application, its windows, and one Document per window.
//!
//! A single-instance `GtkApplication` with `HANDLES_OPEN`, so a file opened
//! from a file manager or a second shell reaches the running instance and
//! becomes another window rather than another process. The Editor, Preview and
//! Stats belong to a window; the Library and settings belong to the
//! application.
//!
//! Three things happen before the first window, in this order and for the same
//! reason — none of them can be changed once a frame has been drawn. The
//! command line is read ([`flags`]), so that a launch knows what it is. The
//! Faces are given to fontconfig ([`fonts`]). And the writer's `settings.toml`
//! and `state.toml` are read ([`session`]), with the flags over the top for
//! this launch alone.
//!
//! A launch carrying a flag is the harness's rather than a writer's, and is
//! served by the process that was launched: it runs non-unique, so a judged
//! shot or a bench never lands in a window of the Quill a writer already has
//! open, and it leaves both files exactly as it found them.

mod caret;
mod editor;
mod flags;
mod fonts;
mod harness;
mod session;
mod tags;
mod window;

use std::rc::Rc;

use gtk::gio::ApplicationFlags;
use gtk::glib;
use gtk::prelude::*;

use flags::Flags;
use session::Session;

/// The application id, also the `.desktop` file's and the icon's name.
const APP_ID: &str = "io.github.danielbaldwin47.Quill";

fn main() -> glib::ExitCode {
    // The command line first, because everything below reads it. A flag Quill
    // does not know is one line and no window: a harness that misspelled a
    // flag would otherwise shoot the default state and call it a state.
    let flags = match Flags::parse(std::env::args_os().skip(1)) {
        Ok(flags) => flags,
        Err(err) => {
            eprintln!("quill: {err}");
            return glib::ExitCode::FAILURE;
        }
    };
    if flags.help {
        println!("{}", flags::USAGE);
        return glib::ExitCode::SUCCESS;
    }

    // Before anything GTK: Pango builds its font map from the current
    // fontconfig the first time it lays text out, and the Faces have to be in
    // it by then. A writer whose Faces are missing gets a line on stderr and a
    // working editor in whatever fontconfig does have, not a dead launch.
    if let Err(err) = fonts::load_private(&quill_engine::data::fonts()) {
        eprintln!("quill: {err}");
    }

    // And before any window: what the writer chose, what the last session
    // left, and what this command line says instead. All read here and nowhere
    // else, and the session is what everything below asks.
    let session = Session::open(flags);
    let harness = session.is_harness();

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        // HANDLES_OPEN: the primary instance is handed the files, and the
        // second process exits. One Document per window, any number of
        // windows. NON_UNIQUE for a launch of the harness's: there is no
        // primary instance to hand anything to, so the shot lands here.
        .flags(if harness {
            ApplicationFlags::HANDLES_OPEN | ApplicationFlags::NON_UNIQUE
        } else {
            ApplicationFlags::HANDLES_OPEN
        })
        .build();

    // Startup runs once, after GTK has a display and before any window: the
    // place for the Gate's determinism settings and for the stylesheet every
    // Editor reads. Each handler outlives this scope, so each holds the
    // session it uses.
    let starting = Rc::clone(&session);
    app.connect_startup(move |_| {
        if starting.flags().deterministic {
            harness::determine();
        }
        // Before the window rather than with it: the file a bench was promised
        // is there even if nothing opens.
        if let Some(out) = starting.flags().measure.as_deref() {
            harness::capture(out);
        }
        // The ground is in the stylesheet, and the stylesheet is loaded here:
        // before any window exists, so the first frame a writer sees is
        // already on the paper they asked for and never flashes the other one.
        editor::install_type(starting.scheme(), starting.settings().face, starting.step());
    });

    // The accelerators the Commands answer to, from the Appearance rows of
    // `docs/shortcuts.md`. On the application because that is where GTK keeps
    // them, naming window Commands, because that is where `shortcuts.md` puts
    // them: `font.bigger` is `win.font.bigger`. `Ctrl++` is Bigger Text's
    // alias, the chord a writer reaches for on a keyboard whose `+` is not a
    // shifted `=`, and is never labelled.
    app.set_accels_for_action("win.font.bigger", &["<Ctrl>equal", "<Ctrl>plus"]);
    app.set_accels_for_action("win.font.smaller", &["<Ctrl>minus"]);
    app.set_accels_for_action("win.font.reset", &["<Ctrl>0"]);
    app.set_accels_for_action("win.theme.toggle", &["<Ctrl><Shift>l"]);

    // Launched with no file: an empty Editor, a Document with nothing in it.
    let activated = Rc::clone(&session);
    app.connect_activate(move |app| window::present_launch(app, &activated));
    let opened = Rc::clone(&session);
    app.connect_open(move |app, files, _hint| window::present_files(app, files, &opened));

    // Shutdown runs once, after the last window: the place state is written.
    // The capture goes first, because a bench is waiting on that file and
    // nothing here can fail in a way that should cost it its last keys.
    app.connect_shutdown(move |app| {
        harness::flush();
        window::remember_open(app);
        session.store();
    });

    if harness {
        // The flags have been read already, and GTK would refuse most of them.
        // Only the program's own name is handed on, so the application opens
        // what the flags name rather than what the command line looks like.
        app.run_with_args(&["quill"])
    } else {
        app.run()
    }
}
