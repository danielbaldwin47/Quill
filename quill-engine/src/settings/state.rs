//! What the app observed: the shape of the last session.
//!
//! One file, `$XDG_STATE_HOME/quill/state.toml`, written on quit and read on
//! launch. Nothing here is a preference and nothing here is a Document: it is
//! what Quill noticed while the writer worked, so that a relaunch puts back the
//! window they left rather than a window the defaults chose
//! ([ADR 0010](../../../docs/adr/0010-settings-in-toml-under-xdg.md)). It may
//! be deleted at any time; the next launch is simply a first launch.
//!
//! Four things live here: the size of each window, the last Document in each,
//! where the caret was in each Document the writer visited, and the recents
//! list. The Dark and light Piece adds a fifth, `last_scheme`: the ground the
//! last session ended on, which an `auto` launch paints while the desktop is
//! still being asked, and the Library a sixth, `library_width`: how wide the
//! writer dragged the pane, one width for the app and every window in it.
//! Beside the file sits `blind-keys/`, which is the Gate's.
//!
//! Three of them are the Library's: [`State::visited`] puts a Document at the
//! front of the recents, [`State::caret`] hands back where its caret was, so
//! that opening a recent Document puts the writer back where they left, and
//! [`State::library_width`] is the width the pane comes back at, pulled into
//! range by [`library_width`] on the way in as it was on the way out.
//!
//! **Position is not here.** GTK4 gives a client no way to ask where its window
//! is or to put it back, on Wayland or on X11: placement belongs to the
//! compositor, which restores it by its own rules. A pair of `x` and `y` keys
//! would be two numbers nothing could ever read.

use std::collections::BTreeMap;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use super::reading::Reading;
use super::writing::Writing;
use super::{file, xdg};
use crate::theme::Scheme;

/// The file Quill writes for its next launch, under [`xdg::state_dir`].
pub const STATE_FILE: &str = "state.toml";

/// The directory the Gate keeps its blind keys in, beside the state file.
const BLIND_KEYS: &str = "blind-keys";

/// How many Documents the recents remember: the twenty-five most recently
/// opened, which is what Open Recent narrows and what bounds every walk of the
/// list ([`State::visited`] is the only thing that grows it).
const RECENTS: usize = 25;

/// What a note about a state file Quill could not read ends with. State is
/// what quitting leaves, so there is nothing to lose by starting again.
const INSTEAD: &str = "starting fresh";

/// The shape a window opens at when nothing has been remembered yet: what the
/// spike was judged at.
const WIDTH: u32 = 1100;
const HEIGHT: u32 = 760;

/// The range outside which a remembered size is not a window: no compositor
/// gives out a window narrower than this, and nothing survives a state file
/// that says a window was two million pixels wide.
const SMALLEST: u32 = 100;
const LARGEST: u32 = 32768;

/// The sizes a window may open at.
///
/// A range rather than two constants, because two places hold this line and
/// they must hold the same one: a remembered size in the state file, and the
/// `--w` and `--h` flags a judged shot is taken at.
#[must_use]
pub fn window_sizes() -> RangeInclusive<u32> {
    SMALLEST..=LARGEST
}

/// The width the Library pane stands at until a writer drags the divider:
/// `quill::sidebar::WIDTH`, the pane as the Design oracle measures it
/// (`ref/ia/mac-native/NOTES.md` § State 28, *Pane, total*), and the narrowest
/// a saved width is read back at.
const LIBRARY: u32 = 360;

/// The widest a saved pane width is read back at: the File List alone widens
/// between [`LIBRARY`] and this (#441 § The pane).
const WIDEST: u32 = 500;

/// The widths the Library pane is dragged between, which a saved width is
/// pulled into on the way in before [`library_width`] fits it to the window.
#[must_use]
pub const fn library_widths() -> RangeInclusive<u32> {
    LIBRARY..=WIDEST
}

/// The narrowest the pane may be dragged. Below this a row is a truncated
/// name rather than a Document.
const NARROWEST: u32 = 240;

/// What the page keeps of the window whatever the pane is dragged to: the
/// divider stops here rather than pushing the writing off the screen.
const PAGE: u32 = 320;

/// The width the Library pane stands at in a window `window` pixels wide,
/// given the `wanted` width a drag or a state file asked for.
///
/// One pure function for all three askers — a drag, a launch reading the
/// state file, and a window that has since been made narrower — so that the
/// pane is never at a width one of them would refuse. A window with room for
/// neither keeps the pane at [`NARROWEST`] and gives the page what is left.
#[must_use]
pub fn library_width(wanted: u32, window: u32) -> u32 {
    wanted.clamp(NARROWEST, window.saturating_sub(PAGE).max(NARROWEST))
}

