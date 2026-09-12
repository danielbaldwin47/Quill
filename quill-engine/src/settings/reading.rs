//! Reading a TOML table into typed settings, one key at a time.
//!
//! Two rules run through every getter here, and they are the reason this is a
//! helper rather than a `serde` derive. **Nothing is fatal**: a key with the
//! wrong type or a value outside its range leaves the default in place and adds
//! one line to the notes, so a writer's typo costs them one setting and not
//! their session. **Nothing is lost**: every key that is read is *taken* out of
//! the table, so whatever is left at the end is exactly what this Quill did not
//! recognise — the unknown keys and tables a write has to put back.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use toml::Value;

use super::Choice;

/// A TOML table being read, and the notes the reading produced.
pub struct Reading<'a> {
    /// What is left of the table: shrinks as keys are read.
    table: toml::Table,
    /// What to call these keys in a note — `""` at the top level, and the
    /// table's own name and a dot inside one.
    within: &'static str,
    /// One line per key that could not be applied.
    notes: &'a mut Vec<String>,
}

impl<'a> Reading<'a> {
    /// Reads `table`, whose keys are named `within` in any note.
    pub fn new(table: toml::Table, within: &'static str, notes: &'a mut Vec<String>) -> Self {
        Self {
            table,
            within,
            notes,
        }
    }

    /// A `true`/`false` key.
    pub fn boolean(&mut self, key: &str, default: bool) -> bool {
        match self.take(key) {
            None => default,
            Some(value) => value.as_bool().unwrap_or_else(|| {
                self.wrong(key, &value, "true or false", &default.to_string());
                default
            }),
        }
    }

    /// A whole number inside `range`; anything else is a typo, not a
    /// preference, and keeps the default.
    pub fn whole(&mut self, key: &str, default: u32, range: &RangeInclusive<u32>) -> u32 {
        let Some(value) = self.take(key) else {
            return default;
        };
        let number = value
            .as_integer()
            .and_then(|number| u32::try_from(number).ok())
            .filter(|number| range.contains(number));
        number.unwrap_or_else(|| {
            let wanted = format!("a whole number from {} to {}", range.start(), range.end());
            self.wrong(key, &value, &wanted, &default.to_string());
            default
        })
    }

    /// A number from 0 to 1. A writer who writes `1` rather than `1.0` means
    /// the same thing, so an integer is read as one too.
    pub fn fraction(&mut self, key: &str, default: f64) -> f64 {
        let Some(value) = self.take(key) else {
            return default;
        };
        let number = value
            .as_float()
            .or_else(|| value.as_integer().map(|number| number as f64))
            .filter(|number| (0.0..=1.0).contains(number));
        number.unwrap_or_else(|| {
            self.wrong(key, &value, "a number from 0 to 1", &default.to_string());
            default
        })
    }

    /// A string key.
    pub fn text(&mut self, key: &str, default: &str) -> String {
        match self.take(key) {
            None => default.to_string(),
            Some(value) => value.as_str().map_or_else(
                || {
                    self.wrong(key, &value, "some text", default);
                    default.to_string()
                },
                str::to_string,
            ),
        }
    }

    /// One of the values a [`Choice`] takes.
    pub fn choice<C: Choice>(&mut self, key: &str) -> C {
        let Some(value) = self.take(key) else {
            return C::default();
        };
        let chosen = value.as_str().and_then(C::parse);
        chosen.unwrap_or_else(|| {
            let wanted = format!("one of {}", C::VALUES.join(", "));
            self.wrong(key, &value, &wanted, C::default().as_str());
            C::default()
        })
    }

    /// A directory or file a writer named, or `None` when the key is empty.
    ///
    /// Empty is how "no Library yet" and "no Document yet" are written: the key
    /// stays in the file where a writer can see it, holding nothing. A
    /// leading `~/` is the home directory ([`under_home`]), because that is
    /// how a hand writes a path in a file it will carry between machines,
    /// and how the README's `palette` step writes one.
    pub fn path(&mut self, key: &str) -> Option<PathBuf> {
        let text = self.text(key, "");
        (!text.is_empty()).then(|| under_home(&text, std::env::home_dir().as_deref()))
    }

    /// A list of paths, skipping any entry that is not a string.
    ///
    /// A leading `~/` is the home directory in every entry, as it is in
    /// [`Reading::path`]: the Locations and the Pinned list are written by
    /// hand as readily as the single `library` path they replaced.
    pub fn paths(&mut self, key: &str) -> Vec<PathBuf> {
        let Some(value) = self.take(key) else {
            return Vec::new();
        };
        let Some(entries) = value.as_array() else {
            self.wrong(key, &value, "a list of paths", "nothing");
            return Vec::new();
        };
        let home = std::env::home_dir();
        entries
            .iter()
            .filter_map(|entry| entry.as_str())
            .filter(|path| !path.is_empty())
            .map(|path| under_home(path, home.as_deref()))
            .collect()
    }

