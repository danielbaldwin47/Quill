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
//! A span covers its whole construct — `**bold**` including both pairs of
//! asterisks — and the markers are spanned over the top of it. Nesting is
//! therefore ordinary: a span inside another span is resolved after it, and
//! [`flatten`] hands the app runs that no longer overlap at all. #40 (Focus &
//! typewriter) and #28 (Syntax highlight) each add a tier by adding an arm to
//! [`resolve`]; nothing else about the shape moves.
//!
//! All offsets are UTF-8 bytes from the start of the Document, because that is
//! what the parser emits.

use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Tag};

use crate::markdown;

/// What an Annotator says a range of bytes is.
///
/// Every mark but [`Mark::Markup`] covers a whole construct, markers and all;
/// the markers are then spanned over the top of it, so the two are read in that
/// order and the marker wins. The tickets after #87 add the block constructs —
/// quotes, lists, fences, rules — to the same enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    /// Delimiter bytes: a heading's `#`s and the space after them, the
    /// asterisks around emphasis, a code span's backticks, a link's brackets
    /// and parentheses, and the backslash of an escape. Drawn in the marker
    /// grey, upright and at the prose's weight however deep inside a bold
    /// heading it sits, and the only thing that ever hangs into the margin.
    Markup,
    /// A heading, at its level, 1 to 6. Bold at body size: the level reads from
    /// the markers, so nothing about the text image jumps when a `#` is typed
    /// or deleted.
    Heading(u8),
    /// `*emphasis*`: italic, in the Face's own Italic cut.
    Emphasis,
    /// `**strong**`: bold, on the Faces' weight axis.
    Strong,
    /// `~~struck~~`: the marker grey, with the line through it drawn as a
    /// decoration over the run rather than resolved into it.
    Strikethrough,
    /// A code span, backticks included, so that its ground runs under them: the
    /// oracle pads the ground with a box-shadow precisely so that no glyph
    /// moves, and a ground that stops at the backticks moves none either.
    Code,
    /// A link, from its opening bracket to its closing parenthesis. The text
    /// takes the link colour; the brackets and the destination inside it are
    /// spanned over the top in the marker grey.
    Link,
    /// An image, `![alt](src)`: a link whose text is the alt text.
    Image,
    /// A link's destination and title: plumbing, in the marker grey.
    Url,
    /// Inline or block HTML: not prose, so it stays in the marker grey.
    Html,
}

/// The colour a run's text is drawn in, named by the role it plays.
///
/// A role rather than a colour because the engine cannot see a display: the app
/// holds the three constants (`docs/architecture.md` § Annotators), and the
/// Dark & light ticket moves them into the palette table without this enum
/// noticing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Ink {
    /// The ink prose is set in.
    Prose,
    /// The marker grey.
    Marker,
    /// The link colour.
    Link,
}

/// The weight a run is set at, on the Faces' variable axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Weight {
    /// The weight the prose is set at.
    Regular,
    /// Bold: headings and strong.
    Bold,
}

/// Whether a run is upright or slanted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Slant {
    /// Upright: the Face's Roman.
    Upright,
    /// Italic: the Face's Italic, which is a family of its own (ADR 0007) and
    /// not a slanted Roman.
    Italic,
}

/// What a run is drawn on.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Ground {
    /// The page.
    Page,
    /// The code ground.
    Code,
}

/// Everything the app needs to draw one run, with nothing left to resolve.
///
/// The tag table is keyed by these: one tag per `(ink, alpha)`, one per
/// `(weight, slant)`, one per ground. Overlaps are gone by the time a `Look`
/// exists, which is what the flattening is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Look {
    /// The colour of the text, as a role.
    pub ink: Ink,
    /// How opaque that colour is, 0 to 255. One value until #40 dims what is
    /// out of focus; the key is written for it now, so that the table does not
    /// have to be re-keyed then.
    pub alpha: u8,
    /// The weight the run is set at.
    pub weight: Weight,
    /// Whether the run is slanted.
    pub slant: Slant,
    /// What the run is drawn on.
    pub ground: Ground,
}

