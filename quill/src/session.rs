//! What Quill was launched with: the flags, the settings and the state.
//!
//! All three belong to the application rather than to a window, and all three
//! are read before the first window is built and written after the last one is
//! gone. Nothing here fails: a file that cannot be read or written is one line
//! on stderr and a Quill that opens anyway.
//!
//! Two files are read more than once: `settings.toml`, and the palette file
//! its `palette` key names. [`watch_settings`] puts
//! [`quill_engine::watch::Watch`] on both and drains it from the main context,
//! so a saved edit reaches [`Session::apply`] and every window without a
//! restart, and a palette a desktop's theme tool rewrote reaches
//! [`Session::ground`] the same way. What a version of either file cannot
//! apply — a line that is not settings, a `[shortcuts]` entry Quill refuses, a
//! palette value that is not a colour — is one `g_warning` under [`DOMAIN`]
//! and never fatal, said once per distinct line per version of the file
//! ([`Session::warn`]).
//!
//! A launch of the harness's ([`Flags::is_harness`]) is the same session with
//! two files' worth of the writer's own removed. It reads their `settings.toml`,
//! because a flag overrides a setting rather than replacing every setting, but
//! it writes nothing to it — unless `--settings` pointed it at a file of its
//! own, which it reads and writes instead ([`Session::settings_path`]); and it
//! neither reads nor writes `state.toml`, so it
//! opens at the shape its flags name rather than at the window a writer left,
//! and the window a writer left is still there after a bench has run.