    /// A list of the names of one of Quill's own sets, or `None` when the key
    /// is absent or is not a list at all.
    ///
    /// `None` rather than an empty list, because a set has a default and an
    /// empty list is not it: `[stats]`'s `show` defaults to three Statistics
    /// and `show = []` is a writer asking for none of them, and the two have
    /// to be told apart. Entries that are not strings are skipped, as they are
    /// in [`Reading::paths`].
    ///
    /// What a name may be is the caller's set, not this module's: the caller
    /// parses each and notes what it could not place with [`Reading::dropped`].
    pub fn names(&mut self, key: &str) -> Option<Vec<String>> {
        let value = self.take(key)?;
        let Some(entries) = value.as_array() else {
            self.wrong(key, &value, "a list of names", "the default");
            return None;
        };
        Some(
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
        )
    }

    /// A table of its own, empty when the key is absent, so that the caller
    /// reads it with a [`Reading`] of its own or carries it as data.
    pub fn table(&mut self, key: &str) -> toml::Table {
        match self.take(key) {
            None => toml::Table::new(),
            Some(Value::Table(table)) => table,
            Some(value) => {
                self.wrong(key, &value, "a table", "an empty one");
                toml::Table::new()
            }
        }
    }

    /// Every list of tables under `key`, empty when the key is absent.
    pub fn tables(&mut self, key: &str) -> Vec<toml::Table> {
        let Some(value) = self.take(key) else {
            return Vec::new();
        };
        let Some(entries) = value.as_array() else {
            self.wrong(key, &value, "a list of tables", "nothing");
            return Vec::new();
        };
        entries
            .iter()
            .filter_map(|entry| entry.as_table())
            .cloned()
            .collect()
    }

    /// Everything the reading did not recognise, to be written back untouched.
    pub fn rest(self) -> toml::Table {
        self.table
    }

    /// Takes `key` out of the table, so what is left is what is unknown.
    fn take(&mut self, key: &str) -> Option<Value> {
        self.table.remove(key)
    }

    /// Notes one entry of a list ([`Reading::names`]) that names nothing Quill
    /// knows, and so is dropped while the rest of the list stands.
    ///
    /// A list is the writer's to type into, and an entry Quill cannot place is
    /// no reason to refuse the others: `show = ["sentences", "nonsense"]` is a
    /// writer who wants Sentences and mistyped something (#387).
    pub fn dropped(&mut self, key: &str, entry: &str, wanted: &str) {
        let within = self.within;
        self.notes.push(format!(
            "{within}{key}: \"{entry}\" is not {wanted}; dropping it"
        ));
    }

    /// Notes a value that could not be applied, in one line a writer can act
    /// on: what they wrote, what the key takes, and what Quill is using
    /// instead.
    fn wrong(&mut self, key: &str, value: &Value, wanted: &str, kept: &str) {
        let found = match value.as_str() {
            Some(text) => format!("\"{text}\""),
            None => value.type_str().to_string(),
        };
        let within = self.within;
        self.notes.push(format!(
            "{within}{key}: {found} is not {wanted}; keeping {kept}"
        ));
    }
}