impl Look {
    /// Fully opaque, which everything #87 draws is.
    pub const OPAQUE: u8 = u8::MAX;

    /// Plain prose: what a run is before any mark is resolved into it.
    pub const PROSE: Self = Self {
        ink: Ink::Prose,
        alpha: Self::OPAQUE,
        weight: Weight::Regular,
        slant: Slant::Upright,
        ground: Ground::Page,
    };
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

/// A stretch of a Document that one set of tags is applied to.
///
/// Runs never overlap and run in order, and only bytes some span covered are in
/// one: plain prose is left as the buffer already draws it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
    /// Absolute UTF-8 bytes from the start of the Document.
    pub at: Range<usize>,
    /// How those bytes are drawn.
    pub look: Look,
}

/// The Markup spans of `text`, in the order the bytes appear.
///
/// Spans nest — a `Strong` inside a `Heading`, an `Emphasis` inside that — and
/// a nested span is always wholly inside its parent rather than straddling it,
/// because that is what the parser's own nesting guarantees. [`flatten`] turns
/// them into runs.
#[must_use]
pub fn markup(text: &str) -> Vec<Span> {
    let Reading {
        constructs,
        content,
    } = read(text);
    let mut spans = Vec::new();
    for (index, (at, mark)) in constructs.iter().enumerate() {
        push_span(text, at.clone(), *mark, &mut spans);
        markers(text, index, &constructs, &content, &mut spans);
    }
    escapes(text, &content, &mut spans);
    spans.sort_by(|a, b| a.at.start.cmp(&b.at.start).then(b.at.end.cmp(&a.at.end)));
    spans.dedup();
    spans
}

/// `spans` resolved into runs that do not overlap.
///
/// A span's mark is resolved over whatever the spans containing it resolved to,
/// outermost first, so that a marker inside a bold heading comes out grey and
/// at the prose's weight while the heading around it stays bold. Runs that come
/// out alike and touching are one run: the app applies tags per run, and two
/// runs where one would do is work on every keystroke for nothing.
#[must_use]
pub fn flatten(spans: &[Span]) -> Vec<Run> {
    let mut nested: Vec<&Span> = spans.iter().collect();
    nested.sort_by(|a, b| a.at.start.cmp(&b.at.start).then(b.at.end.cmp(&a.at.end)));
    let mut runs = Vec::new();
    let mut open: Vec<(usize, Look)> = Vec::new();
    let mut cursor = 0;
    for span in nested {
        while let Some(&(end, look)) = open.last() {
            if end > span.at.start {
                break;
            }
            push_run(&mut runs, cursor..end, look);
            cursor = end;
            open.pop();
        }
        if let Some(&(_, look)) = open.last() {
            push_run(&mut runs, cursor..span.at.start, look);
        }
        cursor = span.at.start;
        let under = open.last().map_or(Look::PROSE, |&(_, look)| look);
        open.push((span.at.end, resolve(span.mark, under)));
    }
    while let Some((end, look)) = open.pop() {
        push_run(&mut runs, cursor..end, look);
        cursor = end;
    }
    runs
}

