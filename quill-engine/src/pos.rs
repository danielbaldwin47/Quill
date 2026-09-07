//! Syntax highlight: byte-range Category spans over paragraph prose.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. Tags stay private to this module. The caller supplies prose,
//! with Markdown removed, and schedules this work off the keystroke lane.
//! Harper's process-wide lazy cell deserialises the model on the first call.

use std::ops::Range;

use harper_brill::{Tagger, UPOS};

/// The five independently switchable Syntax highlight Categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    /// Common and proper nouns.
    Nouns,
    /// Verbs and auxiliaries.
    Verbs,
    /// Adjectives.
    Adjectives,
    /// Adverbs.
    Adverbs,
    /// Coordinating and subordinating conjunctions.
    Conjunctions,
}

/// Tag paragraph prose, returning one span per coloured token in byte order.
/// Ranges address the supplied UTF-8 string, not the original Markdown.
pub fn tag(prose: &str) -> Vec<(Range<usize>, Category)> {
    let tokens = tokenize(prose);
    if tokens.is_empty() {
        return Vec::new();
    }
    // Normalise curly apostrophes for the model without changing byte ranges.
    let words: Vec<_> = tokens
        .iter()
        .map(|range| prose[range.clone()].replace('’', "'"))
        .collect();
    let tags = harper_brill::brill_tagger().tag_sentence(&words);
    tokens
        .into_iter()
        .zip(tags)
        .enumerate()
        .filter_map(|(index, (range, tag))| {
            let word = &words[index];
            // Punctuation and numbers remain context for the tagger, never ink.
            // Treat 's after a noun as possession; pronoun contractions retain
            // the model's decision (it's, she's, who's). Ambiguous noun + is
            // also stays plain: possession takes precedence in this seam.
            if !word.chars().any(char::is_alphabetic)
                || (word.eq_ignore_ascii_case("'s")
                    && index > 0
                    && !matches!(
                        words[index - 1].to_ascii_lowercase().as_str(),
                        "it" | "he" | "she" | "that" | "there" | "here" | "what" | "who"
                    ))
            {
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
    let mut chars = prose.char_indices().peekable();
    let mut tokens = Vec::new();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        let mut end = start + ch.len_utf8();
        let suffix = matches!(ch, '\'' | '’')
            && start > 0
            && prose[..start].ends_with(char::is_alphanumeric);
        if ch.is_alphanumeric() || suffix {
            while let Some(&(at, next)) = chars.peek() {
                let internal_hyphen =
                    next == '-' && prose[at + 1..].starts_with(char::is_alphanumeric);
                if !next.is_alphanumeric() && !internal_hyphen {
                    break;
                }
                chars.next();
                end = at + next.len_utf8();
            }
        }
        tokens.push(start..end);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn token_boundaries_preserve_contractions_compounds_and_possessives() {
        let text = "I'll can't rabbit-hole Alice's 1865, I’ll café.";
        let words: Vec<_> = tokenize(text).into_iter().map(|r| &text[r]).collect();
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
                "café",
                "."
            ]
        );
    }

    #[test]
    fn mapping_keeps_only_the_five_categories() {
        for (tags, expected) in [
            (&[UPOS::NOUN, UPOS::PROPN][..], Some(Category::Nouns)),
            (&[UPOS::VERB, UPOS::AUX][..], Some(Category::Verbs)),
            (&[UPOS::ADJ][..], Some(Category::Adjectives)),
            (&[UPOS::ADV][..], Some(Category::Adverbs)),
            (
                &[UPOS::CCONJ, UPOS::SCONJ][..],
                Some(Category::Conjunctions),
            ),
            (
                &[
                    UPOS::ADP,
                    UPOS::DET,
                    UPOS::INTJ,
                    UPOS::NUM,
                    UPOS::PART,
                    UPOS::PRON,
                    UPOS::PUNCT,
                    UPOS::SYM,
                ][..],
                None,
            ),
        ] {
            for &tag in tags {
                assert_eq!(category(Some(tag)), expected);
            }
        }
        // Harper represents the universal X tag as None.
        assert_eq!(category(None), None);
    }

    #[test]
    fn spans_leave_possessives_numbers_and_punctuation_plain() {
        let text = "Alice's rabbit-hole, 1865.";
        let spans = tag(text);
        assert!(spans.contains(&(0..5, Category::Nouns)));
        assert!(
            spans
                .iter()
                .all(|(r, _)| matches!(&text[r.clone()], "Alice" | "rabbit-hole"))
        );
        assert!(tag("").is_empty());
        assert!(tag("1865, !").is_empty());
    }

    #[test]
    fn prose_categories_include_auxiliaries_proper_nouns_and_subordinators() {
        let text = "Alice can sing because she is happy.";
        let words: Vec<_> = tag(text).into_iter().map(|(r, c)| (&text[r], c)).collect();
        assert!(words.contains(&("Alice", Category::Nouns)));
        assert!(words.contains(&("can", Category::Verbs)));
        assert!(words.contains(&("because", Category::Conjunctions)));
    }

    #[test]
    fn contraction_suffixes_keep_their_original_utf8_ranges() {
        for text in ["I'll go.", "I’ll go."] {
            let suffix_end = text.find(' ').unwrap();
            assert!(tag(text).contains(&(1..suffix_end, Category::Verbs)));
        }
    }

    #[derive(Deserialize)]
    struct Fixture {
        passage: Vec<Passage>,
    }

    #[derive(Deserialize)]
    struct Passage {
        still: String,
        text: String,
        enabled: Vec<String>,
        coloured: Vec<(String, String)>,
    }

    fn colour(category: Category) -> &'static str {
        match category {
            Category::Nouns => "red",
            Category::Verbs => "blue",
            Category::Adjectives => "brown",
            Category::Adverbs => "purple",
            Category::Conjunctions => "green",
        }
    }

    #[test]
    fn still_agreement_is_at_least_86_percent_of_every_coloured_word() {
        let fixture: Fixture = toml::from_str(include_str!("pos-stills.toml")).unwrap();
        let mut total = 0;
        let mut correct = 0;
        for passage in fixture.passage {
            // Locate repeated fixture words in reading order, independently of
            // model output. Unlisted tokens are plain in the still.
            let mut expected = Vec::new();
            let mut after = 0;
            for (word, colour) in passage.coloured {
                let start = after + passage.text[after..].find(&word).unwrap();
                after = start + word.len();
                assert!(passage.enabled.contains(&colour));
                assert!(tokenize(&passage.text).contains(&(start..after)));
                expected.push((start..after, colour));
            }
            let mut scored = 0;
            for (range, category) in tag(&passage.text) {
                let colour = colour(category);
                if !passage.enabled.iter().any(|enabled| enabled == colour) {
                    continue;
                }
                total += 1;
                scored += 1;
                if expected.iter().any(|(r, c)| *r == range && c == colour) {
                    correct += 1;
                } else {
                    eprintln!(
                        "{}: {:?} predicted {colour}",
                        passage.still, &passage.text[range]
                    );
                }
            }
            assert!(scored > 0, "{} yielded no coloured words", passage.still);
        }
        eprintln!("still agreement: {correct}/{total}");
        assert!(correct * 100 >= total * 86, "{correct}/{total} below 86%");
    }
}
