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

use quill_engine::settings::{Settings, State, Theme, WindowState};
use quill_engine::theme::{self, Scheme};

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
    /// The step of the type ladder this launch is running at: the setting
    /// until the writer steps it, and then whatever they stepped it to. Held
    /// apart from [`Session::settings`] so that what was read stays readable,
    /// which is how [`Session::store`] knows whether there is anything to
    /// write.
    step: Cell<u32>,
    /// What the writer's `theme` setting says now: what was read, until they
    /// toggle it. Held apart from [`Session::settings`] for the reason
    /// [`Session::step`] is — what was read has to stay readable for
    /// [`Session::store_settings`] to know there is anything to write.
    theme: Cell<Theme>,
    /// The ground this launch is painting on, resolved once before the first
    /// window: the flag, then the setting, then — once #111 wires it — the
    /// desktop, then what the last session left.
    ///
    /// Resolved rather than read, because `auto` is not a ground. Every colour
    /// the app paints is read off this scheme's row of the engine's table.
    scheme: Cell<Scheme>,
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
    pub fn open(flags: Flags, portal: Option<Scheme>) -> Rc<Self> {
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

        Self::launch(flags, settings, state, opening, harness, portal)
    }

    /// The session those three make, with no file in sight.
    ///
    /// Split from [`Session::open`] because the two halves answer different
    /// questions and only one of them can fail: reading and writing files is
    /// [`Session::open`]'s and [`Session::store`]'s, and what this launch is
    /// *running* — the ground above all, which nothing else resolves — is
    /// decided here, out of three values, where it can be checked without a
    /// writer's `settings.toml` under it.
    fn launch(
        flags: Flags,
        settings: Settings,
        mut state: State,
        opening: WindowState,
        harness: bool,
        portal: Option<Scheme>,
    ) -> Rc<Self> {
        let settings = flags.over(settings);
        // The ground, before anything can paint on it. The flag is passed as
        // itself rather than read back off the setting it already overrode,
        // because the two are different answers to a different question once
        // the portal is asked: a setting of `auto` follows the desktop and a
        // `--theme` never does. The portal is `None` where the desktop has no
        // answer, or none it gave in time, and the ground this Quill left is
        // what an `auto` launch paints instead.
        let scheme = theme::effective(flags.theme, settings.theme, portal, state.last_scheme);
        // What this session will leave for the next one to open on while the
        // desktop is still being asked.
        state.last_scheme = scheme;
        Rc::new(Self {
            opening: flags.shape(opening),
            step: Cell::new(settings.step),
            theme: Cell::new(settings.theme),
            scheme: Cell::new(scheme),
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

    /// The step of the type ladder this launch is reading at.
    pub fn step(&self) -> u32 {
        self.step.get()
    }

    /// Steps the type size, for this launch and — for a writer's — the next.
    pub fn set_step(&self, step: u32) {
        self.step.set(step);
    }

    /// The ground this launch is painting on.
    pub fn scheme(&self) -> Scheme {
        self.scheme.get()
    }

    /// What the writer asked for, which is a question where it is `auto`.
    ///
    /// The live one rather than `settings().theme`, because the toggle moves
    /// it: a writer who has pressed the key has pinned a ground, and the
    /// desktop is no longer being followed.
    pub fn theme(&self) -> Theme {
        self.theme.get()
    }

    /// Moves to the other ground, the way `legacy/app/js/theme.js` does: the
    /// setting becomes the ground that is not on screen now.
    ///
    /// From `auto` that is the opposite of whatever the desktop was answering,
    /// which is what a writer means by the key: they are looking at a ground
    /// they want changed, and `auto` is not a ground. So three presses from
    /// `auto` leave the setting at the opposite of the ground this launch
    /// opened on, and the file says so.
    pub fn toggle_scheme(&self) -> Scheme {
        let scheme = self.scheme.get().other();
        self.scheme.set(scheme);
        self.theme.set(scheme.setting());
        self.leaving.borrow_mut().last_scheme = scheme;
        scheme
    }

    /// Follows the desktop to `scheme`, for this launch and for the next.
    ///
    /// [`Session::toggle_scheme`]'s sibling, and deliberately not the same
    /// method: a writer pressing the key is choosing a ground, so the setting
    /// moves with it, and a desktop changing under a writer who asked for
    /// `auto` is answering the question they left open. Writing `light` into
    /// their `settings.toml` because their desktop went light this morning
    /// would take `auto` away from them without their touching anything.
    ///
    /// The ground itself is remembered either way: `last_scheme` is what the
    /// next launch paints before the portal has answered, so it wants to be
    /// the ground actually on screen however that was arrived at.
    ///
    /// Whether a signal reaches here at all is
    /// [`theme::followed`](quill_engine::theme::followed)'s, not this
    /// function's: the decision is in the engine, tested there.
    pub fn follow(&self, scheme: Scheme) {
        self.scheme.set(scheme);
        self.leaving.borrow_mut().last_scheme = scheme;
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

    /// The settings this launch would leave in the file, or `None` when it
    /// would leave them exactly as it found them.
    ///
    /// The decision, without the write: what has moved is a question about
    /// three values in memory, and a test that asks it should not need a
    /// writer's file on disk to be asked it.
    fn stored(&self) -> Option<Settings> {
        if self.step.get() == self.settings.step && self.theme.get() == self.settings.theme {
            return None;
        }
        let mut settings = self.settings.clone();
        settings.step = self.step.get();
        settings.theme = self.theme.get();
        Some(settings)
    }

    /// Writes `settings.toml` when this launch changed something in it.
    ///
    /// The size and the ground are what can move so far, and only a writer's
    /// launch can move either: the flags a launch of the harness's carries are
    /// this launch's alone and have no business in the writer's file, which is
    /// why a harness launch has already returned before this is reached. A
    /// file that cannot be written is one line on stderr, like every other
    /// file here.
    ///
    /// Both keys are compared against what was read rather than against a
    /// default, so a writer who toggles the ground twice leaves the file
    /// exactly as they found it — `auto` included, which no toggle can reach
    /// and no write should quietly replace.
    fn store_settings(&self) {
        let Some(settings) = self.stored() else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The state a session that ended on `last` left behind.
    ///
    /// Built from its default by hand rather than by struct-update syntax:
    /// both files keep a private `rest` — every key this Quill did not know,
    /// carried through a write — which `..Default::default()` cannot reach
    /// from here.
    fn left_on(last: Scheme) -> State {
        let mut state = State::default();
        state.last_scheme = last;
        state
    }

    /// A writer's launch on `setting`, whose last session ended on `last`.
    ///
    /// Built from its default by hand for the reason [`left_on`] is.
    fn launched(setting: Theme, last: Scheme) -> Rc<Session> {
        following(setting, last, None)
    }

    /// The same launch, on a desktop that answered `desktop`.
    fn following(setting: Theme, last: Scheme, desktop: Option<Scheme>) -> Rc<Session> {
        let mut settings = Settings::default();
        settings.theme = setting;
        let state = left_on(last);
        Session::launch(
            Flags::default(),
            settings,
            state,
            WindowState::default(),
            false,
            desktop,
        )
    }

    /// `auto` is not a ground, so the key that swaps grounds has to start from
    /// the one on screen.
    ///
    /// Three presses rather than one, because one press cannot tell a toggle
    /// that reads the setting from a toggle that reads the ground: both leave
    /// `dark`. The third press is where they part — a toggle reading the
    /// setting would be back at `auto`, or stuck. What a writer means by the
    /// key is "not this ground", so the answer is the opposite of the ground
    /// they opened on, however many times they press it an odd number of
    /// times. `legacy/app/js/theme.js`, `set(resolve() === 'dark' ? …)`.
    #[test]
    fn three_presses_from_auto_leave_the_setting_at_the_other_ground() {
        for opened in [Scheme::Light, Scheme::Dark] {
            let session = launched(Theme::Auto, opened);
            assert_eq!(session.scheme(), opened, "the ground it opened on");
            for _ in 0..3 {
                session.toggle_scheme();
            }
            assert_eq!(
                session.scheme(),
                opened.other(),
                "three presses from {opened:?}"
            );
            let stored = session.stored().expect("three presses changed the file");
            assert_eq!(
                stored.theme,
                opened.other().setting(),
                "the file does not say what is on screen"
            );
        }
    }

    /// Two presses put the ground back and leave `auto` behind.
    ///
    /// The oracle's rule, and it is a choice rather than an oversight:
    /// `theme.js`'s `set()` writes the setting on every press, so a writer who
    /// has pressed the key has picked a ground, and Quill stops following the
    /// desktop even when the ground they land back on is the one the desktop
    /// was already giving them. `auto` is not reachable by the key — it is a
    /// third thing, and the menu row that offers it is the Chrome ticket's.
    #[test]
    fn a_press_is_a_choice_and_stops_following_the_desktop() {
        let session = launched(Theme::Auto, Scheme::Light);
        session.toggle_scheme();
        session.toggle_scheme();
        assert_eq!(session.scheme(), Scheme::Light, "back where it started");
        assert_eq!(
            session.stored().expect("a press is written").theme,
            Theme::Light,
            "the ground the writer chose twice is still `auto` in the file"
        );
    }

    /// A launch that touched nothing writes nothing.
    #[test]
    fn a_launch_that_changed_nothing_leaves_the_file_alone() {
        assert!(launched(Theme::Auto, Scheme::Dark).stored().is_none());
        assert!(launched(Theme::Dark, Scheme::Light).stored().is_none());
    }

    /// The flag pins the ground and never the writer's file.
    ///
    /// `--theme dark` is how `tools/gate judge theme` shoots the dark state on
    /// a desktop that is not dark, so it has to reach the ground rather than
    /// only the setting behind it — which is exactly what it failed to do
    /// before this ticket, and why #89's `markup/dark` came back byte-identical
    /// to its light shot.
    #[test]
    fn the_flag_names_the_ground_the_shot_is_taken_on() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let flags = Flags {
                theme: Some(scheme),
                ..Flags::default()
            };
            // The last session's ground is the other one, and so is the
            // desktop's answer, so nothing but the flag can be what comes back.
            let state = left_on(scheme.other());
            let session = Session::launch(
                flags,
                Settings::default(),
                state,
                WindowState::default(),
                true,
                Some(scheme.other()),
            );
            assert_eq!(session.scheme(), scheme, "--theme {scheme:?}");
        }
    }

    /// The desktop's answer is the first frame, and it is left for the next
    /// launch to open on.
    ///
    /// The second half is the whole of what `last_scheme` is for: a writer on
    /// `auto` whose desktop is dark must not get a white first frame the next
    /// morning while the portal is still being asked, and the only way it can
    /// avoid one is if this launch wrote down the ground the desktop gave it.
    #[test]
    fn a_desktop_that_answers_is_the_ground_and_is_what_is_left_behind() {
        for desktop in [Scheme::Light, Scheme::Dark] {
            let session = following(Theme::Auto, desktop.other(), Some(desktop));
            assert_eq!(session.scheme(), desktop, "the desktop said {desktop:?}");
            assert_eq!(session.leaving.borrow().last_scheme, desktop);
        }
    }

    /// A desktop that says nothing costs the launch nothing: it opens on the
    /// ground it left, which is the no-flash promise on a machine with no
    /// portal on its bus.
    #[test]
    fn a_silent_desktop_leaves_the_launch_on_the_ground_it_left() {
        for last in [Scheme::Light, Scheme::Dark] {
            assert_eq!(following(Theme::Auto, last, None).scheme(), last);
        }
    }

    /// A writer who pinned a ground is not following the desktop, whatever it
    /// answered.
    #[test]
    fn a_pinned_ground_does_not_hear_the_desktop() {
        for setting in [Theme::Light, Theme::Dark] {
            for desktop in [Scheme::Light, Scheme::Dark] {
                let session = following(setting, Scheme::Light, Some(desktop));
                assert_eq!(session.theme(), setting);
                assert_eq!(session.scheme().setting(), setting, "{setting:?}");
            }
        }
    }

    /// Following the desktop moves the ground and what the next launch opens
    /// on, and leaves `auto` where the writer put it.
    ///
    /// The toggle is the comparison: it means "not this ground", so it writes
    /// the setting. A desktop moving is an answer to `auto` rather than a
    /// replacement for it, so the writer still has `auto` tomorrow.
    #[test]
    fn following_the_desktop_moves_the_ground_and_not_the_setting() {
        let session = launched(Theme::Auto, Scheme::Light);
        session.follow(Scheme::Dark);
        assert_eq!(session.scheme(), Scheme::Dark);
        assert_eq!(session.theme(), Theme::Auto, "`auto` is still the setting");
        assert_eq!(session.leaving.borrow().last_scheme, Scheme::Dark);
        assert!(
            session.stored().is_none(),
            "a desktop moving writes nothing to the writer's `settings.toml`"
        );
    }
}
