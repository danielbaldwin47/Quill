//! Building the table a settings or state file is written from.
//!
//! The mirror of [`super::reading`]: every key Quill knows is put in, and then
//! everything the reading did not recognise is put back, so a file written by a
//! newer Quill survives a launch of an older one. Two orderings are held here
//! rather than left to the serialiser. Keys come out in the order
//! `docs/architecture.md` lists them, because this file is meant to be read and
//! hand-edited. And every plain key comes out before the first table header,
//! because in TOML a key after a header belongs to that header's table: a file
//! written the other way round would not read back as itself.

use std::path::{Path, PathBuf};

use toml::Value;

use super::Choice;

/// A table being built, in two halves that are joined at the end.
#[derive(Default)]
pub struct Writing {
    /// Everything that writes as `key = value`.
    plain: toml::Table,
    /// Everything that writes under a `[header]`, which must come last.
    tabled: toml::Table,
}

impl Writing {
    /// A table with nothing in it yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// A `true`/`false` key.
    pub fn boolean(&mut self, key: &str, value: bool) {
        self.plain.insert(key.to_string(), Value::Boolean(value));
    }

    /// A whole number.
    pub fn whole(&mut self, key: &str, value: u32) {
        self.plain
            .insert(key.to_string(), Value::Integer(i64::from(value)));
    }

    /// A number from 0 to 1.
    pub fn fraction(&mut self, key: &str, value: f64) {
        self.plain.insert(key.to_string(), Value::Float(value));
    }

    /// A string key.
    pub fn text(&mut self, key: &str, value: &str) {
        self.plain
            .insert(key.to_string(), Value::String(value.to_string()));
    }

    /// One of the values a [`Choice`] takes.
    pub fn choice<C: Choice>(&mut self, key: &str, value: C) {
        self.text(key, value.as_str());
    }

    /// A path, or the empty string for none.
    ///
    /// A path that is not UTF-8 cannot be a TOML string; it is written as none,
    /// which is the same as being forgotten. Quill never invents a name for a
    /// writer's directory.
    pub fn path(&mut self, key: &str, value: Option<&Path>) {
        self.text(key, value.and_then(Path::to_str).unwrap_or_default());
    }

    /// A list of paths, skipping any that is not UTF-8.
    pub fn paths(&mut self, key: &str, values: &[PathBuf]) {
        let written: Vec<Value> = values
            .iter()
            .filter_map(|path| path.to_str())
            .map(|path| Value::String(path.to_string()))
            .collect();
        self.plain.insert(key.to_string(), Value::Array(written));
    }

    /// A table of its own, under its own header.
    pub fn table(&mut self, key: &str, table: toml::Table) {
        self.tabled.insert(key.to_string(), Value::Table(table));
    }

    /// A list of tables, each under a repeated header.
    pub fn tables(&mut self, key: &str, tables: Vec<toml::Table>) {
        if tables.is_empty() {
            // An empty list of tables writes as `key = []`, which is a plain
            // key and has to come before the headers like any other.
            self.plain.insert(key.to_string(), Value::Array(Vec::new()));
            return;
        }
        let written = tables.into_iter().map(Value::Table).collect();
        self.tabled.insert(key.to_string(), Value::Array(written));
    }

    /// Puts back everything the reading did not recognise.
    pub fn rest(&mut self, rest: toml::Table) {
        for (key, value) in rest {
            if writes_under_a_header(&value) {
                self.tabled.insert(key, value);
            } else {
                self.plain.insert(key, value);
            }
        }
    }

    /// The whole table, plain keys first.
    pub fn finish(mut self) -> toml::Table {
        self.plain.extend(self.tabled);
        self.plain
    }

    /// The file's text.
    pub fn into_toml(self) -> String {
        toml::to_string(&self.finish()).expect("a settings table is TOML by construction")
    }
}

/// Whether a value writes as `[header]` rather than as `key = value`.
fn writes_under_a_header(value: &Value) -> bool {
    match value {
        Value::Table(_) => true,
        Value::Array(entries) => {
            !entries.is_empty() && entries.iter().all(|entry| entry.is_table())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_come_out_before_the_headers_they_would_otherwise_fall_into() {
        let mut writing = Writing::new();
        writing.table("shortcuts", toml::Table::new());
        writing.whole("size", 20);
        // A plain key of a newer Quill's, and a table of one: the key has to
        // come out before both headers or it reads back as one of their own.
        writing.rest(
            "colour = \"blue\"\n\n[wayfinder]\nmap = \"fog\"\n"
                .parse()
                .expect("the test's own TOML parses"),
        );
        let text = writing.into_toml();
        let read: toml::Table = text.parse().expect("what was written reads back: {text}");
        assert_eq!(read["size"].as_integer(), Some(20), "{text}");
        assert_eq!(read["colour"].as_str(), Some("blue"), "{text}");
        assert!(read["shortcuts"].is_table(), "{text}");
    }

    #[test]
    fn a_list_of_tables_survives_a_list_of_keys_beside_it() {
        let mut writing = Writing::new();
        writing.tables("window", vec!["width = 1100".parse().unwrap()]);
        writing.paths("recents", &[PathBuf::from("/home/writer/one.md")]);
        let text = writing.into_toml();
        let read: toml::Table = text.parse().expect("what was written reads back");
        assert_eq!(read["window"].as_array().map(Vec::len), Some(1), "{text}");
        assert_eq!(read["recents"].as_array().map(Vec::len), Some(1), "{text}");
    }

    #[test]
    fn an_empty_list_of_tables_is_a_plain_key() {
        let mut writing = Writing::new();
        writing.tables("window", Vec::new());
        writing.table("caret", toml::Table::new());
        let text = writing.into_toml();
        let read: toml::Table = text.parse().expect("what was written reads back");
        assert_eq!(read["window"].as_array().map(Vec::len), Some(0), "{text}");
        assert!(
            read["caret"].as_table().is_some_and(toml::Table::is_empty),
            "{text}"
        );
    }

    #[test]
    fn a_path_that_is_not_utf_8_is_written_as_none_rather_than_as_a_guess() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let awkward = PathBuf::from(OsString::from_vec(vec![0xff, 0xfe]));
        let mut writing = Writing::new();
        writing.path("library", Some(&awkward));
        writing.paths(
            "recents",
            &[awkward.clone(), PathBuf::from("/home/writer/one.md")],
        );
        let text = writing.into_toml();
        let read: toml::Table = text.parse().expect("what was written reads back");
        assert_eq!(read["library"].as_str(), Some(""), "{text}");
        assert_eq!(read["recents"].as_array().map(Vec::len), Some(1), "{text}");
    }
}
