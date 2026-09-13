//! Quill: the application, its windows, and one Document per window.
//!
//! A single-instance `GtkApplication` with `HANDLES_OPEN`, so a file opened
//! from a file manager or a second shell reaches the running instance and
//! becomes another window rather than another process. The Editor, Preview and
//! Stats belong to a window; the Library and settings belong to the
//! application.
//!
//! Four things happen before the first window, in this order and for the same
//! reason — none of them can be changed once a frame has been drawn. The
//! command line is read ([`flags`]), so that a launch knows what it is. The
//! Faces are given to fontconfig ([`fonts`]). The desktop is asked which ground
//! it prefers ([`portal`]), which is a question only worth asking before the
//! answer would have to be a repaint. And the writer's `settings.toml` and
//! `state.toml` are read ([`session`]), with the flags over the top for this
//! launch alone and the desktop's answer resolved against them.
//!
//! A launch carrying a flag is the harness's rather than a writer's, and is
//! served by the process that was launched: it runs non-unique, so a judged
//! shot or a bench never lands in a window of the Quill a writer already has
//! open, and it leaves both files exactly as it found them.

mod caret;
mod choices;
mod chrome;
mod column;
mod conflict;
mod corrections;
mod editor;
mod export;
mod export_dialog;
mod files;
mod flags;
mod fonts;
mod ground;
mod harness;
mod menus;
mod palette;
mod portal;
mod preview;
mod print;
mod session;
mod settings;
mod shortcuts;
mod sidebar;
mod syntax;
mod tags;
mod window;

use std::rc::Rc;

use gtk::gio::ApplicationFlags;
use gtk::glib;
use gtk::prelude::*;

use quill_engine::theme;

use flags::Flags;
use session::Session;

/// The application id, also the `.desktop` file's and the icon's name.
const APP_ID: &str = "io.github.danielbaldwin47.Quill";

