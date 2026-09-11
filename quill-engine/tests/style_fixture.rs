//! The shipped lists have to strike the passage the whole feature is judged on.
//!
//! `ref/style.md` is one passage carrying a filler, a redundancy and a cliché
//! of every shape the spec names, and it is the same file three ways: the
//! engine's fixture here, the `style` Piece's Document, and the capture ticket's
//! state (#354). So this test is what keeps the three from drifting — a list
//! edit that stops striking `brass tacks`, or starts striking a word of the
//! heading, fails `cargo test` rather than turning up in a shot.
//!
//! It reads the three files through [`quill_engine::data::style`], which is the
//! resolution an installed Quill uses, so it also proves the checkout's copy is
//! readable and parses with no notes at all.

use std::fs;
use std::ops::Range;
use std::path::PathBuf;

use quill_engine::style::{List, Lists, struck};

/// Every phrase the passage is written to catch, in order: its byte range into
/// the file, the text struck, and the List that struck it.
///
/// A redundancy names the word it strikes, not the whole phrase — `together`
/// out of `combined together`, `past` out of `past history`. The ranges are
/// spelt out rather than searched for because two of these words appear twice
/// in the passage: the `down` struck is the one in `fell down`, and the `down`
/// of `get down to brass tacks` is ordinary prose.
const STRUCK: [(usize, usize, &str, List); 13] = [
    (15, 24, "Basically", List::Fillers),
    (39, 50, "pretty much", List::Fillers),
    (73, 80, "sort of", List::Fillers),
    (102, 113, "brass tacks", List::Cliches),
    (115, 131, "Against all odds", List::Cliches),
    (150, 158, "together", List::Redundancies),
    (169, 181, "fundamentals", List::Redundancies),
    (191, 195, "very", List::Fillers),
    (213, 233, "long and short of it", List::Cliches),
    (249, 257, "a little", List::Fillers),
    (277, 281, "down", List::Redundancies),
    (297, 301, "past", List::Redundancies),
    (314, 317, "too", List::Fillers),
];

#[test]
fn the_shipped_lists_strike_every_phrase_of_the_fixture_passage_and_nothing_else() {
    let passage = passage();
    let mut notes = Vec::new();
    let lists = Lists::load(&mut notes);
    assert!(
        notes.is_empty(),
        "the shipped lists read with nothing to note: {notes:?}"
    );

    let spans = struck(&passage, &lists);
    let marks: Vec<(usize, usize, &str, List)> = spans
        .iter()
        .map(|(range, list)| (range.start, range.end, &passage[range.clone()], *list))
        .collect();
    assert_eq!(
        marks, STRUCK,
        "the passage strikes exactly what `ref/style.md` was written to strike"
    );

    // The ranges address the passage itself, ascending and never overlapping,
    // which is what the Editor's tags are applied over.
    let mut taken: Range<usize> = 0..0;
    for (range, _) in &spans {
        assert!(
            range.start >= taken.end,
            "{range:?} overlaps {taken:?} in `ref/style.md`"
        );
        assert!(
            range.end <= passage.len(),
            "{range:?} is inside the passage"
        );
        taken = range.clone();
    }
}

#[test]
fn the_heading_and_the_ordinary_words_of_the_passage_are_left_alone() {
    let passage = passage();
    let mut notes = Vec::new();
    let lists = Lists::load(&mut notes);
    let struck: Vec<&str> = struck(&passage, &lists)
        .into_iter()
        .map(|(range, _)| &passage[range])
        .collect();
    for word in ["Style", "check", "plan", "team", "draft", "rough", "ran"] {
        assert!(
            !struck.contains(&word),
            "`{word}` is ordinary prose and must not be struck"
        );
    }
}

/// The fixture passage, read from the checkout the way the Piece reads it.
fn passage() -> String {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../ref/style.md"));
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{} is readable: {err}", path.display()))
}