/// One window as it was left.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowState {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// Whether it was maximized, in which case the size is what it would go
    /// back to.
    pub maximized: bool,
    /// Whether it was full screen.
    pub fullscreen: bool,
    /// The Document it was showing, or `None` for an untitled one.
    pub document: Option<PathBuf>,
    /// Anything else in the table, carried through a write.
    rest: toml::Table,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: WIDTH,
            height: HEIGHT,
            maximized: false,
            fullscreen: false,
            document: None,
            rest: toml::Table::new(),
        }
    }
}

impl WindowState {
    /// Reads one `[[window]]` table.
    fn read(table: toml::Table, notes: &mut Vec<String>) -> Self {
        let defaults = Self::default();
        let mut reading = Reading::new(table, "window.", notes);
        let width = reading.whole("width", defaults.width, &window_sizes());
        let height = reading.whole("height", defaults.height, &window_sizes());
        let maximized = reading.boolean("maximized", defaults.maximized);
        let fullscreen = reading.boolean("fullscreen", defaults.fullscreen);
        let document = reading.path("document");
        Self {
            width,
            height,
            maximized,
            fullscreen,
            document,
            rest: reading.rest(),
        }
    }

    /// One `[[window]]` table as it is written.
    fn to_table(&self) -> toml::Table {
        let mut writing = Writing::new();
        writing.whole("width", self.width);
        writing.whole("height", self.height);
        writing.boolean("maximized", self.maximized);
        writing.boolean("fullscreen", self.fullscreen);
        writing.path("document", self.document.as_deref());
        writing.rest(self.rest.clone());
        writing.finish()
    }
}

/// Everything the app observed.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// The windows of the last session, the one left last first.
    pub windows: Vec<WindowState>,
    /// The Documents the writer opened, most recent first.
    pub recents: Vec<PathBuf>,
    /// Where the caret was in each Document, as a byte offset.
    pub carets: BTreeMap<PathBuf, u64>,
    /// The ground the last session ended on.
    ///
    /// What the next `auto` launch paints while the desktop is still being
    /// asked which colour scheme it is in, so that a dark desktop never sees a
    /// white first frame.
    pub last_scheme: Scheme,
    /// How wide the writer dragged the Library pane, in logical pixels.
    ///
    /// One width for the app: every window stands its pane at it, and a drag
    /// in any of them moves all of them. Read back through [`library_width`],
    /// so a file written beside a wider monitor never opens a pane with no
    /// page beside it.
    pub library_width: u32,
    /// How wide the writer dragged the Preview pane, in logical pixels, and
    /// [`None`] where they never dragged it — which is what the pair reads as
    /// "divide evenly".
    ///
    /// One width for the app, as [`State::library_width`] is: every window
    /// stands its pane at it and a drag in any of them moves all of them. It
    /// is a width and not a fraction because that is what the writer dragged,
    /// and it is pulled into range against the pair it stands in rather than
    /// here, where the window it will open beside is not known yet. Half a pair
    /// is not a width until there is a window to halve, so there is no width to
    /// carry until the writer has dragged one.
    pub preview_width: Option<u32>,
    /// Every key and table this Quill did not know, kept for the next write.
    rest: toml::Table,
}

impl Default for State {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            recents: Vec::new(),
            carets: BTreeMap::new(),
            last_scheme: Scheme::default(),
            library_width: LIBRARY,
            preview_width: None,
            rest: toml::Table::new(),
        }
    }
}

impl State {
    /// The directory the state file and the Gate's blind keys sit in.
    #[must_use]
    pub fn dir() -> PathBuf {
        xdg::state_dir()
    }

    /// The file Quill writes on quit.
    #[must_use]
    pub fn path() -> PathBuf {
        Self::dir().join(STATE_FILE)
    }

    /// The directory the Gate keeps its blind keys in.
    #[must_use]
    pub fn blind_keys() -> PathBuf {
        Self::dir().join(BLIND_KEYS)
    }

    /// The last session's state, and one note per thing worth telling the
    /// writer.
    ///
    /// No file is a first launch and not a failure. Unlike `settings.toml`,
    /// nothing is written here on the way in: state is what quitting leaves.
    #[must_use]
    pub fn open() -> (Self, Vec<String>) {
        Self::read_from(&Self::path())
    }

    /// Writes the state to its file, atomically.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written.
    pub fn store(&self) -> io::Result<()> {
        self.write_to(&Self::path())
    }

