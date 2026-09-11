//! Style check: fillers, redundancies and clichés.
//!
//! The Annotator that strikes through matches from Quill's shipped lists, one
//! decoration per list so a list's toggle drops its marks without a re-match
//! (#29). It reads the prose stream and matches over it; the lists are data
//! files resolved from the one data directory, not compiled in
//! ([`crate::data::style`]).
//!
//! The three files compile into **one** automaton over their union
//! ([`Lists`]), because the match never knows a toggle: a writer who has
//! switched Redundancies off still has its spans computed and simply unpainted,
//! so flipping a list is a repaint rather than a re-match. [`struck`] is the
//! whole of the matching, pure over a paragraph of prose, and the worker thread
//! is what calls it.
//!
//! Matching is over a **shadow** of the paragraph rather than the paragraph
//! itself: lower cased, both apostrophes folded to the typewriter one, and each
//! run of whitespace collapsed to one space, so a phrase wrapped across a soft
//! line break in the source still matches and `Basically` is the same word as
//! `basically`. Every shadow byte remembers the source characters it came from,
//! which is what turns a match back into the byte range this module emits. The
//! folding is Unicode's rather than the automaton's `ascii_case_insensitive`,
//! which would leave a capital `É` unmatched.

use std::ops::Range;

use aho_corasick::{AhoCorasick, MatchKind};

/// The three phrase lists Style check matches.
///
/// The unit of every toggle, tag and test: the writer switches Clichés on, not
/// a file name, and the settings file writes `cliches`. Style check is the
/// master toggle over the three and the two are separate state (#356), which is
/// why nothing here carries an on-or-off.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum List {
    /// Words and phrases that add nothing to a sentence: `basically`, `very`.
    Fillers,
    /// Phrases that say a thing twice: `combine together`, `past history`. Only
    /// the redundant words are struck.
    Redundancies,
    /// Worn-out figures of speech: `against all odds`, `brass tacks`.
    Cliches,
}

impl List {
    /// The three, in the order the submenu, the Settings group and the files
    /// list them.
    pub const ALL: [Self; 3] = [Self::Fillers, Self::Redundancies, Self::Cliches];

    /// What `[style_check]` calls this List, which is also its Command's
    /// suffix (`style.fillers`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Fillers => "fillers",
            Self::Redundancies => "redundancies",
            Self::Cliches => "cliches",
        }
    }

    /// This List's file under [`crate::data::style`].
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Fillers => "fillers.txt",
            Self::Redundancies => "redundancies.txt",
            Self::Cliches => "cliches.txt",
        }
    }
}

/// One phrase of one list, as the matcher holds it.
///
/// The needle is the phrase with its brackets taken out and the same folding
/// the shadow gets, so a needle matches the shadow byte for byte and a struck
/// group's offsets inside the needle are offsets inside the match.
struct Phrase {
    /// What the automaton searches for.
    needle: String,
    /// Which List it came from.
    list: List,
    /// The words to strike, as byte ranges into `needle`, ascending and
    /// non-overlapping. Empty means the whole phrase is struck, which is every
    /// filler and every cliché.
    struck: Vec<Range<usize>>,
}

/// The three lists compiled into one automaton.
///
/// Built once on the worker thread at first use and shared from then on, which
/// is why nothing here happens at startup or on a keystroke. Cheap to hold: the
/// automaton plus one `Phrase` per line of the three files.
pub struct Lists {
    automaton: AhoCorasick,
    /// One per pattern, indexed by the automaton's pattern id.
    phrases: Vec<Phrase>,
}

impl Lists {
    /// The three shipped files, read and compiled.
    ///
    /// A file that cannot be read costs its List and nothing else: a note, an
    /// empty list, and the other two still matching. Style check with one list
    /// missing is worth more to a writer than no Style check at all.
    #[must_use]
    pub fn load(notes: &mut Vec<String>) -> Self {
        let directory = crate::data::style();
        let files: Vec<(List, String)> = List::ALL
            .into_iter()
            .map(|list| {
                let text =
                    std::fs::read_to_string(directory.join(list.file())).unwrap_or_else(|err| {
                        notes.push(format!(
                            "{}: {err}; Style check runs without its {} list",
                            list.file(),
                            list.key()
                        ));
                        String::new()
                    });
                (list, text)
            })
            .collect();
        Self::compile(
            files.iter().map(|(list, text)| (*list, text.as_str())),
            notes,
        )
    }

