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
//! word Syntax highlight colours is the word iA colours, and
//! `ref/ia/mac-native/CAPTURE-ORIGINAL-MBP.md` § A contraction splits reads
//! that boundary off the running app, cell by cell, in five places at once: a
//! contraction's suffix is a word of its own (`'ll` is blue where `I` beside
//! it is plain), `n't` takes the `n` with it (`ca` blue, `n't` purple), a
//! hyphenated compound is **two** words either side of a plain hyphen (`well`
//! purple, `known` blue), and a `'s` is plain after a noun and a verb after
//! anything else (`writer's` red then plain, `it's` plain then blue). The
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

impl Category {
    /// This Category's bit in a [`Categories`] set.
    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// Which Categories are coloured.
///
/// A set rather than five `bool`s because every reader asks it the one
/// question — is this Category coloured? — and none asks after a particular
/// one. It is separate state from the master toggle (#310), so that a writer
/// who turns Syntax highlight off and on again finds the Categories they had
/// chosen: the master toggle is the absence of spans and not an empty set.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Categories(u8);

impl Categories {
    /// Nothing coloured: every tagged word paints the body's ink.
    pub const NONE: Self = Self(0);

    /// All five coloured, which is what the shipped `[syntax_highlight]` table
    /// says once the master toggle is on.
    pub const ALL: Self = Self::NONE
        .with(Category::Nouns)
        .with(Category::Verbs)
        .with(Category::Adjectives)
        .with(Category::Adverbs)
        .with(Category::Conjunctions);

    /// This set with `category` in it.
    #[must_use]
    pub const fn with(self, category: Category) -> Self {
        Self(self.0 | category.bit())
    }

    /// Whether `category` is coloured.
    #[must_use]
    pub const fn contains(self, category: Category) -> bool {
        self.0 & category.bit() != 0
    }

