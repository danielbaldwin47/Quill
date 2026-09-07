//! Syntax highlight: Category spans over a paragraph's prose.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. It reads the prose stream and emits `(byte range, Category)`
//! spans. Universal POS tags never leave this module. The tagger model loads
//! once, at first use, rather than at application startup.

use std::ops::Range;
use std::sync::{Arc, LazyLock};

use harper_brill::{BrillTagger, FreqDict, Tagger, UPOS};

static TAGGER: LazyLock<Arc<BrillTagger<FreqDict>>> = LazyLock::new(harper_brill::brill_tagger);

/// The five kinds of prose Syntax highlight can colour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    /// Common and proper nouns.
    Nouns,
    /// Lexical verbs and auxiliaries.
    Verbs,
    /// Adjectives.
    Adjectives,
    /// Adverbs.
    Adverbs,
    /// Coordinating and subordinating conjunctions.
    Conjunctions,
}

#[derive(Debug, Eq, PartialEq)]
struct Token {
    range: Range<usize>,
    text: String,
}

/// Tag a paragraph of prose and return one span for each coloured word.
///
/// `paragraph` is already the parser's prose stream. Markdown markers, code,
/// URLs and front matter must be removed before this seam is called.
pub fn category_spans(paragraph: &str) -> Vec<(Range<usize>, Category)> {
    let tokens = tokenize(paragraph);
    let words: Vec<String> = tokens.iter().map(|token| token.text.clone()).collect();
    let tags = TAGGER.tag_sentence(&words);

    tokens
        .into_iter()
        .zip(tags)
        .filter_map(|(token, tag)| {
            if !can_be_coloured(&token.text) {
                return None;
            }
            category_for(tag).map(|category| (token.range, category))
        })
        .collect()
}

fn can_be_coloured(token: &str) -> bool {
    token.chars().any(char::is_alphabetic) && !token.eq_ignore_ascii_case("'s")
}

fn category_for(tag: Option<UPOS>) -> Option<Category> {
    match tag {
        Some(UPOS::NOUN | UPOS::PROPN) => Some(Category::Nouns),
        Some(UPOS::VERB | UPOS::AUX) => Some(Category::Verbs),
        Some(UPOS::ADJ) => Some(Category::Adjectives),
        Some(UPOS::ADV) => Some(Category::Adverbs),
        Some(UPOS::CCONJ | UPOS::SCONJ) => Some(Category::Conjunctions),
        Some(
            UPOS::ADP
            | UPOS::DET
            | UPOS::INTJ
            | UPOS::NUM
            | UPOS::PART
            | UPOS::PRON
            | UPOS::PUNCT
            | UPOS::SYM,
        )
        | None => None,
    }
}

