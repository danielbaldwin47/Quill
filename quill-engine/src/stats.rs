//! Stats: the six numbers the bar can show, counted on the parser's events.
//!
//! What is here was the Parity oracle's word rule (`legacy/app/js/chrome.js`
//! `count`), landed for the stats bar by the Chrome Piece (#128): a word was a
//! run of non-whitespace with at least one letter or digit in it, counted over
//! the raw text, Markdown included. The Stats spec (#387) replaces the text it
//! is counted over — the parser's own events rather than the file's bytes, so
//! Markup never counts — and adds the other five numbers and the cells they
//! read as.
//!
//! The stream counted is neither the file nor the [prose stream]: it is the
//! writer's prose **plus** the code they typed. `Text` and `Code` events are
//! prose, a soft or hard break is one whitespace scalar, front matter and an
//! autolink's destination are hidden by the predicate the prose stream hides
//! them with, and a footnote definition's label never reaches an event at all.
//! A code span and a fenced block are the writer's words and count; the prose
//! stream drops both, which is why this walks the events itself.
//!
//! It counts an event's own string rather than the source bytes its range
//! names, where [`crate::markdown::prose`] counts the source: an Annotator has
//! to name bytes the Document holds, and a count has to be the number the
//! writer would reach by counting — so `&amp;` is one character here and a
//! code span's backticks are none.
//!
//! A whole-Document pass runs on idle after the synchronous keystroke lane,
//! never inside it: the caller counts from a timer, not from the keystroke. A
//! selection's pass runs on the selection change, over the held slice.
//!
//! [prose stream]: crate::markdown::prose

use pulldown_cmark::{Event, Tag};

use crate::markdown;

/// One of the six numbers the stats bar can show, in the bar's order.
///
/// The unit of every check, cell, settings name and test. [`Statistic::name`]
/// is the Command id's last segment (`stats.charactersNoSpaces` gives
/// `charactersNoSpaces`), which is also the name the `[stats]` table's `show`
/// list is written with.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Statistic {
    /// Whitespace-delimited tokens of the stream holding a letter or a digit.
    Words,
    /// Unicode scalar values of the stream.
    Characters,
    /// The same, with every whitespace scalar removed.
    CharactersNoSpaces,
    /// Runs of the stream ending in a full stop, and never crossing a block.
    Sentences,
    /// Prose blocks: a paragraph, a heading, a list item, a code block, an
    /// HTML block or a table.
    Paragraphs,
    /// [`Statistic::Words`] at the pace `WORDS_PER_MINUTE` holds.
    ReadingTime,
}

impl Statistic {
    /// All six, in the order their cells are laid out.
    ///
    /// The one order there is: a reader shows the Statistics it was given in
    /// this order and never in the order they were named to it, so that a
    /// Document reads the same on every install (#387).
    pub const ALL: [Self; 6] = [
        Self::Words,
        Self::Characters,
        Self::CharactersNoSpaces,
        Self::Sentences,
        Self::Paragraphs,
        Self::ReadingTime,
    ];

    /// Its settings name, which is its Command id's last segment.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Words => "words",
            Self::Characters => "characters",
            Self::CharactersNoSpaces => "charactersNoSpaces",
            Self::Sentences => "sentences",
            Self::Paragraphs => "paragraphs",
            Self::ReadingTime => "readingTime",
        }
    }

    /// The Statistic `name` names, or nothing.
    ///
    /// Nothing rather than an error, because the settings file is the writer's
    /// to type into: a name Quill does not know is dropped with a note and the
    /// rest of the list still stands (#387).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stat| stat.name() == name)
    }
}

/// The reading pace Reading Time counts at (`legacy/app/js/chrome.js` `WPM`).
///
/// Brysbaert 2019, the meta-analysis of silent reading of English non-fiction.
/// iA's own bar implies about 200; a writer is better served by the honest
/// number than by a flattering one. Not a setting (#387 § Out of Scope).
const WORDS_PER_MINUTE: f64 = 238.0;

/// All six numbers over one text, counted in one pass.
///
/// Reading Time is not a field: it is [`Counts::words`] over the pace
/// `WORDS_PER_MINUTE` holds, and carrying it twice would let the two
/// disagree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    /// [`Statistic::Words`].
    pub words: usize,
    /// [`Statistic::Characters`].
    pub characters: usize,
    /// [`Statistic::CharactersNoSpaces`].
    pub characters_no_spaces: usize,
    /// [`Statistic::Sentences`].
    pub sentences: usize,
    /// [`Statistic::Paragraphs`].
    pub paragraphs: usize,
}