/// `text` as a path, with a leading `~` — alone, or followed by `/` — standing
/// for `home`.
///
/// Only the writer's own home: `~name/` is left as written, because guessing
/// another account's home directory is a shell's business. With no home
/// directory to expand against, the text is the path.
fn under_home(text: &str, home: Option<&Path>) -> PathBuf {
    match (text.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) if rest.starts_with('/') => home.join(&rest[1..]),
        _ => PathBuf::from(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every getter, against a table that has each key at the wrong type.
    #[test]
    fn a_wrong_type_keeps_the_default_and_says_so() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(
            table("focus = 3\nsize = \"big\"\ntypewriter_anchor = 4\nspell_language = 7\n"),
            "",
            &mut notes,
        );
        assert!(!reading.boolean("focus", false));
        assert_eq!(reading.whole("size", 20, &(6..=200)), 20);
        assert!((reading.fraction("typewriter_anchor", 0.5) - 0.5).abs() < f64::EPSILON);
        assert_eq!(reading.text("spell_language", "en"), "en");
        assert_eq!(notes.len(), 4, "one note each: {notes:?}");
        assert!(
            notes[0].starts_with("focus: integer is not true or false"),
            "{notes:?}"
        );
        assert!(
            notes[1].contains("\"big\""),
            "the note quotes what was written: {notes:?}"
        );
    }

    #[test]
    fn a_leading_tilde_is_the_home_directory_and_only_the_writers_own() {
        let home = Path::new("/home/writer");
        assert_eq!(
            under_home("~/x/quill.toml", Some(home)),
            PathBuf::from("/home/writer/x/quill.toml")
        );
        assert_eq!(under_home("~", Some(home)), PathBuf::from("/home/writer"));
        assert_eq!(
            under_home("~other/x", Some(home)),
            PathBuf::from("~other/x"),
            "another account's home is not guessed at"
        );
        assert_eq!(
            under_home("/etc/quill.toml", Some(home)),
            PathBuf::from("/etc/quill.toml")
        );
        assert_eq!(
            under_home("~/x", None),
            PathBuf::from("~/x"),
            "with no home directory the text is the path"
        );
    }

    #[test]
    fn a_missing_key_takes_the_default_in_silence() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(toml::Table::new(), "", &mut notes);
        assert!(reading.boolean("focus", true));
        assert_eq!(reading.whole("size", 20, &(6..=200)), 20);
        assert_eq!(reading.text("template", "default"), "default");
        assert_eq!(reading.path("library"), None);
        assert!(reading.paths("recents").is_empty());
        assert!(reading.table("shortcuts").is_empty());
        assert!(
            notes.is_empty(),
            "a missing key is not worth a line: {notes:?}"
        );
    }

    #[test]
    fn a_number_outside_its_range_is_a_typo_not_a_preference() {
        let mut notes = Vec::new();
        let mut reading =
            Reading::new(table("size = 0\ntypewriter_anchor = 2.5\n"), "", &mut notes);
        assert_eq!(reading.whole("size", 20, &(6..=200)), 20);
        assert!((reading.fraction("typewriter_anchor", 0.5) - 0.5).abs() < f64::EPSILON);
        assert_eq!(notes.len(), 2, "{notes:?}");
    }

    #[test]
    fn a_whole_number_is_read_as_a_fraction() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(table("typewriter_anchor = 1\n"), "", &mut notes);
        assert!((reading.fraction("typewriter_anchor", 0.5) - 1.0).abs() < f64::EPSILON);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn what_was_never_read_is_what_is_left() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(
            table("size = 22\nfrobnicate = true\n\n[wayfinder]\nmap = \"fog\"\n"),
            "",
            &mut notes,
        );
        assert_eq!(reading.whole("size", 20, &(6..=200)), 22);
        let rest = reading.rest();
        assert_eq!(rest.len(), 2, "{rest:?}");
        assert!(rest.contains_key("frobnicate") && rest.contains_key("wayfinder"));
    }

    #[test]
    fn a_note_inside_a_table_names_the_table_it_is_in() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(table("nouns = 3\n"), "syntax_highlight.", &mut notes);
        assert!(!reading.boolean("nouns", false));
        assert!(notes[0].starts_with("syntax_highlight.nouns:"), "{notes:?}");
    }

    #[test]
    fn a_list_of_paths_skips_what_is_not_a_path() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(
            table("recents = [\"/home/writer/one.md\", 4, \"\", \"/home/writer/two.md\"]\n"),
            "",
            &mut notes,
        );
        assert_eq!(
            reading.paths("recents"),
            [
                PathBuf::from("/home/writer/one.md"),
                PathBuf::from("/home/writer/two.md")
            ]
        );
    }

    #[test]
    fn a_list_of_names_skips_what_is_not_a_name_and_an_absent_key_is_no_list() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(
            table("show = [\"words\", 4, \"\", \"sentences\"]\nbroken = 4\n"),
            "stats.",
            &mut notes,
        );
        assert_eq!(
            reading.names("show").as_deref(),
            Some(["words".to_string(), "sentences".to_string()].as_slice())
        );
        assert_eq!(
            reading.names("absent"),
            None,
            "an absent key leaves the caller its default"
        );
        assert_eq!(
            reading.names("broken"),
            None,
            "and so does a key that is not a list at all"
        );
        assert_eq!(
            notes,
            ["stats.broken: integer is not a list of names; keeping the default"]
        );
    }

    #[test]
    fn a_name_that_is_not_one_of_a_set_is_dropped_by_name() {
        let mut notes = Vec::new();
        let mut reading = Reading::new(table(""), "stats.", &mut notes);
        reading.dropped("show", "nonsense", "one of words, sentences");
        assert_eq!(
            notes,
            ["stats.show: \"nonsense\" is not one of words, sentences; dropping it"]
        );
    }

    fn table(text: &str) -> toml::Table {
        text.parse().expect("the test's own TOML parses")
    }
}
