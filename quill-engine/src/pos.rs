//! Syntax highlight: Category spans over a paragraph's prose.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. It reads the prose stream and emits `(byte range, Category)`.
//! Universal part-of-speech tags stay inside this module. The tagger model
//! loads on the calling worker thread at first use (ADR 0018).

use std::ops::Range;

use harper_brill::{Tagger, UPOS};

/// One of the five independently switchable parts of speech Syntax colours.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Category {
    /// Common and proper nouns.
    Nouns,
    /// Verbs, including auxiliaries.
    Verbs,
    /// Adjectives.
    Adjectives,
    /// Adverbs.
    Adverbs,
    /// Coordinating and subordinating conjunctions.
    Conjunctions,
}

/// Colour a paragraph of prose, returning UTF-8 byte ranges within `prose`.
///
/// The caller supplies the prose stream, with Markdown and excluded content
/// already removed. Every range is one token; numbers, punctuation and
/// possessive suffixes remain uncoloured. Harper's process-wide `LazyLock`
/// deserialises its model once, on the first nonempty call, so callers put
/// this work on the Annotators' worker thread rather than the keystroke lane.
pub fn tag(prose: &str) -> Vec<(Range<usize>, Category)> {
    let tokens = tokenize(prose);
    if tokens.is_empty() {
        return Vec::new();
    }
    let words: Vec<String> = tokens
        .iter()
        .map(|range| prose[range.clone()].replace('’', "'"))
        .collect();
    let tags = harper_brill::brill_tagger().tag_sentence(&words);
    tokens
        .into_iter()
        .zip(words)
        .zip(tags)
        .filter_map(|((range, word), tag)| {
            // Punctuation and numbers still give the tagger context, but
            // never acquire ink even when its unknown-word guess is a noun.
            if !word.chars().any(char::is_alphabetic) {
                return None;
            }
            // The same spelling can be a possessive or contracted "is/has".
            // Only the tagger's auxiliary reading colours an 's suffix.
            if word.eq_ignore_ascii_case("'s") && tag != Some(UPOS::AUX) {
                return None;
            }
            category(tag).map(|category| (range, category))
        })
        .collect()
}

fn category(tag: Option<UPOS>) -> Option<Category> {
    match tag? {
        UPOS::NOUN | UPOS::PROPN => Some(Category::Nouns),
        UPOS::VERB | UPOS::AUX => Some(Category::Verbs),
        UPOS::ADJ => Some(Category::Adjectives),
        UPOS::ADV => Some(Category::Adverbs),
        UPOS::CCONJ | UPOS::SCONJ => Some(Category::Conjunctions),
        // Harper represents Universal X as None rather than an enum variant.
        UPOS::ADP
        | UPOS::DET
        | UPOS::INTJ
        | UPOS::NUM
        | UPOS::PART
        | UPOS::PRON
        | UPOS::PUNCT
        | UPOS::SYM => None,
    }
}

