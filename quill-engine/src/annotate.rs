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

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag};

use crate::markdown;

/// What an Annotator says a range of bytes is.
///
/// A construct's mark covers the whole of it, markers and all; the markers are
/// then spanned over the top, so the two are read in that order and the marker
/// wins. The block constructs — quotes, lists, fences, rules, front matter —
/// join the inline ones here rather than in a vocabulary of their own, because
/// the app resolves all of them through the one [`resolve`] and the one tag
/// table.
///
/// A few marks are the markers themselves rather than a construct, and they are
/// the ones whose *width* the app needs: a list marker is hung by its own cells,
/// so it has to arrive as a span the app can measure.
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
    /// A block quote, from its first `>` to the end of its last line. It draws
    /// nothing itself — the oracle sets quoted text in full ink
    /// (`.md-quote { color: var(--fg) }`) — and exists so that the subtraction
    /// has a construct to take the writer's words out of, leaving the `>`s.
    /// #37 asks only that the markers go quiet, so there is no margin rule: the
    /// oracle draws one with a CSS pseudo-element because a `<textarea>` cannot
    /// indent a line, and the Editor is under no such constraint.
    Quote,
    /// The `>` of a quoted line, and the space after it. One per line, however
    /// deep the nesting: `> > ` is two quotes' markers, not one.
    QuoteMarker,
    /// A bullet list item's marker: the `-`, `*` or `+`, the whitespace before
    /// it and the whitespace after it, so that its width is the whole distance
    /// from the line's start to the item's first word. That width is what the
    /// app hangs the line by, which is why the indentation is inside the span.
    BulletMarker,
    /// An ordered list item's marker — `1. `, `2) ` — measured the same way and
    /// for the same reason. It is a mark of its own rather than a width on
    /// [`Mark::BulletMarker`] because the two are different punctuation, and a
    /// later ticket may well draw them differently.
    OrderedMarker,
    /// A task list's `[ ]` or `[x]`, without the space after it.
    TaskBox,
    /// A fenced or indented code block, fences and all, so that its ground runs
    /// under them. The text keeps the prose's ink — the oracle's
    /// `.md-codeblock { color: var(--fg) }` — and the ground is a paragraph
    /// background the app puts on the block's lines, not a run property, which
    /// is what lets it run past both edges of the measure.
    CodeBlock,
    /// The backticks or tildes of an opening or closing fence.
    Fence,
    /// The info string after an opening fence: the `rust` of ```` ```rust ````.
    InfoString,
    /// A thematic break: the `---`, `***` or `___` on a line of its own.
    ThematicBreak,
    /// A front-matter block, its `---` delimiters and the metadata between
    /// them. Quiet whole: it is the file's plumbing rather than its prose, and
    /// it never reaches the prose stream at all ([`crate::markdown::prose`]).
    FrontMatter,
    /// The label of a link-reference or footnote definition — the `[ref]:` of
    /// `[ref]: https://example.org`, the `[^1]:` of a footnote. Read
    /// **lexically**, from the shape of the line: whether the label resolves to
    /// anything is a whole-document question that belongs to an idle pass, and
    /// dimming punctuation must never wait on one.
    DefinitionLabel,
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
    /// Fully opaque, which everything the Markup Annotator draws is.
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
    definitions(text, &constructs, &mut spans);
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
        debug_assert!(
            open.last()
                .is_none_or(|&(end, _)| span.at.start >= end || span.at.end <= end),
            "{span:?} straddles the span it is inside, which flattening cannot resolve"
        );
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
        // Every marker, inline or block, is the same three properties set the
        // same way. The ground is not among them: a fence's backticks sit on
        // the code ground like the rest of the block.
        Mark::Markup
        | Mark::QuoteMarker
        | Mark::BulletMarker
        | Mark::OrderedMarker
        | Mark::TaskBox
        | Mark::Fence
        | Mark::InfoString
        | Mark::ThematicBreak
        | Mark::FrontMatter
        | Mark::DefinitionLabel => Look {
            ink: Ink::Marker,
            weight: Weight::Regular,
            slant: Slant::Upright,
            ..under
        },
        // A quote draws nothing: its words are the writer's, in the writer's
        // ink, and only its `>`s go quiet.
        Mark::Quote => under,
        // Code block text is prose ink on the page — the ground is a paragraph
        // background the app applies from the mark, so that it can run past the
        // measure, which a run property could never do.
        Mark::CodeBlock => Look {
            ink: Ink::Prose,
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
            // A list item draws nothing of its own: its marker does, and the
            // marker is not the item's range — the parser starts an item at its
            // bullet and leaves the indentation that nests it outside. So the
            // marker is measured off the line instead.
            Event::Start(Tag::Item) => {
                if let Some((marker, mark)) = list_marker(text, &at) {
                    constructs.push((marker.clone(), mark));
                    content.push(marker);
                }
            }
            Event::Start(tag) => {
                let Some(mark) = tag_mark(tag, text, &at) else {
                    continue;
                };
                constructs.push((at.clone(), mark));
                // A fence is punctuation inside the block rather than around
                // it, so it is read off the text the way a destination is.
                if let Tag::CodeBlock(CodeBlockKind::Fenced(_)) = tag {
                    for (fence, mark) in fences(text, &at) {
                        constructs.push((fence.clone(), mark));
                        content.push(fence);
                    }
                }
                // Only a link has one. Asking every construct would find the
                // parentheses of a link *inside* a quote and claim them twice.
                if matches!(mark, Mark::Link | Mark::Image)
                    && let Some(url) = destination(text, &at)
                {
                    // Both lists: a destination carries a mark of its own, and
                    // it is not markup, so the link's markers come out as the
                    // brackets and parentheses around it and nothing else.
                    constructs.push((url.clone(), Mark::Url));
                    content.push(url);
                }
            }
            Event::Rule => {
                constructs.push((at.clone(), Mark::ThematicBreak));
                content.push(at);
            }
            Event::TaskListMarker(_) => {
                constructs.push((at.clone(), Mark::TaskBox));
                content.push(at);
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
    /// Every construct the Markup Annotator draws: its range, and its mark.
    constructs: Vec<(Range<usize>, Mark)>,
    /// The ranges an enclosing construct must not claim: the writer's words,
    /// and the punctuation that already carries a mark of its own. A link's
    /// destination is in here for the second reason, and so is every block
    /// marker, which is what stops a quote from claiming the bullet of a list
    /// inside it.
    content: Vec<Range<usize>>,
}

/// The mark `tag` carries, or `None` for a tag the Editor draws nothing for.
fn tag_mark(tag: &Tag<'_>, text: &str, at: &Range<usize>) -> Option<Mark> {
    match tag {
        Tag::Heading { level, .. } => is_atx(text, at).then(|| Mark::Heading(level_number(*level))),
        Tag::Emphasis => Some(Mark::Emphasis),
        Tag::Strong => Some(Mark::Strong),
        Tag::Strikethrough => Some(Mark::Strikethrough),
        Tag::Link { .. } => Some(Mark::Link),
        Tag::Image { .. } => Some(Mark::Image),
        Tag::BlockQuote(_) => Some(Mark::Quote),
        Tag::CodeBlock(_) => Some(Mark::CodeBlock),
        Tag::MetadataBlock(_) => Some(Mark::FrontMatter),
        _ => None,
    }
}

/// The mark the delimiter bytes of a construct marked `mark` carry.
///
/// Almost always [`Mark::Markup`], because almost every delimiter is drawn the
/// one way. A quote is the exception the app needs named: its `>`s are the only
/// markers that repeat down a construct rather than closing it, and a later
/// ticket dims them by tier.
const fn marker_mark(mark: Mark) -> Mark {
    match mark {
        Mark::Quote => Mark::QuoteMarker,
        _ => Mark::Markup,
    }
}

/// The marker of the list item at `at`, and which kind of list it is.
///
/// The span reaches back to the start of the line whenever only whitespace
/// stands between, because its width is what the app hangs the line by and a
/// nested item hangs by its indentation too. When something else stands there —
/// `> - quoted`, where the `>` belongs to the quote — the span starts at the
/// item, so that the two markers are not claimed by one of them.
///
/// Ordered or bullet is read from the first byte, exactly as the oracle's
/// `RE_LIST` reads it: a digit opens `1.` or `2)`, anything else is `-`, `*`
/// or `+`.
fn list_marker(text: &str, at: &Range<usize>) -> Option<(Range<usize>, Mark)> {
    let item = text.get(at.clone())?;
    let digits = item.bytes().take_while(u8::is_ascii_digit).count();
    let (punctuation, mark) = if digits == 0 {
        (1, Mark::BulletMarker)
    } else {
        // The digits and the `.` or `)` that closes them.
        (digits + 1, Mark::OrderedMarker)
    };
    let after = item.get(punctuation..)?;
    let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
    let line = text[..at.start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let start = if text[line..at.start]
        .bytes()
        .all(|byte| byte == b' ' || byte == b'\t')
    {
        line
    } else {
        at.start
    };
    Some((start..at.start + punctuation + spaces, mark))
}

/// The fences of the fenced code block at `at`, and its info string.
///
/// The parser's range runs from the first backtick to the last, so both fences
/// are inside it and the ground runs under them. The opening run's length is
/// counted rather than assumed: CommonMark allows any number from three up, and
/// tildes as well as backticks.
fn fences(text: &str, at: &Range<usize>) -> Vec<(Range<usize>, Mark)> {
    let block = &text[at.clone()];
    let Some(fence) = block
        .bytes()
        .next()
        .filter(|byte| matches!(byte, b'`' | b'~'))
    else {
        return Vec::new();
    };
    let open = block.bytes().take_while(|byte| *byte == fence).count();
    let mut found = vec![(at.start..at.start + open, Mark::Fence)];
    let line = block[open..]
        .find('\n')
        .map_or(block.len(), |newline| open + newline);
    let rest = &block[open..line];
    let info = rest.trim();
    if !info.is_empty() {
        let start = at.start + open + (rest.len() - rest.trim_start().len());
        found.push((start..start + info.len(), Mark::InfoString));
    }
    // A fence the writer has not closed yet ends in its own text, not in
    // backticks, and there is nothing to mark at the bottom.
    let close = block
        .bytes()
        .rev()
        .take_while(|byte| *byte == fence)
        .count();
    if close > 0 && at.end - close > at.start + open {
        found.push((at.end - close..at.end, Mark::Fence));
    }
    found
}

/// Adds a marker span for every link-reference and footnote definition label.
///
/// Lexical, and it has to be: the parser resolves link-reference definitions
/// and reports no event at all for them, so reading the line is the only way
/// they can be drawn. That is not a compromise. Whether `[ref]` *resolves* is a
/// whole-document question and belongs to the idle pass Preview (#26) brings;
/// whether it is punctuation is visible in the line by itself, and a marker
/// must never wait on a pass to go quiet.
///
/// The rule is the oracle's `RE_DEF`: up to three spaces, `[`, an optional `^`,
/// a label that neither is empty nor opens with whitespace, then `]:`. Lines a
/// verbatim construct already owns are left alone, which is what keeps a
/// definition written *as an example* inside a fence from going quiet.
fn definitions(text: &str, constructs: &[(Range<usize>, Mark)], spans: &mut Vec<Span>) {
    let verbatim: Vec<Range<usize>> = constructs
        .iter()
        .filter(|(_, mark)| matches!(mark, Mark::CodeBlock | Mark::FrontMatter))
        .map(|(at, _)| at.clone())
        .collect();
    let mut at = 0usize;
    for line in text.split_inclusive('\n') {
        if let Some(label) = definition_label(line)
            && !covers(&verbatim, at)
        {
            spans.push(Span::new(
                at + label.start..at + label.end,
                Mark::DefinitionLabel,
            ));
        }
        at += line.len();
    }
}

/// The `[ref]:` or `[^1]:` that opens `line`, if one does.
fn definition_label(line: &str) -> Option<Range<usize>> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.get(indent..)?;
    if !rest.starts_with('[') {
        return None;
    }
    let label = usize::from(rest.as_bytes().get(1) == Some(&b'^')) + 1;
    let first = rest.get(label..)?.chars().next()?;
    if first == ']' || first.is_whitespace() {
        return None;
    }
    let close = label + rest.get(label..)?.find(']')?;
    (rest.as_bytes().get(close + 1) == Some(&b':')).then(|| indent..indent + close + 2)
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
    let (at, mark) = &constructs[index];
    let marker = marker_mark(*mark);
    // Both lists are in the order they start in, so what can be inside this
    // construct is a window of each rather than the whole of either. Found by
    // binary search, because a Document's constructs would otherwise cost the
    // square of their number, and a Document is 55,000 words.
    let first = content.partition_point(|run| run.end <= at.start);
    let mut covered: Vec<Range<usize>> = content[first..]
        .iter()
        .take_while(|run| run.start < at.end)
        .cloned()
        .collect();
    let after = constructs.partition_point(|(run, _)| run.start < at.start);
    covered.extend(
        constructs[after..]
            .iter()
            .enumerate()
            .take_while(|(_, (run, _))| run.start < at.end)
            .filter(|(other, (run, _))| after + other != index && run.end <= at.end)
            .map(|(_, (run, _))| run.clone()),
    );
    covered.sort_by_key(|run| run.start);
    let mut cursor = at.start;
    for run in covered {
        if run.start > cursor {
            push_span(text, cursor..run.start, marker, spans);
        }
        cursor = cursor.max(run.end).min(at.end);
    }
    push_span(text, cursor..at.end, marker, spans);
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
        // A backslash the writer escaped is content of the run before this
        // one, not the marker of this one: `\\*` is a backslash and a star,
        // and greying the second backslash would grey one of their words.
        if text.as_bytes()[before] == b'\\' && !covers(content, before) {
            spans.push(Span::new(before..run.start, Mark::Markup));
        }
    }
}