/// `mark` resolved over the look of the spans it sits inside.
///
/// Each mark sets the properties it is about and leaves the rest as it found
/// them, which is what makes nesting work: emphasis inside strong sets the
/// slant and keeps the weight. The one mark that resets is [`Mark::Markup`],
/// because a marker is grey, upright and at the prose's weight wherever it
/// sits — `legacy/app/css/markup.css` says it in one line, `.md-mark { color:
/// var(--mark); font-weight: 400; font-style: normal }`.
fn resolve(mark: Mark, under: Look) -> Look {
    match mark {
        Mark::Markup => Look {
            ink: Ink::Marker,
            weight: Weight::Regular,
            slant: Slant::Upright,
            ..under
        },
        Mark::Heading(_) | Mark::Strong => Look {
            weight: Weight::Bold,
            ..under
        },
        Mark::Emphasis => Look {
            slant: Slant::Italic,
            ..under
        },
        Mark::Strikethrough | Mark::Url | Mark::Html => Look {
            ink: Ink::Marker,
            ..under
        },
        // The ground, and with it the ink the oracle sets back to the prose's
        // (`.md-code { color: var(--fg) }`), so that code inside a struck or
        // linked run still reads as code. Nothing else: a code span is padded
        // by its ground, never by moving a glyph.
        Mark::Code => Look {
            ink: Ink::Prose,
            ground: Ground::Code,
            ..under
        },
        Mark::Link | Mark::Image => Look {
            ink: Ink::Link,
            ..under
        },
    }
}

/// Adds `at` as a run, or lengthens the last one if it is the same look.
fn push_run(runs: &mut Vec<Run>, at: Range<usize>, look: Look) {
    if at.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut()
        && last.at.end == at.start
        && last.look == look
    {
        last.at.end = at.end;
        return;
    }
    runs.push(Run { at, look });
}

/// The constructs of `text`, and the bytes inside them that are not markup.
///
/// One pass over the events. A construct is a range and the mark it carries; a
/// content range is one the parser says holds the writer's words rather than
/// the punctuation that shapes them. The markers are the difference between the
/// two, which is the subtraction this module is built on: no delimiter span is
/// ever expected from the parser.
fn read(text: &str) -> Reading {
    let mut constructs = Vec::new();
    let mut content = Vec::new();
    for (event, at) in markdown::events(text) {
        match &event {
            Event::Start(tag) => {
                let Some(mark) = tag_mark(tag, text, &at) else {
                    continue;
                };
                constructs.push((at.clone(), mark));
                if let Some(url) = destination(text, &at) {
                    // Both lists: a destination carries a mark of its own, and
                    // it is not markup, so the link's markers come out as the
                    // brackets and parentheses around it and nothing else.
                    constructs.push((url.clone(), Mark::Url));
                    content.push(url);
                }
            }
            // A code span's event covers its backticks, and its ground has to
            // as well, so the construct is the whole of it and the content is
            // only the text between them.
            Event::Code(_) => {
                constructs.push((at.clone(), Mark::Code));
                content.push(code_text(text, &at));
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                constructs.push((at.clone(), Mark::Html));
                content.push(at);
            }
            Event::Text(_) => content.push(at),
            _ => {}
        }
    }
    content.sort_by_key(|run| run.start);
    Reading {
        constructs,
        content,
    }
}

/// What one pass over the events found.
struct Reading {
    /// Every construct #87 draws: its range, and the mark it carries.
    constructs: Vec<(Range<usize>, Mark)>,
    /// The ranges inside them that hold the writer's words rather than the
    /// punctuation that shapes them.
    content: Vec<Range<usize>>,
}

/// The mark `tag` carries, or `None` for a construct #87 does not draw.
fn tag_mark(tag: &Tag<'_>, text: &str, at: &Range<usize>) -> Option<Mark> {
    match tag {
        Tag::Heading { level, .. } => is_atx(text, at).then(|| Mark::Heading(level_number(*level))),
        Tag::Emphasis => Some(Mark::Emphasis),
        Tag::Strong => Some(Mark::Strong),
        Tag::Strikethrough => Some(Mark::Strikethrough),
        Tag::Link { .. } => Some(Mark::Link),
        Tag::Image { .. } => Some(Mark::Image),
        _ => None,
    }
}