use std::cell::{Cell, Ref, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use quill_engine::focus::Focus;
use quill_engine::focus::typewriter::Typewriter;
use quill_engine::library::Library;
use quill_engine::settings::{Chrome, Face, FocusScope, Settings, State, Theme, WindowState};
use quill_engine::shortcuts::Refusal;
use quill_engine::theme::{self, Palette, Scheme};
use quill_engine::watch::{Placed, Watch, unsaid};

use crate::flags::Flags;
use crate::ground::Ground;

/// The log domain every warning about the settings file carries, which is what
/// `journalctl --user` and `G_MESSAGES_DEBUG` filter on.
pub const DOMAIN: &str = "quill-settings";

/// How often the main context looks for a save the watch has passed on.
///
/// The watch answers on a thread of its own and a setting can only be applied
/// on this one, so the receiver is drained from a timer. Together with
/// [`quill_engine::watch::DEBOUNCE`] this is what stands between a writer's
/// save and the page following it.
const DRAIN_EVERY: Duration = Duration::from_millis(100);

/// The settings and state of one run of Quill.
pub struct Session {
    /// What this launch was asked for.
    flags: Flags,
    /// Whether this launch is the harness's, asked once of the flags and
    /// answered from here afterwards.
    harness: bool,
    /// What the writer chose, with the flags over the top, as it was last
    /// read: replaced when a saved edit is applied ([`Session::apply`]), so
    /// that what [`Session::store_settings`] compares the live values against
    /// is the file as it is now and not as it was at launch.
    settings: RefCell<Settings>,
    /// What the last read of the settings file had to say, kept because the
    /// chords are installed after the file is read and one version of the
    /// file says its notes and its refusals in one breath ([`Session::warn`]).
    notes: RefCell<Vec<String>>,
    /// What the last read refused, for the Settings window to show (#125).
    refusals: RefCell<Vec<Refusal>>,
    /// Every line the last version of the settings file said, so that a
    /// writer who saves the same refused line again hears nothing
    /// ([`quill_engine::watch::unsaid`]).
    said: RefCell<BTreeSet<String>>,
    /// The file the settings were read from and are written back to: the
    /// writer's own, or the one `--settings` named
    /// ([`Session::settings_path`]).
    settings_path: PathBuf,
    /// The palette laid over the built-in grounds: what the file `palette`
    /// names said when it was last read, or nothing where it names none.
    /// Read again when the file is saved, when its directory is replaced and
    /// when the setting moves ([`Session::reread_palette`]), and compared
    /// before a repaint, so that a theme tool writing the same bytes costs
    /// nothing.
    palette: RefCell<Palette>,
    /// What the last read of the palette file had to say — one line per value
    /// that is not a colour — said once under [`DOMAIN`] alongside the
    /// settings file's own lines ([`Session::say`]).
    palette_notes: RefCell<Vec<String>>,
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
    /// Whether Focus is on now: the setting until the writer presses the key,
    /// and then what they pressed it to. Held apart from [`Session::settings`]
    /// for the reason [`Session::step`] is.
    focus: Cell<bool>,
    /// The scope Focus is at, which outlives Focus being switched off because
    /// it is what `Ctrl+D` restores (ADR 0006). Held live for the same reason
    /// [`Session::focus`] is.
    focus_scope: Cell<FocusScope>,
    /// Whether Typewriter is on now. Nothing scrolls to it yet — #115 is what
    /// makes it move — so this launch only remembers it.
    typewriter: Cell<bool>,
    /// The face the page is set in now: the setting until the writer picks
    /// one from View › Typeface, and then the one they picked. Held live for
    /// the reason [`Session::step`] is.
    face: Cell<Face>,
    /// The ground the desktop last answered, or `None` where it has not
    /// answered: what `theme.auto` returns the page to when a writer who had
    /// pinned a ground asks to follow the desktop again.
    desktop: Cell<Option<Scheme>>,
    /// Whether the two bars are there now: the setting until the writer
    /// presses `Ctrl+Shift+H`, and then what they pressed it to. Held live for
    /// the reason [`Session::focus`] is.
    chrome: Cell<Chrome>,
    /// Whether the stats bar is shown while the bars are: `chrome.stats`.
    /// Live only — no settings key holds it until the Stats spec (#30)
    /// decides what the bar remembers — so every launch shows it.
    stats: Cell<bool>,
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
    /// The Library: the Locations the settings named, walked at launch. It
    /// belongs to the application and every window reads this one
    /// (`docs/architecture.md` § Library).
    library: RefCell<Library>,
    /// The one `notify` instance, put here by [`watch_settings`] once GTK is
    /// up: the settings file, the palette file it names, every Location's tree
    /// and every open Document's file are subjects of it, which is the
    /// architecture's "the Documents and the Library join the same watch".
    /// `None` until then, and where the watch could not be made at all.
    watch: RefCell<Option<Watch>>,
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
        // A launch pointed at a settings file of its own owns that file: the
        // rule that a launch of the harness's writes nothing is about the
        // writer's own (`docs/architecture.md` § Command-line flags), which is
        // the file a missing `--settings` leaves this reading and writing.
        let path = settings_path(&flags);
        let (settings, notes) = if harness && flags.settings.is_none() {
            Settings::read_from(&path)
        } else {
            Settings::open_at(&path)
        };
        report(&path, &notes);

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

        let session = Self::launch(flags, settings, state, opening, harness, portal);
        if let Some(palette) = session.palette_path() {
            report(&palette, &session.palette_notes.borrow());
        }
        session.take_down(notes);
        session
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
        // The palette, read after the flags have gone over the setting: a
        // `--theme` with no `--palette` has emptied the path by then, which is
        // how the pin keeps the Gate's shots to the built-ins.
        let (palette, palette_notes) = read_palette(settings.palette.as_deref());
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
        // The Locations are walked here, once, before the first window: the
        // Library is in memory and nothing persists an index of it (ADR 0002),
        // so launch is the only place the trees can come from. A launch of the
        // harness's walks none of the writer's: a judged shot and a bench are
        // the same launch on every machine, and their folders are neither
        // theirs to read nor a cost the cold-start budget agreed to. The one
        // Library the harness does walk is the fixture `--library` named,
        // whose copy this launch stamped and whose rows are the judged shot
        // ([`crate::files::stage`]).
        let library = if harness && flags.library.is_none() {
            Library::new()
        } else {
            Library::open(&settings.library.locations, &settings.library.pinned)
        };
        Rc::new(Self {
            opening: flags.shape(opening),
            step: Cell::new(settings.step),
            theme: Cell::new(settings.theme),
            focus: Cell::new(settings.focus),
            focus_scope: Cell::new(settings.focus_scope),
            typewriter: Cell::new(settings.typewriter),
            face: Cell::new(settings.face),
            desktop: Cell::new(portal),
            chrome: Cell::new(settings.chrome),
            stats: Cell::new(true),
            scheme: Cell::new(scheme),
            settings: RefCell::new(settings),
            notes: RefCell::new(Vec::new()),
            refusals: RefCell::new(Vec::new()),
            said: RefCell::new(BTreeSet::new()),
            settings_path: settings_path(&flags),
            palette: RefCell::new(palette),
            palette_notes: RefCell::new(palette_notes),
            flags,
            harness,
            leaving: RefCell::new(state),
            library: RefCell::new(library),
            watch: RefCell::new(None),
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
    pub fn settings(&self) -> Ref<'_, Settings> {
        self.settings.borrow()
    }

    /// What the last read of the settings file refused, one per entry it
    /// could not apply, in the file's order.
    pub fn refusals(&self) -> Ref<'_, Vec<Refusal>> {
        self.refusals.borrow()
    }

    /// Everything the last read of the settings file could not apply, one
    /// line each: what the file as a whole had to say — not TOML, a key with
    /// no such value — and then the `[shortcuts]` entries it refused. The same
    /// lines the app warns under `quill-settings`, for the Settings window,
    /// which is where a writer with no terminal reads them.
    pub fn unapplied(&self) -> Vec<String> {
        lines(&self.notes.borrow(), &self.refusals.borrow())
    }

    /// Puts a settings file read while Quill is running on to this launch.
    ///
    /// Everything the file carries, not only what moved: a file is written
    /// whole and read whole, and a value the writer left alone costs one Cell
    /// being set to what it already held. The flags go over the top again,
    /// because a launch of the harness's runs what its command line says for
    /// as long as it runs (`docs/architecture.md` § Command-line flags) — so a
    /// fixture edited under one moves everything the flags do not name, which
    /// is what lets a `--settings` test drive its own file.
    ///
    /// The ground is resolved rather than read ([`Session::set_theme`]),
    /// because a file that says `auto` is asking the desktop, which is the
    /// same question a launch asks itself.
    ///
    /// The palette file is read again too, because the path that names it is
    /// one of the settings: a moved `palette` line is a new file, and a line
    /// left alone may name a file that has appeared since, which is the retry
    /// the watch leaves to this save ([`watch_settings`]).
    ///
    /// Answers with whether anything moved — the file as this launch is
    /// running it ([`Session::running`]), before against after, and the
    /// palette against what it was — so that the
    /// commonest save of all costs no repaint: the one Quill made itself,
    /// writing a key press back to the file ([`Session::store_settings`]),
    /// carries the values the live half is already holding.
    pub fn apply(&self, settings: Settings) -> bool {
        let before = self.running();
        let settings = self.flags.over(settings);
        self.step.set(settings.step);
        self.focus.set(settings.focus);
        self.focus_scope.set(settings.focus_scope);
        self.typewriter.set(settings.typewriter);
        self.face.set(settings.face);
        self.chrome.set(settings.chrome);
        let theme = settings.theme;
        self.settings.replace(settings);
        self.set_theme(theme);
        let moved = self.running() != before;
        let repainted = self.reread_palette();
        moved || repainted
    }

    /// Takes down what the read of the settings file at launch said, which
    /// [`report`] has already put on stderr.
    ///
    /// Kept rather than answered here, because the `[shortcuts]` entries this
    /// version of the file refuses are only known once the chords have been
    /// installed, and the two are one version's worth of complaint
    /// ([`Session::warn`]). Marked as said in the same breath, so that the
    /// warnings the first save makes are about the save.
    fn take_down(&self, notes: Vec<String>) {
        self.notes.replace(notes);
        self.said
            .replace(self.worth_saying(&[]).into_iter().collect());
    }

    /// Reads the settings file again, taking down what it had to say.
    ///
    /// `None` where there is nothing to read — a file that is not TOML at all,
    /// one that cannot be read, one that is gone — because the settings this
    /// launch is running on are the last good ones and stay
    /// ([`Settings::reread`]). The writer hears what happened either way, once
    /// ([`Session::warn`], and [`Session::warn_unread`] where there was
    /// nothing to read).
    fn read_again(&self) -> Option<Settings> {
        let (settings, notes) = Settings::reread(&self.settings_path);
        self.notes.replace(notes);
        settings
    }

    /// Warns about what the settings file cannot apply — a line per note and a
    /// line per refusal — and takes the refusals down for the Settings window.
    ///
    /// Once per distinct line per version of the file
    /// ([`quill_engine::watch::unsaid`]): a writer who saves the same refused
    /// line again hears nothing. The lines the read at launch made are said on
    /// stderr as every file's are ([`report`]) and marked said here, so the
    /// first save does not repeat them.
    pub(crate) fn warn(&self, refusals: Vec<Refusal>) {
        self.say(&refusals);
        self.refusals.replace(refusals);
    }

    /// Warns about a read that came to nothing, and leaves the refusals where
    /// the last read that did apply left them.
    ///
    /// A file that is not TOML at all, one that cannot be read, one that is
    /// gone: none of them applied a `[shortcuts]` entry, so none of them
    /// refused one either. What the Settings window is showing is what this
    /// launch is running on ([`Session::refusals`]), which is the same reason
    /// the settings themselves stay. The line about the file is said the way
    /// every other line is, once per version.
    pub(crate) fn warn_unread(&self) {
        self.say(&[]);
    }

    /// Warns about what the palette file cannot apply, once per distinct line
    /// per version of it, the way [`Session::warn`] does for the settings
    /// file. The settings file's own lines are handed back in so they stay
    /// said: the rule is one set for the two files.
    pub(crate) fn warn_palette(&self) {
        self.say(&self.refusals.borrow());
    }

    /// Says every line this version of the two files is worth that has not
    /// been said already, and takes down that it has been.
    fn say(&self, refusals: &[Refusal]) {
        let lines = self.worth_saying(refusals);
        let unsaid = unsaid(&self.said.borrow(), &lines);
        self.said.replace(unsaid.said);
        for line in unsaid.lines {
            glib::g_warning!(DOMAIN, "{line}");
        }
    }

    /// Every line the two files are worth, each naming its file: what the
    /// settings file said and refused ([`lines`]), then what the palette file
    /// said. One list rather than two, because the say-once rule is one set
    /// ([`quill_engine::watch::unsaid`] replaces it whole), and a palette said
    /// on its own would drop the settings file's lines from what has been said.
    fn worth_saying(&self, refusals: &[Refusal]) -> Vec<String> {
        let settings = self.settings_path.display().to_string();
        let palette = self
            .palette_path()
            .map_or_else(String::new, |path| path.display().to_string());
        lines(&self.notes.borrow(), refusals)
            .into_iter()
            .map(|line| format!("{settings}: {line}"))
            .chain(
                self.palette_notes
                    .borrow()
                    .iter()
                    .map(|note| format!("{palette}: {note}")),
            )
            .collect()
    }

    /// The palette file this launch reads, or `None` where the settings name
    /// none — the flags already over the top, so a `--theme` launch without
    /// `--palette` answers `None` whatever the writer's file says
    /// ([`Flags::over`]).
    pub fn palette_path(&self) -> Option<PathBuf> {
        self.settings.borrow().palette.clone()
    }

    /// Reads the palette file again and answers with whether the palette
    /// moved.
    ///
    /// The same bytes are not a repaint, and that is [`Palette`]'s equality
    /// and not the watch's: the watch tells a save by its length and write
    /// time, and a theme tool writes the same file whole. A file that is gone
    /// is the empty palette, so an `rm` is a move back to the built-ins.
    fn reread_palette(&self) -> bool {
        let (palette, notes) = read_palette(self.palette_path().as_deref());
        self.palette_notes.replace(notes);
        let moved = *self.palette.borrow() != palette;
        self.palette.replace(palette);
        moved
    }

    /// The step of the type ladder this launch is reading at.
    pub fn step(&self) -> u32 {
        self.step.get()
    }

    /// Steps the type size, for this launch and — for a writer's — the next.
    pub fn set_step(&self, step: u32) {
        self.step.set(step);
    }

    /// How much Focus leaves lit now, as the one value the Editor draws from.
    ///
    /// The live pair rather than `settings().focus`, for the reason
    /// [`Session::theme`] is live: the keys move it, and a window opened after
    /// one was pressed opens the way the writer is reading rather than the way
    /// they started.
    pub fn focus(&self) -> Focus {
        Focus::at(self.focus.get(), self.focus_scope.get())
    }

    /// Switches Focus off, or back on at the scope it left. ADR 0006's
    /// `Ctrl+D`.
    ///
    /// The scope is not touched, which is the whole of "back on at its last
    /// scope": it is remembered while Focus is off, and only
    /// [`Session::swap_focus_scope`] moves it. So a writer who works in
    /// Paragraph scope and switches Focus off and on twice is still in
    /// Paragraph scope.
    pub fn toggle_focus(&self) -> Focus {
        self.focus.set(!self.focus.get());
        self.focus()
    }

    /// Swaps Sentence and Paragraph, switching Focus on if it was off. ADR
    /// 0006's `Ctrl+Shift+D`.
    ///
    /// Both halves always happen, so the key means the same thing from either
    /// state: from off it is "show me the other scope", which is a scope the
    /// writer can only be shown with Focus on. Picking a scope switching Focus
    /// on is ADR 0006's rule for the menu's radios too.
    pub fn swap_focus_scope(&self) -> Focus {
        self.focus_scope.set(match self.focus_scope.get() {
            FocusScope::Sentence => FocusScope::Paragraph,
            FocusScope::Paragraph => FocusScope::Sentence,
        });
        self.focus.set(true);
        self.focus()
    }

    /// Puts Focus on at `scope`: View › Focus's Sentence and Paragraph radios.
    ///
    /// Picking a scope switches Focus on, which is ADR 0006's rule for the
    /// menu's radios as it is for the key: a scope is a thing the writer can
    /// only be shown with Focus on, and a tick beside Paragraph on a page that
    /// is not dimmed would be a tick beside nothing.
    pub fn set_focus_scope(&self, scope: FocusScope) -> Focus {
        self.focus_scope.set(scope);
        self.focus.set(true);
        self.focus()
    }

    /// The face the page is set in now.
    ///
    /// The live value rather than `settings().face`, because View › Typeface
    /// moves it and a window opened after a pick opens in the face the writer
    /// is reading.
    pub fn face(&self) -> Face {
        self.face.get()
    }

    /// Sets the face, for this launch and — for a writer's — the next.
    pub fn set_face(&self, face: Face) {
        self.face.set(face);
    }

    /// Whether Typewriter is on now, and where it holds the caret's row.
    ///
    /// The live value paired with the anchor the settings file holds, for the
    /// reason [`Session::focus`] is live: the key moves it, and a window
    /// opened after one was pressed opens the way the writer is writing.
    pub fn typewriter(&self) -> Typewriter {
        Typewriter::at(self.typewriter.get(), self.settings().typewriter_anchor)
    }

    /// Turns Typewriter on or off. ADR 0006's `Ctrl+T`.
    pub fn toggle_typewriter(&self) -> Typewriter {
        self.typewriter.set(!self.typewriter.get());
        self.typewriter()
    }

    /// Whether the two bars are shown now.
    ///
    /// The live value rather than `settings().chrome`, because `Ctrl+Shift+H`
    /// moves it and a window opened after the key was pressed opens the way
    /// the writer is writing.
    pub fn chrome(&self) -> Chrome {
        self.chrome.get()
    }

    /// Hides the bars, or shows them again. `docs/shortcuts.md`'s
    /// `chrome.toggle` row.
    pub fn toggle_chrome(&self) -> Chrome {
        let chrome = match self.chrome.get() {
            Chrome::Shown => Chrome::Hidden,
            Chrome::Hidden => Chrome::Shown,
        };
        self.chrome.set(chrome);
        chrome
    }

    /// Whether the stats bar is shown now, while the bars are.
    pub fn stats(&self) -> bool {
        self.stats.get()
    }

    /// Hides the stats bar, or shows it again: `docs/shortcuts.md`'s
    /// `chrome.stats` row, the View menu's Statistics check and the Stats
    /// menu's Hide Statistics.
    pub fn toggle_stats(&self) -> bool {
        self.stats.set(!self.stats.get());
        self.stats.get()
    }

    /// The ground this launch is painting on.
    pub fn scheme(&self) -> Scheme {
        self.scheme.get()
    }

    /// The ground this launch is painting on, with the table it is painted
    /// from: what every painter reads a colour off.
    ///
    /// The one place the table is chosen. Every pass that puts a ground on to
    /// the windows ([`crate::window::repaint`], [`crate::window::reapply`], a
    /// window being built) asks this once and hands the answer down, so a
    /// palette laid over the built-ins is a change to this answer and to
    /// nothing downstream of it: the file `palette` names, over the built-ins
    /// for the ground on screen ([`Ground::overlaid`]), and with no file the
    /// built-ins themselves.
    pub fn ground(&self) -> Ground {
        Ground::overlaid(self.scheme.get(), &self.palette.borrow())
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

    /// Takes down what the desktop answered, whether or not it is being
    /// followed, so that `theme.auto` has an answer to return to.
    ///
    /// Apart from [`Session::follow`] because a writer on a pinned ground
    /// hears nothing when the desktop moves — [`theme::followed`] says so —
    /// and still means "whatever the desktop is on now" when they pick
    /// Follow System later.
    pub fn desktop_moved(&self, desktop: Option<Scheme>) {
        self.desktop.set(desktop);
    }

    /// Sets the theme to `theme`: the Palette's Light Theme, Dark Theme and
    /// Follow System.
    ///
    /// The two grounds pin themselves, as the toggle does. `auto` is the
    /// question, and its answer is the ground the desktop last gave — or,
    /// where the desktop never answered, the ground on screen, which is what
    /// [`theme::effective`] answers for a launch on `auto` with no portal.
    pub fn set_theme(&self, theme: Theme) -> Scheme {
        let scheme = match theme {
            Theme::Light => Scheme::Light,
            Theme::Dark => Scheme::Dark,
            Theme::Auto => self.desktop.get().unwrap_or(self.scheme.get()),
        };
        self.theme.set(theme);
        self.scheme.set(scheme);
        self.leaving.borrow_mut().last_scheme = scheme;
        scheme
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
    /// The decision, without the write: what has moved is a question about the
    /// values held live beside [`Session::settings`], and a test that asks it
    /// should not need a writer's file on disk to be asked it.
    ///
    /// The whole of what was read is compared against the whole of what would
    /// be written, rather than the live values one by one, so that a value
    /// given a live twin later is compared without this function being
    /// remembered: the keys a writer can press are the only things that can
    /// differ, and every one of them has been laid over the copy by then.
    fn stored(&self) -> Option<Settings> {
        let settings = self.running();
        if settings == *self.settings.borrow() {
            return None;
        }
        Some(settings)
    }

    /// The settings this launch is running now: what was read, with every
    /// value a key can move laid over it.
    ///
    /// What the windows are showing, in one value, which is what makes
    /// [`Session::apply`] able to say whether a saved edit moved anything they
    /// would have to be told about.
    fn running(&self) -> Settings {
        let mut settings = self.settings.borrow().clone();
        settings.step = self.step.get();
        settings.theme = self.theme.get();
        settings.focus = self.focus.get();
        settings.focus_scope = self.focus_scope.get();
        settings.typewriter = self.typewriter.get();
        settings.face = self.face.get();
        settings.chrome = self.chrome.get();
        settings
    }

    /// Writes `settings.toml` when this launch changed something in it.
    ///
    /// The size, the ground, Focus, its scope, Typewriter, the face and the bars
    /// are what can move so far, and only a writer's launch can move any of them: the flags a
    /// launch of the harness's carries are this launch's alone and have no
    /// business in the writer's file, which is why a harness launch has already
    /// returned before this is reached and why `--focus` and `--typewriter`
    /// leave nothing behind (`docs/architecture.md` § Command-line flags). A
    /// file that cannot be written is one line on stderr, like every other file
    /// here.
    ///
    /// Called both as a key is pressed and once on the way out, and safe to
    /// call either way round: a launch that has already written what it changed
    /// finds nothing left to write, and a launch killed between the two has
    /// left the writer's choice in the file rather than in a process that is
    /// gone. The harness is turned away here rather than by the caller, because
    /// there is now more than one caller and only one of them is shutdown.
    ///
    /// Every key is compared against what was read rather than against a
    /// default, so a writer who toggles the ground twice leaves the file
    /// exactly as they found it — `auto` included, which no toggle can reach
    /// and no write should quietly replace, and the Typewriter anchor with it,
    /// which nothing but the file itself can move.
    pub fn store_settings(&self) {
        let Some(settings) = self.stored() else {
            return;
        };
        self.write_settings(&settings);
    }

    /// Puts one setting the Settings window moved into the file, and leaves
    /// applying it to the watch (#125).
    ///
    /// The window never sets a live value itself: a row moved here and the
    /// same key edited in a text editor are one path from then on — the file
    /// is saved, the watch reads it back a moment later, and
    /// [`Session::apply`] puts the whole of it on to this launch. That is what
    /// keeps the Typewriter anchor, which no key can move and nothing holds
    /// live, in one place rather than two.
    ///
    /// `edit` is handed the settings this launch is running
    /// ([`Session::running`]) rather than the ones it read, so that a row
    /// written after a key was pressed carries what the key did with it.
    pub fn edit_settings(&self, edit: impl FnOnce(&mut Settings)) {
        let mut settings = self.running();
        edit(&mut settings);
        self.write_settings(&settings);
    }

    /// Writes `settings` to this launch's settings file, saying so on stderr
    /// where it cannot be written, as every file here does.
    ///
    /// The one place the writer's own file is defended: the flags a launch of
    /// the harness's carries are this launch's alone and have no business in
    /// it. A launch given `--settings` is reading and writing a file of its
    /// own, and that one it may write.
    fn write_settings(&self, settings: &Settings) {
        if self.harness && self.settings_path == Settings::path() {
            return;
        }
        if let Err(err) = settings.write_to(&self.settings_path) {
            eprintln!(
                "quill: {}: cannot be written ({err})",
                self.settings_path.display()
            );
        }
    }

    /// The settings file this launch reads and writes.
    #[must_use]
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    /// The Library, which belongs to the application and is the same one in
    /// every window (`docs/architecture.md` § Library).
    pub fn library(&self) -> Ref<'_, Library> {
        self.library.borrow()
    }

    /// The Location an untitled Document's first save goes into, or `None`
    /// where the writer has pointed Quill at no folder yet.
    pub fn first_location(&self) -> Option<PathBuf> {
        self.library
            .borrow()
            .locations()
            .first()
            .map(|location| location.root().to_path_buf())
    }

    /// Whether the writer asked to be asked where every first save goes
    /// (`library.ask_where_to_save`).
    pub fn always_asks(&self) -> bool {
        self.settings.borrow().library.ask_where_to_save
    }

    /// Takes in that `path` was opened: it becomes the newest of the recents,
    /// and with an empty Library its folder becomes the first Location
    /// ([`crate::files::location_for`]).
    ///
    /// A launch of the harness's takes in nothing: it leaves no state behind
    /// and points the writer's Library at nothing, so `ref/sample.md` never
    /// makes `ref/` a Location of theirs.
    pub fn opened_at(&self, path: &Path) {
        if self.harness {
            return;
        }
        self.leaving.borrow_mut().visited(path);
        let root = crate::files::location_for(&self.library.borrow(), path);
        if let Some(root) = root {
            self.add_location(&root);
        }
    }

    /// Adds `root` as a Location: walked now, watched from now on, and written
    /// to the settings file, which is the only place Locations are remembered.
    ///
    /// A folder the Library already holds, or one that is not a folder, is
    /// refused by the model and nothing is written.
    pub fn add_location(&self, root: &Path) {
        if !self.library.borrow_mut().add_location(root) {
            return;
        }
        self.watch_tree(root);
        // The list this launch is running is moved as well as the file, and
        // not left to the watch to bring back: two folders added in one breath
        // would otherwise be written from a list that had heard about neither,
        // and the second write would drop the first.
        self.settings
            .borrow_mut()
            .library
            .locations
            .push(root.to_path_buf());
        let settings = self.running();
        self.write_settings(&settings);
    }

    /// Puts every Location the settings named on to the watch.
    ///
    /// Called once, when the watch is made: the trees were walked at launch,
    /// before there was a watch to add them to.
    fn watch_locations(&self) {
        let roots: Vec<PathBuf> = self
            .library
            .borrow()
            .locations()
            .iter()
            .map(|location| location.root().to_path_buf())
            .collect();
        for root in roots {
            self.watch_tree(&root);
        }
    }

    /// Listens under `root` for everything that happens in it.
    fn watch_tree(&self, root: &Path) {
        let mut watch = self.watch.borrow_mut();
        let Some(watch) = watch.as_mut() else {
            return;
        };
        if let Err(err) = watch.add_tree(root) {
            eprintln!("quill: {}: cannot be watched ({err})", root.display());
        }
    }

    /// Listens for `path`, the file an open Document is.
    ///
    /// A Document under a Location is already listened for as part of its
    /// tree, and asking for it again costs nothing — the watch holds its
    /// subjects as a set. A Document outside every Location is why this is
    /// asked at all: the writer still sees another editor's save of it.
    ///
    /// Nothing is taken off again when the Document is closed: the subject is
    /// one directory, an event for a file no window holds is a look at a path
    /// nothing answers, and a watch dropped while a second window still shows
    /// the same file would be the bug worth avoiding.
    pub fn watch_document(&self, path: &Path) {
        let mut watch = self.watch.borrow_mut();
        let Some(watch) = watch.as_mut() else {
            return;
        };
        if let Err(err) = watch.add(path) {
            eprintln!("quill: {}: cannot be watched ({err})", path.display());
        }
    }

    /// Puts what happened at `path` on to the Library's tree.
    ///
    /// Answers whether a row moved, which is the sidebar's signal to draw
    /// again (#246).
    pub fn patch_library(&self, path: &Path) -> bool {
        self.library.borrow_mut().patch(path)
    }

    /// Drops `root` as a Location: out of the model and out of the settings
    /// file, which is the only place Locations are remembered.
    ///
    /// Nothing on disk is touched — a Location is a folder Quill was pointed
    /// at, and forgetting it is not deleting it — and the watch keeps the
    /// subject: a path arriving from a folder no Location holds patches
    /// nothing ([`quill_engine::library::Library::patch`]), and a folder added
    /// back is a folder already listened for.
    pub fn remove_location(&self, root: &Path) {
        if !self.library.borrow_mut().remove_location(root) {
            return;
        }
        self.settings
            .borrow_mut()
            .library
            .locations
            .retain(|held| held != root);
        let settings = self.running();
        self.write_settings(&settings);
    }

    /// Pins `path`, and says whether it took: the model first, then
    /// `[library].pinned` in the settings file, which is where Pinned is
    /// remembered.
    pub fn pin(&self, path: &Path) -> bool {
        if !matches!(
            self.library.borrow_mut().pin(path),
            quill_engine::library::Pin::Held
        ) {
            return false;
        }
        let mut settings = self.settings.borrow_mut();
        if !settings.library.pinned.iter().any(|held| held == path) {
            settings.library.pinned.push(path.to_path_buf());
        }
        drop(settings);
        let settings = self.running();
        self.write_settings(&settings);
        true
    }

    /// Unpins `path`, and says whether it was pinned.
    pub fn unpin(&self, path: &Path) -> bool {
        if !self.library.borrow_mut().unpin(path) {
            return false;
        }
        self.settings
            .borrow_mut()
            .library
            .pinned
            .retain(|held| held != path);
        let settings = self.running();
        self.write_settings(&settings);
        true
    }

    /// Renames the file at `path` to what a writer typed
    /// ([`quill_engine::library::Library::rename`]).
    ///
    /// # Errors
    ///
    /// What the rename could not do.
    pub fn rename_file(&self, path: &Path, typed: &str) -> std::io::Result<PathBuf> {
        self.library.borrow_mut().rename(path, typed)
    }

    /// Copies the file at `path` beside itself
    /// ([`quill_engine::library::Library::duplicate`]).
    ///
    /// # Errors
    ///
    /// What the copy could not do.
    pub fn duplicate_file(&self, path: &Path) -> std::io::Result<PathBuf> {
        self.library.borrow_mut().duplicate(path)
    }

    /// Takes in that the app has put `path` in the system trash
    /// ([`quill_engine::library::Library::trashed`]).
    pub fn trashed_file(&self, path: &Path) {
        self.library.borrow_mut().trashed(path);
    }
}

/// Where a launch reads and writes its settings: the file `--settings` names,
/// and the writer's own where it names none.
fn settings_path(flags: &Flags) -> PathBuf {
    flags.settings.clone().unwrap_or_else(Settings::path)
}

/// The palette the file at `path` holds, with what it had to say; the empty
/// palette and nothing to say where no file is named.
fn read_palette(path: Option<&Path>) -> (Palette, Vec<String>) {
    match path {
        Some(path) => Palette::read_from(path),
        None => (Palette::default(), Vec::new()),
    }
}

/// Says what a file was worth saying, one line each, naming the file.
fn report(path: &Path, notes: &[String]) {
    for note in notes {
        eprintln!("quill: {}: {note}", path.display());
    }
}

/// What a read of the settings file has to say: what could not be read, then
/// one line per `[shortcuts]` entry that could not be applied, each quoting
/// the entry as the writer wrote it.
fn lines(notes: &[String], refusals: &[Refusal]) -> Vec<String> {
    notes
        .iter()
        .cloned()
        .chain(refusals.iter().map(Refusal::to_string))
        .collect()
}

/// Watches the settings file and the palette file it names, so that a saved
/// edit of either applies without a restart.
///
/// Every launch watches, a launch of the harness's included, so that a judged
/// run or a test can drive the fixture `--settings` named; the flags still
/// override what the file says for as long as the launch runs
/// ([`Session::apply`]). The palette file is the watch's second subject, on
/// the same receiver, and follows the setting ([`follow_palette`]).
///
/// The saves are drained from a timer on the main context rather than answered
/// on the watch's own thread, because that is the only thread a window may be
/// touched from. A file that cannot be watched is one line on stderr and a
/// Quill that runs on what it read at launch.
pub fn watch_settings(app: &gtk::Application, session: &Rc<Session>) {
    let (watch, saves) = match Watch::on(session.settings_path()) {
        Ok(watching) => watching,
        Err(err) => {
            eprintln!(
                "quill: {}: cannot be watched ({err})",
                session.settings_path().display()
            );
            return;
        }
    };
    session.watch.replace(Some(watch));
    // The Locations were walked at launch, before there was a watch to put
    // them on; the Documents add themselves as their windows show them.
    session.watch_locations();
    let mut following = None;
    follow_palette(&mut following, session);
    let app = app.clone();
    let session = Rc::clone(session);
    glib::timeout_add_local(DRAIN_EVERY, move || {
        // The watch is held here and nowhere else, so it stops when the
        // application does. Every save waiting is one re-read: a file is read
        // whole, so reading it twice would apply the same file twice. The
        // settings file's save is the larger of the two — applying it reads
        // the palette again and may have moved where the palette is — so it
        // is the one taken when both are waiting.
        let mut settings_saved = false;
        let mut palette_saved = false;
        // Everything else is a Location's tree or an open Document's file, and
        // each of those is answered per path: the Library re-stats the one row
        // it names, and the window showing it asks what happened to it.
        let mut touched: BTreeSet<PathBuf> = BTreeSet::new();
        for saved in saves.try_iter() {
            if saved == *session.settings_path() {
                settings_saved = true;
            } else if following
                .as_ref()
                .is_some_and(|palette: &Following| palette.path == saved)
            {
                palette_saved = true;
            } else {
                touched.insert(saved);
            }
        }
        if settings_saved {
            reread(Some(&app), &session);
            follow_palette(&mut following, &session);
        } else if palette_saved {
            repaint_palette(Some(&app), &session);
        }
        let mut patched = false;
        for path in touched {
            patched |= session.patch_library(&path);
            crate::window::noticed(&app, &path);
        }
        // One pass over the windows for however many rows moved, rather than
        // a redraw per path: a folder saved into ten times in one drain is one
        // Library and one list.
        if patched {
            crate::window::relist(&app);
        }
        glib::ControlFlow::Continue
    });
}

/// Points the watch's second subject at the palette file the session names
/// now, so that the watch follows the setting: re-pointed when `palette`
/// moves, dropped when it is unset, and left alone when it is where it was.
///
/// `following` is the file the watch was last asked for and whether it is
/// being listened for yet. A file whose directory is not there is asked for
/// again on the settings file's next save rather than polled for, which is
/// what [`Placed::NoDirectory`] leaves to the caller; a directory that cannot
/// be watched at all is one line on stderr and the same retry.
fn follow_palette(following: &mut Option<Following>, session: &Session) {
    let mut held = session.watch.borrow_mut();
    let Some(watch) = held.as_mut() else {
        return;
    };
    let wanted = session.palette_path();
    if let Some(Following { path, placed }) = following {
        if Some(&*path) == wanted.as_ref() {
            if *placed == Placed::Listening {
                return;
            }
        } else {
            watch.remove(path);
        }
    }
    *following = wanted.map(|path| {
        let placed = watch.add(&path).unwrap_or_else(|err| {
            eprintln!("quill: {}: cannot be watched ({err})", path.display());
            Placed::NoDirectory
        });
        Following { path, placed }
    });
}

/// The palette file the watch was last pointed at, and how that went.
struct Following {
    /// The file, as the setting named it and the watch was asked for it.
    path: PathBuf,
    /// Whether its directory was there to be listened to.
    placed: Placed,
}

/// Reads the settings file again and puts what it says on to this launch: the
/// values, the chords every Command is installed with, and every window.
///
/// `app` is `None` where there is no application to install a chord on or a
/// window to tell — a test with no display, which is how a save can be driven
/// through this without one. The entries a `[shortcuts]` table refuses are the
/// engine's either way; the ones only GTK can find are
/// [`crate::chrome::install_chords`]'.
fn reread(app: Option<&gtk::Application>, session: &Rc<Session>) {
    let Some(settings) = session.read_again() else {
        // Nothing to read: a file that is not TOML at all, one that cannot be
        // read, or one that is gone. The last good settings are the ones Quill
        // is running on and they stay, and so does what the last good read
        // refused; the writer hears what happened once.
        session.warn_unread();
        return;
    };
    let moved = session.apply(settings);
    // The chords are installed either way, because they are cheap and the map
    // is the file's every time; the windows are told only when something they
    // are showing moved.
    let refusals = match app {
        Some(app) => crate::chrome::install_chords(app, session),
        None => session.settings().shortcuts().refusals,
    };
    session.warn(refusals);
    if let Some(app) = app.filter(|_| moved) {
        crate::window::reapply(app, session);
    }
}

/// Reads the palette file again and puts it on to every window where it
/// moved — a theme tool's rewrite, a hand's edit, an `rm`, a directory
/// replaced — through the pass a theme toggle uses, so a palette change and a
/// scheme change are the same one repaint. What the file cannot apply is said
/// once; the same bytes are not a repaint ([`Session::reread_palette`]).
///
/// `app` is `None` where there is no window to tell — a test with no display.
fn repaint_palette(app: Option<&gtk::Application>, session: &Session) {
    let moved = session.reread_palette();
    session.warn_palette();
    if let Some(app) = app.filter(|_| moved) {
        crate::window::repaint(app, session);
    }
}

#[cfg(test)]
mod tests {
    use quill_engine::theme::{Colour, Role};

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

    /// `Ctrl+Shift+H` flips the `chrome` setting, and the file follows it:
    /// hidden after one press, back to what was read after two.
    #[test]
    fn the_bars_key_flips_the_setting_and_the_file_follows() {
        let session = launched(Theme::Light, Scheme::Light);
        assert_eq!(session.chrome(), Chrome::Shown, "the default");
        assert!(session.stored().is_none(), "nothing has moved yet");
        assert_eq!(session.toggle_chrome(), Chrome::Hidden);
        assert_eq!(session.chrome(), Chrome::Hidden);
        assert_eq!(
            session.stored().map(|settings| settings.chrome),
            Some(Chrome::Hidden),
            "the file would say hidden"
        );
        assert_eq!(session.toggle_chrome(), Chrome::Shown);
        assert!(session.stored().is_none(), "back where it was read");
    }

    /// A launch given `--settings` writes what a key moved to the file it was
    /// given, though it is a launch of the harness's and so may not write the
    /// writer's own.
    #[test]
    fn a_launch_given_a_settings_file_writes_that_one() {
        let path = std::env::temp_dir().join(format!("quill-settings-{}.toml", std::process::id()));
        std::fs::remove_file(&path).ok();
        let flags = Flags {
            settings: Some(path.clone()),
            ..Flags::default()
        };
        assert!(
            flags.is_harness(),
            "a launch pointed at a settings file of its own is its own process"
        );
        let session = Session::launch(
            flags,
            Settings::default(),
            State::default(),
            WindowState::default(),
            true,
            None,
        );
        assert_eq!(session.settings_path(), path);
        assert_ne!(session.settings_path(), Settings::path());
        assert_eq!(session.toggle_chrome(), Chrome::Hidden);
        session.store_settings();
        let (written, notes) = Settings::read_from(&path);
        assert_eq!(notes, Vec::<String>::new());
        assert_eq!(written.chrome, Chrome::Hidden);
        std::fs::remove_file(&path).ok();
    }

    /// View › Typeface picks a face, and the file follows it: Mono after the
    /// pick, back to what was read after picking the default again.
    #[test]
    fn a_typeface_radio_moves_the_face_and_the_file_follows() {
        let session = launched(Theme::Light, Scheme::Light);
        assert_eq!(session.face(), Face::default(), "the default");
        assert!(session.stored().is_none(), "nothing has moved yet");
        session.set_face(Face::Mono);
        assert_eq!(session.face(), Face::Mono);
        assert_eq!(
            session.stored().map(|settings| settings.face),
            Some(Face::Mono),
            "the file would say mono"
        );
        session.set_face(Face::default());
        assert!(session.stored().is_none(), "back where it was read");
    }

    /// A Focus radio picks its scope and switches Focus on with it (ADR
    /// 0006), from either state.
    #[test]
    fn a_focus_radio_picks_the_scope_and_switches_focus_on() {
        let session = launched(Theme::Light, Scheme::Light);
        assert_eq!(session.focus(), Focus::Off, "the default");
        assert_eq!(
            session.set_focus_scope(FocusScope::Paragraph),
            Focus::On(FocusScope::Paragraph)
        );
        assert_eq!(
            session.set_focus_scope(FocusScope::Sentence),
            Focus::On(FocusScope::Sentence),
            "already on: the scope moves, Focus stays on"
        );
        session.toggle_focus();
        assert_eq!(session.focus(), Focus::Off);
        assert_eq!(
            session.set_focus_scope(FocusScope::Sentence),
            Focus::On(FocusScope::Sentence),
            "picking the scope Focus left is still a pick: Focus comes back"
        );
    }

    /// Light Theme and Dark Theme pin a ground; Follow System returns to what
    /// the desktop last answered, and to the ground on screen where it never
    /// did.
    #[test]
    fn the_theme_radios_pin_a_ground_and_auto_returns_to_the_desktops() {
        let session = following(Theme::Auto, Scheme::Light, Some(Scheme::Light));
        assert_eq!(session.set_theme(Theme::Dark), Scheme::Dark);
        assert_eq!(session.theme(), Theme::Dark);
        assert_eq!(session.scheme(), Scheme::Dark);
        // The desktop goes dark and back to light while the writer is pinned:
        // they hear nothing, and the session only takes it down.
        session.desktop_moved(Some(Scheme::Dark));
        session.desktop_moved(Some(Scheme::Light));
        assert_eq!(session.scheme(), Scheme::Dark, "pinned");
        assert_eq!(
            session.set_theme(Theme::Auto),
            Scheme::Light,
            "the desktop's"
        );
        assert!(session.stored().is_none(), "auto is what was read");

        let alone = launched(Theme::Light, Scheme::Light);
        assert_eq!(alone.set_theme(Theme::Dark), Scheme::Dark);
        assert_eq!(
            alone.set_theme(Theme::Auto),
            Scheme::Dark,
            "no desktop ever answered: the ground on screen stays"
        );
        assert_eq!(
            alone.stored().map(|settings| settings.theme),
            Some(Theme::Auto)
        );
    }

    /// `--chrome off` is the setting for that launch alone.
    #[test]
    fn the_chrome_flag_hides_the_bars_for_the_launch() {
        let flags = Flags {
            chrome: Some(Chrome::Hidden),
            ..Flags::default()
        };
        let session = Session::launch(
            flags,
            Settings::default(),
            State::default(),
            WindowState::default(),
            true,
            None,
        );
        assert_eq!(session.chrome(), Chrome::Hidden);
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

    /// A writer's launch reading `settings`, on a desktop with no answer.
    fn writing(settings: Settings) -> Rc<Session> {
        Session::launch(
            Flags::default(),
            settings,
            State::default(),
            WindowState::default(),
            false,
            None,
        )
    }

    /// The launch the three keys start from: Focus off at `scope`, Typewriter
    /// off, which is what a writer who has never pressed any of them has.
    fn focused_at(scope: FocusScope) -> Rc<Session> {
        let mut settings = Settings::default();
        settings.focus_scope = scope;
        writing(settings)
    }

    /// `Ctrl+D` off and on again comes back to the scope it left, not to the
    /// default one.
    ///
    /// The scope is the half of Focus the key does not touch, and a writer who
    /// works in Paragraph scope would find Sentence scope waiting for them if
    /// it did. Two presses rather than one, because one press only proves the
    /// off.
    #[test]
    fn the_focus_key_off_and_on_comes_back_to_the_scope_it_left() {
        let session = focused_at(FocusScope::Paragraph);
        assert_eq!(session.toggle_focus(), Focus::On(FocusScope::Paragraph));
        assert_eq!(session.toggle_focus(), Focus::Off);
        assert_eq!(
            session.toggle_focus(),
            Focus::On(FocusScope::Paragraph),
            "the scope outlives Focus being switched off"
        );
    }

    /// `Ctrl+Shift+D` from off switches Focus on in the other scope, so the key
    /// means the same thing whichever state it is pressed from.
    #[test]
    fn the_scope_key_from_off_switches_focus_on_in_the_other_scope() {
        let session = focused_at(FocusScope::Sentence);
        assert_eq!(session.focus(), Focus::Off, "it starts off");
        assert_eq!(
            session.swap_focus_scope(),
            Focus::On(FocusScope::Paragraph),
            "on, and in the scope it was not in"
        );
        assert_eq!(
            session.swap_focus_scope(),
            Focus::On(FocusScope::Sentence),
            "and back, with Focus left on"
        );
    }

    /// `Ctrl+T` flips Typewriter and nothing else.
    #[test]
    fn the_typewriter_key_flips_typewriter_and_leaves_focus_alone() {
        let session = focused_at(FocusScope::Sentence);
        let anchor = session.settings().typewriter_anchor;
        assert_eq!(session.toggle_typewriter(), Typewriter::On(anchor));
        assert_eq!(session.focus(), Focus::Off, "Typewriter is not a scope");
        assert_eq!(session.toggle_typewriter(), Typewriter::Off);
    }

    /// What the three keys moved is what the launch would leave in the file,
    /// and a launch that moved nothing leaves nothing.
    ///
    /// Asked of [`Session::stored`] rather than of a file on disk, because the
    /// decision is the thing being proved and a writer's `settings.toml` is not
    /// this test's to touch.
    #[test]
    fn the_three_keys_are_what_the_launch_leaves_in_the_file() {
        let session = focused_at(FocusScope::Sentence);
        assert!(
            session.stored().is_none(),
            "a launch that pressed nothing rewrites nothing"
        );
        session.toggle_focus();
        session.swap_focus_scope();
        session.toggle_typewriter();
        let mut expected = session.settings().clone();
        expected.focus = true;
        expected.focus_scope = FocusScope::Paragraph;
        expected.typewriter = true;
        assert_eq!(
            session.stored().expect("three keys moved three values"),
            expected,
            "the three the keys moved, and every other key exactly as it was \
             read — the anchor included, which no key can reach"
        );

        session.toggle_typewriter();
        session.swap_focus_scope();
        session.toggle_focus();
        assert!(
            session.stored().is_none(),
            "and pressing them back leaves the file exactly as it was found"
        );
    }

    /// The settings a writer would have saved: every value moved off its
    /// default, so that a value the apply forgets shows as the default it was
    /// left at.
    fn edited() -> Settings {
        let mut settings = Settings::default();
        settings.theme = Theme::Dark;
        settings.face = Face::Mono;
        settings.step = 7;
        settings.focus = true;
        settings.focus_scope = FocusScope::Paragraph;
        settings.typewriter = true;
        settings.typewriter_anchor = 0.3;
        settings.chrome = Chrome::Hidden;
        settings
    }

    /// A settings file whose only edit is the `[shortcuts]` entry `line`.
    ///
    /// Read out of the text of a file rather than built, because a
    /// `[shortcuts]` table is the one thing in [`Settings`] the writer's own
    /// hand fills in and the entry is quoted here as they would write it.
    fn rebound(line: &str) -> Settings {
        let (settings, notes) = Settings::parse(&format!("[shortcuts]\n{line}\n"));
        assert_eq!(notes, Vec::<String>::new(), "the fixture reads cleanly");
        settings
    }

    /// The chords `id` is bound to once the file is over the registry.
    fn bound(session: &Session, id: &str) -> Vec<String> {
        session.settings().shortcuts().chords[id]
            .iter()
            .map(|chord| chord.as_str().to_owned())
            .collect()
    }

    /// The chords the registry gives `id`, which is what a file that says
    /// nothing about it leaves it bound to.
    fn defaults(id: &str) -> Vec<String> {
        quill_engine::commands::by_id(id)
            .expect("the registry has it")
            .accels()
    }

    /// Every chord the file leaves installed, over every Command.
    fn every_chord(session: &Session) -> Vec<String> {
        session
            .settings()
            .shortcuts()
            .chords
            .values()
            .flatten()
            .map(|chord| chord.as_str().to_owned())
            .collect()
    }

    /// A saved edit moves every setting the file carries, in one call.
    ///
    /// Whole-struct equality on what the launch would store rather than value
    /// by value, so that a setting given a live twin later is compared here
    /// without this test being remembered: after an apply the live values and
    /// the file agree about everything, which is exactly
    /// [`Session::stored`] finding nothing to write.
    #[test]
    fn a_saved_edit_moves_every_setting_the_file_carries() {
        let session = writing(Settings::default());
        session.apply(edited());
        assert_eq!(session.step(), 7);
        assert_eq!(session.face(), Face::Mono);
        assert_eq!(session.theme(), Theme::Dark);
        assert_eq!(session.scheme(), Scheme::Dark, "the ground is repainted");
        assert_eq!(session.focus(), Focus::On(FocusScope::Paragraph));
        assert_eq!(session.typewriter(), Typewriter::On(0.3));
        assert_eq!(session.chrome(), Chrome::Hidden);
        assert_eq!(*session.settings(), edited());
        assert!(
            session.stored().is_none(),
            "what was applied is what is in the file, so there is nothing to \
             write back over the writer's save"
        );
    }

    /// The save Quill makes itself moves nothing, so no window is told.
    ///
    /// A mode key writes the file as it is pressed, and the watch reads that
    /// write back a moment later. What comes back is what the windows are
    /// already showing, and repainting every window a fifth of a second after
    /// every `Ctrl+T` is the one thing the watch must not cost a writer.
    #[test]
    fn the_file_quill_wrote_itself_comes_back_having_moved_nothing() {
        let path = fixture("written");
        let session = pointed_at(&path);
        session.toggle_typewriter();
        session.toggle_focus();
        session.store_settings();
        let saved = session.read_again().expect("the file Quill wrote is TOML");
        assert!(
            !session.apply(saved),
            "the values in it are the ones this launch is running"
        );
        assert_eq!(session.typewriter(), Typewriter::On(0.5));
        assert!(
            session.stored().is_none(),
            "and what was read is now what is in the file"
        );
        std::fs::remove_dir_all(path.parent().expect("the fixture has a directory")).ok();
    }

    /// A hand's edit of the file moves something, and the windows are told.
    #[test]
    fn a_hand_edit_of_the_file_moves_what_the_windows_show() {
        let session = writing(Settings::default());
        assert!(session.apply(edited()), "everything in it moved");
        assert!(
            !session.apply(edited()),
            "and saving it again moves nothing"
        );
    }

    /// A file that says `auto` asks the desktop, as a launch on `auto` does.
    #[test]
    fn a_saved_auto_follows_the_ground_the_desktop_last_answered() {
        let session = following(Theme::Light, Scheme::Light, Some(Scheme::Dark));
        let mut settings = Settings::default();
        settings.theme = Theme::Auto;
        session.apply(settings);
        assert_eq!(session.theme(), Theme::Auto);
        assert_eq!(session.scheme(), Scheme::Dark);
    }

    /// A launch of the harness's keeps its flags over a file saved under it,
    /// so that a `--settings` fixture can be driven without the flags of the
    /// state being shot going with it.
    #[test]
    fn the_flags_still_override_a_file_saved_under_a_harness_launch() {
        let session = Session::launch(
            Flags {
                theme: Some(Scheme::Light),
                step: Some(3),
                ..Flags::default()
            },
            Settings::default(),
            State::default(),
            WindowState::default(),
            true,
            None,
        );
        session.apply(edited());
        assert_eq!(session.theme(), Theme::Light, "the flag, not the file");
        assert_eq!(session.step(), 3, "the flag, not the file");
        assert_eq!(session.face(), Face::Mono, "the file, which no flag names");
    }

    /// The rebind of the acceptance: `library.toggle` on `F9` alone, and
    /// `Ctrl+E`, which the registry gives it, bound to nothing at all.
    #[test]
    fn a_rebound_command_takes_its_chord_and_leaves_its_default_bound_to_nothing() {
        let session = writing(Settings::default());
        assert_eq!(
            bound(&session, "library.toggle"),
            defaults("library.toggle")
        );
        session.apply(rebound("\"library.toggle\" = [\"F9\"]"));
        assert_eq!(bound(&session, "library.toggle"), ["F9"]);
        assert!(
            !every_chord(&session).contains(&"<Control>e".to_owned()),
            "the default it replaced is bound to nothing"
        );
    }

    /// An entry taken out of the file puts the default back, because the map
    /// is computed on every read and nothing is remembered between two.
    #[test]
    fn a_rebind_taken_out_of_the_file_restores_the_default() {
        let session = writing(rebound("\"library.toggle\" = [\"F9\"]"));
        session.apply(Settings::default());
        assert_eq!(
            bound(&session, "library.toggle"),
            defaults("library.toggle")
        );
    }

    /// A refused entry leaves the Command bound to its default.
    #[test]
    fn a_super_chord_leaves_the_command_on_its_default() {
        let session = writing(Settings::default());
        session.apply(rebound("\"library.toggle\" = [\"<Super>l\"]"));
        assert_eq!(
            bound(&session, "library.toggle"),
            defaults("library.toggle")
        );
        let refusals = session.settings().shortcuts().refusals;
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].id, "library.toggle");
    }

    /// The first file a writer opens is where they write: with no Location,
    /// its folder becomes one and the settings file says so.
    #[test]
    fn a_first_open_with_an_empty_library_writes_its_folder_as_a_location() {
        let path = fixture("empty-library");
        let folder = path
            .parent()
            .expect("the fixture is in a folder")
            .to_owned();
        let document = folder.join("sample.md");
        std::fs::write(&document, "# A passage\n").expect("writes the Document");
        let session = pointed_at(&path);
        assert!(session.library().locations().is_empty(), "nothing yet");

        session.opened_at(&document);

        assert_eq!(
            session
                .library()
                .locations()
                .iter()
                .map(|location| location.root().to_owned())
                .collect::<Vec<PathBuf>>(),
            vec![folder.clone()]
        );
        assert_eq!(session.settings().library.locations, vec![folder.clone()]);
        let (written, _) = Settings::read_from(&path);
        assert_eq!(written.library.locations, vec![folder]);
    }

    /// A writer who has already pointed Quill at a folder has said which
    /// folders it shows: opening a file changes nothing, and the settings file
    /// is not even written.
    #[test]
    fn a_first_open_with_a_location_leaves_the_settings_alone() {
        let path = fixture("one-location");
        let folder = path
            .parent()
            .expect("the fixture is in a folder")
            .to_owned();
        let document = folder.join("sample.md");
        std::fs::write(&document, "# A passage\n").expect("writes the Document");
        let mut settings = Settings::default();
        settings.library.locations = vec![folder.clone()];
        let session = Session::launch(
            Flags {
                settings: Some(path.clone()),
                ..Flags::default()
            },
            settings,
            State::default(),
            WindowState::default(),
            false,
            None,
        );

        session.opened_at(&document);

        assert_eq!(session.library().locations().len(), 1);
        assert!(!path.exists(), "the settings file was never written");
    }

    /// The file this test writes and edits, in a directory of its own.
    ///
    /// Named for the test and carrying the pid, because worktrees test
    /// concurrently.
    fn fixture(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("quill-session-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).expect("makes its own scratch directory");
        directory.join("settings.toml")
    }

    /// A writer's launch reading and writing `path` rather than the writer's
    /// own file.
    fn pointed_at(path: &Path) -> Rc<Session> {
        Session::launch(
            Flags {
                settings: Some(path.to_owned()),
                ..Flags::default()
            },
            Settings::default(),
            State::default(),
            WindowState::default(),
            false,
            None,
        )
    }

    /// Held for as long as a test is listening for warnings, because the
    /// listening is the process's rather than the test's ([`warnings`]).
    static HANDLER: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Every `quill-settings` warning `saving` makes, in order.
    ///
    /// The handler is GLib's own, because the domain is the thing being
    /// proved: a warning raised anywhere else in this process during the call
    /// would be caught here too, and there is nowhere else that warns.
    fn warnings(saving: impl FnOnce()) -> Vec<String> {
        // GLib keeps one handler per domain for the whole process and hands a
        // warning to the last one set, so two tests listening at once would
        // hear each other's and one of them would hear nothing.
        let _listening = HANDLER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // GLib's handler is `Send + Sync`, so the warnings come back through a
        // lock even though the call below raises them on this thread.
        let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let heard = std::sync::Arc::clone(&said);
        let handler = glib::log_set_handler(
            Some(DOMAIN),
            glib::LogLevels::LEVEL_WARNING,
            false,
            false,
            move |_, _, message| {
                if let Ok(mut heard) = heard.lock() {
                    heard.push(message.to_owned());
                }
            },
        );
        saving();
        glib::log_remove_handler(Some(DOMAIN), handler);
        let said = said.lock().expect("the handler is gone");
        said.clone()
    }

    /// A refused line is warned about once per version of the file: the save
    /// that brought it says it, and a save that leaves it alone says nothing.
    ///
    /// Driven through the file and [`reread`], which is what the watch calls,
    /// so that what is proved is what a writer saving `settings.toml` hears.
    #[test]
    fn a_refused_line_is_warned_about_once_and_not_again_on_the_next_save() {
        let path = fixture("refused");
        let session = pointed_at(&path);
        let file = "[shortcuts]\n\"library.toggle\" = [\"<Super>l\"]\n";
        std::fs::write(&path, file).expect("writes its own fixture");
        let first = warnings(|| reread(None, &session));
        assert_eq!(first.len(), 1, "one warning, naming the entry: {first:?}");
        assert!(
            first[0].contains("\"library.toggle\" = [\"<Super>l\"]"),
            "the entry as the writer wrote it: {first:?}"
        );
        std::fs::write(&path, file).expect("writes its own fixture");
        assert!(
            warnings(|| reread(None, &session)).is_empty(),
            "saving the same file again says nothing"
        );
        assert_eq!(
            session.refusals().len(),
            1,
            "and the Settings window can still show it"
        );
        std::fs::remove_dir_all(path.parent().expect("the fixture has a directory")).ok();
    }

    /// A save that is not TOML at all leaves the refusal the Settings window
    /// is showing where it is: it applied no `[shortcuts]` entry, so it
    /// refused none.
    #[test]
    fn a_broken_save_leaves_the_refused_line_the_settings_window_shows() {
        let path = fixture("kept-refusal");
        let session = pointed_at(&path);
        std::fs::write(&path, "[shortcuts]\n\"library.toggle\" = [\"<Super>l\"]\n")
            .expect("writes its own fixture");
        warnings(|| reread(None, &session));
        let refused = session.refusals().clone();
        assert_eq!(refused.len(), 1, "{refused:?}");
        std::fs::write(&path, "theme = \n").expect("writes its own fixture");
        let said = warnings(|| reread(None, &session));
        assert_eq!(said.len(), 1, "one line about the file: {said:?}");
        assert!(said[0].contains("is not TOML"), "{said:?}");
        assert_eq!(
            *session.refusals(),
            refused,
            "the Settings window still shows what the last good read refused"
        );
        std::fs::remove_dir_all(path.parent().expect("the fixture has a directory")).ok();
    }

    /// A file that is not TOML at all leaves the settings where they are and
    /// says so once.
    #[test]
    fn broken_toml_keeps_the_last_good_settings_and_is_said_once() {
        let path = fixture("broken");
        let session = pointed_at(&path);
        session.apply(edited());
        std::fs::write(&path, "theme = \n").expect("writes its own fixture");
        assert!(
            session.read_again().is_none(),
            "there is nothing to apply, so nothing is applied"
        );
        let first = warnings(|| session.warn_unread());
        assert_eq!(first.len(), 1, "one line about the file: {first:?}");
        assert!(first[0].contains("is not TOML"), "{first:?}");
        assert!(
            warnings(|| {
                session.read_again();
                session.warn_unread();
            })
            .is_empty(),
            "and the same broken file again says nothing"
        );
        assert_eq!(
            *session.settings(),
            edited(),
            "the last good settings are still the ones this launch is running"
        );
        assert_eq!(session.step(), 7);
        assert_eq!(session.scheme(), Scheme::Dark);
        std::fs::remove_dir_all(path.parent().expect("the fixture has a directory")).ok();
    }

    /// A file mended after a broken save applies, and is not warned about
    /// again.
    #[test]
    fn a_file_mended_after_a_broken_save_applies() {
        let path = fixture("mended");
        let session = pointed_at(&path);
        std::fs::write(&path, "theme = \n").expect("writes its own fixture");
        assert!(session.read_again().is_none());
        warnings(|| session.warn_unread());
        edited().write_to(&path).expect("writes its own fixture");
        let settings = session.read_again().expect("the mended file is TOML");
        session.apply(settings);
        assert_eq!(session.step(), 7);
        assert_eq!(session.scheme(), Scheme::Dark);
        assert!(
            warnings(|| session.warn(Vec::new())).is_empty(),
            "a file with nothing wrong with it says nothing"
        );
        std::fs::remove_dir_all(path.parent().expect("the fixture has a directory")).ok();
    }

    /// A palette file beside `settings`, holding `text`.
    fn palette_beside(settings: &Path, text: &str) -> PathBuf {
        let path = settings.with_file_name("quill.toml");
        std::fs::write(&path, text).expect("writes its own palette");
        path
    }

    /// A launch of the harness's on `flags`, reading `settings` and no file.
    fn shot(flags: Flags, settings: Settings) -> Rc<Session> {
        Session::launch(
            flags,
            settings,
            State::default(),
            WindowState::default(),
            true,
            None,
        )
    }

    /// The paper `session` is painting on.
    fn paper(session: &Session) -> Colour {
        session.ground().colours.colour(Role::Paper)
    }

    /// A light paper in a colour nothing in the design uses.
    const RED_PAPER: &str = "[light]\npaper = \"#c81e1e\"\n";

    #[test]
    fn the_palette_flag_lays_its_file_over_the_ground_the_theme_flag_names() {
        let palette = palette_beside(&fixture("palette-flag"), RED_PAPER);
        let session = shot(
            Flags {
                theme: Some(Scheme::Light),
                palette: Some(palette),
                ..Flags::default()
            },
            Settings::default(),
        );
        assert_eq!(paper(&session), Colour::from_hex("#c81e1e"));
        assert_eq!(
            session.ground().colours.colour(Role::Ink),
            Ground::of(Scheme::Light).colours.colour(Role::Ink),
            "a role the file leaves out is the built-in"
        );
    }

    #[test]
    fn a_theme_flag_without_a_palette_flag_paints_the_built_ins_whatever_the_setting_names() {
        let palette = palette_beside(&fixture("palette-pinned"), RED_PAPER);
        let mut settings = Settings::default();
        settings.palette = Some(palette);
        let pinned = shot(
            Flags {
                theme: Some(Scheme::Light),
                ..Flags::default()
            },
            settings.clone(),
        );
        assert_eq!(pinned.ground(), Ground::of(Scheme::Light));
        assert_eq!(pinned.palette_path(), None, "the setting is not read");
        let writers = writing(settings);
        assert_eq!(
            paper(&writers),
            Colour::from_hex("#c81e1e"),
            "the same setting with no --theme is the writer's palette"
        );
    }

    #[test]
    fn a_dark_only_palette_leaves_the_light_ground_designed_and_the_toggle_finds_it() {
        let palette = palette_beside(
            &fixture("palette-dark-only"),
            "[dark]\npaper = \"#1e1ec8\"\n",
        );
        let mut settings = Settings::default();
        settings.theme = Theme::Light;
        settings.palette = Some(palette);
        let session = writing(settings);
        assert_eq!(session.ground(), Ground::of(Scheme::Light));
        session.toggle_scheme();
        assert_eq!(session.scheme(), Scheme::Dark);
        assert_eq!(paper(&session), Colour::from_hex("#1e1ec8"));
    }

    /// The watch tells a save by its length and write time; whether it is a
    /// repaint is the palette's equality, which is what is asked here.
    #[test]
    fn the_same_bytes_saved_again_are_not_a_repaint_and_a_removed_file_is_the_built_ins() {
        let palette = palette_beside(&fixture("palette-same-bytes"), RED_PAPER);
        let mut settings = Settings::default();
        settings.palette = Some(palette.clone());
        let session = writing(settings);
        assert!(!session.reread_palette(), "the same bytes");
        std::fs::write(&palette, "[light]\npaper = \"#1ec81e\"\n").expect("rewrites the palette");
        assert!(session.reread_palette(), "a new colour");
        assert_eq!(paper(&session), Colour::from_hex("#1ec81e"));
        std::fs::remove_file(&palette).expect("removes the palette");
        assert!(session.reread_palette(), "the file gone is a change");
        assert_eq!(
            session.ground(),
            Ground::of(Scheme::Light),
            "and it is the built-ins"
        );
        assert!(session.palette_notes.borrow().is_empty(), "with no note");
    }

    /// A `palette` line edited in the settings file moves the ground the way
    /// any other setting moves what it names, through the same re-read.
    #[test]
    fn a_palette_line_saved_into_the_settings_file_moves_the_ground() {
        let path = fixture("palette-setting");
        let palette = palette_beside(&path, RED_PAPER);
        let session = pointed_at(&path);
        assert_eq!(session.ground(), Ground::of(Scheme::Light));
        std::fs::write(&path, format!("palette = \"{}\"\n", palette.display()))
            .expect("writes its own fixture");
        reread(None, &session);
        assert_eq!(session.palette_path(), Some(palette));
        assert_eq!(paper(&session), Colour::from_hex("#c81e1e"));
        std::fs::write(&path, "").expect("unsets the palette");
        reread(None, &session);
        assert_eq!(session.palette_path(), None);
        assert_eq!(session.ground(), Ground::of(Scheme::Light));
    }

    /// A value that is not a colour costs its line and is said once, naming
    /// the palette file rather than the settings file; saying the same version
    /// again says nothing; and the other lines of the file land.
    #[test]
    fn a_palette_value_that_is_not_a_colour_is_said_once_and_names_its_file() {
        let path = fixture("palette-said-once");
        let palette = palette_beside(&path, "[light]\npaper = \"#c81e1e\"\nink = \"red\"\n");
        let session = pointed_at(&path);
        std::fs::write(&path, format!("palette = \"{}\"\n", palette.display()))
            .expect("writes its own fixture");
        let said = warnings(|| reread(None, &session));
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].starts_with(&format!("{}: ", palette.display())),
            "{}",
            said[0]
        );
        assert!(said[0].contains("[light] ink = \"red\""), "{}", said[0]);
        assert_eq!(
            paper(&session),
            Colour::from_hex("#c81e1e"),
            "the other line lands"
        );
        let again = warnings(|| repaint_palette(None, &session));
        assert!(again.is_empty(), "the same version says nothing: {again:?}");
    }
}