fn tokenize(paragraph: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < paragraph.len() {
        let ch = paragraph[cursor..]
            .chars()
            .next()
            .expect("cursor is in bounds");
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }

        let start = cursor;
        if ch.is_alphanumeric() {
            cursor += ch.len_utf8();
            while cursor < paragraph.len() {
                let next = paragraph[cursor..]
                    .chars()
                    .next()
                    .expect("cursor is in bounds");
                if next.is_alphanumeric() {
                    cursor += next.len_utf8();
                    continue;
                }
                if next == '-'
                    && paragraph[cursor + next.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphanumeric)
                {
                    cursor += next.len_utf8();
                    continue;
                }
                break;
            }
        } else if ch == '\''
            && paragraph[cursor + ch.len_utf8()..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric)
        {
            cursor += ch.len_utf8();
            while cursor < paragraph.len() {
                let next = paragraph[cursor..]
                    .chars()
                    .next()
                    .expect("cursor is in bounds");
                if !next.is_alphanumeric() {
                    break;
                }
                cursor += next.len_utf8();
            }
        } else {
            cursor += ch.len_utf8();
        }

        tokens.push(Token {
            range: start..cursor,
            text: paragraph[start..cursor].to_owned(),
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    type FixturePassage<'a> = (&'a str, Vec<Category>, Vec<(&'a str, usize, Category)>);

    #[test]
    fn tokenizer_keeps_quills_word_boundaries() {
        let text = "I'll can't rabbit-hole Alice's 1865,";
        let tokens = tokenize(text);
        let words: Vec<&str> = tokens.iter().map(|token| token.text.as_str()).collect();
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
                ","
            ]
        );
        for token in &tokens {
            assert_eq!(&text[token.range.clone()], token.text);
        }
    }

    #[test]
    fn numbers_punctuation_and_possessives_are_never_coloured() {
        let text = "Alice's 1865,";
        let spans = category_spans(text);
        assert!(
            spans
                .iter()
                .all(|(range, _)| &text[range.clone()] == "Alice")
        );
    }

    #[test]
    fn universal_tags_map_to_writer_categories() {
        assert_eq!(category_for(Some(UPOS::AUX)), Some(Category::Verbs));
        assert_eq!(category_for(Some(UPOS::PROPN)), Some(Category::Nouns));
        assert_eq!(
            category_for(Some(UPOS::SCONJ)),
            Some(Category::Conjunctions)
        );
        assert_eq!(category_for(Some(UPOS::DET)), None);
        // Harper represents the Universal X tag as no UPOS value.
        assert_eq!(category_for(None), None);
    }

    #[test]
    fn agrees_with_the_syntax_highlight_stills() {
        let fixture = include_str!("pos-accuracy.fixture");
        let mut passages: HashMap<&str, FixturePassage<'_>> = HashMap::new();

        for line in fixture
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let fields: Vec<&str> = line.split('|').collect();
            match fields.as_slice() {
                ["passage", name, text] => {
                    passages.insert(name, (text, Vec::new(), Vec::new()));
                }
                ["categories", name, categories] => {
                    passages
                        .get_mut(name)
                        .expect("passage precedes categories")
                        .1 = categories.split(',').map(parse_category).collect();
                }
                ["word", name, word, occurrence, category] => {
                    passages
                        .get_mut(name)
                        .expect("passage precedes words")
                        .2
                        .push((
                            word,
                            occurrence.parse().expect("numeric occurrence"),
                            parse_category(category),
                        ));
                }
                _ => panic!("bad fixture line: {line}"),
            }
        }

        let mut agreed = 0;
        let mut coloured = 0;
        for (text, visible, expected_words) in passages.values() {
            let tokens = tokenize(text);
            let mut expected = HashMap::new();
            for (word, occurrence, category) in expected_words {
                let range = nth_word_range(&tokens, word, *occurrence);
                expected.insert((range.start, range.end), *category);
            }
            for (range, category) in category_spans(text) {
                if visible.contains(&category) {
                    coloured += 1;
                    agreed +=
                        usize::from(expected.get(&(range.start, range.end)) == Some(&category));
                }
            }
        }

        assert!(coloured > 0, "the tagger coloured no fixture words");
        let accuracy = agreed as f64 / coloured as f64;
        let percent = accuracy * 100.0;
        assert!(
            accuracy >= 0.86,
            "Category agreement was {percent:.1}% ({agreed}/{coloured}), below 86%"
        );
    }

    fn parse_category(name: &str) -> Category {
        match name {
            "Nouns" => Category::Nouns,
            "Verbs" => Category::Verbs,
            "Adjectives" => Category::Adjectives,
            "Adverbs" => Category::Adverbs,
            "Conjunctions" => Category::Conjunctions,
            _ => panic!("unknown Category: {name}"),
        }
    }

    fn nth_word_range(tokens: &[Token], word: &str, occurrence: usize) -> Range<usize> {
        tokens
            .iter()
            .filter(|token| token.text.eq_ignore_ascii_case(word))
            .nth(occurrence - 1)
            .unwrap_or_else(|| panic!("fixture word {word} occurrence {occurrence} is absent"))
            .range
            .clone()
    }
}
