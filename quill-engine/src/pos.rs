//! Syntax highlight: a Category per word.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. It reads the prose stream and emits `(byte range, Category)`
//! spans. The part-of-speech tag stops here: everything downstream — the five
//! toggles, the five Roles, the flattening, the tests — is written in
//! [`Category`] alone, so a different tagger behind this seam changes nothing
//! but the accuracy figure. The tagger model loads on the worker thread at
//! first use.
//!
//! The tokenizer is Quill's own rather than the tagger crate's, because the
//! word Syntax highlight colours is the word iA colours: a contraction's
//! suffix is a word of its own (`'ll` is blue where `I` beside it is plain), a
//! hyphenated compound is one word taking one colour, and a possessive `'s` is
//! a word that takes no colour at all, so only the noun before it is red. The
//! tagger is handed the numbers and the punctuation too, because its patch
//! rules read the tokens on either side of the one they are deciding; it
//! colours neither.

use std::ops::Range;

use harper_brill::{Tagger, UPOS, brill_tagger};

/// The five parts of speech Syntax highlight colours.
///
/// The unit of every toggle, Role and test: the writer switches Adjectives on,
/// not `ADJ`, and the theme names `syntax_adjective`. Universal POS is the
/// tagger's vocabulary and never leaves this module.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Category {
    /// Nouns and proper nouns. Red.
    Nouns,
    /// Verbs and auxiliaries — iA colours `can` and `'ll` too. Blue.
    Verbs,
    /// Adjectives. Brown.
    Adjectives,
    /// Adverbs. Purple.
    Adverbs,
    /// Coordinating and subordinating conjunctions. Green.
    Conjunctions,
}

/// What Syntax highlight colours in one paragraph of prose: byte ranges into
/// `prose`, ascending, never overlapping, one per coloured word.
///
/// `prose` is the prose stream's text — the parser's `Text` events with
/// Markup, code spans, fenced code, URLs and front matter already removed — so
/// this function never sees a `#` or a `*` and never has to know it is
/// reading Markdown. Words it leaves uncoloured are absent rather than present
/// with no Category.
///
/// The model deserialises on the first call and is shared from then on, which
/// is why the worker thread makes that call and the keystroke lane does not.
pub fn categories(prose: &str) -> Vec<(Range<usize>, Category)> {
    let words = words(prose);
    if words.is_empty() {
        return Vec::new();
    }
    let sentence: Vec<String> = words
        .iter()
        .map(|word| prose[word.clone()].to_owned())
        .collect();
    let tags = brill_tagger().tag_sentence(&sentence);
    words
        .into_iter()
        .zip(tags)
        .filter_map(|(word, tag)| {
            if is_possessive(&prose[word.clone()]) {
                return None;
            }
            Some((word, category(tag?)?))
        })
        .collect()
}

/// Every token of `prose`, as byte ranges into it, ascending: words, numbers,
/// contraction suffixes, and one token per remaining non-space character.
///
/// The tagger sees all of them. Punctuation is not noise to a Brill tagger —
/// its patch rules ask what stands one or two places to the left — and dropping
/// it here would cost the tags that context.
fn words(prose: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = prose.char_indices().collect();
    let end_of = |at: usize| {
        let (offset, character) = chars[at];
        offset + character.len_utf8()
    };
    let mut words = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let (start, character) = chars[at];
        if character.is_whitespace() {
            at += 1;
        } else if is_apostrophe(character) {
            // A contraction splits at the apostrophe and the suffix is a token
            // of its own: `I'll` is `I` and `'ll`, and iA colours only the
            // second of them.
            let mut end = end_of(at);
            at += 1;
            while at < chars.len() && chars[at].1.is_alphabetic() {
                end = end_of(at);
                at += 1;
            }
            words.push(start..end);
        } else if character.is_alphanumeric() {
            let mut end = end_of(at);
            at += 1;
            loop {
                if at < chars.len() && chars[at].1.is_alphanumeric() {
                    end = end_of(at);
                    at += 1;
                } else if at + 1 < chars.len()
                    && chars[at].1 == '-'
                    && chars[at + 1].1.is_alphanumeric()
                {
                    // A hyphenated compound is one word taking one colour:
                    // `so-called` is one adjective, as iA's own still shows it.
                    // The hyphen has to be followed by more word for that, or
                    // an em-dash-as-hyphen would swallow the next sentence.
                    end = end_of(at + 1);
                    at += 2;
                } else {
                    break;
                }
            }
            words.push(start..end);
        } else {
            words.push(start..end_of(at));
            at += 1;
        }
    }
    words
}

/// The Category a Universal POS tag colours as, and the one place the tagger's
/// vocabulary is read. `None` is a word left in the body's own ink.
///
/// The match is exhaustive on purpose: a tagset that grows a tag has to be
/// answered here rather than silently colouring nothing. Universal POS's
/// seventeenth tag, `X`, is not among the crate's sixteen — a word it cannot
/// place arrives as no tag at all, which colours nothing either way.
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

