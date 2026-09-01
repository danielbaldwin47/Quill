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
//! [`flatten`] hands the app runs that no longer overlap at all.
//!
//! Markup is one tier and Focus is the second. They compose in [`paint`]: the
//! Markup runs are cut at Focus's tier boundaries and each piece's `(mark,
//! tier)` is resolved to one colour by [`colour`]. Focus is the tier that ends
//! the resolving — a role cannot say "dim", because dim is a colour of the
//! ground's and not a part a mark plays — so [`paint`] hands back [`Painted`]
//! runs where [`flatten`] hands back [`Run`]s. #28 (Syntax highlight) is the
//! third tier and still adds an arm to [`resolve`]; nothing else about the
//! shape moves.
//!
//! All offsets are UTF-8 bytes from the start of the Document, because that is
//! what the parser emits.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag};

use crate::focus::{Focus, LineTiers, Tier};
use crate::markdown;
use crate::theme::{Colour, Colours, Role};

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
    /// A link, from its opening bracket to its closing parenthesis. It draws
    /// nothing itself: the Design oracle sets a link's words in the body's own
    /// ink, and what it quiets is the machinery around them —
    /// [`Mark::LinkMark`] and [`Mark::Url`], spanned over the top.
    Link,
    /// An image, `![alt](src)`: a link whose text is the alt text.
    Image,
    /// A link's destination and title: plumbing, in the link grey, and
    /// underlined, because the Design oracle underlines a URL and not the words
    /// that stand for it.
    Url,
    /// The `[`, `]`, `(` and `)` of a link or an image — a marker of its own
    /// rather than [`Mark::Markup`] because a link's punctuation goes quiet
    /// with its destination while every other marker rests at the body's ink,
    /// and because it is the one delimiter the oracle leaves un-underlined.
    LinkMark,
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
    /// The ink Markdown's markers are set in. The Design oracle rests every
    /// mark kind at the prose's own ink (#198), so on the built-in grounds this
    /// is the same colour as [`Ink::Prose`]; it stays an ink of its own because
    /// a writer's `palette` file may set the markers apart.
    ///
    /// Two runs that are not markers ride with it and move when it does:
    /// struck text and inline HTML, both of which took the marker grey for
    /// being not-prose. Neither is in the measured passage, so both follow the
    /// markers until a capture says otherwise.
    Marker,
    /// The grey a link's plumbing goes quiet in: its `[`, `]`, `(`, `)` and the
    /// destination between them. Not its words, which are the writer's.
    Link,
}

impl Ink {
    /// The palette role this ink is.
    ///
    /// The one place the flattening's three inks meet [`Colours`]. It is a
    /// mapping and not a merge because the palette answers for the whole app —
    /// chrome, rules, grounds — and the flattening only ever draws text.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Prose => Role::Ink,
            Self::Marker => Role::Mark,
            Self::Link => Role::Link,
        }
    }
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
    /// How opaque that colour is, 0 to 255. One value: Focus turned out to dim
    /// by moving to another colour of the palette rather than by thinning this
    /// one ([`paint`]), so the key is spare rather than spoken for, and
    /// [`paint`] folds it into the colour's own opacity.
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

/// How a run is drawn, with nothing left to resolve.
///
/// [`Look`] names an ink as a role because Markup alone cannot know which
/// ground it is on. Focus can: the dim tier is a colour of the palette's rather
/// than a role of Markup's, and no role survives it — a dim marker, a dim link
/// and dim prose are the one grey (`legacy/app/css/focus.css:31-40`). So the
/// tiered flattening resolves the last of it and hands the app a colour, which
/// is what `docs/architecture.md` § Annotators means by runs carrying "one
/// precomputed colour and alpha each".
///
/// The four travel together — they are one tag's worth of drawing — so they are
/// one value, and two runs are the same run when their [`Paint`]s are equal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    /// The colour the text is drawn in, opacity and all.
    pub colour: Colour,
    /// The weight the run is set at.
    pub weight: Weight,
    /// Whether the run is slanted.
    pub slant: Slant,
    /// What the run is drawn on.
    pub ground: Ground,
}

impl Paint {
    /// What `look` draws as in `tier`, on the ground `colours` is.
    fn of(look: Look, tier: Tier, colours: &Colours) -> Self {
        let mut colour = colour(look.ink, tier, colours);
        colour.alpha *= f64::from(look.alpha) / f64::from(Look::OPAQUE);
        Self {
            colour,
            weight: look.weight,
            slant: look.slant,
            // Out of focus a code span keeps its glyphs and loses its box:
            // `focus.css:42-43` sets the background transparent, so that the
            // dim is one flat grey rather than a row of lit panels.
            ground: match tier {
                Tier::Bright => look.ground,
                Tier::Dim => Ground::Page,
            },
        }
    }
}

