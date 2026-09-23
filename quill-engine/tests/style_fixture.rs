//! The shipped lists have to strike the passage the whole feature is judged on.
//!
//! `dev/ref/style.md` is one passage carrying a filler, a redundancy and a cliché
//! of every shape the spec names, and it is the same file three ways: the
//! engine's fixture here, the `style` Piece's Document, and the capture's own
//! state (#354). So this test is what keeps the three from drifting — a list
//! edit that stops striking `brass tacks`, or starts striking a word of the
//! heading, fails `cargo test` rather than turning up in a shot. Since the
//! capture landed, what it asserts is what iA itself struck in this passage.
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
/// **This table is the Design oracle's own marks**, read off iA Writer striking
/// this passage (#354, `dev/ref/ia/mac-native/style-354-read.json`), so a list edit
/// that drifts from what iA draws fails here. It says four things the lists
/// alone would not:
///
/// - `Basically,` carries its comma, which is the rendering rule in
///   [`struck`] and not a list entry; the full stop after `tacks` and the colon
///   after `it` stay outside their marks.
/// - `get down to brass tacks` is struck whole, not the bare `brass tacks`.
/// - a redundancy names the word it strikes — `together` out of `combined
///   together`, `basic` out of `basic fundamentals`, which is the word iA
///   strikes where its own marketing page bolds the other one.
/// - `past history` is the cliché's twelve characters rather than the
///   redundancy's four: the longest mark wins.
///
/// The one mark the oracle draws that we do not is `only` in `fell down only
/// where`, which no flat list can catch without striking every `only` on the
/// page (#354 § 3, recommendation 5). The ranges are spelt out rather than
/// searched for because `down` appears twice: the one struck is in `fell down`,
/// and the `down` of `get down to brass tacks` is inside a cliché's mark.
const STRUCK: [(usize, usize, &str, List); 13] = [
    (15, 25, "Basically,", List::Fillers),
    (39, 50, "pretty much", List::Fillers),
    (73, 80, "sort of", List::Fillers),
    (90, 113, "get down to brass tacks", List::Cliches),
    (115, 131, "Against all odds", List::Cliches),
    (150, 158, "together", List::Redundancies),
    (163, 168, "basic", List::Redundancies),
    (191, 195, "very", List::Fillers),
    (213, 233, "long and short of it", List::Cliches),
    (249, 257, "a little", List::Fillers),
    (277, 281, "down", List::Redundancies),
    (297, 309, "past history", List::Cliches),
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
        "the passage strikes exactly what `dev/ref/style.md` was written to strike"
    );

    // The ranges address the passage itself, ascending and never overlapping,
    // which is what the Editor's tags are applied over.
    let mut taken: Range<usize> = 0..0;
    for (range, _) in &spans {
        assert!(
            range.start >= taken.end,
            "{range:?} overlaps {taken:?} in `dev/ref/style.md`"
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
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../dev/ref/style.md"));
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{} is readable: {err}", path.display()))
}
