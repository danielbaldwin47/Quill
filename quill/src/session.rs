//! What Quill read at launch and what it writes at quit.
//!
//! The settings and the state belong to the application rather than to a
//! window, and both are touched once: read before the first window is built,
//! written after the last one is gone. Nothing here re-reads a file while Quill
//! is running — the `notify` watch that applies a saved edit without a restart
//! is the settings ticket's
//! ([#44](https://github.com/danielbaldwin47/Quill/issues/44)) — and nothing
//! here fails: a file that cannot be read or written is one line on stderr and
//! a Quill that opens anyway.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use quill_engine::settings::{Settings, State, WindowState};

/// The settings and state of one run of Quill.
pub struct Session {
    /// What the writer chose.
    settings: Settings,
    /// The shape the next window opens at: what the last session left.
    opening: WindowState,
    /// What this session will leave behind, filled as windows close.
    leaving: RefCell<State>,
}

impl Session {
    /// Reads both files, saying on stderr whatever they were worth saying.
    ///
    /// A first launch writes `settings.toml` with every key at its default, so
    /// that a writer looking for something to edit finds it.
    #[must_use]
    pub fn open() -> Rc<Self> {
        let (settings, notes) = Settings::open();
        report(&Settings::path(), &notes);
        let (mut state, notes) = State::open();
        report(&State::path(), &notes);

        // The windows the last session left are this session's opening shape,
        // and the list is cleared for the windows this one leaves. Nothing is
        // lost with them: a window opens in that shape and carries it, keys
        // this Quill does not know included, back into the file on the way out.
        let opening = state.window();
        state.windows.clear();
        Rc::new(Self {
            settings,
            opening,
            leaving: RefCell::new(state),
        })
    }

    /// What the writer chose.
    pub fn settings(&self) -> &Settings {
        &self.settings
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
}

/// Says what a file was worth saying, one line each, naming the file.
fn report(path: &Path, notes: &[String]) {
    for note in notes {
        eprintln!("quill: {}: {note}", path.display());
    }
}