/// Whether a token is a possessive `'s`, which takes no colour: `Alice's` is
/// one red noun and a plain suffix, never two red words.
///
/// `he's` is spelt the same way, and this rule costs that one the blue the
/// tagger would give it. Telling the two apart wants the sentence, and the
/// possessive is much the commoner of them in prose.
fn is_possessive(word: &str) -> bool {
    let mut characters = word.chars();
    match (characters.next(), characters.next(), characters.next()) {
        (Some(first), Some('s' | 'S'), None) => is_apostrophe(first),
        _ => false,
    }
}

/// The two characters prose writes an apostrophe with: the typewriter one, and
/// the right single quote a smart-quote pass leaves behind.
fn is_apostrophe(character: char) -> bool {
    character == '\'' || character == '\u{2019}'
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens of `prose`, as text, which is what the rules below are about.
    fn tokens(prose: &str) -> Vec<&str> {
        words(prose).into_iter().map(|word| &prose[word]).collect()
    }

    /// The Category of each coloured word, as `(word, Category)`.
    fn coloured(prose: &str) -> Vec<(&str, Category)> {
        categories(prose)
            .into_iter()
            .map(|(word, category)| (&prose[word], category))
            .collect()
    }

    #[test]
    fn a_contraction_splits_at_the_apostrophe_and_the_suffix_is_a_token_of_its_own() {
        assert_eq!(tokens("I'll"), ["I", "'ll"]);
        assert_eq!(tokens("can't"), ["can", "'t"]);
        assert_eq!(tokens("I\u{2019}ll"), ["I", "\u{2019}ll"]);
    }

    #[test]
    fn a_hyphenated_compound_is_one_token() {
        assert_eq!(tokens("rabbit-hole"), ["rabbit-hole"]);
        assert_eq!(tokens("so-called centre"), ["so-called", "centre"]);
        // A dash with no word after it is punctuation, not a joint.
        assert_eq!(tokens("well- known"), ["well", "-", "known"]);
    }

    #[test]
    fn a_possessive_is_its_own_token_after_the_noun() {
        assert_eq!(tokens("Alice's"), ["Alice", "'s"]);
    }

    #[test]
    fn a_number_and_a_comma_are_tokens_with_no_category() {
        assert_eq!(tokens("in 1865, she"), ["in", "1865", ",", "she"]);
        let coloured = coloured("The book was printed in 1865, she said.");
        assert!(
            !coloured
                .iter()
                .any(|(word, _)| *word == "1865" || *word == ","),
            "a number and a comma take no colour, but this did: {coloured:?}"
        );
    }

    #[test]
    fn a_possessive_takes_no_category_and_the_noun_before_it_does() {
        let coloured = coloured("Alice's sister was reading a book.");
        assert!(
            !coloured.iter().any(|(word, _)| *word == "'s"),
            "the possessive took a colour: {coloured:?}"
        );
        assert_eq!(
            coloured.iter().find(|(word, _)| *word == "Alice"),
            Some(&("Alice", Category::Nouns)),
            "the noun before the possessive is red: {coloured:?}"
        );
    }

    #[test]
    fn an_auxiliary_is_a_verb_and_a_proper_noun_is_a_noun() {
        assert_eq!(category(UPOS::AUX), Some(Category::Verbs));
        assert_eq!(category(UPOS::VERB), Some(Category::Verbs));
        assert_eq!(category(UPOS::PROPN), Some(Category::Nouns));
        assert_eq!(category(UPOS::NOUN), Some(Category::Nouns));
    }

    #[test]
    fn a_subordinating_conjunction_is_a_conjunction_and_a_determiner_is_nothing() {
        assert_eq!(category(UPOS::SCONJ), Some(Category::Conjunctions));
        assert_eq!(category(UPOS::CCONJ), Some(Category::Conjunctions));
        assert_eq!(category(UPOS::DET), None);
    }

    #[test]
    fn the_uncoloured_tags_are_uncoloured() {
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
            assert_eq!(category(tag), None, "{tag:?} colours nothing");
        }
    }

    #[test]
    fn a_coloured_span_is_one_token() {
        for (span, _) in categories("The so-called centre of the earth.") {
            let word = &"The so-called centre of the earth."[span.clone()];
            assert!(
                !word.contains(' '),
                "a span covered more than one word: {word:?} at {span:?}"
            );
        }
    }

    #[test]
    fn empty_prose_has_no_spans() {
        assert!(categories("").is_empty());
        assert!(categories("   \n  ").is_empty());
    }

    #[test]
    fn the_spans_ascend_and_never_overlap() {
        let prose = "Alice was beginning to get very tired of sitting by her sister.";
        let mut previous = 0;
        for (span, _) in categories(prose) {
            assert!(
                span.start >= previous,
                "spans ascend, but {span:?} follows a span ending at {previous}"
            );
            previous = span.end;
        }
    }
}