/// A stretch of a Document, and how it is drawn.
///
/// What [`flatten`] hands back once Focus has had its say: [`Run`] is a range
/// and a [`Look`] with a role still in it, and this is a range and a [`Paint`]
/// with nothing still in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Painted {
    /// Absolute UTF-8 bytes from the start of the Document.
    pub at: Range<usize>,
    /// How those bytes are drawn.
    pub paint: Paint,
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
/// slant and keeps the weight. The marks that reset are the delimiters —
/// [`Mark::Markup`] and its two named kin — because a marker is upright and at
/// the prose's weight wherever it sits; `legacy/app/css/markup.css` says it in
/// one line, `.md-mark { color: var(--mark); font-weight: 400; font-style:
/// normal }`, and only its colour has moved since (#198).
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
        Mark::Strikethrough | Mark::Html => Look {
            ink: Ink::Marker,
            ..under
        },
        // A link's plumbing. The destination keeps whatever weight and slant it
        // sits inside, the way a struck or an HTML run does; the brackets reset
        // like any other marker, because punctuation is upright and regular
        // wherever it lands.
        Mark::Url => Look {
            ink: Ink::Link,
            ..under
        },
        Mark::LinkMark => Look {
            ink: Ink::Link,
            weight: Weight::Regular,
            slant: Slant::Upright,
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
        // A link draws nothing, the way a quote draws nothing: the words
        // between its brackets are the writer's, in the writer's ink, and only
        // its machinery goes quiet.
        Mark::Link | Mark::Image => under,
    }
}

/// The colour `ink` is drawn in, in `tier`, on the ground `colours` is.
///
/// The whole of what Focus does to the page, in one table:
///
/// | | bright | dim |
/// |---|---|---|
/// | prose | `ink` | `ink_dim` |
/// | marker | `mark` | `ink_dim` |
/// | link | `link` | `ink_dim` |
///
/// The bright column is the Markup colour untouched — Focus lights what the
/// writer is in by leaving it alone — and the dim column is one grey, because
/// out of focus nothing has a voice of its own: `focus.css:31-40` flattens
/// markers, link text, URLs, quotes, code and struck text alike to
/// `--ink-dim`. That the marker on the caret's own line holds the full `mark`
/// (`legacy/app/css/markup.css:39`, which #37 left to this ticket) is this
/// table's first row: the caret's line is bright, and bright is the Markup
/// colour.
#[must_use]
pub fn colour(ink: Ink, tier: Tier, colours: &Colours) -> Colour {
    match tier {
        Tier::Bright => colours.colour(ink.role()),
        Tier::Dim => colours.colour(Role::InkDim),
    }
}

/// `spans` flattened over `tiers`, every run carrying the colour it is drawn in.
///
/// The tiered flattening: [`flatten`] resolves Markup into runs, and this cuts
/// those runs at the tier boundaries and resolves each piece's ink and tier into
/// one colour through [`colour`]. `len` is the Document's length in bytes, and
/// `tiers` is [`crate::focus::tiers_by_line`]'s answer for where the caret is —
/// already clipped to the lines, so no run here straddles a line start either.
///
/// With Focus off the runs are the untiered ones with their roles resolved:
/// same bytes, same weights, same grounds, and the colour each role has always
/// meant. With Focus on every byte of the Document is in a run, including the
/// plain prose Markup had no reason to speak for, because the buffer's own ink
/// is the wrong colour everywhere but the one lit sentence. Focus on also takes
/// the code ground off what is dim (`focus.css:42-43`), which is why a dim code
/// span does not sit in a lit box.
#[must_use]
pub fn paint(
    spans: &[Span],
    len: usize,
    tiers: &[LineTiers],
    focus: Focus,
    colours: &Colours,
) -> Vec<Painted> {
    paint_in(spans, &(0..len), tiers, focus, colours)
}