/// Whether the heading at `at` is written with `#`s rather than underlined.
///
/// CommonMark has two heading syntaxes and pulldown-cmark reports one event for
/// both. Only the ATX heading has markers, so only it has anything to hang: a
/// setext title underlined with `===` sits at the prose margin already, and
/// hanging it would push it a marker's width out into the margin with nothing
/// there to fill it. #86 derived the ATX heading and left the other to the
/// ticket that draws its underline.
/// A heading's range begins at its first `#`, and never at the up-to-three
/// spaces CommonMark allows in front of one, so the first byte decides.
fn is_atx(text: &str, at: &Range<usize>) -> bool {
    text[at.clone()].starts_with('#')
}

/// The text of the code span at `at`, without the backticks on either side.
///
/// CommonMark allows any number of backticks as long as the two runs are the
/// same length, so the opening run is counted and that many are taken off both
/// ends.
fn code_text(text: &str, at: &Range<usize>) -> Range<usize> {
    let ticks = text[at.clone()]
        .bytes()
        .take_while(|byte| *byte == b'`')
        .count();
    let start = at.start + ticks;
    start..at.end.saturating_sub(ticks).max(start)
}

/// The destination and title of the link or image at `at`, if it has one
/// written out.
///
/// Read backwards from the closing bracket, because that is the end whose shape
/// is unambiguous: `](` and `)` for an inline link, `][` and `]` for a
/// reference. A destination may hold balanced parentheses, so the depth is
/// counted. A shortcut link — `[text]`, defined elsewhere — has no destination
/// on the line at all, and the opener it finds is its own first bracket, which
/// is not preceded by a `]`: that is how the two are told apart. An autolink
/// ends in `>` and stops at the first test, because its text *is* its
/// destination and takes the link colour rather than the marker grey.
fn destination(text: &str, at: &Range<usize>) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let (open, close) = match bytes.get(at.end.checked_sub(1)?)? {
        b')' => (b'(', b')'),
        b']' => (b'[', b']'),
        _ => return None,
    };
    let mut depth = 0usize;
    let mut index = at.end - 1;
    let opener = loop {
        if bytes[index] == close {
            depth += 1;
        } else if bytes[index] == open {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                break index;
            }
        }
        index = index.checked_sub(1).filter(|next| *next >= at.start)?;
    };
    if opener == at.start || bytes[opener - 1] != b']' {
        return None;
    }
    let url = opener + 1..at.end - 1;
    (!url.is_empty()).then_some(url)
}

/// Adds the marker spans of the construct at `index`.
///
/// Its own bytes, less the content inside it and less the constructs nested in
/// it: what is left is punctuation. Taking the nested constructs out is what
/// keeps a heading from claiming the asterisks of the strong inside it — those
/// are the strong's markers, and the strong spans them itself.
fn markers(
    text: &str,
    index: usize,
    constructs: &[(Range<usize>, Mark)],
    content: &[Range<usize>],
    spans: &mut Vec<Span>,
) {
    let at = &constructs[index].0;
    let mut covered: Vec<Range<usize>> = content
        .iter()
        .filter(|run| run.start < at.end && run.end > at.start)
        .cloned()
        .collect();
    covered.extend(
        constructs
            .iter()
            .enumerate()
            .filter(|(other, (run, _))| {
                *other != index && run.start >= at.start && run.end <= at.end
            })
            .map(|(_, (run, _))| run.clone()),
    );
    covered.sort_by_key(|run| run.start);
    let mut cursor = at.start;
    for run in covered {
        if run.start > cursor {
            push_span(text, cursor..run.start, Mark::Markup, spans);
        }
        cursor = cursor.max(run.end).min(at.end);
    }
    push_span(text, cursor..at.end, Mark::Markup, spans);
}

/// Adds a marker span for every backslash that escapes the byte after it.
///
/// An escape is not a construct and has no event of its own: the parser starts
/// the text at the character being escaped and leaves the backslash uncovered.
/// Inside a construct the subtraction has it already; in plain prose nothing
/// else would, so it is looked for here rather than left to whichever construct
/// happens to be around it.
fn escapes(text: &str, content: &[Range<usize>], spans: &mut Vec<Span>) {
    for run in content {
        let Some(before) = run.start.checked_sub(1) else {
            continue;
        };
        if text.as_bytes()[before] == b'\\' {
            spans.push(Span::new(before..run.start, Mark::Markup));
        }
    }
}

