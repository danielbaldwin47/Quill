//! What Quill was launched with: the flags, the settings and the state.
//!
//! All three belong to the application rather than to a window, and all three
//! are touched once: read before the first window is built, written after the
//! last one is gone. Nothing here re-reads a file while Quill is running — the
//! `notify` watch that applies a saved edit without a restart is the settings
//! ticket's ([#44](https://github.com/danielbaldwin47/Quill/issues/44)) — and
//! nothing here fails: a file that cannot be read or written is one line on
//! stderr and a Quill that opens anyway.
//!
//! A launch of the harness's ([`Flags::is_harness`]) is the same session with
//! two files' worth of the writer's own removed. It reads their `settings.toml`,
//! because a flag overrides a setting rather than replacing every setting, but
//! it writes nothing to it; and it neither reads nor writes `state.toml`, so it
//! opens at the shape its flags name rather than at the window a writer left,
//! and the window a writer left is still there after a bench has run.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use quill_engine::settings::{Settings, State, WindowState};

use crate::flags::Flags;

/// The settings and state of one run of Quill.
pub struct Session {
    /// What this launch was asked for.
    flags: Flags,
    /// Whether this launch is the harness's, asked once of the flags and
    /// answered from here afterwards.
    harness: bool,
    /// What the writer chose, with the flags over the top, as it was read.
    settings: Settings,
    /// The type size this launch is running at: the setting until the writer
    /// steps it, and then whatever they stepped it to. Held apart from
    /// [`Session::settings`] so that what was read stays readable, which is
    /// how [`Session::store`] knows whether there is anything to write.
    size: Cell<u32>,
    /// The shape the next window opens at: what the last session left, at the
    /// size the flags name.
    opening: WindowState,
    /// What this session will leave behind, filled as windows close.
    leaving: RefCell<State>,
}

impl Session {
    /// Reads both files, saying on stderr whatever they were worth saying.
    ///
    /// A first launch writes `settings.toml` with every key at its default, so
    /// that a writer looking for something to edit finds it. A launch of the
    /// harness's writes nothing at all.
    #[must_use]
    pub fn open(flags: Flags) -> Rc<Self> {
        let harness = flags.is_harness();
        let (settings, notes) = if harness {
            Settings::read_from(&Settings::path())
        } else {
            Settings::open()
        };
        report(&Settings::path(), &notes);

        // The windows the last session left are this session's opening shape,
        // and the list is cleared for the windows this one leaves. Nothing is
        // lost with them: a window opens in that shape and carries it, keys
        // this Quill does not know included, back into the file on the way out.
        let (state, opening) = if harness {
            (State::default(), WindowState::default())
        } else {
            let (mut state, notes) = State::open();
            report(&State::path(), &notes);
            let opening = state.window();
            state.windows.clear();
            (state, opening)
        };

        let settings = flags.over(settings);
        Rc::new(Self {
            opening: flags.shape(opening),
            size: Cell::new(settings.size),
            settings,
            flags,
            harness,
            leaving: RefCell::new(state),
        })
    }

    /// What this launch was asked for.
    pub fn flags(&self) -> &Flags {
        &self.flags
    }

    /// Whether this launch is the harness's rather than a writer's.
    pub fn is_harness(&self) -> bool {
        self.harness
    }

    /// What the writer chose, as this launch is running it.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The type size this launch is reading at.
    pub fn size(&self) -> u32 {
        self.size.get()
    }

    /// Steps the type size, for this launch and — for a writer's — the next.
    pub fn set_size(&self, size: u32) {
        self.size.set(size);
    }

    /// The shape a new window opens at.
    pub fn opening(&self) -> &WindowState {
        &self.opening
    }

    /// Takes down a window's shape as it goes, most recently closed first.
    pub fn remember(&self, window: WindowState) {
        self.leaving.borrow_mut().windows.insert(0, window);
    }

    /// Writes the state file. Called once, when the application shuts down.
    pub fn store(&self) {
        if self.harness {
            // A launch of the harness's read no state and leaves none: the
            // window a writer left is theirs, and a bench at 1440×900 is not
            // a writer resizing it.
            return;
        }
        self.store_settings();
        let mut state = self.leaving.borrow().clone();
        if state.windows.is_empty() {
            // A run that never opened a window, or one whose windows were
            // never seen closing: what the last session left is still true,
            // and is better than forgetting it.
            state.windows.push(self.opening.clone());
        }
        if let Err(err) = state.store() {
            eprintln!(
                "quill: {}: cannot be written ({err})",
                State::path().display()
            );
        }
    }

    /// Writes `settings.toml` when this launch changed something in it.
    ///
    /// Only the size can move so far, and only a writer's launch can move it:
    /// the flags a launch of the harness's carries are this launch's alone and
    /// have no business in the writer's file, which is why a harness launch
    /// has already returned before this is reached. A file that cannot be
    /// written is one line on stderr, like every other file here.
    fn store_settings(&self) {
        if self.size.get() == self.settings.size {
            return;
        }
        let mut settings = self.settings.clone();
        settings.size = self.size.get();
        if let Err(err) = settings.write_to(&Settings::path()) {
            eprintln!(
                "quill: {}: cannot be written ({err})",
                Settings::path().display()
            );
        }
    }
}

/// Says what a file was worth saying, one line each, naming the file.
fn report(path: &Path, notes: &[String]) {
    for note in notes {
        eprintln!("quill: {}: {note}", path.display());
    }
}