/// [`paint`], over the bytes `at` and no others.
///
/// What a retag draws: the lines an edit or a caret move changed, rather than
/// the Document. `spans` is [`crate::document::Document::spans_in`]'s answer
/// for `at` — every span that could reach those bytes and no others before them
/// — so the cost is the block the retag is in, not the manuscript it is in the
/// middle of. Its runs are absolute, as they are in [`paint`], and the ones
/// lying outside `at` are cut away here.
#[must_use]
pub fn paint_in(
    spans: &[Span],
    at: &Range<usize>,
    tiers: &[LineTiers],
    focus: Focus,
    colours: &Colours,
) -> Vec<Painted> {
    let runs = flatten(spans);
    // With Focus on every byte is spoken for, because the buffer's own ink is
    // the wrong colour for most of the page and the writer must not see it
    // anywhere. With Focus off the buffer is right about plain prose, and
    // saying so again would be a tag for nothing.
    let covers = matches!(focus, Focus::On(_));
    let mut out = Vec::new();
    // The runs behind `at` are the ones a widened `spans_in` brought with it;
    // the loop below walks forward only, so they are stepped over here rather
    // than dragging the cursor back over bytes the caller did not ask for.
    let mut next = runs.partition_point(|run| run.at.end <= at.start);
    for (segment, tier) in segments_in(at, tiers, focus) {
        let plain = covers.then(|| Paint::of(Look::PROSE, tier, colours));
        let mut cursor = segment.start;
        while let Some(run) = runs.get(next).filter(|run| run.at.start < segment.end) {
            let drawn = run.at.start.max(segment.start)..run.at.end.min(segment.end);
            push_painted(&mut out, cursor..drawn.start, plain);
            push_painted(
                &mut out,
                drawn.clone(),
                Some(Paint::of(run.look, tier, colours)),
            );
            cursor = drawn.end;
            if run.at.end > segment.end {
                break;
            }
            next += 1;
        }
        push_painted(&mut out, cursor..segment.end, plain);
    }
    out
}

/// The bytes `at` cut into tiers, ascending and with no gaps between them.
///
/// The dim tier is what the bright ranges leave over, which is why [`Tiers`]
/// lists only one of the two: dim is the page, and bright are the holes cut in
/// it. With Focus off there is one segment and it is bright — not because
/// nothing is dim, but because nothing is anything, and the bright arm of
/// [`colour`] is the untiered colour.
///
/// Bright ranges outside `at` are clipped away rather than skipped, so that a
/// sentence straddling the first or last line of a retag keeps the part of it
/// that is being redrawn.
///
/// [`Tiers`]: crate::focus::Tiers
fn segments_in(at: &Range<usize>, tiers: &[LineTiers], focus: Focus) -> Vec<(Range<usize>, Tier)> {
    if matches!(focus, Focus::Off) {
        return vec![(at.clone(), Tier::Bright)];
    }
    let mut out = Vec::new();
    let mut cursor = at.start;
    for lit in tiers.iter().flat_map(|on| &on.tiers.bright) {
        let bright = lit.start.max(cursor)..lit.end.max(cursor).min(at.end);
        if bright.is_empty() {
            continue;
        }
        if bright.start > cursor {
            out.push((cursor..bright.start, Tier::Dim));
        }
        cursor = bright.end;
        out.push((bright, Tier::Bright));
    }
    if cursor < at.end {
        out.push((cursor..at.end, Tier::Dim));
    }
    out
}

/// Adds `at` as a painted run, or lengthens the last one if it draws alike.
///
/// `paint` is [`None`] where there is nothing to draw: plain prose with Focus
/// off, which the buffer already draws in the right ink.
fn push_painted(out: &mut Vec<Painted>, at: Range<usize>, paint: Option<Paint>) {
    let Some(paint) = paint else {
        return;
    };
    if at.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.at.end == at.start
        && last.paint == paint
    {
        last.at.end = at.end;
        return;
    }
    out.push(Painted { at, paint });
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
/// one way. Two are not: a quote's `>`s are the only markers that repeat down a
/// construct rather than closing it, and a later ticket dims them by tier; and
/// a link's brackets go quiet with its destination rather than resting at the
/// ink every other marker rests at, which is what the Design oracle draws.
const fn marker_mark(mark: Mark) -> Mark {
    match mark {
        Mark::Quote => Mark::QuoteMarker,
        Mark::Link | Mark::Image => Mark::LinkMark,
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
            marker_spans(text, cursor..run.start, marker, spans);
        }
        cursor = cursor.max(run.end).min(at.end);
    }
    marker_spans(text, cursor..at.end, marker, spans);
}