impl Counts {
    /// One Statistic's cell, as `("1,234", "words")`.
    ///
    /// The number grouped as `toLocaleString` writes it and the label
    /// pluralised, both exactly as the Parity oracle's bar writes them
    /// (`chrome.js` `FIELDS`) — Characters keeps its plural at one, and
    /// Reading Time's number is a formatted span rather than a count.
    #[must_use]
    pub fn cell(self, stat: Statistic) -> (String, &'static str) {
        match stat {
            Statistic::Words => (
                grouped(self.words),
                if self.words == 1 { "word" } else { "words" },
            ),
            Statistic::Characters => (grouped(self.characters), "characters"),
            Statistic::CharactersNoSpaces => (grouped(self.characters_no_spaces), "without spaces"),
            Statistic::Sentences => (
                grouped(self.sentences),
                if self.sentences == 1 {
                    "sentence"
                } else {
                    "sentences"
                },
            ),
            Statistic::Paragraphs => (
                grouped(self.paragraphs),
                if self.paragraphs == 1 {
                    "paragraph"
                } else {
                    "paragraphs"
                },
            ),
            Statistic::ReadingTime => (reading_time(self.words), "read"),
        }
    }
}

/// The words in `text`, by the rule of [`count`].
///
/// Kept as its own function for the two callers that want one number: the
/// stats bar's Words cell and the Library sidebar's word column, which reads a
/// file's first 4 KB and so agrees with the bar for files under that size
/// (#387).
#[must_use]
pub fn words(text: &str) -> usize {
    count(text).words
}

/// All six Statistics over `text`.
///
/// One parse and one walk of its events, so the bound is linear in `text` and
/// the six cost what one costs; the block being counted is the only text held
/// beyond the parser's own. `a_fifty_three_thousand_word_draft_counts_six_ways_inside_the_budget`
/// pins it against the bench's longest draft.
///
/// A selection's readout calls this over the selected byte range's slice of
/// the Document, so a selection that cuts a word counts the cut token as the
/// stream sees it, and one that holds only Markup counts nothing.
#[must_use]
pub fn count(text: &str) -> Counts {
    let mut counts = Counts::default();
    // One frame per open tag: `Event::End` carries too little to recognise an
    // autolink again, and a bare depth could not be unwound.
    let mut open: Vec<Frame> = Vec::new();
    let mut hidden = 0usize;
    // The current block's stream. Blocks are counted one at a time and never
    // joined: a sentence must not run from a heading into the paragraph below,
    // and two paragraphs' words must not fuse into one token. Within a block
    // the fragments *are* joined, because the parser splits `un**bel**ievable`
    // into three and it is one word.
    let mut block = String::new();

    for (event, _) in markdown::events(text) {
        match event {
            Event::Start(tag) => {
                let kind = Kind::of(&tag);
                if kind.is_block() {
                    flush(&mut counts, &mut block);
                }
                if kind.opens_a_paragraph(open.last().map(|frame| frame.kind)) {
                    counts.paragraphs += 1;
                }
                let hides = markdown::hides_words(&tag);
                hidden += usize::from(hides);
                open.push(Frame { kind, hides });
            }
            Event::End(_) => {
                let Some(frame) = open.pop() else { continue };
                if frame.kind.is_block() {
                    flush(&mut counts, &mut block);
                }
                hidden -= usize::from(frame.hides);
            }
            // Code is the writer's text as much as prose is: a technical draft
            // counts the code it holds (#387, user story 3).
            Event::Text(run) | Event::Code(run) if hidden == 0 => block.push_str(&run),
            // One whitespace scalar, never a sentence end: a wrapped line is
            // still one sentence.
            Event::SoftBreak | Event::HardBreak if hidden == 0 => block.push('\n'),
            _ => {}
        }
    }
    flush(&mut counts, &mut block);
    counts
}

/// One open tag: what it is, and whether its text is counted.
struct Frame {
    kind: Kind,
    hides: bool,
}

