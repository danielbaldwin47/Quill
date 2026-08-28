//! Quill writes nothing into the writer's folder.
//!
//! [ADR 0002](../../docs/adr/0002-plain-markdown-documents.md) makes the file
//! the only source of truth: a Document's folder holds Documents and nothing
//! of Quill's — no index, no dot-file, no sidecar. Everything Quill remembers
//! is under the two XDG directories, and this test is the one that watches both
//! ends of that at once, with a Document opened and both files written.
//!
//! One test in this file, deliberately: it sets the two XDG variables, and a
//! second test running beside it would read them from under itself.

use std::fs;
use std::path::{Path, PathBuf};

use quill_engine::document::Document;
use quill_engine::settings::{Settings, State};

#[test]
fn opening_a_document_writes_nothing_beside_it_and_nothing_under_the_library() {
    let scratch = scratch();
    let library = scratch.join("Library");
    let document = library.join("chapter").join("one.md");
    fs::create_dir_all(document.parent().expect("the Document is in a folder"))
        .expect("makes a Library to open from");
    fs::write(&document, "# One\n\nThe lamp had been lit.\n").expect("writes the Document");

    // SAFETY: this test binary holds one test, so nothing else is reading the
    // environment while it is set.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", scratch.join("config"));
        std::env::set_var("XDG_STATE_HOME", scratch.join("state"));
    }

    let opened = Document::open(&document).expect("opens the Document");
    assert_eq!(opened.text(), "# One\n\nThe lamp had been lit.\n");
    let (settings, notes) = Settings::open();
    assert!(notes.is_empty(), "{notes:?}");
    let (state, notes) = State::open();
    assert!(notes.is_empty(), "{notes:?}");
    state.store().expect("writes the state file on the way out");

    // Both files are where the ADR puts them, under the writer's own name for
    // those directories.
    assert!(Settings::path().starts_with(scratch.join("config")));
    assert!(State::path().starts_with(scratch.join("state")));
    assert!(
        Settings::path().is_file(),
        "the first launch writes settings"
    );
    assert!(State::path().is_file(), "quitting writes state");
    assert_eq!(Settings::read_from(&Settings::path()).0, settings);

    // And the Library is exactly as it was left.
    assert_eq!(names_in(&library), ["chapter"]);
    assert_eq!(names_in(&library.join("chapter")), ["one.md"]);
    assert_eq!(names_in(&scratch), ["Library", "config", "state"]);
}

/// An empty directory of this test's own.
fn scratch() -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("quill-nothing-beside-{}", std::process::id()));
    fs::remove_dir_all(&directory).ok();
    fs::create_dir_all(&directory).expect("makes its own scratch directory");
    directory
}

/// What is in `directory`, sorted, so that anything left behind shows up.
fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("reads a directory this test made")
        .map(|entry| {
            entry
                .expect("reads an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