    /// Whether nothing at all is coloured.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl FromIterator<Category> for Categories {
    fn from_iter<I: IntoIterator<Item = Category>>(categories: I) -> Self {
        categories.into_iter().fold(Self::NONE, Self::with)
    }
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
        .enumerate()
        .filter_map(|(index, word)| {
            // `get` rather than an index: the tagger answers one tag per word
            // it was handed, and a build that ever answered short would drop
            // the tail here rather than panic on the worker thread.
            let tag = |at: usize| tags.get(at).copied().flatten();
            if is_possessive(&sentence[index], index.checked_sub(1).and_then(tag)) {
                return None;
            }
            if is_negation(&sentence[index]) {
                return Some((word, Category::Adverbs));
            }
            Some((word, category(tag(index)?)?))
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
        } else if is_nt(&chars, at) {
            // `n't` is one token and its `n` belongs to it rather than to the
            // word in front: `can't` is `ca` and `n't`, `won't` is `wo` and
            // `n't`. The Design oracle colours the two halves differently —
            // the stem a verb, the `n't` an adverb — so the boundary falls
            // where iA puts it (`CAPTURE-ORIGINAL-MBP.md` § A contraction
            // splits).
            words.push(start..end_of(at + 2));
            at += 3;
        } else if is_apostrophe(character) {
            // Every other contraction splits at the apostrophe and the suffix
            // is a token of its own: `I'll` is `I` and `'ll`, and iA colours
            // only the second of them.
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
            while at < chars.len() && chars[at].1.is_alphanumeric() && !is_nt(&chars, at) {
                end = end_of(at);
                at += 1;
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

/// Whether a token is a bare `'s`, the suffix that is a possessive after a
/// noun and a contracted verb after anything else.
///
/// The Design oracle draws both, in the same passage: `writer's` is a red noun
/// and a plain `'s`, and `it's` is a plain `it` and a **blue** `'s`
/// (`CAPTURE-ORIGINAL-MBP.md` § A contraction splits). What tells them apart
/// is the word in front, which is why [`is_possessive`] takes the tag of that
/// word rather than only these three characters.
fn is_bare_s(word: &str) -> bool {
    let mut characters = word.chars();
    match (characters.next(), characters.next(), characters.next()) {
        (Some(first), Some('s' | 'S'), None) => is_apostrophe(first),
        _ => false,
    }
}

/// Whether a `'s` is the possessive rather than a contracted verb, which is
/// decided by what stands in front of it: a noun owns, and everything else —
/// a pronoun, an adverb, a name the tagger could not place — contracts.
///
/// So `Alice's` is one red noun and a plain suffix, never two red words, and
/// `it's` keeps the blue the tagger gives its `'s`. The one this costs is a
/// possessive after a pronoun (`its` is spelt without the apostrophe, so that
/// is rarer than it sounds) and a contraction after a noun (`the writer's
/// gone`), which wants the sentence rather than the neighbour.
fn is_possessive(word: &str, before: Option<UPOS>) -> bool {
    is_bare_s(word) && matches!(before, Some(UPOS::NOUN | UPOS::PROPN))
}

/// Whether a token is the `n't` of a negated auxiliary, which the Design
/// oracle colours purple and the tagger does not colour at all.
///
/// `CAPTURE-ORIGINAL-MBP.md` § A contraction splits reads `can't` as a blue
/// `ca` and a **purple** `n't`. Universal POS calls `n't` a `PART`, which
/// [`category`] leaves in the body's ink, so the reading is put back here — a
/// measured word, not a tag — rather than in [`category`], which is the
/// tagger's whole vocabulary and no place for one word.
///
/// `not` written out is left where the tagger puts it, which is `PART` too and
/// so plain. No capture holds it, and a row is written on a capture.
fn is_negation(word: &str) -> bool {
    starts_nt(word.chars()) && word.chars().nth(3).is_none()
}

/// Whether `n't` starts at `at`, which is what [`words`] breaks a word before.
fn is_nt(chars: &[(usize, char)], at: usize) -> bool {
    starts_nt(chars[at..].iter().map(|&(_, character)| character))
}

/// Whether the first three characters are the `n` of a negated auxiliary, the
/// apostrophe and the `t` — the one place that shape is spelt, so the boundary
/// [`words`] cuts on and the token [`is_negation`] colours cannot drift apart.
fn starts_nt(mut characters: impl Iterator<Item = char>) -> bool {
    matches!(
        (characters.next(), characters.next(), characters.next()),
        (Some('n' | 'N'), Some(second), Some('t' | 'T')) if is_apostrophe(second)
    )
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
        assert_eq!(tokens("I\u{2019}ll"), ["I", "\u{2019}ll"]);
    }

    #[test]
    fn a_negated_auxiliary_keeps_its_n_with_the_t() {
        assert_eq!(tokens("can't"), ["ca", "n't"]);
        assert_eq!(tokens("won't"), ["wo", "n't"]);
        assert_eq!(tokens("isn't"), ["is", "n't"]);
        assert_eq!(tokens("can\u{2019}t"), ["ca", "n\u{2019}t"]);
        // A lone `n` before an apostrophe that is not a `t` is an ordinary
        // suffix, and a word ending in `n` is left whole.
        assert_eq!(tokens("Ann's"), ["Ann", "'s"]);
        assert_eq!(tokens("then"), ["then"]);
    }

    #[test]
    fn a_hyphenated_compound_is_two_tokens_around_a_plain_hyphen() {
        assert_eq!(tokens("rabbit-hole"), ["rabbit", "-", "hole"]);
        assert_eq!(tokens("so-called centre"), ["so", "-", "called", "centre"]);
        assert_eq!(tokens("well- known"), ["well", "-", "known"]);
    }

    #[test]
    fn a_possessive_is_its_own_token_after_the_noun() {
        assert_eq!(tokens("Alice's"), ["Alice", "'s"]);
    }

    /// The Design oracle's own line, read cell by cell off
    /// `mac-native-308-original-mbp-light-syntax-tokens.png`
    /// (`ref/ia/mac-native/CAPTURE-ORIGINAL-MBP.md` § A contraction splits).
    /// Five splits, and the colour on each side of each of them.
    #[test]
    fn the_five_splits_the_design_oracle_shows_are_the_five_this_tokenizer_makes() {
        let line = "I'll write, but I can't stop. it's a well-known writer's book.";
        assert_eq!(
            tokens(line),
            [
                "I", "'ll", "write", ",", "but", "I", "ca", "n't", "stop", ".", "it", "'s", "a",
                "well", "-", "known", "writer", "'s", "book", "."
            ]
        );
        let split = coloured(line);
        for (word, category) in [
            ("'ll", Some(Category::Verbs)),
            ("ca", Some(Category::Verbs)),
            ("n't", Some(Category::Adverbs)),
            ("well", Some(Category::Adverbs)),
            ("known", Some(Category::Verbs)),
            ("writer", Some(Category::Nouns)),
            ("-", None),
        ] {
            assert_eq!(
                split
                    .iter()
                    .find(|(token, _)| *token == word)
                    .map(|(_, category)| *category),
                category,
                "{word:?} is not what the oracle colours it: {split:?}"
            );
        }
        // Two `I`s and two `'s`s, and the pairs differ: the second `'s` is the
        // possessive and takes no colour, the first is a verb.
        assert_eq!(
            categories(line)
                .into_iter()
                .filter(|(span, _)| &line[span.clone()] == "'s")
                .map(|(_, category)| category)
                .collect::<Vec<_>>(),
            [Category::Verbs],
            "only the `'s` of `it's` is coloured: {split:?}"
        );
        // `not` written out is the tagger's own answer and no capture holds
        // it, so it is left where the tagger puts it — a `PART`, which colours
        // nothing. Only the `n't` the oracle was read on is overridden.
        assert!(
            !coloured("I can not stop.")
                .iter()
                .any(|(word, _)| *word == "not"),
            "`not` written out is the tagger's, not the oracle's"
        );
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

    /// The five switches: a set holds what it was given and nothing else, and
    /// a writer revising for adjectives and adverbs leaves every noun plain.
    #[test]
    fn a_categories_set_holds_the_switches_it_was_given_and_no_others() {
        let revising = Categories::NONE
            .with(Category::Adjectives)
            .with(Category::Adverbs);

        assert!(revising.contains(Category::Adjectives));
        assert!(revising.contains(Category::Adverbs));
        assert!(
            !revising.contains(Category::Nouns),
            "a pass for adjectives and adverbs is a page where nothing else is coloured"
        );
        assert!(!revising.is_empty());
        assert!(Categories::NONE.is_empty());
        assert_eq!(
            [Category::Adjectives, Category::Adverbs]
                .into_iter()
                .collect::<Categories>(),
            revising,
            "collected from the switches a writer set, it is the same set"
        );
        for category in [
            Category::Nouns,
            Category::Verbs,
            Category::Adjectives,
            Category::Adverbs,
            Category::Conjunctions,
        ] {
            assert!(
                Categories::ALL.contains(category),
                "{category:?} is missing from the whole set"
            );
        }
    }
}