/// As much of a tag as counting has to tell apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// A paragraph, which is a prose block unless a list item or a footnote
    /// definition already counted for it.
    Paragraph,
    /// A prose block in its own right: a heading, a list item, a code block,
    /// an HTML block or a table.
    Block,
    /// A list item, which is a prose block and swallows the paragraph inside
    /// it.
    Item,
    /// A footnote definition, which is not a prose block and also swallows the
    /// paragraph inside it: its words count where the label does not.
    FootnoteDefinition,
    /// Structure that is not itself a prose block but ends a sentence: a
    /// quote, a list, a table row, a cell, front matter.
    Container,
    /// An inline tag — emphasis, a link, an image. No block edge, no
    /// paragraph.
    Inline,
}

impl Kind {
    fn of(tag: &Tag<'_>) -> Self {
        match tag {
            Tag::Paragraph => Self::Paragraph,
            Tag::Heading { .. }
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::Table(_)
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition => Self::Block,
            Tag::Item => Self::Item,
            Tag::FootnoteDefinition(_) => Self::FootnoteDefinition,
            Tag::BlockQuote(_)
            | Tag::List(_)
            | Tag::DefinitionList
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_) => Self::Container,
            _ => Self::Inline,
        }
    }

    /// Whether opening or closing this tag is a sentence edge.
    fn is_block(self) -> bool {
        self != Self::Inline
    }

    /// Whether this tag is one prose block, given the tag it opened inside.
    ///
    /// A paragraph is one, except inside a list item or a footnote
    /// definition, which already counted: a loose list's item holds a
    /// paragraph and a tight one's does not, and a list of two items counts
    /// two either way.
    fn opens_a_paragraph(self, within: Option<Self>) -> bool {
        match self {
            Self::Block | Self::Item => true,
            Self::Paragraph => !matches!(within, Some(Self::Item | Self::FootnoteDefinition)),
            Self::FootnoteDefinition | Self::Container | Self::Inline => false,
        }
    }
}

/// Counts the block that just ended into `counts`, and empties it.
fn flush(counts: &mut Counts, block: &mut String) {
    if block.is_empty() {
        return;
    }
    counts.words += block
        .split_whitespace()
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count();
    counts.characters += block.chars().count();
    counts.characters_no_spaces += block.chars().filter(|c| !c.is_whitespace()).count();
    counts.sentences += sentences(block);
    block.clear();
}

/// Full stops, as the oracle's `SENT` regex lists them.
const ENDS: [char; 4] = ['.', '!', '?', '…'];
/// What may follow a full stop and still be inside the sentence it ends.
const CLOSERS: [char; 6] = ['\'', '"', '”', '’', ')', ']'];

/// The sentences in one block's stream.
///
/// A run ending at a full stop, plus any closing quotes or brackets, followed
/// by whitespace or the block's end; an unpunctuated tail counts one. A run
/// with no letter or digit in it counts none, so a block of Markup alone
/// counts no sentence. The rule and its punctuation are the Parity oracle's
/// (`chrome.js` `SENT`), read a character at a time so that `3.14` is not two.
fn sentences(text: &str) -> usize {
    let mut count = 0;
    // A letter or a digit since the last sentence ended. Without it a block of
    // `**` would count one.
    let mut content = false;
    // Inside the full stop and its closers, waiting to see whitespace.
    let mut ending = false;
    for c in text.chars() {
        if ENDS.contains(&c) {
            ending |= content;
            continue;
        }
        if ending {
            if CLOSERS.contains(&c) {
                continue;
            }
            if c.is_whitespace() {
                count += 1;
                (content, ending) = (false, false);
                continue;
            }
            // `3.14`: the stop was inside a token, and the run goes on.
            ending = false;
        }
        content |= c.is_alphanumeric();
    }
    count + usize::from(content)
}

