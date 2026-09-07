//! Syntax highlight: Category spans over prose words.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. It reads the prose stream and emits byte ranges paired with
//! Categories; the tagger's UPOS values stay inside this module. The model
//! loads once, on first use, behind a lazy cell.

use std::ops::Range;
use std::sync::{Arc, LazyLock};

use harper_brill::{BrillTagger, FreqDict, Tagger, UPOS};

static TAGGER: LazyLock<Arc<BrillTagger<FreqDict>>> = LazyLock::new(harper_brill::brill_tagger);

/// A part of speech that Syntax highlight can colour.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

/// Tags one paragraph of prose and returns the byte range and Category of each
/// word Syntax highlight can colour.
///
/// The input is prose, after Markdown markers, code, URLs and front matter have
/// been removed. Returned ranges are UTF-8 byte offsets into `paragraph`.
#[must_use]
pub fn category_spans(paragraph: &str) -> Vec<(Range<usize>, Category)> {
    let tokens = tokenize(paragraph);
    let words = tokens
        .iter()
        .map(|token| token.text.to_owned())
        .collect::<Vec<_>>();

    TAGGER
        .tag_sentence(&words)
        .into_iter()
        .zip(tokens)
        .filter_map(|(tag, token)| {
            if !token.can_take_category() {
                return None;
            }

            Some((token.range, category(tag?)?))
        })
        .collect()
}

fn category(tag: UPOS) -> Option<Category> {
    match tag {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Word,
    Suffix,
    Number,
    Punctuation,
}

#[derive(Debug, Eq, PartialEq)]
struct Token<'a> {
    range: Range<usize>,
    text: &'a str,
    kind: TokenKind,
}

impl Token<'_> {
    fn can_take_category(&self) -> bool {
        match self.kind {
            TokenKind::Word => true,
            TokenKind::Suffix => !matches!(self.text, "'s" | "’s" | "'S" | "’S"),
            TokenKind::Number | TokenKind::Punctuation => false,
        }
    }
}

fn tokenize(paragraph: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut chars = paragraph.char_indices().peekable();

    while let Some((start, character)) = chars.next() {
        if character.is_whitespace() {
            continue;
        }

        if character.is_alphanumeric() {
            let mut end = start + character.len_utf8();
            let mut has_letter = character.is_alphabetic();

            while let Some(&(index, next)) = chars.peek() {
                let joins_hyphenated_word = next == '-'
                    && chars
                        .clone()
                        .nth(1)
                        .is_some_and(|(_, after)| after.is_alphanumeric());
                if !next.is_alphanumeric() && !joins_hyphenated_word {
                    break;
                }

                chars.next();
                end = index + next.len_utf8();
                has_letter |= next.is_alphabetic();
            }

            tokens.push(Token {
                range: start..end,
                text: &paragraph[start..end],
                kind: if has_letter {
                    TokenKind::Word
                } else {
                    TokenKind::Number
                },
            });
            continue;
        }

        if matches!(character, '\'' | '’')
            && tokens.last().is_some_and(|token: &Token<'_>| {
                token.range.end == start && token.kind == TokenKind::Word
            })
        {
            let mut end = start + character.len_utf8();
            while let Some(&(index, next)) = chars.peek() {
                if !next.is_alphabetic() {
                    break;
                }
                chars.next();
                end = index + next.len_utf8();
            }

            if end > start + character.len_utf8() {
                tokens.push(Token {
                    range: start..end,
                    text: &paragraph[start..end],
                    kind: TokenKind::Suffix,
                });
                continue;
            }
        }

        let end = start + character.len_utf8();
        tokens.push(Token {
            range: start..end,
            text: &paragraph[start..end],
            kind: TokenKind::Punctuation,
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quill_tokenizer_keeps_the_boundaries_syntax_highlight_needs() {
        let text = "I'll can't rabbit-hole Alice's 1865,";
        let actual = tokenize(text)
            .into_iter()
            .map(|token| (token.text, token.kind))
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            [
                ("I", TokenKind::Word),
                ("'ll", TokenKind::Suffix),
                ("can", TokenKind::Word),
                ("'t", TokenKind::Suffix),
                ("rabbit-hole", TokenKind::Word),
                ("Alice", TokenKind::Word),
                ("'s", TokenKind::Suffix),
                ("1865", TokenKind::Number),
                (",", TokenKind::Punctuation),
            ]
        );
    }

    #[test]
    fn possessive_suffix_numbers_and_punctuation_have_no_category() {
        let text = "Alice's 1865,";
        let spans = category_spans(text);

        assert_eq!(
            spans,
            [(0..5, Category::Nouns)],
            "only Alice should be coloured: {spans:?}"
        );
    }

    #[test]
    fn universal_tags_map_to_writer_facing_categories() {
        let text = "Alice will sing because the rabbit listens.";
        let spans = category_spans(text);
        let tagged = spans
            .iter()
            .map(|(range, category)| (&text[range.clone()], *category))
            .collect::<Vec<_>>();

        assert!(tagged.contains(&("Alice", Category::Nouns)));
        assert!(tagged.contains(&("will", Category::Verbs)));
        assert!(tagged.contains(&("because", Category::Conjunctions)));
        assert!(tagged.contains(&("rabbit", Category::Nouns)));
        assert!(!tagged.iter().any(|(word, _)| *word == "the"));
    }
}