/// Adds `at` as marker spans: one per line it crosses, and none of them prose.
///
/// A marker belongs to the line it opens, and the subtraction cannot see lines.
/// Inside a quote the bytes no event covers run `" \n> "` — a space the writer
/// left, the line ending, and the next line's marker — and [`push_span`] on its
/// own keeps the three as one span, so two lines hang by one measurement and
/// the writer's space is greyed with the punctuation (#102). The run is
/// therefore cut at every line ending, and each piece asked where its own
/// marker stops.
fn marker_spans(text: &str, at: Range<usize>, mark: Mark, spans: &mut Vec<Span>) {
    let mut start = at.start;
    while start < at.end {
        let end = text[start..at.end]
            .find('\n')
            .map_or(at.end, |offset| start + offset);
        push_span(text, start..marker_end(text, start..end, mark), mark, spans);
        start = end + 1;
    }
}

/// Where the marker stops inside `at`, which is one line's worth of one.
///
/// Two things inside it are the writer's rather than the parser's. A quote's
/// marker is `>` and at most one space or tab after it — CommonMark's
/// definition, and the one the oracle's `RE_QUOTE`, `(?:>[ \t]?)+`, always
/// tokenised by — so the padding of `>      Beans` is where the writer put
/// their word: it keeps their ink, and the line hangs by two cells rather than
/// swinging seven into the margin. And whitespace left at the end of a line is
/// theirs too, so a piece that is nothing else is no marker at all (#102).
fn marker_end(text: &str, at: Range<usize>, mark: Mark) -> usize {
    let run = text[at.start..at.end].trim_end_matches('\r');
    if let (Mark::QuoteMarker, Some(caret)) = (mark, run.rfind('>')) {
        // Clamped, because the space may be the first byte of the content the
        // subtraction stopped at rather than the marker's own.
        let after = at.start + caret + 1;
        let padded = matches!(text.as_bytes().get(after), Some(b' ' | b'\t'));
        return (after + usize::from(padded)).min(at.end);
    }
    let end = at.start + run.len();
    let ends_line = text.as_bytes().get(end).is_none_or(u8::is_ascii_whitespace);
    if ends_line && run.bytes().all(|byte| byte.is_ascii_whitespace()) {
        at.start
    } else {
        end
    }
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
    use std::path::Path;

    use super::*;
    use crate::document::Document;
    use crate::focus;
    use crate::settings::FocusScope;
    use crate::theme::Scheme;

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

    /// The markers' ink, upright, at the prose's weight: what every marker is.
    const MARKER: Look = Look {
        ink: Ink::Marker,
        ..Look::PROSE
    };

    /// A link's plumbing: the quiet grey its brackets and its destination take.
    const LINK: Look = Look {
        ink: Ink::Link,
        ..Look::PROSE
    };

    // The tiered flattening: Markup × Focus into one colour a run.

    /// The passage the judged states are shot against, at the caret they are
    /// shot at (`shots/oracle/states.json`).
    fn sample() -> Document {
        Document::open(Path::new("../ref/sample.md"))
            .expect("the shared test passage is in the repo")
    }

    /// A Document holding `text`, for the shapes the passage does not have.
    fn document(text: &str) -> Document {
        let mut doc = Document::untitled();
        doc.insert(0, text);
        doc
    }

    /// The bright ranges `doc` has at `at` under `focus`, line by line.
    fn tiers_of(doc: &Document, at: Range<usize>, focus: Focus) -> Vec<focus::LineTiers> {
        focus::tiers_by_line(doc, &focus::tiers(doc, &at, focus))
    }

    /// What `doc` draws at `at` under `focus`. The one walk the tiered tests
    /// share, so that none of them can be measuring a different page.
    fn painted_runs(
        doc: &Document,
        at: Range<usize>,
        focus: Focus,
        colours: &Colours,
    ) -> Vec<Painted> {
        let text = doc.text();
        let tiers = tiers_of(doc, at, focus);
        paint(&markup(text), text.len(), &tiers, focus, colours)
    }

    /// [`painted_runs`] as `(source text, colour)`, which is what an owner would
    /// see on the page.
    fn painted<'a>(
        doc: &'a Document,
        at: Range<usize>,
        focus: Focus,
        colours: &Colours,
    ) -> Vec<(&'a str, Colour)> {
        let text = doc.text();
        painted_runs(doc, at, focus, colours)
            .into_iter()
            .map(|run| (&text[run.at.clone()], run.paint.colour))
            .collect()
    }

    /// The colour a byte of `doc` is drawn in, whichever run holds it.
    fn colour_at(
        doc: &Document,
        at: Range<usize>,
        byte: usize,
        focus: Focus,
        colours: &Colours,
    ) -> Colour {
        painted_runs(doc, at, focus, colours)
            .into_iter()
            .find(|run| run.at.contains(&byte))
            .unwrap_or_else(|| panic!("byte {byte} is in no run"))
            .paint
            .colour
    }

    /// The caret of the judged `focus/sentence` and `focus/paragraph` states,
    /// in both scopes and on both grounds: every run carries the colour
    /// [`colour`] predicts for the `(mark, tier)` of every byte under it.
    ///
    /// Predicted from the two inputs rather than from a list of colours, so
    /// that a bright/dim assignment which came out inverted would fail here
    /// instead of passing on set membership.
    #[test]
    fn every_run_at_the_judged_caret_is_the_colour_the_table_predicts() {
        let doc = sample();
        let text = doc.text();
        let marked = flatten(&markup(text));

        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = Colours::of(scheme);
            for scope in [FocusScope::Sentence, FocusScope::Paragraph] {
                let focus = Focus::On(scope);
                let bright: Vec<_> = tiers_of(&doc, 403..403, focus)
                    .into_iter()
                    .flat_map(|on| on.tiers.bright)
                    .collect();
                let runs = painted_runs(&doc, 403..403, focus, &colours);
                let mut lit = 0;

                for run in &runs {
                    for byte in run.at.clone() {
                        let tier = if bright.iter().any(|at| at.contains(&byte)) {
                            lit += 1;
                            Tier::Bright
                        } else {
                            Tier::Dim
                        };
                        let ink = marked
                            .iter()
                            .find(|span| span.at.contains(&byte))
                            .map_or(Ink::Prose, |span| span.look.ink);
                        assert_eq!(
                            run.paint.colour,
                            colour(ink, tier, &colours),
                            "{scheme:?} {scope:?}: byte {byte} of {:?} is {ink:?} in {tier:?}",
                            &text[run.at.clone()]
                        );
                    }
                }

                // Neither tier is allowed to be empty, or the run above would
                // be asserting one column of the table twice.
                assert!(
                    lit > 0 && lit < text.len(),
                    "{scope:?} lit {lit} of {}",
                    text.len()
                );
            }
        }
    }

    /// Paragraph scope lights more of the page than Sentence scope and dims the
    /// rest the same way, which is the whole of the difference between them
    /// now that there is one dim tier (`docs/design.md` § Dim tiers).
    #[test]
    fn paragraph_scope_lights_its_whole_block_and_sentence_scope_one_sentence() {
        let doc = sample();
        let colours = Colours::of(Scheme::Light);
        let ink = colours.colour(Role::Ink);
        let lit = |focus| {
            painted(&doc, 403..403, focus, &colours)
                .into_iter()
                .filter(|&(_, colour)| colour == ink)
                .map(|(text, _)| text.len())
                .sum::<usize>()
        };
        let sentence = lit(Focus::On(FocusScope::Sentence));
        let paragraph = lit(Focus::On(FocusScope::Paragraph));
        assert!(
            paragraph > sentence && sentence > 0,
            "the paragraph is the sentence and more: {sentence} then {paragraph}"
        );
    }

    /// `legacy/app/css/focus.css:31-40`: out of focus nothing keeps a voice of
    /// its own. On the caret's own line the marker is the full marker grey —
    /// `markup.css:39`, which #37 left to this ticket — and bright is what says
    /// so, because bright is the Markup colour untouched.
    #[test]
    fn a_marker_is_the_dimmed_grey_out_of_focus_and_the_marker_grey_on_the_carets_line() {
        let doc = document("# A heading\n\nThe caret is *here*, in this sentence.\n");
        let colours = Colours::of(Scheme::Light);
        let caret = doc.text().find("here").expect("the passage says here");
        let focus = Focus::On(FocusScope::Sentence);

        let star = doc.text().find('*').expect("the emphasis is marked");
        assert_eq!(
            colour_at(&doc, caret..caret, star, focus, &colours),
            colours.colour(Role::Mark),
            "the caret's line is bright, so its markers hold the full marker grey"
        );

        let hash = doc.text().find('#').expect("the heading is marked");
        assert_eq!(
            colour_at(&doc, caret..caret, hash, focus, &colours),
            colours.colour(Role::InkDim),
            "a marker in the dim tier is the dimmed grey, not the marker grey"
        );
        assert_eq!(
            colour_at(&doc, caret..caret, hash + 2, focus, &colours),
            colours.colour(Role::InkDim),
            "and so is the heading's own text: focus.css dims the heading whole"
        );
    }

    /// A dim heading keeps its weight and loses only its colour, and a dim code
    /// span loses its ground (`focus.css:42-43`). Focus dims; it does not
    /// re-set the page.
    #[test]
    fn the_dim_tier_takes_the_colour_and_the_code_ground_and_leaves_the_weight() {
        let doc = document("# A heading with `code` in it\n\nThe caret is here.\n");
        let colours = Colours::of(Scheme::Light);
        let text = doc.text();
        let caret = text.find("caret").expect("the passage says caret");
        let runs = painted_runs(
            &doc,
            caret..caret,
            Focus::On(FocusScope::Sentence),
            &colours,
        );

        let heading = runs
            .iter()
            .find(|run| text[run.at.clone()].contains("A heading"))
            .expect("the heading is in a run");
        assert_eq!(
            heading.paint,
            Paint {
                colour: colours.colour(Role::InkDim),
                weight: Weight::Bold,
                slant: Slant::Upright,
                ground: Ground::Page,
            },
            "a dim heading loses its colour and stays a heading"
        );

        let code = text.find("code").expect("the heading has a code span");
        let run = runs
            .iter()
            .find(|run| run.at.contains(&code))
            .expect("the code span is in a run");
        assert_eq!(
            run.paint,
            Paint {
                colour: colours.colour(Role::InkDim),
                weight: Weight::Bold,
                slant: Slant::Upright,
                ground: Ground::Page,
            },
            "out of focus a code span keeps its glyphs and loses its box"
        );
    }

    /// The engine's table and the app's `hex` are the same three colours. The
    /// app resolves the roles itself until #113 moves it to [`colour`], and two
    /// tables that disagree would be two Quills.
    #[test]
    fn the_bright_tier_is_the_colour_the_app_already_draws_each_role_in() {
        let colours = Colours::of(Scheme::Light);
        for (ink, role) in [
            (Ink::Prose, Role::Ink),
            (Ink::Marker, Role::Mark),
            (Ink::Link, Role::Link),
        ] {
            assert_eq!(
                colour(ink, Tier::Bright, &colours),
                colours.colour(role),
                "{ink:?} is {role:?} while Focus leaves it alone"
            );
            assert_eq!(
                colour(ink, Tier::Dim, &colours),
                colours.colour(Role::InkDim),
                "{ink:?} has no voice of its own out of focus"
            );
        }
    }

    /// Focus off draws what the app has always drawn: the untiered runs, with
    /// each role resolved to the colour it has always meant and nothing added.
    ///
    /// Every field, so that a weight or a slant dropped on the way through
    /// [`paint`] fails here rather than in the Editor.
    #[test]
    fn focus_off_paints_the_untiered_runs_and_adds_nothing() {
        let doc = sample();
        let colours = Colours::of(Scheme::Light);
        let text = doc.text();
        let was: Vec<_> = flatten(&markup(text))
            .into_iter()
            .map(|run| Painted {
                at: run.at,
                paint: Paint {
                    colour: colours.colour(run.look.ink.role()),
                    weight: run.look.weight,
                    slant: run.look.slant,
                    ground: run.look.ground,
                },
            })
            .collect();
        assert_eq!(
            paint(&markup(text), text.len(), &[], Focus::Off, &colours),
            was,
            "Focus off is the flattening with its roles resolved"
        );
    }

    /// Every byte of the Document is drawn exactly once, and in order — the
    /// property the app leans on when it retags a line.
    #[test]
    fn painted_runs_never_overlap_and_run_in_order() {
        let doc = sample();
        let colours = Colours::of(Scheme::Dark);
        let text = doc.text();
        let focus = Focus::On(FocusScope::Sentence);
        let runs = painted_runs(&doc, 403..403, focus, &colours);
        let mut end = 0;
        for run in &runs {
            assert_eq!(
                run.at.start, end,
                "with Focus on every byte is coloured, and {run:?} leaves a gap"
            );
            assert!(!run.at.is_empty(), "{run:?} is empty");
            end = run.at.end;
        }
        assert_eq!(end, text.len(), "the last run ends at the end of the text");
    }

    /// Where the oracle passage's runs are written down, so that a change to
    /// the flattening has to be seen and agreed rather than merely compiled.
    const GOLDEN: &str = "tests/oracle-markup-runs.txt";

    /// One run per line: the bytes, then every field of the [`Look`].
    fn written(runs: &[Run]) -> String {
        runs.iter()
            .map(|run| {
                let Look {
                    ink,
                    alpha,
                    weight,
                    slant,
                    ground,
                } = run.look;
                format!(
                    "{}..{} {ink:?} {alpha} {weight:?} {slant:?} {ground:?}\n",
                    run.at.start, run.at.end
                )
            })
            .collect()
    }

    /// The runs of the oracle passage, byte for byte, against what they were.
    ///
    /// Focus resolves a tier into a colour ([`paint`]) without touching
    /// [`flatten`], so the untiered runs are the ones the app has always drawn.
    /// This is the check that says so: run the tests with `QUILL_UPDATE_GOLDEN`
    /// set to write the file, and read the diff before committing it.
    ///
    /// The file was written from the flattening as it stood before #126 and has
    /// moved once since, in #198: the passage's one link now sets its words in
    /// the prose's ink and quiets its brackets with its destination, which the
    /// Design oracle measures and the old runs had the other way round. Every
    /// other run in the file is byte for byte what it was.
    #[test]
    fn the_oracle_passages_untiered_runs_are_what_they_were() {
        let runs = written(&flatten(&markup(&oracle())));
        if std::env::var_os("QUILL_UPDATE_GOLDEN").is_some() {
            std::fs::write(GOLDEN, &runs).expect("the golden file is writable");
            return;
        }
        let was = std::fs::read_to_string(GOLDEN).expect("the golden file is committed");
        assert_eq!(runs, was, "the flattening moved the oracle passage's runs");
    }

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
    fn a_links_words_are_the_writers_ink_and_its_plumbing_goes_quiet() {
        let text = "in [the keeper's book](https://example.org/keepers-book), and\n";
        assert_eq!(
            marked(text),
            [
                (
                    "[the keeper's book](https://example.org/keepers-book)",
                    Mark::Link
                ),
                ("[", Mark::LinkMark),
                ("](", Mark::LinkMark),
                ("https://example.org/keepers-book", Mark::Url),
                (")", Mark::LinkMark),
            ]
        );
        assert_eq!(
            drawn(text),
            [
                ("[", LINK),
                ("the keeper's book", Look::PROSE),
                ("](https://example.org/keepers-book)", LINK),
            ],
            "the words are the writer's and stay at the prose's ink; the \
             brackets and the address are plumbing and go quiet"
        );
    }

    #[test]
    fn a_reference_link_marks_its_label_and_a_shortcut_has_nothing_to_mark() {
        assert_eq!(
            marked("[ref][r] and [short]\n\n[r]: /x\n"),
            [
                ("[ref][r]", Mark::Link),
                ("[", Mark::LinkMark),
                ("][", Mark::LinkMark),
                ("r", Mark::Url),
                ("]", Mark::LinkMark),
                ("[r]:", Mark::DefinitionLabel),
            ],
            "`[short]` has no definition, so the parser says it is not a link \
             at all; a shortcut that had one would carry no destination here \
             either, and its brackets alone would be marked. The definition \
             the link resolves against is drawn from the line's own shape, \
             which is why it is marked without the parser reporting it"
        );
    }

    /// An autolink's own text is a URL the writer reads, so it keeps the
    /// prose's ink; what goes quiet is the angles that make it one, the way a
    /// bracketed link's brackets do. The Design oracle sets a bare URL in the
    /// body's ink and underlines it (#198); the underline is a mark of the
    /// destination and an autolink has none, so ours draws no rule under it.
    #[test]
    fn an_autolinks_own_text_is_the_writers_and_only_the_angles_go_quiet() {
        assert_eq!(
            drawn("see <https://example.org> now\n"),
            [
                ("<", LINK),
                ("https://example.org", Look::PROSE),
                (">", LINK),
            ]
        );
    }

    #[test]
    fn an_images_alt_text_reads_as_a_links_does() {
        assert_eq!(
            marked("an ![alt](/pic.png \"t\") image\n"),
            [
                ("![alt](/pic.png \"t\")", Mark::Image),
                ("![", Mark::LinkMark),
                ("](", Mark::LinkMark),
                ("/pic.png \"t\"", Mark::Url),
                (")", Mark::LinkMark),
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
    fn a_space_at_the_end_of_a_quote_line_never_joins_it_to_the_next() {
        let text = "> one \n> two\n";
        assert_eq!(
            marked(text),
            [
                ("> one \n> two", Mark::Quote),
                ("> ", Mark::QuoteMarker),
                ("> ", Mark::QuoteMarker),
            ],
            "#102: subtraction left the space, the line ending and the next \
             `> ` in one span, so two lines hung by one measurement"
        );
    }

    #[test]
    fn a_quote_line_ending_in_a_space_is_marked_as_if_it_did_not() {
        let text = "> Why hello there \n";
        assert_eq!(
            marked(text),
            [
                ("> Why hello there ", Mark::Quote),
                ("> ", Mark::QuoteMarker),
            ],
            "#102: the trailing space was a marker of its own, greyed and \
             measured, and the line hung by the whole of itself"
        );
        assert_eq!(
            drawn(text),
            [("> ", MARKER), ("Why hello there ", Look::PROSE)],
            "what the writer left at the end of a line is their prose"
        );
    }

    #[test]
    fn a_quotes_marker_is_the_caret_and_at_most_one_space_after_it() {
        // The owner's decision on #102: `>` plus one space or tab, as
        // CommonMark defines it and as the oracle's `RE_QUOTE`, `(?:>[ \t]?)+`,
        // has always tokenised it. Padding past that is where the writer put
        // their word, so it stays in their ink and the line hangs by two.
        let text = "> Why hello there \n> Beans\n> Beans\n>      Beans\n";
        let markers: Vec<&str> = markup(text)
            .iter()
            .filter(|span| span.mark == Mark::QuoteMarker)
            .map(|span| &text[span.at.clone()])
            .collect();
        assert_eq!(
            markers,
            ["> ", "> ", "> ", "> "],
            "the four lines of #102's Reproduce passage carry one marker each"
        );
        let runs = drawn(text);
        assert!(
            !runs
                .iter()
                .any(|(run, look)| *look == MARKER && *run != "> "),
            "and nothing else on the page is grey: the trailing space and the \
             padding are the writer's ink, {runs:?}"
        );
    }

    #[test]
    fn no_marker_span_of_any_construct_covers_a_line_break() {
        // The four marks the subtraction emits, which is every construct's
        // punctuation. The other marks drawn in the marker grey are not
        // punctuation and are left alone: `Mark::Url` is a link's destination,
        // which CommonMark lets a writer break across lines, and greying only
        // its first line would be the worse of the two wrongs. `FrontMatter`
        // and `CodeBlock` are blocks rather than markers by construction.
        for passage in [
            oracle(),
            "> one \n> two\n".into(),
            "# Heading \n\n- item \n- item\n\n1. one \n2. two\n\n> quoted \n> - item \n".into(),
        ] {
            for (run, mark) in marked(&passage) {
                assert!(
                    !matches!(
                        mark,
                        Mark::Markup | Mark::QuoteMarker | Mark::BulletMarker | Mark::OrderedMarker
                    ) || !run.contains('\n'),
                    "#102: a marker belongs to one line, and {run:?} of {mark:?} \
                     covers the break between two"
                );
            }
        }
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
            ("[", Mark::LinkMark),
            ("](", Mark::LinkMark),
            (")", Mark::LinkMark),
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

    // A retag draws a range of the page, and must draw it the same.

    #[test]
    fn a_range_is_painted_as_the_whole_document_paints_it() {
        let doc = sample();
        let colours = Colours::of(Scheme::Light);
        let text = doc.text();
        for focus in [Focus::Off, Focus::On(FocusScope::Sentence)] {
            let tiers = tiers_of(&doc, 403..403, focus);
            let whole = paint(&markup(text), text.len(), &tiers, focus, &colours);
            for line in 0..20 {
                let at = doc.line_bytes(line);
                let part = paint_in(doc.spans_in(&at), &at, &tiers, focus, &colours);
                let cut: Vec<_> = whole
                    .iter()
                    .filter(|run| run.at.start < at.end && at.start < run.at.end)
                    .map(|run| Painted {
                        at: run.at.start.max(at.start)..run.at.end.min(at.end),
                        paint: run.paint,
                    })
                    .collect();
                assert_eq!(
                    part, cut,
                    "line {line} under {focus:?} came out differently when it \
                     was retagged on its own"
                );
            }
        }
    }

    #[test]
    fn a_retag_reads_only_the_spans_of_the_lines_it_draws() {
        let doc = sample();
        let colours = Colours::of(Scheme::Dark);
        let focus = Focus::On(FocusScope::Sentence);
        let tiers = tiers_of(&doc, 403..403, focus);
        let at = doc.line_bytes(8);
        let spans = doc.spans_in(&at);
        assert!(
            spans.len() < doc.spans().len(),
            "a line's retag was handed all {} spans of the Document, so its cost \
             is the manuscript's length",
            doc.spans().len()
        );
        for run in paint_in(spans, &at, &tiers, focus, &colours) {
            assert!(
                run.at.start >= at.start && run.at.end <= at.end,
                "{run:?} reaches outside the line the retag asked for"
            );
        }
    }
}