    /// Reads `path`, falling back to an empty state for anything it cannot.
    #[must_use]
    pub fn read_from(path: &Path) -> (Self, Vec<String>) {
        let (table, mut notes) = file::read_table(path, INSTEAD);
        let state = table.map_or_else(Self::default, |table| Self::read(table, &mut notes));
        (state, notes)
    }

    /// Writes the state to `path`, atomically.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written.
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        file::write(path, &self.to_toml())
    }

    /// Reads state out of the text of a file. Never fails.
    #[must_use]
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let (table, mut notes) = file::parse(text, INSTEAD);
        let state = table.map_or_else(Self::default, |table| Self::read(table, &mut notes));
        (state, notes)
    }

    /// Reads a parsed table.
    fn read(table: toml::Table, notes: &mut Vec<String>) -> Self {
        let mut reading = Reading::new(table, "", notes);
        let recents = reading.paths("recents");
        let last_scheme = reading.choice("last_scheme");
        // Read at every whole number a file could hold and then pulled into
        // range, rather than read at the range: the width is one Quill wrote
        // itself, so a value outside it is a monitor that has gone away and
        // not a writer's typo, and the pane opens at the nearest width it can
        // stand at: inside [`library_widths`] first, so the old pane's 240
        // opens as 360, and then inside the window.
        let width = reading.whole("library_width", LIBRARY, &(0..=LARGEST));
        // Read at every whole number for the reason above, and not pulled into
        // range at all: the range a Preview pane stands in is the pair's, and
        // the pair is a window that has not opened yet.
        // Zero on disk is the one number that is not a width: it is what a
        // file written before any drag says, and what this writes back for
        // [`None`], so the file keeps its one key either way.
        let preview_width = reading.whole("preview_width", 0, &(0..=LARGEST));
        let preview_width = (preview_width > 0).then_some(preview_width);
        // The windows are taken here and read below, once the reading of the
        // top level is done with the notes it is writing into.
        let windows = reading.tables("window");
        let carets = reading.table("caret");
        let rest = reading.rest();
        Self {
            windows: windows
                .into_iter()
                .map(|window| WindowState::read(window, notes))
                .collect(),
            recents,
            carets: read_carets(&carets),
            last_scheme,
            library_width: library_width(width.clamp(LIBRARY, WIDEST), LARGEST),
            preview_width,
            rest,
        }
    }

    /// The state as the file's text.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut writing = Writing::new();
        writing.paths("recents", &self.recents);
        writing.choice("last_scheme", self.last_scheme);
        writing.whole("library_width", self.library_width);
        writing.whole("preview_width", self.preview_width.unwrap_or(0));
        writing.rest(self.rest.clone());
        writing.tables(
            "window",
            self.windows.iter().map(WindowState::to_table).collect(),
        );
        writing.table("caret", write_carets(&self.carets));
        writing.into_toml()
    }

    /// Notes that `path` was opened: it goes to the front of the recents, and
    /// the list is cut back to the [`RECENTS`] newest.
    ///
    /// A Document that is already in the list moves rather than repeats, so
    /// re-opening the one file a writer lives in never fills the list with it.
    pub fn visited(&mut self, path: &Path) {
        self.recents.retain(|recent| recent != path);
        self.recents.insert(0, path.to_path_buf());
        self.recents.truncate(RECENTS);
    }

    /// Where the caret was in `path` when the writer last left it, or `None`
    /// for a Document this state has never seen.
    #[must_use]
    pub fn caret(&self, path: &Path) -> Option<u64> {
        self.carets.get(path).copied()
    }

    /// The shape a window opens at: the one left last, or the default.
    #[must_use]
    pub fn window(&self) -> WindowState {
        self.windows.first().cloned().unwrap_or_default()
    }
}

/// The `[caret]` table as a map. An entry that is not a byte offset is Quill's
/// own bug or a corrupted file, not a writer's typo, so it goes quietly.
fn read_carets(table: &toml::Table) -> BTreeMap<PathBuf, u64> {
    table
        .iter()
        .filter_map(|(path, offset)| {
            let offset = u64::try_from(offset.as_integer()?).ok()?;
            Some((PathBuf::from(path), offset))
        })
        .collect()
}