    /// The same from text already in hand, which is what a test hands it.
    #[must_use]
    pub fn compile<'a>(
        files: impl IntoIterator<Item = (List, &'a str)>,
        notes: &mut Vec<String>,
    ) -> Self {
        let phrases: Vec<Phrase> = files
            .into_iter()
            .flat_map(|(list, text)| read(text, list, notes))
            .collect();
        // `Standard` rather than a leftmost match kind because the automaton is
        // asked for every candidate: a leftmost-longest hit that fails the
        // whole-word test has to fall back to a shorter phrase starting in the
        // same place (`very unique` under `very uniquely`), which only the
        // overlapping search can offer. [`struck`] resolves the rest.
        let automaton = AhoCorasick::builder()
            .match_kind(MatchKind::Standard)
            .build(phrases.iter().map(|phrase| &phrase.needle))
            .expect("the lists compile: every needle is a non-empty phrase");
        Self { automaton, phrases }
    }
}

/// What Style check strikes in one paragraph of prose: byte ranges into
/// `prose`, ascending, never overlapping, each carrying the List it came from.
///
/// `prose` is the prose stream's text — the parser's `Text` events with Markup,
/// code spans, fenced code, URLs and front matter already removed — so a phrase
/// inside a fence is never struck, and this function never has to know it is
/// reading Markdown.
///
/// Every List is matched whatever the writer has switched on; a List switched
/// off has its spans emitted here and left unpainted downstream. Where two
/// phrases could match at one place the leftmost wins and, among those starting
/// together, the longest, so `very unique` is one redundancy rather than a
/// filler with a redundancy under it. A redundancy emits one span per bracketed
/// group — `together` alone out of `combine together` — while the whole phrase
/// still claims its extent, so nothing else matches inside it.
#[must_use]
pub fn struck(prose: &str, lists: &Lists) -> Vec<(Range<usize>, List)> {
    let shadow = Shadow::of(prose);
    let mut candidates: Vec<(usize, usize, usize)> = lists
        .automaton
        .find_overlapping_iter(&shadow.text)
        .filter(|hit| is_whole_word(&shadow.text, hit.start(), hit.end()))
        .map(|hit| (hit.start(), hit.end(), hit.pattern().as_usize()))
        .collect();
    // Leftmost, then longest, then the earlier list: the third is only for a
    // phrase two lists both carry, and it is there so the same prose strikes
    // the same way twice.
    candidates.sort_unstable_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(right.1.cmp(&left.1))
            .then(left.2.cmp(&right.2))
    });
    let mut spans = Vec::new();
    let mut taken = 0;
    for (start, end, pattern) in candidates {
        if start < taken {
            continue;
        }
        taken = end;
        let phrase = &lists.phrases[pattern];
        if phrase.struck.is_empty() {
            spans.push((shadow.source(start..end), phrase.list));
        } else {
            for group in &phrase.struck {
                spans.push((
                    shadow.source(start + group.start..start + group.end),
                    phrase.list,
                ));
            }
        }
    }
    spans
}

/// The paragraph as the automaton searches it, with the way back to the source.
struct Shadow {
    /// Lower cased, apostrophes folded, each run of whitespace one space.
    text: String,
    /// For each byte of `text`, the source byte range of what it came from: the
    /// character that folded to it, or the whole whitespace run a space stands
    /// for.
    source: Vec<(usize, usize)>,
}

