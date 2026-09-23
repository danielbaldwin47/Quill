//! The Spell check seam against the fixture dictionary, `dev/ref/spell/`, read through the temporary
//! copy [`fixture`] makes, so no test reads the machine's dictionaries or writes the checkout.

mod spell_fixture;

use std::fs;
use std::path::Path;

use quill_engine::markdown;
use quill_engine::spell::{self, Enchant, Position, SUGGESTIONS, SpellChecker};
use spell_fixture::fixture;

/// The passage's seven marked misspellings, as the capture reads them off the Design oracle
/// (`dev/ref/ia/mac-native/NOTES.md` § State 26 § What is marked).
///
/// `DRAFFT` is misspelled and is not here: an all-caps word the dictionary does not hold is left
/// alone, as `2b` and `Q3` are, and [`quill_engine::spell::words`] drops both kinds before the
/// dictionary is asked.
const MISSPELLINGS: [&str; 7] = [
    "definately",
    "recieved",
    "comittee",
    "Teh",
    "seperate",
    "mispelled",
    "accomodate",
];

#[test]
fn check_holds_a_fixture_word_and_rejects_a_misspelling() {
    let checker = en_us();
    assert!(checker.check("committee"));
    assert!(!checker.check("recieve"));
}

#[test]
fn suggest_offers_the_correction_and_at_most_five() {
    let suggestions = en_us().suggest("recieve");
    assert!(
        suggestions.iter().any(|word| word == "received"),
        "`received` is among {suggestions:?}"
    );
    assert!(suggestions.len() <= SUGGESTIONS);
}

#[test]
fn an_added_word_is_held_and_written_to_the_personal_dictionary() {
    let mut checker = en_us();
    assert!(!checker.check("quillish"));
    checker.add("quillish");
    assert!(checker.check("quillish"));
    let personal = fs::read_to_string(fixture().join("en_US.dic")).unwrap_or_default();
    assert!(
        personal.lines().any(|line| line == "quillish"),
        "the copy's en_US.dic lists the added word, not {personal:?}"
    );
}

#[test]
fn an_ignored_word_is_held_and_nothing_is_written() {
    let mut checker = en_us();
    assert!(!checker.check("quillesque"));
    checker.ignore("quillesque");
    assert!(checker.check("quillesque"));
    let personal = fs::read_to_string(fixture().join("en_US.dic")).unwrap_or_default();
    assert!(!personal.contains("quillesque"));
    assert!(Enchant::new("en_US").is_some_and(|fresh| !fresh.check("quillesque")));
}

#[test]
fn the_installed_languages_contain_the_fixtures_tag() {
    fixture();
    assert!(
        spell::installed_languages()
            .iter()
            .any(|tag| tag == "en_US")
    );
    assert!(en_us().languages().iter().any(|tag| tag == "en_US"));
}

#[test]
fn a_tag_no_provider_serves_is_no_dictionary() {
    fixture();
    assert!(Enchant::new("xx_XX").is_none());
}

#[test]
fn the_word_character_rule_takes_letters_and_a_mid_word_apostrophe() {
    let checker = en_us();
    assert!(checker.is_word_character('a', Position::Start));
    assert!(checker.is_word_character('\'', Position::Middle));
    assert!(!checker.is_word_character(' ', Position::Middle));
}

#[test]
fn the_passage_misspellings_fail_and_its_other_words_pass() {
    let checker = en_us();
    let passage =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../dev/ref/spell.md"))
            .expect("dev/ref/spell.md is readable");
    let words: Vec<&str> = passage
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    for misspelling in MISSPELLINGS {
        assert!(
            words.contains(&misspelling),
            "`{misspelling}` is in the passage"
        );
        assert!(!checker.check(misspelling), "`{misspelling}` is misspelled");
    }
    // The two kinds `spell::words` never asks about are not "correct" — `DRAFFT` is misspelled and
    // unmarked, and `2b` and `Q3` are not spellings at all.
    let correct: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| {
            !MISSPELLINGS.contains(word)
                && !word.chars().any(|c| c.is_ascii_digit())
                && word.chars().any(char::is_lowercase)
        })
        .collect();
    assert!(
        correct.len() >= 12,
        "a dozen correct words, not {correct:?}"
    );
    for word in correct {
        assert!(checker.check(word), "`{word}` is held");
    }
}

#[test]
fn the_prose_stream_marks_exactly_the_passage_s_seven_misspellings() {
    let checker = en_us();
    let passage =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../dev/ref/spell.md"))
            .expect("dev/ref/spell.md is readable");
    let marked: Vec<&str> = markdown::prose(&passage)
        .iter()
        .flat_map(|prose| {
            spell::misspelled(prose.text, &checker)
                .into_iter()
                .map(|word| &passage[prose.at.start + word.start..prose.at.start + word.end])
        })
        .collect();
    assert_eq!(marked, MISSPELLINGS);
}

/// The fixture's `en_US`, loaded under the temporary copy.
fn en_us() -> Enchant {
    fixture();
    Enchant::new("en_US").expect("the fixture serves en_US")
}
