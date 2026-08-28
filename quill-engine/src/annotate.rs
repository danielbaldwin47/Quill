//! The Annotator trait, its spans, and flattening them into runs.
//!
//! An Annotator turns a byte range of a Document into spans, each a byte range
//! and a mark. Four exist: Markup, Syntax highlight ([`crate::pos`]), Style
//! check ([`crate::style`]) and Spell check ([`crate::spell`]). Markup runs
//! synchronously on the keystroke; the other three run on a worker thread
//! against a Document generation, and a result computed against a stale
//! generation is discarded.
//!
//! Overlapping `GtkTextTag`s override colour by priority rather than blending,
//! so Markup × Focus × Syntax highlight are flattened here into
//! non-overlapping runs carrying one precomputed colour each. Decorations
//! (underlines) stay separate: underline and colour are different properties,
//! so those overlaps are safe.
//!
//! All offsets are UTF-8 bytes from the start of the Document, because that is
//! what the parser emits.

use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Tag};

use crate::markdown;

/// What an Annotator says a range of bytes is.
///
/// One construct at a time: the tickets that follow #86 add emphasis, lists,
/// code, quotes and links to this enum, and the app's tag table grows a row
/// for each without the mapping changing shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    /// Delimiter bytes: a heading's `#`s and the space after them, its
    /// optional closing `#`s, and the backslash of an escape. Drawn in the
    /// marker grey, and the only thing that ever hangs into the margin.
    Markup,
    /// The text of a heading, at its level, 1 to 6. Bold at body size: the
    /// level reads from the markers, so nothing about the text image jumps
    /// when a `#` is typed or deleted.
    Heading(u8),
}

/// One Annotator's judgement about one byte range of a Document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Span {
    /// Absolute UTF-8 bytes from the start of the Document.
    pub at: Range<usize>,
    /// What those bytes are.
    pub mark: Mark,
}

impl Span {
    /// A span of `mark` over `at`.
    #[must_use]
    pub fn new(at: Range<usize>, mark: Mark) -> Self {
        Self { at, mark }
    }
}

/// The Markup spans of `text`, in the order the bytes appear.
///
/// Spans never overlap, because they are the two halves of one partition: the
/// bytes a content event covers, and the bytes it does not.
///
/// #86 derives one construct, the ATX heading. Every later construct is
/// another arm on the same subtraction, so the shape here is the shape they
/// all take.
#[must_use]
pub fn markup(text: &str) -> Vec<Span> {
    let events: Vec<_> = markdown::events(text).collect();
    let mut spans = Vec::new();
    let mut at_event = 0;
    while at_event < events.len() {
        let (event, at) = &events[at_event];
        at_event += 1;
        let Event::Start(Tag::Heading { level, .. }) = event else {
            continue;
        };
        // One forward pass, not one pass per heading: the events inside a
        // construct are the ones that follow it until its own end, so the
        // subtraction costs the Document once however many headings it has.
        let mut content = Vec::new();
        while at_event < events.len() && events[at_event].1.end <= at.end {
            let (inner, inner_at) = &events[at_event];
            if markdown::is_content(inner) && inner_at.start >= at.start {
                content.push(inner_at.clone());
            }
            at_event += 1;
        }
        if is_atx(text, at) {
            heading(text, at, level_number(*level), &content, &mut spans);
        }
    }
    spans
}

/// Whether the heading at `at` is written with `#`s rather than underlined.
///
/// CommonMark has two heading syntaxes and pulldown-cmark reports one event for
/// both. Only the ATX heading has markers, so only it has anything to hang: a
/// setext title underlined with `===` sits at the prose margin already, and
/// hanging it would push it a marker's width out into the margin with nothing
/// there to fill it. #86 derives the ATX heading and leaves the other to the
/// ticket that draws its underline.
/// A heading's range begins at its first `#`, and never at the up-to-three
/// spaces CommonMark allows in front of one, so the first byte decides.
fn is_atx(text: &str, at: &Range<usize>) -> bool {
    text[at.clone()].starts_with('#')
}

/// Splits one heading's range into its markers and its text.
///
/// The parser reports the heading's whole range — markers, text, and the
/// newline that ends the line — and `content` holds a range per run of content
/// inside it. The bytes in between are the markers, which is the subtraction
/// this module is built on. The trailing newline is not markup and is trimmed
/// off, or the marker grey would run to the end of the line.
fn heading(
    text: &str,
    at: &Range<usize>,
    level: u8,
    content: &[Range<usize>],
    spans: &mut Vec<Span>,
) {
    let mut cursor = at.start;
    for run in content {
        push_markup(text, cursor..run.start, spans);
        spans.push(Span::new(run.clone(), Mark::Heading(level)));
        cursor = run.end;
    }
    push_markup(text, cursor..at.end, spans);
}

/// Adds `at` as a Markup span, once the line ending is off the end of it.
///
/// An empty range is not a span: a heading whose text runs to the end of the
/// line has no closing markers, and saying so with a zero-width span would
/// make the app apply a tag to nothing.
fn push_markup(text: &str, at: Range<usize>, spans: &mut Vec<Span>) {
    let end = text[at.clone()].trim_end_matches(['\n', '\r']).len() + at.start;
    if end > at.start {
        spans.push(Span::new(at.start..end, Mark::Markup));
    }
}

