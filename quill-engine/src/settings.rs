//! Settings: what the writer chose, and what the app observed.
//!
//! Config is one TOML file at `$XDG_CONFIG_HOME/quill/settings.toml`; state
//! lives under `$XDG_STATE_HOME/quill/`. Missing keys take defaults and unknown
//! keys are kept, so an older Quill never destroys a newer file. The file is
//! watched like a Document: a saved edit applies without a restart, and a line
//! that cannot be applied is logged once and skipped, never fatal.
//!
//! Three promises hold that together, and each has a piece of this module
//! behind it. **Nothing is fatal**: [`Settings::parse`] never fails, and hands
//! back one note per key it could not apply for the caller to log
//! ([`reading`]). **Nothing is lost**: every key Quill does not know is carried
//! and written back where a writer can still see it ([`writing`]). **Nothing is
//! half-written**: every write goes through a temporary file and a rename
//! ([`file`]).
//!
//! Quill writes `settings.toml` only when there is none. A file it could not
//! read is left exactly as the writer left it, because the way to fix a file
//! Quill misunderstands is to open it, and a Quill that overwrote it first
//! would have taken that away.

mod file;
mod reading;
mod state;
mod writing;
mod xdg;

use std::io;
use std::path::{Path, PathBuf};

pub use state::{STATE_FILE, State, WindowState};

use reading::Reading;
use writing::Writing;

/// The file a writer edits, under [`xdg::config_dir`].
pub const SETTINGS_FILE: &str = "settings.toml";

/// The Editor's type size in pixels, and the range outside which a number is a
/// typo rather than a preference.
const SIZE: u32 = 20;
const SMALLEST: u32 = 6;
const LARGEST: u32 = 200;

/// Where the caret sits down the window when Typewriter is on: the middle.
const ANCHOR: f64 = 0.5;

/// The Template Preview and Export start from.
const TEMPLATE: &str = "default";

/// What a note about a settings file Quill could not read ends with: a writer
/// wants to know what became of their preferences, not only what went wrong.
const INSTEAD: &str = "using the defaults, and leaving the file alone";

/// One setting that takes one of a few named values.
///
/// Written as a string, because `theme = "dark"` says in the file what a writer
/// meant and `theme = 2` does not. Every implementation comes from the
/// [`choice!`] macro below.
pub trait Choice: Copy + Default {
    /// Every value this setting takes, in the order a note lists them.
    const VALUES: &'static [&'static str];

    /// What this value is written as.
    #[must_use]
    fn as_str(self) -> &'static str;

    /// The value `text` names, or `None` when it names none of them.
    #[must_use]
    fn parse(text: &str) -> Option<Self>
    where
        Self: Sized;
}