/// Adds `at` as a span of `mark`, once the line ending is off the end of it.
///
/// An empty range is not a span: a heading whose text runs to the end of the
/// line has no closing markers, and saying so with a zero-width span would make
/// the app apply a tag to nothing. The line ending goes for the reason it
/// always did — a mark that swallowed it would run to the end of the line on
/// screen.
fn push_span(text: &str, at: Range<usize>, mark: Mark, spans: &mut Vec<Span>) {
    let end = text[at.clone()].trim_end_matches(['\n', '\r']).len() + at.start;
    if end > at.start {
        spans.push(Span::new(at.start..end, mark));
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

    /// The runs of `text` as `(source text, look)`: what the page shows.
    fn drawn(text: &str) -> Vec<(&str, Look)> {
        flatten(&markup(text))
            .into_iter()
            .map(|run| (&text[run.at], run.look))
            .collect()
    }

    /// A look that is prose but for the fields the caller names.
    const fn like_prose(weight: Weight, slant: Slant) -> Look {
        Look {
            weight,
            slant,
            ..Look::PROSE
        }
    }

    /// The marker grey, upright, at the prose's weight: what every marker is.
    const MARKER: Look = Look {
        ink: Ink::Marker,
        ..Look::PROSE
    };

    /// The judged Markup passage, which the Piece is shot on.
    fn oracle() -> String {
        std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo")
    }

    #[test]
    fn a_heading_is_its_markers_and_its_text_at_its_level() {
        assert_eq!(
            marked("# The Lighthouse\n"),
            [("# The Lighthouse", Mark::Heading(1)), ("# ", Mark::Markup)],
            "the heading spans the whole line and the markers span the head of it"
        );
        assert_eq!(
            drawn("# The Lighthouse\n"),
            [
                ("# ", MARKER),
                ("The Lighthouse", like_prose(Weight::Bold, Slant::Upright)),
            ]
        );
    }

    #[test]
    fn every_level_from_one_to_six_hangs_one_marker_cell_more_than_the_last() {
        for level in 1..=6u8 {
            let text = format!("{} Deep\n", "#".repeat(usize::from(level)));
            assert_eq!(
                marked(&text),
                [
                    (text.trim_end(), Mark::Heading(level)),
                    (&text[..usize::from(level) + 1], Mark::Markup),
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
                ("### Closing ###", Mark::Heading(3)),
                ("### ", Mark::Markup),
                (" ###", Mark::Markup),
            ]
        );
    }

    #[test]
    fn the_line_ending_is_never_part_of_a_span() {
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
        assert_eq!(drawn("#\n"), [("#", MARKER)]);
    }

    #[test]
    fn an_escape_leaves_the_backslash_marked_and_the_rest_text() {
        assert_eq!(
            drawn("A \\* star, not emphasis\n"),
            [("\\", MARKER)],
            "in plain prose the backslash is the only thing drawn"
        );
        assert_eq!(
            drawn("# A \\* star\n"),
            [
                ("# ", MARKER),
                ("A ", like_prose(Weight::Bold, Slant::Upright)),
                ("\\", MARKER),
                ("* star", like_prose(Weight::Bold, Slant::Upright)),
            ],
            "inside a heading the escape is one grey byte in a bold line"
        );
    }

    #[test]
    fn an_entity_is_drawn_as_the_source_bytes_the_writer_typed() {
        assert_eq!(
            drawn("# Tom &amp; Jerry\n"),
            [
                ("# ", MARKER),
                ("Tom &amp; Jerry", like_prose(Weight::Bold, Slant::Upright)),
            ],
            "the five bytes of the entity are heading text like the rest"
        );
    }

    #[test]
    fn prose_around_a_heading_is_left_unmarked() {
        assert_eq!(
            marked("Before.\n\n# The Lighthouse\n\nAfter.\n"),
            [("# The Lighthouse", Mark::Heading(1)), ("# ", Mark::Markup)],
            "prose is drawn as the buffer already draws it"
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
            markup(text)[0].at.start,
            3,
            "the three spaces CommonMark allows in front of a `#` are not markup"
        );
    }

    #[test]
    fn emphasis_is_italic_and_strong_is_bold_with_their_markers_grey() {
        assert_eq!(
            drawn("It was a *small thing*, and **not moving**.\n"),
            [
                ("*", MARKER),
                ("small thing", like_prose(Weight::Regular, Slant::Italic)),
                ("*", MARKER),
                ("**", MARKER),
                ("not moving", like_prose(Weight::Bold, Slant::Upright)),
                ("**", MARKER),
            ]
        );
    }

    #[test]
    fn strong_around_emphasis_inside_a_heading_flattens_to_three_runs() {
        assert_eq!(
            drawn("## **a *b* c**\n"),
            [
                ("## **", MARKER),
                ("a ", like_prose(Weight::Bold, Slant::Upright)),
                ("*", MARKER),
                ("b", like_prose(Weight::Bold, Slant::Italic)),
                ("*", MARKER),
                (" c", like_prose(Weight::Bold, Slant::Upright)),
                ("**", MARKER),
            ],
            "bold, bold-italic, bold, with every marker in the marker grey and \
             back at the prose's weight"
        );
    }

    #[test]
    fn a_code_span_takes_the_ground_under_its_backticks_and_nothing_else() {
        let ground = Look {
            ground: Ground::Code,
            ..Look::PROSE
        };
        assert_eq!(
            drawn("the bearing, `NNE 22\u{b0}`, and\n"),
            [
                (
                    "`",
                    Look {
                        ground: Ground::Code,
                        ..MARKER
                    }
                ),
                ("NNE 22\u{b0}", ground),
                (
                    "`",
                    Look {
                        ground: Ground::Code,
                        ..MARKER
                    }
                ),
            ],
            "the ground runs under the backticks so that no glyph has to move"
        );
        assert_eq!(
            ground,
            Look {
                ground: Ground::Code,
                ..Look::PROSE
            },
            "the code text differs from prose in its ground and in nothing else"
        );
    }

    #[test]
    fn a_code_span_written_with_two_backticks_keeps_both_of_them_grey() {
        assert_eq!(
            marked("a ``x ` y`` span\n"),
            [
                ("``x ` y``", Mark::Code),
                ("``", Mark::Markup),
                ("``", Mark::Markup),
            ],
            "the closing run is as long as the opening one, whatever is between"
        );
    }

    #[test]
    fn a_links_text_takes_the_link_colour_and_its_plumbing_the_marker_grey() {
        let text = "in [the keeper's book](https://example.org/keepers-book), and\n";
        assert_eq!(
            marked(text),
            [
                (
                    "[the keeper's book](https://example.org/keepers-book)",
                    Mark::Link
                ),
                ("[", Mark::Markup),
                ("](", Mark::Markup),
                ("https://example.org/keepers-book", Mark::Url),
                (")", Mark::Markup),
            ]
        );
        assert_eq!(
            drawn(text),
            [
                ("[", MARKER),
                (
                    "the keeper's book",
                    Look {
                        ink: Ink::Link,
                        ..Look::PROSE
                    }
                ),
                ("](https://example.org/keepers-book)", MARKER),
            ],
            "the words are the link; the address is plumbing and stays grey"
        );
    }

    #[test]
    fn a_reference_link_marks_its_label_and_a_shortcut_has_nothing_to_mark() {
        assert_eq!(
            marked("[ref][r] and [short]\n\n[r]: /x\n"),
            [
                ("[ref][r]", Mark::Link),
                ("[", Mark::Markup),
                ("][", Mark::Markup),
                ("r", Mark::Url),
                ("]", Mark::Markup),
            ],
            "`[short]` has no definition, so the parser says it is not a link \
             at all; a shortcut that had one would carry no destination here \
             either, and its brackets alone would be marked"
        );
    }

    #[test]
    fn an_autolinks_own_text_is_the_link_and_only_the_angles_are_markers() {
        assert_eq!(
            drawn("see <https://example.org> now\n"),
            [
                ("<", MARKER),
                (
                    "https://example.org",
                    Look {
                        ink: Ink::Link,
                        ..Look::PROSE
                    }
                ),
                (">", MARKER),
            ]
        );
    }

    #[test]
    fn an_images_alt_text_reads_as_a_links_does() {
        assert_eq!(
            marked("an ![alt](/pic.png \"t\") image\n"),
            [
                ("![alt](/pic.png \"t\")", Mark::Image),
                ("![", Mark::Markup),
                ("](", Mark::Markup),
                ("/pic.png \"t\"", Mark::Url),
                (")", Mark::Markup),
            ],
            "the title travels with the destination: both are plumbing"
        );
    }

    #[test]
    fn struck_text_and_inline_html_are_the_marker_grey() {
        assert_eq!(
            drawn("a ~~struck~~ word\n"),
            [("~~struck~~", MARKER)],
            "the grey is resolved here; the line through it is a decoration \
             the app layers over the run"
        );
        assert_eq!(
            drawn("a <b>bold</b> tag\n"),
            [("<b>", MARKER), ("</b>", MARKER)],
            "the tags are not prose; the word between them is, and prose is \
             left as the buffer already draws it"
        );
    }

    #[test]
    fn every_inline_construct_of_the_oracles_passage_is_spanned() {
        let text = oracle();
        let marks = marked(&text);
        for wanted in [
            ("# The Lighthouse", Mark::Heading(1)),
            ("## What the sea keeps", Mark::Heading(2)),
            ("*small thing*", Mark::Emphasis),
            ("**not moving**", Mark::Strong),
            ("`NNE 22\u{b0}`", Mark::Code),
            (
                "[the keeper's book](https://example.org/keepers-book)",
                Mark::Link,
            ),
            ("https://example.org/keepers-book", Mark::Url),
            ("[", Mark::Markup),
            ("](", Mark::Markup),
            (")", Mark::Markup),
        ] {
            assert!(
                marks.contains(&wanted),
                "the judged passage is missing {wanted:?}"
            );
        }
    }

    #[test]
    fn the_runs_of_a_document_never_overlap_and_run_in_order() {
        let text = oracle();
        let runs = flatten(&markup(&text));
        assert!(!runs.is_empty(), "the passage has Markup to draw");
        for pair in runs.windows(2) {
            assert!(
                pair[0].at.end <= pair[1].at.start,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
        for run in &runs {
            assert!(
                !run.at.is_empty() && run.at.end <= text.len(),
                "{run:?} is not a range of the Document"
            );
        }
    }

    #[test]
    fn a_nested_span_is_wholly_inside_the_one_around_it() {
        let text = "## **a *b* `c`** and [a *b*](/u)\n";
        let spans = markup(text);
        for (outer, inner) in spans.iter().zip(spans.iter().skip(1)) {
            let apart = inner.at.start >= outer.at.end;
            let within = inner.at.start >= outer.at.start && inner.at.end <= outer.at.end;
            assert!(
                apart || within,
                "{inner:?} straddles {outer:?}, which flattening cannot resolve"
            );
        }
    }

    #[test]
    fn a_document_with_no_markup_is_drawn_as_it_is() {
        assert_eq!(
            drawn("Just prose, and a lamp, and the sea.\n"),
            [],
            "a run exists to change something: prose changes nothing"
        );
    }
}