/// The level of `level` as the number a writer counts `#`s in.
fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spans of `text` as `(source text, mark)`, which is what an owner
    /// would see if the marks were drawn on the page.
    fn marked(text: &str) -> Vec<(&str, Mark)> {
        markup(text)
            .into_iter()
            .map(|span| (&text[span.at], span.mark))
            .collect()
    }

    #[test]
    fn a_heading_is_its_markers_and_its_text_at_its_level() {
        assert_eq!(
            marked("# The Lighthouse\n"),
            [("# ", Mark::Markup), ("The Lighthouse", Mark::Heading(1))]
        );
    }

    #[test]
    fn the_oracles_two_headings_are_marked_at_the_bytes_they_occupy() {
        let text = std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo");
        let spans = markup(&text);

        let first = text.find("# The Lighthouse").expect("the first heading");
        let second = text
            .find("## What the sea keeps")
            .expect("the second heading");
        assert_eq!(
            spans,
            [
                Span::new(first..first + 2, Mark::Markup),
                Span::new(first + 2..first + 16, Mark::Heading(1)),
                Span::new(second..second + 3, Mark::Markup),
                Span::new(second + 3..second + 21, Mark::Heading(2)),
            ],
            "the passage's headings are the only Markup #86 draws"
        );
        assert_eq!(&text[first..first + 2], "# ");
        assert_eq!(&text[second..second + 3], "## ");
        assert_eq!(&text[second + 3..second + 21], "What the sea keeps");
    }

    #[test]
    fn every_level_from_one_to_six_hangs_one_marker_cell_more_than_the_last() {
        for level in 1..=6u8 {
            let text = format!("{} Deep\n", "#".repeat(usize::from(level)));
            assert_eq!(
                marked(&text),
                [
                    (&text[..usize::from(level) + 1], Mark::Markup),
                    ("Deep", Mark::Heading(level))
                ],
                "level {level} is marked wrong"
            );
        }
    }

    #[test]
    fn a_closing_run_of_hashes_is_marked_with_the_space_before_it() {
        assert_eq!(
            marked("### Closing ###\n"),
            [
                ("### ", Mark::Markup),
                ("Closing", Mark::Heading(3)),
                (" ###", Mark::Markup),
            ]
        );
    }

    #[test]
    fn the_line_ending_is_never_part_of_a_marker() {
        for text in ["# One\n", "# One\r\n", "## Closing ##\n", "#\n"] {
            for span in markup(text) {
                let bytes = &text[span.at.clone()];
                assert!(!span.at.is_empty(), "{text:?} produced an empty span");
                assert!(
                    !bytes.ends_with('\n') && !bytes.ends_with('\r'),
                    "{span:?} of {text:?} swallowed the line ending: {bytes:?}"
                );
            }
        }
    }

    #[test]
    fn a_heading_with_no_text_is_all_marker() {
        assert_eq!(marked("#\n"), [("#", Mark::Markup)]);
    }

    #[test]
    fn an_escape_in_a_heading_leaves_the_backslash_marked_and_the_rest_text() {
        assert_eq!(
            marked("# A \\* star\n"),
            [
                ("# ", Mark::Markup),
                ("A ", Mark::Heading(1)),
                ("\\", Mark::Markup),
                ("* star", Mark::Heading(1)),
            ],
            "the spans must name the source bytes, backslash and all"
        );
    }

    #[test]
    fn an_entity_in_a_heading_is_spanned_by_its_source_bytes_not_its_character() {
        assert_eq!(
            marked("# Tom &amp; Jerry\n"),
            [
                ("# ", Mark::Markup),
                ("Tom ", Mark::Heading(1)),
                ("&amp;", Mark::Heading(1)),
                (" Jerry", Mark::Heading(1)),
            ],
            "the five bytes the writer typed are all heading text"
        );
    }

    #[test]
    fn prose_around_a_heading_is_left_unmarked() {
        assert_eq!(
            marked("Before.\n\n# The Lighthouse\n\nAfter.\n"),
            [("# ", Mark::Markup), ("The Lighthouse", Mark::Heading(1))],
            "#86 draws headings and nothing else"
        );
    }

    #[test]
    fn a_setext_heading_is_left_alone_because_it_has_no_marker_to_hang() {
        assert_eq!(
            marked("Title\n=====\n\nprose\n"),
            [],
            "an underlined title already sits on the prose margin: hanging it \
             would push it out into the margin with nothing to fill it"
        );
    }

    #[test]
    fn an_indented_atx_heading_is_marked_from_its_hash_not_from_its_indent() {
        let text = "   # Tucked\n";
        assert_eq!(
            marked(text),
            [("# ", Mark::Markup), ("Tucked", Mark::Heading(1))],
            "the three spaces CommonMark allows in front of a `#` are not markup"
        );
        assert_eq!(
            markup(text)[0].at.start,
            3,
            "the marker span starts at the hash the writer typed"
        );
    }

    #[test]
    fn nothing_the_other_constructs_bring_is_marked_yet() {
        assert_eq!(
            marked("*emphasis*, **strong**, `code`, [link](https://example.org)\n"),
            [],
            "emphasis, code and links stay plain until the tickets that follow"
        );
    }

    #[test]
    fn the_spans_of_a_document_never_overlap_and_run_in_order() {
        let text = std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo");
        let spans = markup(&text);
        assert!(!spans.is_empty(), "the passage has headings to mark");
        for pair in spans.windows(2) {
            assert!(
                pair[0].at.end <= pair[1].at.start,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