impl Shadow {
    fn of(prose: &str) -> Self {
        let mut text = String::with_capacity(prose.len());
        let mut source = Vec::with_capacity(prose.len());
        let mut characters = prose.char_indices().peekable();
        while let Some((at, character)) = characters.next() {
            let mut end = at + character.len_utf8();
            if character.is_whitespace() {
                while let Some(&(next, following)) = characters.peek() {
                    if !following.is_whitespace() {
                        break;
                    }
                    end = next + following.len_utf8();
                    characters.next();
                }
                text.push(' ');
                source.push((at, end));
            } else {
                push_folded(&mut text, &mut source, character, (at, end));
            }
        }
        Self { text, source }
    }

    /// The source byte range a shadow range stands for. The range is a match,
    /// so it is never empty.
    fn source(&self, range: Range<usize>) -> Range<usize> {
        self.source[range.start].0..self.source[range.end - 1].1
    }
}

/// The phrases of one list file, with a line the reader cannot parse skipped
/// and noted.
///
/// A bad line loses one phrase, never the feature: the file is data a
/// contributor edits by hand, and a stray bracket in one of eight hundred lines
/// is no reason to stop striking the other seven hundred and ninety-nine.
fn read(text: &str, list: List, notes: &mut Vec<String>) -> Vec<Phrase> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('#')
        })
        .filter_map(|(index, line)| match phrase(line, list) {
            Ok(phrase) => Some(phrase),
            Err(why) => {
                notes.push(format!(
                    "{} line {}: {why}; skipping that phrase and reading the rest of the list",
                    list.file(),
                    index + 1
                ));
                None
            }
        })
        .collect()
}

/// One line read into a [`Phrase`], or why it could not be.
///
/// The file shape is one phrase per line, lower case, ASCII apostrophes, and in
/// a redundancy the words to strike marked inline in square brackets
/// (`combine [together]`, `[past] history`). The folding here is the same the
/// shadow gets, so a file that spells a phrase with a capital or a curly
/// apostrophe still matches rather than silently never firing.
fn phrase(line: &str, list: List) -> Result<Phrase, &'static str> {
    let mut needle = String::new();
    let mut bracketed: Vec<bool> = Vec::new();
    let mut inside = false;
    let mut marked = false;
    for character in line.trim().chars() {
        match character {
            '[' if inside => return Err("a `[` inside a `[`"),
            '[' => {
                inside = true;
                marked = true;
            }
            ']' if !inside => return Err("a `]` with no `[` in front of it"),
            ']' => inside = false,
            _ if character.is_whitespace() => {
                if !needle.is_empty() && !needle.ends_with(' ') {
                    needle.push(' ');
                    bracketed.push(inside);
                }
            }
            _ => push_folded(&mut needle, &mut bracketed, character, inside),
        }
    }
    if inside {
        return Err("a `[` with no `]` after it");
    }
    if needle.is_empty() {
        return Err("no phrase on the line");
    }
    let struck = groups(&needle, &bracketed);
    if marked && struck.is_empty() {
        return Err("a `[]` with no words in it");
    }
    Ok(Phrase {
        needle,
        list,
        struck,
    })
}

/// The runs of bracketed bytes, as ranges into the needle, with the spaces at
/// either end of each trimmed off — `combine [together ]` strikes `together`.
fn groups(needle: &str, bracketed: &[bool]) -> Vec<Range<usize>> {
    let mut groups = Vec::new();
    let mut at = 0;
    while at < bracketed.len() {
        if !bracketed[at] {
            at += 1;
            continue;
        }
        let start = at;
        while at < bracketed.len() && bracketed[at] {
            at += 1;
        }
        let mut group = start..at;
        let bytes = needle.as_bytes();
        while group.start < group.end && bytes[group.start] == b' ' {
            group.start += 1;
        }
        while group.end > group.start && bytes[group.end - 1] == b' ' {
            group.end -= 1;
        }
        if !group.is_empty() {
            groups.push(group);
        }
    }
    groups
}

/// Pushes `character` folded onto `text`, extending `beside` with `each` once
/// for every byte pushed, so the two stay one entry per byte.
///
/// The shadow the automaton searches and the needles it searches for must fold
/// the same way byte for byte or a phrase never matches the prose it was
/// written for, so both fold through here (#356).
fn push_folded<T: Copy>(text: &mut String, beside: &mut Vec<T>, character: char, each: T) {
    for folded in fold(character) {
        let was = text.len();
        text.push(folded);
        beside.extend(std::iter::repeat_n(each, text.len() - was));
    }
}