/// Declares one [`Choice`]: an enum, the string each value is written as, and
/// the `#[default]` one.
///
/// Five settings have this exact shape and no behaviour of their own, so the
/// alternative is five copies of the same twenty lines, each of which could
/// disagree with the file format in its own way.
macro_rules! choice {
    (
        $(#[$about:meta])*
        $name:ident { $( $(#[$value_about:meta])* $value:ident => $written:literal ),+ $(,)? }
    ) => {
        $(#[$about])*
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub enum $name {
            $( $(#[$value_about])* $value ),+
        }

        impl Choice for $name {
            const VALUES: &'static [&'static str] = &[$($written),+];

            fn as_str(self) -> &'static str {
                match self { $( Self::$value => $written ),+ }
            }

            fn parse(text: &str) -> Option<Self> {
                match text {
                    $( $written => Some(Self::$value), )+
                    _ => None,
                }
            }
        }
    };
}

choice! {
    /// Light or dark, or whichever the desktop is in.
    Theme {
        /// Follow the desktop's colour scheme.
        #[default]
        Auto => "auto",
        /// Dark ink on paper.
        Light => "light",
        /// Light ink on a dark ground.
        Dark => "dark",
    }
}

choice! {
    /// Which of the three Faces a Document is set in.
    Face {
        /// Quill Duo.
        #[default]
        Duo => "duo",
        /// Quill Quattro.
        Quattro => "quattro",
        /// Quill Mono.
        Mono => "mono",
    }
}

choice! {
    /// How much Focus leaves lit.
    FocusScope {
        /// The sentence the caret is in.
        #[default]
        Sentence => "sentence",
        /// The paragraph the caret is in.
        Paragraph => "paragraph",
    }
}

choice! {
    /// Whether the two bars around the Editor are there at all.
    Chrome {
        /// Shown, and stepping back while the writer types.
        #[default]
        Shown => "shown",
        /// Hidden until the writer asks for them.
        Hidden => "hidden",
    }
}

choice! {
    /// Where Preview opens.
    PreviewLayout {
        /// Beside the Editor.
        #[default]
        Split => "split",
        /// In place of the Editor.
        Full => "full",
    }
}

impl Face {
    /// The family the Editor asks fontconfig for.
    ///
    /// Read out of [`crate::data::FACES`] rather than spelled again here, so
    /// that the name in the file, the name in the font and the name asked for
    /// cannot drift apart.
    #[must_use]
    pub fn family(self) -> &'static str {
        crate::data::FACES[self.first_face()].0
    }

    /// The family for the italic cut, which is a family of its own (ADR 0007).
    #[must_use]
    pub fn italic_family(self) -> &'static str {
        crate::data::FACES[self.first_face() + 1].0
    }

    /// Where this Face's pair starts in [`crate::data::FACES`]: Roman, then its
    /// Italic.
    fn first_face(self) -> usize {
        match self {
            Self::Duo => 0,
            Self::Quattro => 2,
            Self::Mono => 4,
        }
    }
}

/// Syntax highlight: the master switch, and one toggle per part of speech.
///
/// The categories default to on so that turning the master on shows the whole
/// texture of a sentence; a writer who wants only nouns turns four off.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxHighlight {
    /// The master switch.
    pub enabled: bool,
    /// Colour nouns.
    pub nouns: bool,
    /// Colour verbs.
    pub verbs: bool,
    /// Colour adjectives.
    pub adjectives: bool,
    /// Colour adverbs.
    pub adverbs: bool,
    /// Colour conjunctions.
    pub conjunctions: bool,
    /// Anything else in the table, carried through a write.
    rest: toml::Table,
}

impl Default for SyntaxHighlight {
    fn default() -> Self {
        Self {
            enabled: false,
            nouns: true,
            verbs: true,
            adjectives: true,
            adverbs: true,
            conjunctions: true,
            rest: toml::Table::new(),
        }
    }
}

impl SyntaxHighlight {
    /// Reads the `[syntax_highlight]` table.
    fn read(table: toml::Table, notes: &mut Vec<String>) -> Self {
        let defaults = Self::default();
        let mut reading = Reading::new(table, "syntax_highlight.", notes);
        let enabled = reading.boolean("enabled", defaults.enabled);
        let nouns = reading.boolean("nouns", defaults.nouns);
        let verbs = reading.boolean("verbs", defaults.verbs);
        let adjectives = reading.boolean("adjectives", defaults.adjectives);
        let adverbs = reading.boolean("adverbs", defaults.adverbs);
        let conjunctions = reading.boolean("conjunctions", defaults.conjunctions);
        Self {
            enabled,
            nouns,
            verbs,
            adjectives,
            adverbs,
            conjunctions,
            rest: reading.rest(),
        }
    }

    /// The `[syntax_highlight]` table as it is written.
    fn to_table(&self) -> toml::Table {
        let mut writing = Writing::new();
        writing.boolean("enabled", self.enabled);
        writing.boolean("nouns", self.nouns);
        writing.boolean("verbs", self.verbs);
        writing.boolean("adjectives", self.adjectives);
        writing.boolean("adverbs", self.adverbs);
        writing.boolean("conjunctions", self.conjunctions);
        writing.rest(self.rest.clone());
        writing.finish()
    }
}

/// Style check: the master switch, and one toggle per list.
///
/// The list names are still open ([#29](https://github.com/danielbaldwin47/Quill/issues/29)),
/// so the toggles are carried exactly as they are written rather than named
/// here. A Quill that guessed at them would rewrite a file it does not
/// understand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleCheck {
    /// The master switch.
    pub enabled: bool,
    /// One entry per list, as written.
    pub lists: toml::Table,
}

impl StyleCheck {
    /// Reads the `[style_check]` table.
    fn read(table: toml::Table, notes: &mut Vec<String>) -> Self {
        let mut reading = Reading::new(table, "style_check.", notes);
        let enabled = reading.boolean("enabled", false);
        Self {
            enabled,
            lists: reading.rest(),
        }
    }

    /// The `[style_check]` table as it is written.
    fn to_table(&self) -> toml::Table {
        let mut writing = Writing::new();
        writing.boolean("enabled", self.enabled);
        writing.rest(self.lists.clone());
        writing.finish()
    }
}

/// Everything the writer chose.
///
/// One field per key in `docs/architecture.md`'s Settings section, in that
/// order. The file follows it too, as far as TOML allows: the tables come last
/// however they are written, because a plain key after a header would belong to
/// that header (see [`writing`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Light, dark, or the desktop's.
    pub theme: Theme,
    /// Which Face a Document is set in.
    pub face: Face,
    /// The Editor's type size, in pixels.
    pub size: u32,
    /// Whether Focus is on.
    pub focus: bool,
    /// How much Focus leaves lit.
    pub focus_scope: FocusScope,
    /// Whether Typewriter is on.
    pub typewriter: bool,
    /// Where down the window Typewriter holds the caret's line, 0 to 1.
    pub typewriter_anchor: f64,
    /// Whether the bars around the Editor are there.
    pub chrome: Chrome,
    /// Whether Spell check is on.
    pub spell_check: bool,
    /// The dictionary Spell check asks for; empty is the desktop's own
    /// language, which is what enchant picks when nobody names one.
    pub spell_language: String,
    /// Syntax highlight and its five categories.
    pub syntax_highlight: SyntaxHighlight,
    /// Style check and its lists.
    pub style_check: StyleCheck,
    /// The current Template's name.
    pub template: String,
    /// Where Preview opens.
    pub preview_layout: PreviewLayout,
    /// The folder Quill was pointed at, or `None` until it is pointed at one.
    pub library: Option<PathBuf>,
    /// Command id to chords, carried as written; validating and applying it is
    /// the shortcut ticket's ([#44](https://github.com/danielbaldwin47/Quill/issues/44)).
    pub shortcuts: toml::Table,
    /// Every key and table this Quill did not know, kept for the next write.
    rest: toml::Table,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            face: Face::default(),
            size: SIZE,
            focus: false,
            focus_scope: FocusScope::default(),
            typewriter: false,
            typewriter_anchor: ANCHOR,
            chrome: Chrome::default(),
            spell_check: true,
            spell_language: String::new(),
            syntax_highlight: SyntaxHighlight::default(),
            style_check: StyleCheck::default(),
            template: TEMPLATE.to_string(),
            preview_layout: PreviewLayout::default(),
            library: None,
            shortcuts: toml::Table::new(),
            rest: toml::Table::new(),
        }
    }
}

