//! The tagger has to colour prose the way iA Writer colours it.
//!
//! The shipped Brill model is a lexicon plus 201 rewrite rules, and no figure
//! it was trained to matters here: what matters is whether a writer reading
//! Quill and a writer reading iA see the same words in the same colours. So the
//! test scores against iA's own screenshots — `syntax-accuracy.toml`, word by
//! word — rather than against a treebank, and the floor is the spec's 86 %
//! (#310 § Implementation Decisions, Accuracy acceptance). A tagger that falls
//! under it fails `cargo test` rather than shipping quietly.
//!
//! Scored in both directions and only over the Categories a still had switched
//! on: a word iA colours and we leave plain counts against us exactly as a word
//! we colour and iA leaves plain does. Both stills have Nouns off, so red is
//! not scored at all — the gap is the fixture's, not the tagger's, and it
//! closes when a still with Nouns on turns up.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;

use quill_engine::pos::{Category, categories};

/// The share of scored words that must agree with iA (#310).
const FLOOR: f64 = 0.86;

#[derive(serde::Deserialize)]
struct Fixture {
    passage: Vec<Passage>,
}

#[derive(serde::Deserialize)]
struct Passage {
    name: String,
    still: String,
    shows: Vec<String>,
    text: String,
    coloured: Vec<Coloured>,
}

#[derive(serde::Deserialize)]
struct Coloured {
    at: usize,
    word: String,
    category: String,
}

#[test]
fn the_tagger_agrees_with_ia_on_the_words_its_stills_colour() {
    let fixture = fixture();
    let mut scored = 0usize;
    let mut agreed = 0usize;
    let mut report = String::new();

    for passage in &fixture.passage {
        let shows: HashSet<Category> = passage
            .shows
            .iter()
            .map(|name| category_named(name))
            .collect();
        let ia = expected(passage, &shows);
        let quill = quill_colours(passage, &shows);

        let mut offsets: BTreeSet<usize> = ia.keys().copied().collect();
        offsets.extend(quill.keys().copied());
        let (mut passage_scored, mut passage_agreed) = (0usize, 0usize);
        for offset in offsets {
            let theirs = ia.get(&offset).copied();
            let ours = quill.get(&offset).copied();
            passage_scored += 1;
            if theirs.map(|(category, _)| category) == ours.map(|(category, _)| category) {
                passage_agreed += 1;
                continue;
            }
            report.push_str(&format!(
                "\n  {} @{offset:<4} {:<12} iA {:<13} Quill {}",
                passage.name,
                format!("{:?}", theirs.or(ours).map(|(_, word)| word).unwrap_or("")),
                describe(theirs.map(|(category, _)| category)),
                describe(ours.map(|(category, _)| category)),
            ));
        }
        report.push_str(&format!(
            "\n  {}: {passage_agreed}/{passage_scored} ({})",
            passage.name, passage.still
        ));
        scored += passage_scored;
        agreed += passage_agreed;
    }

    assert!(
        scored > 0,
        "the fixture scored no words, so this test proved nothing"
    );
    let rate = agreed as f64 / scored as f64;
    // The figure is the point of the test, so `--nocapture` prints it whether
    // it passes or fails; what the floor buys is that a fall is a failure.
    println!(
        "Syntax highlight agrees with iA on {agreed} of {scored} scored words ({:.1} %):{report}",
        rate * 100.0
    );
    assert!(
        rate >= FLOOR,
        "the tagger agrees with iA on {agreed} of {scored} scored words ({:.1} %), under the \
         {:.0} % floor #310 sets:{report}",
        rate * 100.0,
        FLOOR * 100.0
    );
}

/// What iA coloured, by byte offset into the passage, and the word it is.
///
/// The fixture's word is checked against the passage as it is read, so a
/// mistyped offset fails here rather than quietly counting as a disagreement.
fn expected<'a>(
    passage: &'a Passage,
    shows: &HashSet<Category>,
) -> BTreeMap<usize, (Category, &'a str)> {
    passage
        .coloured
        .iter()
        .map(|coloured| {
            let category = category_named(&coloured.category);
            let end = coloured.at + coloured.word.len();
            assert_eq!(
                passage.text.get(coloured.at..end),
                Some(coloured.word.as_str()),
                "{}: the fixture puts {:?} at byte {}, and the passage does not",
                passage.name,
                coloured.word,
                coloured.at
            );
            assert!(
                shows.contains(&category),
                "{}: the fixture colours {:?} {category:?}, which the still does not show on",
                passage.name,
                coloured.word
            );
            (coloured.at, (category, coloured.word.as_str()))
        })
        .collect()
}

/// What Quill colours, by byte offset, over the Categories the still shows on.
fn quill_colours<'a>(
    passage: &'a Passage,
    shows: &HashSet<Category>,
) -> BTreeMap<usize, (Category, &'a str)> {
    categories(&passage.text)
        .into_iter()
        .filter(|(_, category)| shows.contains(category))
        .map(|(span, category)| (span.start, (category, &passage.text[span])))
        .collect()
}

fn describe(category: Option<Category>) -> String {
    category.map_or_else(|| "body ink".to_owned(), |category| format!("{category:?}"))
}

/// The [`Category`] a fixture line names, by the five names the fixture writes.
fn category_named(category: &str) -> Category {
    match category {
        "Nouns" => Category::Nouns,
        "Verbs" => Category::Verbs,
        "Adjectives" => Category::Adjectives,
        "Adverbs" => Category::Adverbs,
        "Conjunctions" => Category::Conjunctions,
        other => panic!("{other:?} is not one of the five Categories"),
    }
}

fn fixture() -> Fixture {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/syntax-accuracy.toml");
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{path} is readable: {err}"));
    toml::from_str(&text).unwrap_or_else(|err| panic!("{path} is a readable fixture: {err}"))
}