/// The map as the `[caret]` table, skipping any path that is not UTF-8.
fn write_carets(carets: &BTreeMap<PathBuf, u64>) -> toml::Table {
    carets
        .iter()
        .filter_map(|(path, offset)| {
            let offset = i64::try_from(*offset).ok()?;
            Some((path.to_str()?.to_string(), toml::Value::Integer(offset)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::file::scratch;
    use super::*;

    #[test]
    fn no_file_is_a_first_launch_with_a_window_of_the_default_shape() {
        let path = scratch("no_file").join(STATE_FILE);
        let (state, notes) = State::read_from(&path);
        assert_eq!(state, State::default());
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(state.window(), WindowState::default());
        assert_eq!(state.window().width, 1100);
        assert!(!path.exists(), "reading state writes nothing");
    }

    #[test]
    fn the_size_a_window_was_left_at_is_the_size_it_comes_back_at() {
        let path = scratch("geometry").join(STATE_FILE);
        let left = State {
            windows: vec![WindowState {
                width: 1440,
                height: 900,
                maximized: true,
                ..WindowState::default()
            }],
            ..State::default()
        };
        left.write_to(&path).expect("writes the state file");
        let (back, notes) = State::read_from(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(back, left);
        assert_eq!(back.window().width, 1440);
        assert_eq!(back.window().height, 900);
        assert!(back.window().maximized);
    }

    #[test]
    fn the_ground_the_last_session_ended_on_comes_back_with_it() {
        let path = scratch("last_scheme").join(STATE_FILE);
        let left = State {
            last_scheme: Scheme::Dark,
            ..State::default()
        };
        left.write_to(&path).expect("writes the state file");
        let (back, notes) = State::read_from(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(back.last_scheme, Scheme::Dark);
        assert_eq!(
            State::default().last_scheme,
            Scheme::Light,
            "a first launch has nothing to remember and paints the light ground"
        );

        let (auto, notes) = State::parse("last_scheme = \"auto\"\n");
        assert_eq!(
            auto.last_scheme,
            Scheme::Light,
            "`auto` is a setting, never a ground a session ended on"
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
    }

    /// The recents are the twenty-five newest, newest first, and a Document
    /// opened again moves to the front rather than repeating.
    #[test]
    fn the_recents_hold_the_twenty_five_newest_with_a_reopened_document_first() {
        let mut state = State::default();
        let visit =
            |state: &mut State, n: usize| state.visited(&PathBuf::from(format!("/w/{n}.md")));
        for n in 0..30 {
            visit(&mut state, n);
        }
        assert_eq!(state.recents.len(), RECENTS);
        assert_eq!(state.recents[0], PathBuf::from("/w/29.md"));
        assert_eq!(state.recents[RECENTS - 1], PathBuf::from("/w/5.md"));
        assert!(
            !state.recents.contains(&PathBuf::from("/w/4.md")),
            "the oldest fell off the end: {:?}",
            state.recents
        );

        visit(&mut state, 10);
        assert_eq!(state.recents[0], PathBuf::from("/w/10.md"));
        assert_eq!(
            state.recents.len(),
            RECENTS,
            "a re-open moves, never repeats"
        );
        assert_eq!(
            state
                .recents
                .iter()
                .filter(|r| r.ends_with("10.md"))
                .count(),
            1
        );
    }

    #[test]
    fn a_caret_stored_for_a_document_reads_back_at_its_offset() {
        let path = scratch("caret").join(STATE_FILE);
        let document = PathBuf::from("/w/sea-storm.md");
        let mut left = State::default();
        left.visited(&document);
        left.carets.insert(document.clone(), 412);
        left.write_to(&path).expect("writes the state file");

        let (back, notes) = State::read_from(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(back.caret(&document), Some(412));
        assert_eq!(back.recents, vec![document]);
        assert_eq!(
            back.caret(Path::new("/w/never-opened.md")),
            None,
            "a Document this state never saw has no caret to put back"
        );
    }

    #[test]
    fn every_key_the_state_file_holds_is_in_what_the_defaults_write() {
        let text = State::default().to_toml();
        let written: toml::Table = text.parse().expect("what is written is TOML");
        for key in [
            "recents",
            "last_scheme",
            "library_width",
            "preview_width",
            "window",
            "caret",
        ] {
            assert!(written.contains_key(key), "no `{key}` in:\n{text}");
        }
    }

    #[test]
    fn what_is_written_reads_back_as_itself() {
        let state = State {
            windows: vec![
                WindowState {
                    width: 1440,
                    document: Some(PathBuf::from("/home/writer/one.md")),
                    ..WindowState::default()
                },
                WindowState::default(),
            ],
            recents: vec![
                PathBuf::from("/home/writer/one.md"),
                PathBuf::from("/home/writer/two.md"),
            ],
            carets: [(PathBuf::from("/home/writer/one.md"), 1234)]
                .into_iter()
                .collect(),
            last_scheme: Scheme::Dark,
            library_width: 480,
            preview_width: Some(620),
            rest: toml::Table::new(),
        };
        let text = state.to_toml();
        let (read, notes) = State::parse(&text);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(read, state, "{text}");
        assert_eq!(read.to_toml(), text);
    }

    #[test]
    fn a_size_that_is_not_a_window_is_ignored_rather_than_obeyed() {
        let (state, notes) = State::parse("[[window]]\nwidth = 0\nheight = 900\n");
        assert_eq!(state.window().width, WIDTH, "a zero-wide window is not one");
        assert_eq!(state.window().height, 900, "and the height still counts");
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("window.width:"), "{notes:?}");
    }

    #[test]
    fn a_state_file_from_a_newer_quill_keeps_what_this_one_does_not_know() {
        let (state, notes) = State::parse(
            "recents = []\nsession = \"night\"\n\n[[window]]\nwidth = 1440\nzoom = 2\n",
        );
        assert!(notes.is_empty(), "{notes:?}");
        let written: toml::Table = state.to_toml().parse().expect("writes TOML");
        assert_eq!(written["session"].as_str(), Some("night"));
        assert_eq!(written["window"][0]["zoom"].as_integer(), Some(2));
    }

    #[test]
    fn the_width_the_pane_was_dragged_to_comes_back_and_anything_else_is_clamped() {
        let path = scratch("library_width").join(STATE_FILE);
        let left = State {
            library_width: 480,
            ..State::default()
        };
        left.write_to(&path).expect("writes the state file");
        let (back, notes) = State::read_from(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(back.library_width, 480);

        let (fresh, notes) = State::parse("recents = []\n");
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(
            fresh.library_width, 360,
            "a file with no width opens the pane at the default"
        );
        assert_eq!(State::default().library_width, 360);
        assert_eq!(library_widths(), 360..=500);

        for (saved, opens, why) in [
            (12, 360, "12 is not a pane"),
            (240, 360, "the old pane's narrowest opens at the new one's"),
            (368, 368, "the old pane's default is inside the range"),
            (500, 500, "the widest stands"),
            (600, 500, "past the widest opens at it"),
            (LARGEST, 500, "a width a monitor held opens at the widest"),
        ] {
            let (read, notes) = State::parse(&format!("library_width = {saved}\n"));
            assert!(notes.is_empty(), "{notes:?}");
            assert_eq!(read.library_width, opens, "{saved}: {why}");
        }
    }

    /// The Preview pane's width is the other half of the same story, with one
    /// difference: nothing is clamped here, because the pane stands in a pair
    /// and not in a window, and a file that says nothing is an even Split.
    #[test]
    fn the_width_the_preview_pane_was_dragged_to_comes_back_as_it_was() {
        let path = scratch("preview_width").join(STATE_FILE);
        let left = State {
            preview_width: Some(620),
            ..State::default()
        };
        left.write_to(&path).expect("writes the state file");
        let (back, notes) = State::read_from(&path);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(back.preview_width, Some(620));

        let (fresh, notes) = State::parse("recents = []\n");
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(
            fresh.preview_width, None,
            "a file that never saw a drag divides the pair evenly"
        );
        assert_eq!(State::default().preview_width, None);
    }

    #[test]
    fn the_pane_is_never_narrower_than_its_narrowest_nor_wider_than_leaves_the_page() {
        assert_eq!(library_width(LIBRARY, 1100), LIBRARY, "the default fits");
        assert_eq!(library_width(NARROWEST - 1, 1100), NARROWEST);
        assert_eq!(library_width(0, 1100), NARROWEST);
        assert_eq!(library_width(NARROWEST, 1100), NARROWEST);
        assert_eq!(
            library_width(1000, 1100),
            1100 - PAGE,
            "the page keeps its {PAGE}"
        );
        assert_eq!(library_width(u32::MAX, 1100), 1100 - PAGE);
        assert_eq!(
            library_width(LIBRARY, 400),
            NARROWEST,
            "a window with room for neither keeps the pane at its narrowest"
        );
        assert_eq!(library_width(LIBRARY, 0), NARROWEST);
    }

    #[test]
    fn state_is_a_directory_of_its_own_with_the_gates_blind_keys_in_it() {
        assert_eq!(State::path().parent(), Some(State::dir().as_path()));
        assert_eq!(State::blind_keys().parent(), Some(State::dir().as_path()));
        assert_ne!(State::dir(), crate::settings::Settings::path());
        assert!(!State::dir().starts_with(xdg::config_dir()));
    }
}