fn tokenize(prose: &str) -> Vec<Range<usize>> {
    let mut tokens = Vec::new();
    let mut chars = prose.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        let suffix = matches!(ch, '\'' | '’')
            && prose[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
            && chars.peek().is_some_and(|(_, ch)| ch.is_alphabetic());
        let mut end = start + ch.len_utf8();
        if ch.is_alphanumeric() || suffix {
            while let Some(&(index, next)) = chars.peek() {
                if next.is_alphanumeric() {
                    chars.next();
                    end = index + next.len_utf8();
                } else if next == '-'
                    && prose[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_alphanumeric)
                    && prose[index + 1..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphanumeric)
                {
                    chars.next();
                    end = index + 1;
                } else {
                    break;
                }
            }
        }
        tokens.push(start..end);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_keeps_suffixes_compounds_numbers_and_punctuation_separate() {
        let prose = "I'll can't rabbit-hole Alice's 1865, I’ll can’t Alice’s.";
        let words: Vec<_> = tokenize(prose).into_iter().map(|r| &prose[r]).collect();
        assert_eq!(
            words,
            [
                "I",
                "'ll",
                "can",
                "'t",
                "rabbit-hole",
                "Alice",
                "'s",
                "1865",
                ",",
                "I",
                "’ll",
                "can",
                "’t",
                "Alice",
                "’s",
                "."
            ]
        );
    }

    #[test]
    fn tokenizer_preserves_utf8_ranges_and_separates_quotes_and_dashes() {
        let prose = "‘Élodie’ — café-au-lait, naïve.";
        let words: Vec<_> = tokenize(prose).into_iter().map(|r| &prose[r]).collect();
        assert_eq!(
            words,
            ["‘", "Élodie", "’", "—", "café-au-lait", ",", "naïve", "."]
        );
        assert!(tag(" \n\t").is_empty());
    }

    #[test]
    fn universal_tags_map_to_the_five_categories() {
        for (tags, expected) in [
            (&[UPOS::NOUN, UPOS::PROPN][..], Category::Nouns),
            (&[UPOS::VERB, UPOS::AUX][..], Category::Verbs),
            (&[UPOS::ADJ][..], Category::Adjectives),
            (&[UPOS::ADV][..], Category::Adverbs),
            (&[UPOS::CCONJ, UPOS::SCONJ][..], Category::Conjunctions),
        ] {
            for tag in tags {
                assert_eq!(category(Some(*tag)), Some(expected));
            }
        }
        for tag in [
            UPOS::ADP,
            UPOS::DET,
            UPOS::INTJ,
            UPOS::NUM,
            UPOS::PART,
            UPOS::PRON,
            UPOS::PUNCT,
            UPOS::SYM,
        ] {
            assert_eq!(category(Some(tag)), None);
        }
        assert_eq!(category(None), None);
    }

    #[test]
    fn tagging_never_colours_possessives_numbers_or_punctuation() {
        let prose = "Alice's rabbit-hole, 1865. Alice’s book!";
        let tokens = tokenize(prose);
        for (range, _) in tag(prose) {
            assert!(tokens.contains(&range));
            assert!(!["'s", "’s", ",", "1865", ".", "!"].contains(&&prose[range]));
        }
        assert!(tag("1865, 42! 3.14 — ☀").is_empty());
    }

    #[test]
    fn an_auxiliary_contraction_keeps_its_verb_colour() {
        for prose in ["He's happy.", "He’s happy."] {
            let suffix = 2..prose.find(' ').unwrap();
            assert!(tag(prose).contains(&(suffix, Category::Verbs)));
        }
    }

    #[test]
    fn accuracy_agrees_with_the_categories_visible_in_the_stills() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            passages: Vec<Passage>,
        }
        #[derive(serde::Deserialize)]
        struct Passage {
            name: String,
            still: String,
            text: String,
            shown: Vec<String>,
            coloured: Vec<Word>,
        }
        #[derive(serde::Deserialize)]
        struct Word {
            word: String,
            colour: String,
        }
        fn colour(name: &str) -> Category {
            match name {
                "red" | "Nouns" => Category::Nouns,
                "blue" | "Verbs" => Category::Verbs,
                "brown" | "Adjectives" => Category::Adjectives,
                "purple" | "Adverbs" => Category::Adverbs,
                "green" | "Conjunctions" => Category::Conjunctions,
                _ => panic!("unknown fixture colour or Category: {name}"),
            }
        }
        let fixture: Fixture =
            toml::from_str(include_str!("../tests/fixtures/pos-stills.toml")).unwrap();
        let mut correct = 0;
        let mut scored = 0;
        for passage in fixture.passages {
            let shown: Vec<_> = passage.shown.iter().map(|name| colour(name)).collect();
            assert!(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join(&passage.still)
                    .is_file()
            );
            let mut expected = Vec::new();
            let mut cursor = 0;
            for word in &passage.coloured {
                let start = cursor + passage.text[cursor..].find(&word.word).unwrap();
                let range = start..start + word.word.len();
                let category = colour(&word.colour);
                assert!(shown.contains(&category));
                assert!(
                    tokenize(&passage.text).contains(&range),
                    "fixture word is not a token: {}",
                    word.word
                );
                cursor = range.end;
                expected.push((range, category));
            }
            let mut passage_correct = 0;
            let mut passage_scored = 0;
            for (range, category) in tag(&passage.text) {
                if !shown.contains(&category) {
                    continue;
                }
                passage_scored += 1;
                if expected.contains(&(range.clone(), category)) {
                    passage_correct += 1;
                } else {
                    eprintln!(
                        "{}: {:?} is {:?}, still {:?}",
                        passage.name,
                        &passage.text[range.clone()],
                        category,
                        expected.iter().find(|(r, _)| *r == range).map(|(_, c)| c)
                    );
                }
            }
            assert!(
                passage_scored > 0,
                "{}: tagger coloured no words",
                passage.name
            );
            eprintln!(
                "{}: {passage_correct}/{passage_scored} coloured words agree",
                passage.name
            );
            correct += passage_correct;
            scored += passage_scored;
        }
        assert!(
            scored > 0 && correct * 100 >= scored * 86,
            "{correct}/{scored} coloured words agree; at least 86% required"
        );
    }
}