/// Whether `byte` is inside one of `ranges`.
///
/// `ranges` must be in order and not overlap — every caller's is, because the
/// parser reports in source order — and that is what makes the one range which
/// could hold `byte` the first whose end is past it, rather than a search.
fn covers(ranges: &[Range<usize>], byte: usize) -> bool {
    debug_assert!(
        ranges.windows(2).all(|pair| pair[0].end <= pair[1].start),
        "covers reads ranges in order and not overlapping, and was handed {ranges:?}"
    );
    let at = ranges.partition_point(|run| run.end <= byte);
    ranges.get(at).is_some_and(|run| run.start <= byte)
}

/// Adds `at` as a span of `mark`, once the line ending is off the end of it.
///
/// An empty range is not a span: a heading whose text runs to the end of the
/// line has no closing markers, and saying so with a zero-width span would make
/// the app apply a tag to nothing. The line ending goes for the reason it
/// always did — a mark that swallowed it would run to the end of the line on
/// screen.
///
/// Both ends, because a construct that spans lines leaves gaps that *open* with
/// one: the bytes between two lines of a quote are `\n> `, and only the `> ` is
/// the marker.
fn push_span(text: &str, at: Range<usize>, mark: Mark, spans: &mut Vec<Span>) {
    let run = &text[at.clone()];
    let start = at.start + (run.len() - run.trim_start_matches(['\n', '\r']).len());
    let end = at.start + run.trim_end_matches(['\n', '\r']).len();
    if end > start {
        spans.push(Span::new(start..end, mark));
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
    fn an_escaped_backslash_is_the_writers_word_and_only_the_first_is_grey() {
        assert_eq!(
            drawn("A \\\\* star\n"),
            [("\\", MARKER)],
            "`\\\\*` is a backslash the writer typed and a star: one marker \
             greys the escape, and the backslash it escaped stays ink"
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
            resolve(Mark::Code, Look::PROSE),
            ground,
            "on prose the mark adds the ground and changes nothing else; the \
             ink it names is the prose's own, which shows only inside a struck \
             or linked run"
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
                ("[r]:", Mark::DefinitionLabel),
            ],
            "`[short]` has no definition, so the parser says it is not a link \
             at all; a shortcut that had one would carry no destination here \
             either, and its brackets alone would be marked. The definition \
             the link resolves against is drawn from the line's own shape, \
             which is why it is marked without the parser reporting it"
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
    fn a_block_quotes_marker_goes_quiet_and_its_words_keep_the_writers_ink() {
        let text = "> There are things the sea keeps.\n";
        assert_eq!(
            marked(text),
            [
                ("> There are things the sea keeps.", Mark::Quote),
                ("> ", Mark::QuoteMarker),
            ]
        );
        assert_eq!(
            drawn(text),
            [
                ("> ", MARKER),
                ("There are things the sea keeps.", Look::PROSE),
            ],
            "#37 asks for a quiet marker and no margin rule, so the words are untouched"
        );
    }

    #[test]
    fn each_line_of_a_quote_carries_its_own_marker_and_never_the_line_ending() {
        let text = "> one\n> two\n";
        assert_eq!(
            marked(text),
            [
                ("> one\n> two", Mark::Quote),
                ("> ", Mark::QuoteMarker),
                ("> ", Mark::QuoteMarker),
            ]
        );
    }

    #[test]
    fn a_bullet_marker_is_the_dash_and_the_space_after_it() {
        assert_eq!(
            marked("- the good knife\n"),
            [("- ", Mark::BulletMarker)],
            "the item's words are prose; only its bullet is punctuation"
        );
    }

    #[test]
    fn an_ordered_marker_is_the_number_the_dot_and_the_space() {
        assert_eq!(
            marked("1. Untie the skiff.\n"),
            [("1. ", Mark::OrderedMarker)],
            "three cells against the bullet's two, which is why they hang by different tags"
        );
    }

    #[test]
    fn a_nested_markers_span_carries_the_indentation_that_nests_it() {
        assert_eq!(
            marked("- beta\n  - nested\n"),
            [("- ", Mark::BulletMarker), ("  - ", Mark::BulletMarker)],
            "the span reaches back to the line's start, so its width is the whole hang"
        );
    }

    #[test]
    fn a_quoted_items_marker_stops_at_the_quote_marker_rather_than_swallowing_it() {
        let text = "> - quoted item\n";
        let marks = marked(text);
        assert!(
            marks.contains(&("- ", Mark::BulletMarker)),
            "the bullet is two cells, not four: the `> ` before it is the quote's, {marks:?}"
        );
        assert!(marks.contains(&("> ", Mark::QuoteMarker)), "{marks:?}");
    }

    #[test]
    fn a_task_box_is_marked_beside_the_bullet_that_carries_it() {
        assert_eq!(
            marked("- [ ] todo\n- [x] done\n"),
            [
                ("- ", Mark::BulletMarker),
                ("[ ]", Mark::TaskBox),
                ("- ", Mark::BulletMarker),
                ("[x]", Mark::TaskBox),
            ]
        );
    }

    #[test]
    fn a_fenced_block_is_ground_from_its_opening_fence_to_its_closing_one() {
        let text = "```\n21:40  lamp lit\n```\n";
        assert_eq!(
            marked(text),
            [
                ("```\n21:40  lamp lit\n```", Mark::CodeBlock),
                ("```", Mark::Fence),
                ("```", Mark::Fence),
            ],
            "the fences are inside the block, so its ground runs under them"
        );
        assert_eq!(
            drawn(text),
            [
                ("```", MARKER),
                ("\n21:40  lamp lit\n", Look::PROSE),
                ("```", MARKER),
            ],
            "the code keeps the prose's ink; the ground is a paragraph tag, not a run"
        );
    }

    #[test]
    fn a_fences_info_string_is_marked_apart_from_the_backticks() {
        assert_eq!(
            marked("```rust\nlet x = 1;\n```\n"),
            [
                ("```rust\nlet x = 1;\n```", Mark::CodeBlock),
                ("```", Mark::Fence),
                ("rust", Mark::InfoString),
                ("```", Mark::Fence),
            ]
        );
    }

    #[test]
    fn an_indented_code_block_is_ground_with_no_fence_to_mark() {
        assert_eq!(
            marked("    21:40  lamp lit\n"),
            [("21:40  lamp lit", Mark::CodeBlock)],
            "the parser leaves the four spaces outside the block, and so does the mark"
        );
    }

    #[test]
    fn a_thematic_break_is_quiet() {
        let text = "***\n";
        assert_eq!(marked(text), [("***", Mark::ThematicBreak)]);
        assert_eq!(drawn(text), [("***", MARKER)]);
    }

    #[test]
    fn front_matter_is_quiet_whole_delimiters_and_metadata_alike() {
        let text = "---\ntitle: The Lighthouse\n---\n\nThe lamp was lit.\n";
        let marks = marked(text);
        assert!(
            marks.contains(&("---\ntitle: The Lighthouse\n---", Mark::FrontMatter)),
            "{marks:?}"
        );
        assert_eq!(
            drawn(text),
            [("---\ntitle: The Lighthouse\n---", MARKER)],
            "the whole block is plumbing, and the prose below it is left alone"
        );
    }

    #[test]
    fn a_link_reference_definitions_label_is_marked_with_no_resolution_pass() {
        let marks = marked("[ref]: https://example.org\n");
        assert!(
            marks.contains(&("[ref]:", Mark::DefinitionLabel)),
            "the parser reports no event at all for a definition, so this is lexical: {marks:?}"
        );
    }

    #[test]
    fn a_footnote_definitions_label_is_marked_the_same_lexical_way() {
        let marks = marked("[^1]: a footnote\n");
        assert!(
            marks.contains(&("[^1]:", Mark::DefinitionLabel)),
            "{marks:?}"
        );
    }

    #[test]
    fn a_definition_written_inside_a_fenced_block_is_code_and_not_a_definition() {
        let marks = marked("```\n[ref]: https://example.org\n```\n");
        assert!(
            !marks.iter().any(|(_, mark)| *mark == Mark::DefinitionLabel),
            "the lexical pass must not reach inside a code block: {marks:?}"
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
            ("> ", Mark::QuoteMarker),
            ("- ", Mark::BulletMarker),
            ("1. ", Mark::OrderedMarker),
            ("2. ", Mark::OrderedMarker),
            ("```", Mark::Fence),
            (
                "```\n21:40  lamp lit\n22:35  boat sighted, no oars\n```",
                Mark::CodeBlock,
            ),
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
