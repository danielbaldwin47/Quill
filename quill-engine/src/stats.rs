//! Stats: word count, character count and reading time.
//!
//! What is here is the Parity oracle's word rule (`legacy/app/js/chrome.js`
//! `count`), landed for the stats bar by the Chrome Piece (#128): a word is a
//! run of non-whitespace with at least one letter or digit in it, counted over
//! the raw text, Markdown included, so a lone `#` is not a word and `**bold**`
//! is. The Stats spec (#30) replaces the rule — counting over the prose stream
//! so Markup never counts as a word, and for a selection as well as the whole
//! Document — and adds the other numbers.
//!
//! A whole-Document pass runs on idle after the synchronous keystroke lane,
//! never inside it: the caller counts from a timer, not from the keystroke.

/// The words in `text`, by the oracle's rule.
#[must_use]
pub fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ref/sample.md` counts to the number the oracle's frozen `bars` shot
    /// shows, which is what `chrome.js`'s rule gives for it.
    #[test]
    fn the_sample_counts_as_the_oracle_counts_it() {
        let sample =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../ref/sample.md"))
                .expect("ref/sample.md");
        assert_eq!(words(&sample), 188);
    }

    #[test]
    fn a_word_has_a_letter_or_a_digit_in_it() {
        assert_eq!(words(""), 0);
        assert_eq!(words("one"), 1);
        // A lone `#` is not a word; `**bold**` is; a dash on its own is not.
        assert_eq!(words("# Title\n\n**bold** — text"), 3);
        assert_eq!(words("42 — 3.14"), 2);
        assert_eq!(words("  spaced\tout\n\nwords  "), 3);
    }
}
