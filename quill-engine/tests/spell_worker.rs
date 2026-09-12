//! The worker seam for Spell check against the fixture dictionary: the third span set, the
//! shared dictionary handle and the edits, read through the temporary copy [`fixture`] makes.

mod spell_fixture;

use std::fs;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::{Duration, Instant};

use quill_engine::pos::Category;
use quill_engine::spell::Misspelling;
use quill_engine::style::List;
use quill_engine::worker::{Annotators, Edit, Paragraph, ParagraphResult, Request, Worker};
use spell_fixture::fixture;

const SPELL: Annotators = Annotators {
    syntax: false,
    style: false,
    spell: true,
};

#[test]
fn a_request_wanting_spell_alone_answers_misspellings_and_leaves_the_other_sets_empty() {
    let mut worker = english();
    worker
        .request(wanting(1, "recieved the notes", SPELL))
        .unwrap();
    assert_eq!(
        receive(&worker),
        ParagraphResult {
            generation: 1,
            misspellings: vec![(0..8, Misspelling)],
            ..ParagraphResult::default()
        }
    );
}

#[test]
fn a_request_wanting_all_three_answers_all_three_from_one_paragraph() {
    let mut worker = english();
    let all = Annotators {
        syntax: true,
        style: true,
        spell: true,
    };
    let prose = "She basically recieved the notes.";
    worker.request(wanting(2, prose, all)).unwrap();
    let result = receive(&worker);
    assert!(
        result.categories.contains(&(27..32, Category::Nouns)),
        "`notes` is a noun in {:?}",
        result.categories
    );
    assert_eq!(result.lists, [(4..13, List::Fillers)]);
    // `basically` is not in the fixture, so Spell check marks it beside Style check's strike.
    assert_eq!(
        result.misspellings,
        [(4..13, Misspelling), (14..22, Misspelling)]
    );
}

#[test]
fn an_ignore_edit_sent_before_a_request_leaves_the_word_unmarked() {
    let mut worker = english();
    worker.edit(Edit::Ignore("recieved".into())).unwrap();
    worker
        .request(wanting(3, "recieved the notes", SPELL))
        .unwrap();
    assert!(receive(&worker).misspellings.is_empty());
}

#[test]
fn an_add_edit_sent_before_a_request_leaves_the_word_unmarked_and_writes_it() {
    let mut worker = english();
    worker.edit(Edit::Add("quillworthy".into())).unwrap();
    worker
        .request(wanting(4, "the quillworthy notes", SPELL))
        .unwrap();
    assert!(receive(&worker).misspellings.is_empty());
    let personal = fs::read_to_string(fixture().join("en_US.dic")).unwrap_or_default();
    assert!(
        personal.lines().any(|line| line == "quillworthy"),
        "the copy's en_US.dic lists the added word, not {personal:?}"
    );
}

#[test]
fn a_language_of_none_marks_nothing_and_empties_the_handle_until_a_tag_returns() {
    let mut worker = english();
    worker.request(wanting(5, "recieved", SPELL)).unwrap();
    assert_eq!(receive(&worker).misspellings, [(0..8, Misspelling)]);

    worker.edit(Edit::Language(None)).unwrap();
    worker.request(wanting(6, "recieved", SPELL)).unwrap();
    assert!(receive(&worker).misspellings.is_empty());
    assert!(worker.checker().lock().unwrap().is_none());

    worker.edit(Edit::Language(Some("en_US".into()))).unwrap();
    worker.request(wanting(7, "recieved", SPELL)).unwrap();
    assert_eq!(receive(&worker).misspellings, [(0..8, Misspelling)]);
    assert!(worker.checker().lock().unwrap().is_some());
}

#[test]
fn the_handle_is_empty_until_the_first_spell_request_and_then_suggests() {
    let mut worker = english();
    assert!(
        worker.checker().lock().unwrap().is_none(),
        "a language edit alone loads no dictionary"
    );
    worker.request(wanting(8, "recieved", SPELL)).unwrap();
    receive(&worker);
    let checker = worker.checker();
    let guard = checker.lock().unwrap();
    let suggestions = guard
        .as_deref()
        .expect("the first request wanting Spell check loaded the dictionary")
        .suggest("recieved");
    assert!(
        suggestions.iter().any(|word| word == "received"),
        "`received` is among {suggestions:?}"
    );
}

/// A worker handed the fixture's `en_US`, which it loads at first use.
fn english() -> Worker {
    fixture();
    let mut worker = Worker::default();
    worker
        .edit(Edit::Language(Some("en_US".into())))
        .expect("the worker starts");
    worker
}

fn wanting(generation: u64, prose: &str, wanted: Annotators) -> Request {
    Request {
        generation,
        wanted,
        paragraphs: vec![Paragraph {
            index: 0,
            prose: prose.into(),
        }],
        viewport: 0..1,
    }
}

/// The next answer, polled, since the channel's blocking receive is the worker's own.
fn receive(worker: &Worker) -> ParagraphResult {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match worker.try_recv() {
            Ok(result) => return result,
            Err(TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("the worker answers within ten seconds: {error:?}"),
        }
    }
}