impl Settings {
    /// The file a writer edits.
    #[must_use]
    pub fn path() -> PathBuf {
        xdg::config_dir().join(SETTINGS_FILE)
    }

    /// The writer's settings, and one note per thing worth telling them.
    ///
    /// Writes the file with every key at its default when there is none, which
    /// is what a first launch does; a file that is already there is only read.
    /// Nothing here fails: a home directory that cannot be written leaves a
    /// note and a Quill running on the defaults.
    #[must_use]
    pub fn open() -> (Self, Vec<String>) {
        let path = Self::path();
        let (settings, mut notes) = Self::read_from(&path);
        if !path.exists()
            && let Err(err) = settings.write_to(&path)
        {
            notes.push(format!("cannot be written ({err}); using the defaults"));
        }
        (settings, notes)
    }

    /// Reads `path`, falling back to the defaults for anything it cannot.
    #[must_use]
    pub fn read_from(path: &Path) -> (Self, Vec<String>) {
        let (table, mut notes) = file::read_table(path, INSTEAD);
        let settings = table.map_or_else(Self::default, |table| Self::read(table, &mut notes));
        (settings, notes)
    }

    /// Writes the settings to `path`, atomically.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written.
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        file::write(path, &self.to_toml())
    }

    /// Reads settings out of the text of a file.
    ///
    /// Never fails. A file that is not TOML at all is one note and the
    /// defaults; a key that cannot be applied is one note and its default.
    #[must_use]
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let (table, mut notes) = file::parse(text, INSTEAD);
        let settings = table.map_or_else(Self::default, |table| Self::read(table, &mut notes));
        (settings, notes)
    }

    /// The settings as the file's text.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut writing = Writing::new();
        writing.choice("theme", self.theme);
        writing.choice("face", self.face);
        writing.whole("size", self.size);
        writing.boolean("focus", self.focus);
        writing.choice("focus_scope", self.focus_scope);
        writing.boolean("typewriter", self.typewriter);
        writing.fraction("typewriter_anchor", self.typewriter_anchor);
        writing.choice("chrome", self.chrome);
        writing.boolean("spell_check", self.spell_check);
        writing.text("spell_language", &self.spell_language);
        writing.text("template", &self.template);
        writing.choice("preview_layout", self.preview_layout);
        writing.path("library", self.library.as_deref());
        writing.rest(self.rest.clone());
        writing.table("syntax_highlight", self.syntax_highlight.to_table());
        writing.table("style_check", self.style_check.to_table());
        writing.table("shortcuts", self.shortcuts.clone());
        writing.into_toml()
    }

    /// Reads a parsed table, key by key.
    fn read(table: toml::Table, notes: &mut Vec<String>) -> Self {
        let defaults = Self::default();
        let mut reading = Reading::new(table, "", notes);
        let theme = reading.choice("theme");
        let face = reading.choice("face");
        let size = reading.whole("size", defaults.size, &(SMALLEST..=LARGEST));
        let focus = reading.boolean("focus", defaults.focus);
        let focus_scope = reading.choice("focus_scope");
        let typewriter = reading.boolean("typewriter", defaults.typewriter);
        let typewriter_anchor = reading.fraction("typewriter_anchor", defaults.typewriter_anchor);
        let chrome = reading.choice("chrome");
        let spell_check = reading.boolean("spell_check", defaults.spell_check);
        let spell_language = reading.text("spell_language", &defaults.spell_language);
        let template = reading.text("template", &defaults.template);
        let preview_layout = reading.choice("preview_layout");
        let library = reading.path("library");
        // The tables are taken here and read below, once the reading of the
        // top level is done with the notes it is writing into.
        let syntax_highlight = reading.table("syntax_highlight");
        let style_check = reading.table("style_check");
        let shortcuts = reading.table("shortcuts");
        let rest = reading.rest();
        Self {
            theme,
            face,
            size,
            focus,
            focus_scope,
            typewriter,
            typewriter_anchor,
            chrome,
            spell_check,
            spell_language,
            syntax_highlight: SyntaxHighlight::read(syntax_highlight, notes),
            style_check: StyleCheck::read(style_check, notes),
            template,
            preview_layout,
            library,
            shortcuts,
            rest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::file::scratch;
    use super::*;

    /// Every key `docs/architecture.md`'s Settings section names.
    const KEYS: [&str; 16] = [
        "theme",
        "face",
        "size",
        "focus",
        "focus_scope",
        "typewriter",
        "typewriter_anchor",
        "chrome",
        "spell_check",
        "spell_language",
        "syntax_highlight",
        "style_check",
        "template",
        "preview_layout",
        "library",
        "shortcuts",
    ];

    #[test]
    fn the_defaults_write_every_key_the_architecture_names() {
        let written: toml::Table = Settings::default()
            .to_toml()
            .parse()
            .expect("what the defaults write is TOML");
        for key in KEYS {
            assert!(written.contains_key(key), "no `{key}` in:\n{written}");
        }
        assert_eq!(written.len(), KEYS.len(), "a key nobody named: {written}");
    }

    #[test]
    fn the_defaults_are_the_ones_the_architecture_names() {
        let settings = Settings::default();
        assert_eq!(settings.theme, Theme::Auto);
        assert_eq!(settings.face, Face::Duo);
        assert_eq!(settings.size, 20);
        assert_eq!(settings.focus_scope, FocusScope::Sentence);
        assert!((settings.typewriter_anchor - 0.5).abs() < f64::EPSILON);
        assert_eq!(settings.chrome, Chrome::Shown);
        assert!(settings.spell_check, "spell check is on by default");
        assert_eq!(settings.preview_layout, PreviewLayout::Split);
        assert_eq!(settings.library, None);
        assert!(!settings.focus && !settings.typewriter);
    }

    #[test]
    fn the_five_syntax_highlight_categories_are_on_under_a_master_that_is_off() {
        let syntax = SyntaxHighlight::default();
        assert!(!syntax.enabled);
        assert!(syntax.nouns && syntax.verbs && syntax.adjectives);
        assert!(syntax.adverbs && syntax.conjunctions);
    }

    #[test]
    fn what_is_written_reads_back_as_itself() {
        let (read, notes) = Settings::parse(&Settings::default().to_toml());
        assert_eq!(read, Settings::default());
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn writing_twice_writes_the_same_bytes() {
        // What "a relaunch leaves the file byte-identical" rests on.
        let first = Settings::default().to_toml();
        let (read, _) = Settings::parse(&first);
        assert_eq!(read.to_toml(), first);
    }

    #[test]
    fn a_hand_added_key_and_a_hand_added_table_survive_a_write() {
        let hand_edited = "\
size = 22
wayfinder = \"fog\"

[syntax_highlight]
enabled = true
proper_nouns = true

[style_check]
enabled = true
cliches = true
fillers = false

[shortcuts]
\"library.toggle\" = [\"F9\"]

[preview]
margin = 3
";
        let (settings, notes) = Settings::parse(hand_edited);
        assert!(notes.is_empty(), "nothing here is a complaint: {notes:?}");
        assert_eq!(settings.size, 22);
        assert!(settings.syntax_highlight.enabled && settings.style_check.enabled);
        assert_eq!(settings.style_check.lists.len(), 2, "the lists are carried");
        assert_eq!(settings.shortcuts.len(), 1, "the shortcuts are carried");

        let written: toml::Table = settings.to_toml().parse().expect("writes TOML");
        assert_eq!(written["wayfinder"].as_str(), Some("fog"));
        assert_eq!(written["preview"]["margin"].as_integer(), Some(3));
        assert_eq!(
            written["syntax_highlight"]["proper_nouns"].as_bool(),
            Some(true)
        );
        assert_eq!(written["style_check"]["cliches"].as_bool(), Some(true));
        assert_eq!(
            written["shortcuts"]["library.toggle"][0].as_str(),
            Some("F9")
        );
        assert_eq!(Settings::parse(&settings.to_toml()).0, settings);
    }

    #[test]
    fn a_file_missing_half_its_keys_keeps_the_half_it_has() {
        let (settings, notes) = Settings::parse("face = \"mono\"\nfocus = true\nsize = 24\n");
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(settings.face, Face::Mono);
        assert!(settings.focus);
        assert_eq!(settings.size, 24);
        // And everything absent is the default.
        assert_eq!(settings.theme, Theme::Auto);
        assert!(settings.spell_check);
        assert_eq!(settings.template, TEMPLATE);
        assert_eq!(settings.syntax_highlight, SyntaxHighlight::default());
    }

    #[test]
    fn a_value_that_is_not_one_of_the_names_keeps_the_default_and_says_why() {
        let (settings, notes) = Settings::parse("theme = \"purple\"\n");
        assert_eq!(settings.theme, Theme::Auto);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("auto, light, dark"), "{notes:?}");
    }

    #[test]
    fn a_file_that_is_not_toml_is_one_note_and_the_defaults() {
        let (settings, notes) = Settings::parse("theme = = = dark\n");
        assert_eq!(settings, Settings::default());
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("is not TOML"), "{notes:?}");
        assert!(!notes[0].contains('\n'), "one line, not a stack: {notes:?}");
    }

    #[test]
    fn every_choice_reads_back_every_value_it_writes() {
        assert_eq!(Theme::VALUES, ["auto", "light", "dark"]);
        assert_eq!(Face::VALUES, ["duo", "quattro", "mono"]);
        assert_eq!(FocusScope::VALUES, ["sentence", "paragraph"]);
        assert_eq!(Chrome::VALUES, ["shown", "hidden"]);
        assert_eq!(PreviewLayout::VALUES, ["split", "full"]);
        for value in Theme::VALUES {
            assert_eq!(Theme::parse(value).map(Theme::as_str), Some(*value));
        }
        for value in Face::VALUES {
            assert_eq!(Face::parse(value).map(Face::as_str), Some(*value));
        }
        assert_eq!(Theme::parse("Dark"), None, "the values are lower case");
    }

    #[test]
    fn each_face_names_the_two_families_it_is_built_from() {
        assert_eq!(Face::Duo.family(), "Quill Duo");
        assert_eq!(Face::Duo.italic_family(), "Quill Duo Italic");
        assert_eq!(Face::Quattro.family(), "Quill Quattro");
        assert_eq!(Face::Mono.family(), "Quill Mono");
        for face in [Face::Duo, Face::Quattro, Face::Mono] {
            assert_eq!(face.italic_family(), format!("{} Italic", face.family()));
        }
    }

    #[test]
    fn a_first_launch_writes_the_defaults_and_the_launch_after_it_reads_them_back() {
        let path = scratch("first_launch").join(SETTINGS_FILE);
        let (defaults, notes) = Settings::read_from(&path);
        assert!(
            notes.is_empty(),
            "a missing file is a first launch: {notes:?}"
        );
        defaults.write_to(&path).expect("writes the first file");
        let first = std::fs::read_to_string(&path).expect("reads it back");

        let (read, notes) = Settings::read_from(&path);
        assert_eq!(read, defaults);
        assert!(notes.is_empty(), "{notes:?}");
        read.write_to(&path).expect("writes it again");
        let second = std::fs::read_to_string(&path).expect("reads it back again");
        assert_eq!(first, second, "a relaunch leaves the file byte-identical");
    }

    #[test]
    fn a_failed_write_leaves_the_writers_file_as_it_was() {
        let path = scratch("failed_write").join(SETTINGS_FILE);
        Settings::default()
            .write_to(&path)
            .expect("writes the file");
        let before = std::fs::read_to_string(&path).expect("reads what was written");

        let changed = Settings {
            size: 24,
            ..Settings::default()
        };
        std::fs::create_dir(file::temporary(&path)).expect("stands in the way of the write");
        changed
            .write_to(&path)
            .expect_err("the write cannot finish");

        let after = std::fs::read_to_string(&path).expect("the file is still whole");
        assert_eq!(before, after);
        assert_eq!(Settings::parse(&after).0.size, SIZE);
    }
}
