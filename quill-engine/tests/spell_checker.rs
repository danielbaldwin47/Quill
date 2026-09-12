//! The Spell check seam against the fixture dictionary, `ref/spell/`, read through a temporary
//! copy so no test reads the machine's dictionaries or writes the checkout.
//!
//! `ENCHANT_CONFIG_DIR` is process-wide and enchant reads it when a broker is made, so every
//! test here first calls [`fixture`], which copies the fixture and sets the variable once, before
//! the first broker. That is why these tests live in their own test binary: the variable is set
//! while no other test in the process can be reading the environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use quill_engine::markdown;
use quill_engine::spell::{self, Enchant, Position, SUGGESTIONS, SpellChecker};

/// The passage's eight misspellings, as #400 lists them.
const MISSPELLINGS: [&str; 8] = [
    "definately",
    "recieved",
    "comittee",
    "Teh",
    "DRAFFT",
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
    let passage = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ref/spell.md"))
        .expect("ref/spell.md is readable");
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
    let correct: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| !MISSPELLINGS.contains(word) && !word.chars().any(|c| c.is_ascii_digit()))
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
fn the_prose_stream_marks_exactly_the_passage_s_eight_misspellings() {
    let checker = en_us();
    let passage = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ref/spell.md"))
        .expect("ref/spell.md is readable");
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

/// The temporary copy of `ref/spell/`, made and pointed at once per test process.
fn fixture() -> &'static Path {
    static COPY: OnceLock<PathBuf> = OnceLock::new();
    COPY.get_or_init(|| {
        let copy = std::env::temp_dir().join(format!("quill-spell-{}", std::process::id()));
        let _ = fs::remove_dir_all(&copy);
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ref/spell/hunspell");
        let target = copy.join("hunspell");
        fs::create_dir_all(&target).expect("the copy's hunspell directory is made");
        for name in ["en_US.aff", "en_US.dic"] {
            fs::copy(source.join(name), target.join(name)).expect("the fixture pair copies");
        }
        // SAFETY: set once, under the `OnceLock`, before any broker reads it; this binary's only
        // other environment readers are these tests, each of which waits on this lock first.
        unsafe { std::env::set_var("ENCHANT_CONFIG_DIR", &copy) };
        copy
    })
}
