use std::collections::{BTreeMap, BTreeSet};

use quill_engine::pos::{Category, category_spans};
use serde::Deserialize;

const FIXTURE: &str = include_str!("fixtures/pos-accuracy.toml");

#[derive(Deserialize)]
struct Fixture {
    passages: Vec<Passage>,
}

#[derive(Deserialize)]
struct Passage {
    name: String,
    still: String,
    text: String,
    shown_categories: Vec<String>,
    coloured: Vec<ColouredWord>,
}

#[derive(Deserialize)]
struct ColouredWord {
    word: String,
    category: String,
}

#[test]
fn tagger_agrees_with_the_ia_stills_on_at_least_86_percent_of_coloured_words() {
    let fixture: Fixture = toml::from_str(FIXTURE).expect("accuracy fixture should parse");
    let mut correct = 0;
    let mut compared = 0;
    let mut disagreements = Vec::new();

    for passage in fixture.passages {
        let shown = passage
            .shown_categories
            .iter()
            .map(|name| fixture_category(name))
            .collect::<BTreeSet<_>>();
        let expected = expected_spans(&passage);
        let actual = category_spans(&passage.text)
            .into_iter()
            .filter(|(_, category)| shown.contains(category))
            .map(|(range, category)| ((range.start, range.end), category))
            .collect::<BTreeMap<_, _>>();
        let ranges = expected
            .keys()
            .chain(actual.keys())
            .copied()
            .collect::<BTreeSet<_>>();

        for range in ranges {
            compared += 1;
            if expected.get(&range) == actual.get(&range) {
                correct += 1;
            } else {
                disagreements.push(format!(
                    "{} ({}) {:?}: expected {:?}, got {:?}",
                    passage.name,
                    passage.still,
                    &passage.text[range.0..range.1],
                    expected.get(&range),
                    actual.get(&range)
                ));
            }
        }
    }

    assert!(compared > 0, "fixture should contain coloured words");
    assert!(
        correct * 100 >= compared * 86,
        "tagger agreed on {correct}/{compared} words ({:.1}%):\n{}",
        correct as f64 / compared as f64 * 100.0,
        disagreements.join("\n")
    );
}

fn expected_spans(passage: &Passage) -> BTreeMap<(usize, usize), Category> {
    let mut cursor = 0;
    let mut spans = BTreeMap::new();

    for expected in &passage.coloured {
        let relative = passage.text[cursor..]
            .find(&expected.word)
            .unwrap_or_else(|| {
                panic!(
                    "{} should contain {:?} after byte {cursor}",
                    passage.name, expected.word
                )
            });
        let start = cursor + relative;
        let end = start + expected.word.len();
        spans.insert((start, end), fixture_category(&expected.category));
        cursor = end;
    }

    spans
}

fn fixture_category(name: &str) -> Category {
    match name {
        "Nouns" => Category::Nouns,
        "Verbs" => Category::Verbs,
        "Adjectives" => Category::Adjectives,
        "Adverbs" => Category::Adverbs,
        "Conjunctions" => Category::Conjunctions,
        other => panic!("unknown fixture Category {other:?}"),
    }
}