/// What a character matches as: lower case, with the right single quote a
/// smart-quote pass leaves behind folded to the typewriter apostrophe, so
/// `it's` written either way is the one word.
fn fold(character: char) -> std::char::ToLowercase {
    if character == '\u{2019}' {
        '\''.to_lowercase()
    } else {
        character.to_lowercase()
    }
}

/// Whether a match stands on its own rather than inside a longer word, so that
/// `too` is struck in `ran too long` and never in `tool`.
///
/// An apostrophe counts as part of a word, which is what keeps a bare `it` from
/// being struck out of `it's`; a hyphen does not, so `well` in `well-known` is
/// a word the way [`crate::pos`] reads it.
fn is_whole_word(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    !before.is_some_and(is_word) && !after.is_some_and(is_word)
}

/// Whether a character joins the word beside it.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '\''
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lists a matcher test writes for itself, small enough to read.
    fn lists() -> Lists {
        let mut notes = Vec::new();
        let lists = Lists::compile(
            [
                (List::Fillers, "# a comment\nvery\ntoo\nsort of\nit's\n"),
                (List::Redundancies, "combine [together]\nvery unique\n"),
                (List::Cliches, "against all odds\n"),
            ],
            &mut notes,
        );
        assert!(notes.is_empty(), "the test's own lists read cleanly");
        lists
    }

    /// Every struck phrase of `prose`, as `(text, List)`.
    fn marks(prose: &str) -> Vec<(&str, List)> {
        struck(prose, &lists())
            .into_iter()
            .map(|(range, list)| (&prose[range], list))
            .collect()
    }

    /// The phrases of one list's text, as `(needle, the struck words)`.
    fn phrases(text: &str) -> (Vec<(String, Vec<String>)>, Vec<String>) {
        let mut notes = Vec::new();
        let read = read(text, List::Redundancies, &mut notes);
        let phrases = read
            .into_iter()
            .map(|phrase| {
                let struck = phrase
                    .struck
                    .iter()
                    .map(|group| phrase.needle[group.clone()].to_owned())
                    .collect();
                (phrase.needle, struck)
            })
            .collect();
        (phrases, notes)
    }

    #[test]
    fn the_reader_ignores_a_comment_and_a_blank_line() {
        let (phrases, notes) = phrases("# Redundancies\n#\n\npast history\n\n   \nvery unique\n");
        assert_eq!(
            phrases,
            [
                ("past history".to_owned(), vec![]),
                ("very unique".to_owned(), vec![]),
            ]
        );
        assert!(notes.is_empty(), "nothing to note: {notes:?}");
    }

    #[test]
    fn the_reader_reads_a_bracketed_group_as_the_words_to_strike() {
        let (phrases, _) = phrases("combine [together]\n[past] history\n[in order] to\n");
        assert_eq!(
            phrases,
            [
                ("combine together".to_owned(), vec!["together".to_owned()]),
                ("past history".to_owned(), vec!["past".to_owned()]),
                ("in order to".to_owned(), vec!["in order".to_owned()]),
            ]
        );
    }

    #[test]
    fn the_reader_folds_a_phrase_the_way_the_page_is_folded() {
        let (phrases, _) = phrases("Past History\nit\u{2019}s  a   shame\n");
        assert_eq!(
            phrases,
            [
                ("past history".to_owned(), vec![]),
                ("it's a shame".to_owned(), vec![]),
            ]
        );
    }

    #[test]
    fn the_reader_skips_a_bad_line_with_a_note_and_reads_the_rest() {
        let (phrases, notes) = phrases("combine [together\n[past] history\nfree [] gift\n]\n");
        assert_eq!(
            phrases,
            [("past history".to_owned(), vec!["past".to_owned()])]
        );
        assert_eq!(notes.len(), 3, "one note per bad line: {notes:?}");
        assert!(
            notes[0].starts_with("redundancies.txt line 1: a `[` with no `]` after it;"),
            "the note names the file, the line and what was wrong: {}",
            notes[0]
        );
        assert!(notes[1].contains("line 3"), "{}", notes[1]);
        assert!(notes[2].contains("line 4"), "{}", notes[2]);
    }

    #[test]
    fn a_capital_is_struck_like_a_lower_case_word() {
        assert_eq!(marks("Very good"), [("Very", List::Fillers)]);
        assert_eq!(marks("it was very good"), [("very", List::Fillers)]);
    }

    #[test]
    fn a_phrase_inside_a_longer_word_is_not_struck() {
        assert_eq!(marks("the tool was too heavy"), [("too", List::Fillers)]);
        assert_eq!(marks("overtoo"), []);
        assert_eq!(
            marks("it's"),
            [("it's", List::Fillers)],
            "a whole word is still struck"
        );
        assert_eq!(marks("bits of it"), [], "`sort of` is not `s of`");
    }

    #[test]
    fn a_phrase_matches_across_a_soft_line_break() {
        let prose = "we were sort\nof ready";
        assert_eq!(marks(prose), [("sort\nof", List::Fillers)]);
        let struck = struck(prose, &lists());
        assert_eq!(struck[0].0, 8..15, "the range addresses the source");
    }

    #[test]
    fn either_apostrophe_is_the_same_word() {
        assert_eq!(marks("it's here"), [("it's", List::Fillers)]);
        assert_eq!(
            marks("it\u{2019}s here"),
            [("it\u{2019}s", List::Fillers)],
            "the curly one matches the list's straight one"
        );
    }

    #[test]
    fn the_longest_phrase_at_a_position_wins_and_the_marks_never_overlap() {
        assert_eq!(
            marks("one very unique draft"),
            [("very unique", List::Redundancies)],
            "`very` is not struck under it"
        );
        assert_eq!(
            marks("one very short draft"),
            [("very", List::Fillers)],
            "and the filler still stands on its own"
        );
    }

    #[test]
    fn a_longest_match_that_is_not_a_whole_word_falls_back_to_the_shorter_one() {
        assert_eq!(
            marks("very uniquely put"),
            [("very", List::Fillers)],
            "`very unique` is inside `uniquely`, so the filler is what is left"
        );
    }

    #[test]
    fn a_redundancy_strikes_its_bracketed_words_alone() {
        assert_eq!(
            marks("we combine together the drafts"),
            [("together", List::Redundancies)]
        );
    }

    #[test]
    fn the_whole_phrase_claims_its_extent_even_when_one_word_is_struck() {
        // `very` sits inside `combine together very` nowhere, so the phrase to
        // prove the rule is the one that overlaps: a filler inside the extent
        // of a redundancy is not emitted beside it.
        assert_eq!(
            marks("very unique against all odds"),
            [
                ("very unique", List::Redundancies),
                ("against all odds", List::Cliches),
            ]
        );
    }

    #[test]
    fn every_list_is_matched_whatever_a_toggle_would_say() {
        assert_eq!(
            marks("very, against all odds, we combine together"),
            [
                ("very", List::Fillers),
                ("against all odds", List::Cliches),
                ("together", List::Redundancies),
            ],
            "nothing here knows a toggle: the spans are emitted and painted \
             or not downstream"
        );
    }

    #[test]
    fn an_empty_list_matches_nothing_rather_than_failing() {
        let mut notes = Vec::new();
        let lists = Lists::compile([(List::Fillers, "# nothing but a comment\n")], &mut notes);
        assert_eq!(struck("very good indeed", &lists), []);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_list_names_its_file_and_its_settings_key() {
        assert_eq!(
            List::ALL.map(List::file),
            ["fillers.txt", "redundancies.txt", "cliches.txt"]
        );
        assert_eq!(
            List::ALL.map(List::key),
            ["fillers", "redundancies", "cliches"]
        );
    }
}