/// `n` with thousands separated, as `toLocaleString` writes it.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// How long `words` take to read at [`WORDS_PER_MINUTE`], as the oracle's
/// `readTime` writes it: under 45 seconds is `< 1 min`, then whole minutes,
/// then hours and minutes.
fn reading_time(words: usize) -> String {
    let seconds = (words as f64 / WORDS_PER_MINUTE * 60.0).round();
    if seconds < 45.0 {
        return "< 1 min".to_owned();
    }
    let minutes = (seconds / 60.0).round() as u64;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if minutes == 0 {
        format!("{hours} h")
    } else {
        format!("{hours} h {minutes} min")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../ref/sample.md"))
            .expect("ref/sample.md")
    }

    /// `ref/sample.md`, all six.
    ///
    /// The words stay 188, the number the oracle's frozen `bars` shot shows:
    /// the passage's only Markup is bare markers and one bold word, and
    /// neither was ever a word by the old rule either. The characters fall
    /// from the file's raw 961 — the markers themselves, the blank lines
    /// between the blocks and the newline ending each of them are not the
    /// stream — which is the one judged cell the new rule moves, and the
    /// chrome brief names it as the rule rather than a gap (#387).
    #[test]
    fn the_sample_counts_six_ways() {
        let sample = sample();
        assert_eq!(sample.chars().count(), 961, "the file's raw characters");
        assert_eq!(
            count(&sample),
            Counts {
                words: 188,
                characters: 929,
                characters_no_spaces: 749,
                sentences: 16,
                paragraphs: 9,
            }
        );
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

    #[test]
    fn a_word_split_by_markup_is_one_word() {
        assert_eq!(words("un**bel**ievable"), 1);
        assert_eq!(count("un**bel**ievable").characters, 12);
    }

    /// A Document of Markup alone is nothing to count (#387, user story 30).
    #[test]
    fn markup_alone_counts_no_words_and_no_sentences() {
        let markup = "# \n\n* * *\n\n<https://example.org/a/b>\n";
        let counts = count(markup);
        assert_eq!((counts.words, counts.sentences), (0, 0));
    }

    #[test]
    fn a_links_text_counts_and_its_url_does_not() {
        assert_eq!(words("[text](https://example.com/a/b)"), 1);
        assert_eq!(words("See <https://example.com/a/b> for it."), 3);
    }

    #[test]
    fn code_counts_as_the_words_the_writer_typed() {
        assert_eq!(words("Call `map` on it."), 4);
        assert_eq!(words("```\nlet three words = 1;\n```\n"), 4);
        // The backticks are the fence and the span's delimiters, never the
        // stream: `map` is three characters here and five in the file.
        assert_eq!(count("`map`").characters, 3);
    }

    #[test]
    fn front_matter_and_a_footnote_label_count_nothing_and_the_footnotes_text_counts() {
        let source = "---\ntitle: The Lighthouse\nauthor: A Keeper\n---\n\n\
                      The lamp was lit.[^1]\n\n[^1]: It had been lit for an hour.\n";
        let counts = count(source);
        assert_eq!(
            counts.words, 11,
            "four words above the line and seven below"
        );
        assert_eq!(
            counts.characters,
            "The lamp was lit.".len() + "It had been lit for an hour.".len(),
            "the front matter, the `[^1]` reference and the definition's label \
             are none of them the stream"
        );
    }

    #[test]
    fn a_heading_never_runs_into_the_paragraph_below_it() {
        let source = "# The Lighthouse\n\nThe lamp had been lit\nfor an hour.\n";
        let counts = count(source);
        assert_eq!((counts.sentences, counts.paragraphs), (2, 2));
        // The soft break is one scalar, and no sentence ends on it.
        assert_eq!(
            counts.characters,
            "The Lighthouse".len() + "The lamp had been lit\nfor an hour.".len()
        );
    }

    #[test]
    fn a_full_stop_inside_a_token_is_not_a_sentence_end() {
        assert_eq!(count("Pi is 3.14 and e is 2.71.").sentences, 1);
        assert_eq!(count("One. Two. Three").sentences, 3);
        assert_eq!(count("He said \"Go!\" and left.").sentences, 2);
    }

    #[test]
    fn a_prose_block_is_a_paragraph_and_a_container_is_not() {
        // A list of two items counts two, tight or loose.
        assert_eq!(count("- one\n- two\n").paragraphs, 2);
        assert_eq!(count("- one\n\n- two\n").paragraphs, 2);
        // A quote's paragraph counts; the quote itself does not.
        assert_eq!(count("> Quoted prose.\n").paragraphs, 1);
        // A blank line is nothing, a rule is nothing, a code block is one.
        assert_eq!(count("\n\n---\n\n```\ncode\n```\n").paragraphs, 1);
        // A table is one whatever its rows.
        assert_eq!(count("| a | b |\n|---|---|\n| 1 | 2 |\n").paragraphs, 1);
    }

    #[test]
    fn the_empty_document_counts_nothing() {
        assert_eq!(count(""), Counts::default());
        assert_eq!(count("").cell(Statistic::Words), ("0".to_owned(), "words"));
    }

    #[test]
    fn every_statistic_names_its_command_ids_last_segment_and_parses_back() {
        assert_eq!(
            Statistic::ALL.map(Statistic::name),
            [
                "words",
                "characters",
                "charactersNoSpaces",
                "sentences",
                "paragraphs",
                "readingTime",
            ]
        );
        for stat in Statistic::ALL {
            assert_eq!(Statistic::from_name(stat.name()), Some(stat));
        }
        assert_eq!(Statistic::from_name("tasks"), None);
        assert_eq!(Statistic::from_name("Words"), None);
    }

    /// The cells the bar reads, label and all (`chrome.js` `FIELDS`).
    #[test]
    fn a_cell_is_the_grouped_number_and_the_oracles_label() {
        let counts = Counts {
            words: 1,
            characters: 1,
            characters_no_spaces: 1,
            sentences: 1,
            paragraphs: 1,
        };
        assert_eq!(
            Statistic::ALL.map(|stat| counts.cell(stat)),
            [
                ("1".to_owned(), "word"),
                // Characters keeps its plural at one, as the oracle's does.
                ("1".to_owned(), "characters"),
                ("1".to_owned(), "without spaces"),
                ("1".to_owned(), "sentence"),
                ("1".to_owned(), "paragraph"),
                ("< 1 min".to_owned(), "read"),
            ]
        );
        let many = Counts {
            words: 2,
            sentences: 2,
            paragraphs: 2,
            ..counts
        };
        assert_eq!(many.cell(Statistic::Words).1, "words");
        assert_eq!(many.cell(Statistic::Sentences).1, "sentences");
        assert_eq!(many.cell(Statistic::Paragraphs).1, "paragraphs");
    }

    #[test]
    fn numbers_are_grouped_and_times_written_the_way_the_oracle_writes_them() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(961), "961");
        assert_eq!(grouped(1_234), "1,234");
        assert_eq!(grouped(1_234_567), "1,234,567");
        // 176 words are 44 seconds at 238 a minute; 177 are 45.
        assert_eq!(reading_time(176), "< 1 min");
        assert_eq!(reading_time(177), "1 min");
        assert_eq!(reading_time(2_380), "10 min");
        assert_eq!(reading_time(14_280), "1 h");
        assert_eq!(reading_time(15_470), "1 h 5 min");
        // The two thresholds the spec names: 59 minutes and 60.
        assert_eq!(reading_time(14_042), "59 min");
        assert_eq!(reading_time(15_000), "1 h 3 min");
    }

    /// The count is cheap enough to stay on the main thread.
    ///
    /// The bench's longest draft, counted six ways, inside the frame the
    /// keystroke budget is written in. This is the test that decides whether
    /// Stats ever moves to the worker (#387 § Out of Scope): only its failing
    /// reopens that. Shaped like `crate::watch`'s elapsed-time assertion —
    /// one wall-clock bound, generous enough not to flake under a loaded
    /// machine and tight enough that a slow rule fails it.
    #[test]
    fn a_fifty_three_thousand_word_draft_counts_six_ways_inside_the_budget() {
        let draft = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../shots/latency/doc52k.md"
        ))
        .expect("the bench's longest draft is in the repo");
        let counted = std::time::Instant::now();
        let counts = count(&draft);
        let took = counted.elapsed();
        assert!(counts.words > 50_000, "{counts:?}");
        assert!(took < BUDGET, "counting six ways took {took:?}");
    }

    /// The budget the cost bound holds the count to.
    ///
    /// 5 ms is the Gate's keystroke mean, and the count has to fit inside one
    /// idle frame. A debug build is not what ships and not what the budget is
    /// written for, so it is given the order of magnitude the optimiser is
    /// worth rather than a pass.
    #[cfg(debug_assertions)]
    const BUDGET: std::time::Duration = std::time::Duration::from_millis(100);
    #[cfg(not(debug_assertions))]
    const BUDGET: std::time::Duration = std::time::Duration::from_millis(5);
}