/// The GLib log domain libenchant's provider warnings are raised under.
const ENCHANT_DOMAIN: &str = "libenchant";

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

    // Before any enchant broker, which is every Document opening with Spell
    // check on: libenchant warns once per provider it cannot load — aspell,
    // nuspell, voikko, whichever this machine lacks — on every broker, and a
    // desktop launch's stderr is the journal. Kept, at debug level, so
    // `G_MESSAGES_DEBUG=libenchant` still shows them (#401 § libenchant's
    // noise).
    glib::log_set_handler(
        Some(ENCHANT_DOMAIN),
        glib::LogLevels::LEVEL_WARNING | glib::LogLevels::LEVEL_MESSAGE,
        false,
        false,
        |domain, _, message| {
            glib::log_default_handler(domain, glib::LogLevel::Debug, Some(message))
        },
    );

    // And the fixture Library before the settings, because the Locations are
    // walked as the session opens: `--library` names a tree in the checkout
    // and the launch walks a stamped copy of it, so the copy has to exist
    // before anything asks what the Library holds.
    let flags = match staged(flags) {
        Ok(flags) => flags,
        Err(err) => {
            eprintln!("quill: {err}");
            return glib::ExitCode::FAILURE;
        }
    };

    // Before anything GTK: Pango builds its font map from the current
    // fontconfig the first time it lays text out, and the Faces have to be in
    // it by then. A writer whose Faces are missing gets a line on stderr and a
    // working editor in whatever fontconfig does have, not a dead launch.
    if let Err(err) = fonts::load_private(&quill_engine::data::fonts()) {
        eprintln!("quill: {err}");
    }

    // The desktop, before the session that has to resolve a ground out of what
    // it says. Asked here rather than after GTK is up because the answer is an
    // input to the first frame: a ground resolved any later than this is a
    // flash of the other one. It costs a launch nothing where there is no
    // portal — `Portal::open` is `None` and the ground is the one this Quill
    // left — and at most `portal::TIMEOUT` where there is a bus but no answer.
    let portal = portal::Portal::open();

    // And before any window: what the writer chose, what the last session
    // left, what the desktop answered, and what this command line says
    // instead. All read here and nowhere else, and the session is what
    // everything below asks.
    let session = Session::open(flags, portal.as_ref().and_then(portal::Portal::scheme));
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
    app.connect_startup(move |app| {
        if starting.flags().deterministic {
            harness::determine();
        }
        // GTK's caret is painted transparent by the stylesheet, not switched
        // off (#220), so GTK would still blink it on its own timer, and every
        // blink of a bar nobody can see is a frame an idle window should not
        // present. Off here for every launch, deterministic or not: Quill
        // blinks its own caret.
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_cursor_blink(false);
        }
        // Before the window rather than with it: the file a bench was promised
        // is there even if nothing opens.
        if let Some(out) = starting.flags().measure.as_deref() {
            harness::capture(out);
        }
        // The ground is in the stylesheet, and the stylesheet is loaded here:
        // before any window exists, so the first frame a writer sees is
        // already on the paper they asked for and never flashes the other one.
        // At the class a window with no width yet is in: the first allocation
        // loads the sheet again if the window turns out to be narrower
        // ([`editor::install_type`]).
        editor::install_type(
            starting.ground(),
            starting.settings().face,
            quill_engine::typography::SizeClass::default(),
            starting.step(),
        );
        // The chords every Command is bound to: the registry's, with the
        // writer's `[shortcuts]` table over the top. Here rather than beside
        // the actions below, because reading a chord is
        // `gtk::accelerator_parse`'s and it wants GTK started; and before any
        // window, so the first menu a writer opens is already labelled the way
        // they rebound it.
        starting.warn(chrome::install_chords(app, &starting));
    });

    // And from here on, the desktop can change its mind. Subscribed after the
    // ground is resolved and never before it: a signal that arrived while the
    // first frame was still being decided would be answering a question the
    // launch was in the middle of asking. What a signal *means* is the
    // engine's — `theme::followed` reads it against the setting this launch is
    // on, so a writer who pinned a ground, or launched with `--theme`, hears
    // nothing — and everything below only does what it is told.
    if let Some(portal) = &portal {
        let following = Rc::clone(&session);
        let app = app.clone();
        portal.watch_scheme(move |desktop| {
            // Taken down whether or not it is followed: Follow System, picked
            // later from the Palette, returns to what the desktop is on now.
            following.desktop_moved(desktop);
            let Some(scheme) = theme::followed(following.theme(), desktop, following.scheme())
            else {
                return;
            };
            following.follow(scheme);
            window::repaint(&app, &following);
        });
    }

    // The `app.` actions; the `win.` actions go on each window as it is built,
    // and the chords go on at startup, where GTK can read one.
    chrome::install(&app, &session);
    // And the one `app.` action that is not a Command: what an export's
    // notification opens the file it wrote with.
    export::install(&app);

    // And the settings file is watched from here on: a saved edit applies
    // without a restart, whatever the writer changed.
    session::watch_settings(&app, &session);

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
        // Autosave's last moment: a Quill going down with a window still open
        // — the desktop ending the session, rather than `Ctrl+Q`, which asks
        // each window first — leaves the file holding the last keystroke.
        window::flush_open(app);
        window::remember_open(app);
        session.store();
        // The `--library` copy is this launch's own, and goes with it
        // ([`files::stage`]).
        files::unstage();
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

/// The flags with `--library` pointed at the copy of the fixture this launch
/// walks, and every Document named inside the fixture pointed at the copy with
/// it ([`files::stage`], [`files::restaged`]).
///
/// # Errors
///
/// One line naming the flag and what was missing: a fixture folder that is not
/// there, or one with no `manifest.json` beside it.
fn staged(mut flags: Flags) -> Result<Flags, String> {
    let Some(fixture) = flags.library.clone() else {
        return Ok(flags);
    };
    let root = files::stage(&fixture).map_err(|err| format!("--library: {err}"))?;
    if let Some(text) = flags.text.take() {
        flags.text = Some(files::restaged(&fixture, &root, &text));
    }
    for file in &mut flags.files {
        *file = files::restaged(&fixture, &root, file);
    }
    flags.library = Some(root);
    Ok(flags)
}
