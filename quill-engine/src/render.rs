//! Rendering a whole Document with Pango, for Preview, PDF and HTML.
//!
//! One layout pass from the current [`crate::template`] produces both the
//! layouts the Preview widget snapshots and the pages the PDF surface draws, so
//! Print and Export to PDF are one path. HTML export is the parser's HTML plus
//! the CSS the Template generates, inlined. Annotator marks never reach here:
//! Preview and Export show the Document, not the Editor's styling.
//!
//! This is why the engine may depend on `pango` and `cairo` — layout is not a
//! widget — while `gtk` stays out (ADR 0008).
//!
//! [`render`] walks the Document's block index and lays each block out in the
//! parser's events, so a rendered [`Block`] carries the index of the
//! [`crate::document::Block`] it came from as its [`Block::key`]: that key is
//! what [`crate::sync`] matches the two panes on, and it is why a whole list is
//! one rendered block rather than one per item. A [`crate::document::Kind::Gap`]
//! renders to nothing, which is the case sync is written for.
//!
//! The pass is one walk of the whole Document — every block's events once, and
//! one layout per heading, paragraph, list item or block — which it can afford
//! because nothing here runs on the keystroke lane: Preview re-renders on the
//! debounced idle timer after an edit (`docs/architecture.md` § Preview and
//! Export), where the Annotators' budget does not reach.
//!
//! **No colour goes into a layout.** A rendered page is scheme-free, so one
//! render serves a window that flips from light to dark and the same page can
//! be drawn on paper. What the widget needs to paint it is reported instead:
//! [`Block::ground`] says the block stands on the Template's Well ground, and
//! each [`Placed`] carries its [`Placed::links`] and [`Placed::code`] as byte
//! ranges of its own layout's text, for the link ink, the ground under an
//! inline code span and the hit test that opens a link.
//!
//! The caller hands the [`pango::Context`] the layouts are built in — the
//! widget its own, for the display's resolution and hinting; a test one off
//! `pangocairo::FontMap`, which is what makes the pass testable with no display
//! attached. Sizes are read from the context's resolution, so a Template's
//! points land as the pixels that context draws.

use std::ops::Range;

use pulldown_cmark::{Event, Tag, TagEnd};

use crate::document::{self, Document};
use crate::markdown;
use crate::template::{Face, Paragraphs, Template};

/// The resolution a context that names none is read at, which is what an
/// unconfigured `pangocairo` context answers with.
const DPI: f64 = 96.0;

/// The indent a quotation is set at, and the hanging indent a list marker
/// stands in, in ems of the base size.
///
/// Crate-wide because [`crate::html`] sets the same two indents in CSS: the
/// stylesheet an export carries is the same rendered page in another medium
/// (ADR 0005), so the number lives here once. Nothing outside the engine reads
/// it.
pub(crate) const INDENT: f64 = 1.5;

/// The first line's indent when paragraphs are indented rather than spaced, in
/// ems of the base size.
///
/// Crate-wide for the reason [`INDENT`] is.
pub(crate) const FIRST_LINE: f64 = 1.5;

/// How much room a thematic break's block takes, in ems: the rule itself is a
/// hairline the widget draws through the middle of it.
const RULE: f64 = 1.0;

/// The three Template toggles, as the writer left them.
///
/// They are the `[template]` table's own booleans
/// ([`crate::settings::Template`]) rather than a Template's properties: two
/// writers reading the same Template can hold them differently.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Toggles {
    /// Centre every heading when on; range every heading left when off.
    pub center_headings: bool,
    /// Number the headings under the title `1`, `1.1`, `1.1.1`.
    pub number_headings: bool,
    /// Indent a paragraph's first line rather than space it from the one
    /// before, as [`Paragraphs::Indented`] does for a whole Template.
    pub indent_paragraphs: bool,
}

impl Toggles {
    /// The toggles `table` holds, which is what the Preview and every export
    /// are laid out with.
    ///
    /// One place the three booleans are copied out of the settings table, so
    /// that a reader who forgets one cannot exist.
    #[must_use]
    pub fn of(table: &crate::settings::Template) -> Self {
        Self {
            center_headings: table.center_headings,
            number_headings: table.number_headings,
            indent_paragraphs: table.indent_paragraphs,
        }
    }
}

/// What a rendered block is.
///
/// Coarser than [`crate::document::Kind`], because what the writer wrote it
/// with stops mattering once it is laid out: a fenced and an indented code
/// block are both [`Kind::Code`], and everything the first version does not
/// typeset — a table, an image, a footnote definition, front matter, raw HTML —
/// is [`Kind::Source`], set as the bytes the file holds so that nothing
/// vanishes from the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A heading, at the level it was written.
    Heading {
        /// 1 to 6, as Markdown writes them. The type the parser hands a level
        /// out in, which is what [`crate::annotate::Mark::Heading`] and Live's
        /// ladder carry too.
        level: u8,
    },
    /// Prose.
    Paragraph,
    /// A quotation: indented, in body ink, with no bar.
    Quote,
    /// A whole list, one [`Placed`] per item.
    List,
    /// A code block, on the Well ground.
    Code,
    /// A thematic break: no layout, just the room the widget draws a hairline
    /// in.
    Rule,
    /// The block's own source text, on the Well ground.
    Source,
}

/// A link, and the words that open it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    /// The bytes of the [`Placed`]'s layout text the link's words are, which
    /// is what a hit test turns a click into.
    pub at: Range<usize>,
    /// Where it points, as the file writes it.
    pub destination: String,
}

/// One layout, and where it stands.
#[derive(Clone, Debug)]
pub struct Placed {
    /// The layout, ready to draw.
    pub layout: pango::Layout,
    /// Its left edge, from the measure's left edge: a quotation's indent and a
    /// list item's nesting are here rather than inside the layout.
    pub x: f64,
    /// Its top edge, from the top of the page.
    pub y: f64,
    /// Every link in it, in the layout's own bytes.
    pub links: Vec<Link>,
    /// Every inline code span in it, in the layout's own bytes, for the Well
    /// ground behind them.
    pub code: Vec<Range<usize>>,
}

/// One block of a rendered page.
#[derive(Clone, Debug)]
pub struct Block {
    /// What it is.
    pub kind: Kind,
    /// The index into [`crate::document::Document::blocks`] of the block it
    /// renders: the one key the Editor and the Preview share.
    pub key: usize,
    /// The Document bytes it was rendered from.
    pub source: Range<usize>,
    /// Its top edge, from the top of the page.
    pub top: f64,
    /// Its height, the space above it excluded.
    pub height: f64,
    /// Whether it stands on the Template's Well ground.
    pub ground: bool,
    /// Its layouts, in reading order.
    pub layouts: Vec<Placed>,
}

/// A whole Document, laid out.
#[derive(Clone, Debug)]
pub struct Page {
    /// The blocks, in Document order, which is also top-to-bottom order.
    pub blocks: Vec<Block>,
    /// How tall the page is.
    pub height: f64,
}

/// `document` laid out under `template`, `measure` pixels wide.
///
/// `zoom` is a whole percentage, clamped to
/// [`crate::settings::preview_zooms`] ([`scale`] is where), and
/// scales every size the Template names — the measure is not one of them: it is
/// the pane's, and it is what the text wraps in.
#[must_use]
pub fn render(
    document: &Document,
    template: &Template,
    toggles: Toggles,
    measure: f64,
    zoom: u32,
    context: &pango::Context,
) -> Page {
    let pass = Pass::new(context, template, toggles, measure, zoom);
    let text = document.text();
    let mut blocks = Vec::new();
    let mut numbering = Numbering::default();
    let mut y = 0.0;
    let mut last: Option<Kind> = None;
    for (key, block) in document.blocks().into_iter().enumerate() {
        if block.kind == document::Kind::Gap {
            continue;
        }
        let slice = &text[block.at.clone()];
        let events: Vec<(Event<'_>, Range<usize>)> = markdown::events(slice).collect();
        let after_heading = matches!(last, Some(Kind::Heading { .. }));
        let built = pass.build(&events, slice, block.kind, after_heading, &mut numbering);
        if let Some(before) = last {
            y += pass.space(before, built.kind);
        }
        let top = y;
        let mut layouts = Vec::with_capacity(built.drafts.len());
        for draft in built.drafts {
            let height = f64::from(draft.layout.pixel_size().1);
            layouts.push(Placed {
                layout: draft.layout,
                x: draft.x,
                y,
                links: draft.links,
                code: draft.code,
            });
            y += height;
        }
        if built.kind == Kind::Rule {
            y += RULE * pass.em;
        }
        last = Some(built.kind);
        blocks.push(Block {
            kind: built.kind,
            key,
            source: block.at,
            top,
            height: y - top,
            ground: built.ground,
            layouts,
        });
    }
    Page { blocks, height: y }
}

/// One block on its way to the page: what it is, and the layouts it is, before
/// they are stacked.
struct Built {
    kind: Kind,
    ground: bool,
    drafts: Vec<Draft>,
}

/// One layout before it knows how far down the page it stands.
struct Draft {
    layout: pango::Layout,
    x: f64,
    links: Vec<Link>,
    code: Vec<Range<usize>>,
}

/// Everything one render pass reads: the Template and the toggles, and the
/// sizes they come to in this context's pixels at this zoom.
struct Pass<'a> {
    context: &'a pango::Context,
    template: &'a Template,
    toggles: Toggles,
    measure: f64,
    /// The base size in pixels: the em every rhythm value is a multiple of.
    em: f64,
    /// What a Template's point sizes are multiplied by to reach this context's
    /// pixels: the zoom and the resolution together ([`scale`]).
    scale: f64,
    /// The code size in pixels.
    code: f64,
    /// Whether paragraphs are indented rather than spaced, by the Template or
    /// by the toggle.
    indented: bool,
}

impl<'a> Pass<'a> {
    /// The pass `template` at `zoom` comes to in `context`'s pixels.
    fn new(
        context: &'a pango::Context,
        template: &'a Template,
        toggles: Toggles,
        measure: f64,
        zoom: u32,
    ) -> Self {
        // A Template's sizes are in points, so the context's resolution is
        // what turns them into the pixels this display draws.
        let scale = scale(zoom, resolution(context));
        Self {
            context,
            template,
            toggles,
            measure,
            em: template.sizes.base * scale,
            scale,
            code: template.sizes.base * template.sizes.code * scale,
            indented: toggles.indent_paragraphs || template.paragraphs == Paragraphs::Indented,
        }
    }

    /// The space between a block of kind `before` and one of kind `after`.
    ///
    /// A heading's own space wins on both sides of it, so the toggle that
    /// takes the space out from between two paragraphs never closes a heading
    /// up against the prose above it.
    fn space(&self, before: Kind, after: Kind) -> f64 {
        let rhythm = &self.template.rhythm;
        let ems = match (before, after) {
            (_, Kind::Heading { .. }) => rhythm.space_before_heading,
            (Kind::Heading { .. }, _) => rhythm.space_after_heading,
            _ if self.indented => 0.0,
            _ => rhythm.paragraph_spacing,
        };
        ems * self.em
    }

    /// The size a heading at `level` is set at, in pixels.
    ///
    /// The Template's own walk of its ladder ([`crate::template::Sizes`]),
    /// scaled: a level off the ladder is the base size there and the body size
    /// here, which are the same statement.
    fn heading(&self, level: u8) -> f64 {
        self.template.sizes.heading(level) * self.scale
    }

    /// A font description for `face` at `size` pixels.
    fn font(&self, face: &Face, size: f64, weight: u16) -> pango::FontDescription {
        let mut font = pango::FontDescription::new();
        font.set_family(&face.family);
        font.set_weight(self::weight(weight));
        font.set_absolute_size(size * f64::from(pango::SCALE));
        font
    }

    /// The block `kind` covering `slice`, laid out.
    fn build(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        slice: &str,
        kind: document::Kind,
        after_heading: bool,
        numbering: &mut Numbering,
    ) -> Built {
        match kind {
            document::Kind::Heading { .. } => self.heading_block(events, slice, numbering),
            document::Kind::Paragraph if !holds_an_image(events) => {
                self.flow(events, slice, Kind::Paragraph, 0.0, after_heading)
            }
            document::Kind::Quote => {
                self.flow(events, slice, Kind::Quote, INDENT * self.em, after_heading)
            }
            document::Kind::List => self.list(events, slice),
            document::Kind::Code { .. } => self.code_block(events),
            document::Kind::Rule => Built {
                kind: Kind::Rule,
                ground: false,
                drafts: Vec::new(),
            },
            _ => self.source(slice),
        }
    }

    /// A heading, numbered and aligned as the toggles have it.
    fn heading_block(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        slice: &str,
        numbering: &mut Numbering,
    ) -> Built {
        let level = events
            .iter()
            .find_map(|(event, _)| match event {
                Event::Start(Tag::Heading { level, .. }) => Some(*level as u8),
                _ => None,
            })
            .unwrap_or(1);
        let size = self.heading(level);
        let mut inline = self.inline(&self.template.faces.heading);
        if self.toggles.number_headings
            && let Some(number) = numbering.next(level)
        {
            inline.plain(&format!("{number} "));
        }
        for (event, at) in events {
            match event {
                Event::Start(Tag::Heading { .. }) | Event::End(TagEnd::Heading(_)) => {}
                _ => inline.event(event, at, slice),
            }
        }
        let alignment = if self.toggles.center_headings {
            pango::Alignment::Center
        } else {
            pango::Alignment::Left
        };
        let font = self.font(
            &self.template.faces.heading,
            size,
            self.template.headings.weight,
        );
        let draft = self.draft(inline, 0.0, &font, size, alignment, 0.0);
        Built {
            kind: Kind::Heading { level },
            ground: false,
            drafts: vec![draft],
        }
    }

    /// Prose: a paragraph, or a quotation's paragraphs at `x`.
    ///
    /// One draft per paragraph the block holds, and a stray run of text with no
    /// paragraph around it — a list item inside a quotation, say — opens one of
    /// its own rather than falling out of the page.
    fn flow(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        slice: &str,
        kind: Kind,
        x: f64,
        after_heading: bool,
    ) -> Built {
        let font = self.font(&self.template.faces.body, self.em, 400);
        // The first paragraph after a heading is never indented: there is
        // nothing above it the indent could tell it from.
        let indent = if self.indented && kind == Kind::Paragraph && !after_heading {
            FIRST_LINE * self.em
        } else {
            0.0
        };
        let mut drafts = Vec::new();
        let mut open: Option<Inline> = None;
        // A paragraph with nothing in it is the quotation's own closing tag
        // arriving after the last one was flushed, not a blank line the writer
        // typed: it takes no room on the page.
        let push = |inline: Inline, drafts: &mut Vec<Draft>| {
            if inline.text.trim().is_empty() {
                return;
            }
            let first = drafts.is_empty();
            drafts.push(self.draft(
                inline,
                x,
                &font,
                self.em,
                pango::Alignment::Left,
                if first { indent } else { 0.0 },
            ));
        };
        for (event, at) in events {
            match event {
                Event::Start(Tag::Paragraph) => open = Some(self.inline(&self.template.faces.body)),
                Event::End(TagEnd::Paragraph) => {
                    if let Some(inline) = open.take() {
                        push(inline, &mut drafts);
                    }
                }
                _ => open
                    .get_or_insert_with(|| self.inline(&self.template.faces.body))
                    .event(event, at, slice),
            }
        }
        if let Some(inline) = open {
            push(inline, &mut drafts);
        }
        Built {
            kind,
            ground: false,
            drafts,
        }
    }

    /// A whole list, one draft per item, the marker in the item's own text and
    /// hanging in the indent its wrapped lines are set to.
    fn list(&self, events: &[(Event<'_>, Range<usize>)], slice: &str) -> Built {
        let font = self.font(&self.template.faces.body, self.em, 400);
        let mut drafts = Vec::new();
        let mut counters: Vec<Option<u64>> = Vec::new();
        let mut open: Option<(Inline, f64)> = None;
        let flush = |open: Option<(Inline, f64)>, drafts: &mut Vec<Draft>| {
            if let Some((inline, x)) = open {
                drafts.push(self.draft(
                    inline,
                    x,
                    &font,
                    self.em,
                    pango::Alignment::Left,
                    -INDENT * self.em,
                ));
            }
        };
        for (at, (event, range)) in events.iter().enumerate() {
            match event {
                Event::Start(Tag::List(start)) => {
                    flush(open.take(), &mut drafts);
                    counters.push(*start);
                }
                Event::End(TagEnd::List(_)) => {
                    counters.pop();
                }
                Event::Start(Tag::Item) => {
                    flush(open.take(), &mut drafts);
                    let depth = u32::try_from(counters.len().saturating_sub(1)).unwrap_or(0);
                    let marker = marker(&mut counters, task(&events[at + 1..]));
                    let mut inline = self.inline(&self.template.faces.body);
                    inline.plain(&marker);
                    open = Some((inline, f64::from(depth) * INDENT * self.em));
                }
                Event::End(TagEnd::Item) => flush(open.take(), &mut drafts),
                _ => {
                    if let Some((inline, _)) = open.as_mut() {
                        inline.event(event, range, slice);
                    }
                }
            }
        }
        flush(open, &mut drafts);
        Built {
            kind: Kind::List,
            ground: false,
            drafts,
        }
    }

    /// A code block: the code the parser reports, without its fence.
    fn code_block(&self, events: &[(Event<'_>, Range<usize>)]) -> Built {
        let mut code = String::new();
        for (event, _) in events {
            if let Event::Text(text) = event {
                code.push_str(text);
            }
        }
        Built {
            kind: Kind::Code,
            ground: true,
            drafts: vec![self.mono(code.trim_end_matches('\n'))],
        }
    }

    /// A block the first version does not typeset, as the bytes the file
    /// holds.
    fn source(&self, slice: &str) -> Built {
        Built {
            kind: Kind::Source,
            ground: true,
            drafts: vec![self.mono(slice.trim_end())],
        }
    }

    /// `text` set in the code face across the measure.
    fn mono(&self, text: &str) -> Draft {
        let font = self.font(&self.template.faces.code, self.code, 400);
        let mut inline = self.inline(&self.template.faces.code);
        inline.plain(text);
        self.draft(inline, 0.0, &font, self.code, pango::Alignment::Left, 0.0)
    }

    /// An empty run of text in `face`, ready to take the parser's events.
    fn inline(&self, face: &Face) -> Inline {
        Inline::new(face, &self.template.faces.code, self.code)
    }

    /// `inline` laid out at `x`, `size` pixels tall on the Template's leading.
    fn draft(
        &self,
        inline: Inline,
        x: f64,
        font: &pango::FontDescription,
        size: f64,
        alignment: pango::Alignment,
        indent: f64,
    ) -> Draft {
        let Inline {
            text,
            attributes,
            links,
            code,
            ..
        } = inline;
        attributes.insert(pango::AttrInt::new_line_height_absolute(units(
            self.template.rhythm.line_height * size,
        )));
        let layout = pango::Layout::new(self.context);
        layout.set_font_description(Some(font));
        layout.set_wrap(pango::WrapMode::WordChar);
        layout.set_width(units((self.measure - x).max(self.em)));
        layout.set_alignment(alignment);
        layout.set_indent(units(indent));
        layout.set_text(&text);
        layout.set_attributes(Some(&attributes));
        Draft {
            layout,
            x,
            links,
            code,
        }
    }
}

/// One run of text on its way into a layout: what it says, how it is marked
/// up, and what a click on it would do.
struct Inline {
    text: String,
    attributes: pango::AttrList,
    links: Vec<Link>,
    code: Vec<Range<usize>>,
    open: Vec<Open>,
    face: Face,
    code_face: Face,
    code_size: f64,
}

/// A tag the walk is inside, and the byte it opened at.
enum Open {
    Emphasis(usize),
    Strong(usize),
    Strike(usize),
    Link {
        from: usize,
        destination: String,
    },
    /// A tag that changes nothing about the run, kept so that the stack
    /// unwinds with the parser's.
    Other,
}

impl Inline {
    /// An empty run set in `face`, whose code spans are `code_face` at
    /// `code_size` pixels.
    fn new(face: &Face, code_face: &Face, code_size: f64) -> Self {
        Self {
            text: String::new(),
            attributes: pango::AttrList::new(),
            links: Vec::new(),
            code: Vec::new(),
            open: Vec::new(),
            face: face.clone(),
            code_face: code_face.clone(),
            code_size,
        }
    }

    /// `text` as it stands, with no markup of its own.
    fn plain(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// One parser event, at `at` in `slice`, the block's own bytes.
    fn event(&mut self, event: &Event<'_>, at: &Range<usize>, slice: &str) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(_) => self.end(),
            Event::Text(text) => self.text.push_str(text),
            Event::Code(code) => self.code_span(code),
            Event::SoftBreak => self.text.push(' '),
            Event::HardBreak => self.text.push('\n'),
            // A footnote reference, raw HTML and anything else that is neither
            // the writer's words nor a construct this version typesets stands
            // as the bytes the file holds, so that the page is never quietly
            // shorter than the Document.
            Event::FootnoteReference(_) | Event::Html(_) | Event::InlineHtml(_) => {
                self.text
                    .push_str(slice.get(at.clone()).unwrap_or_default());
            }
            _ => {}
        }
    }

    /// A tag opens over the bytes from here.
    fn start(&mut self, tag: &Tag<'_>) {
        let from = self.text.len();
        self.open.push(match tag {
            Tag::Emphasis => Open::Emphasis(from),
            Tag::Strong => Open::Strong(from),
            Tag::Strikethrough => Open::Strike(from),
            Tag::Link { dest_url, .. } => Open::Link {
                from,
                destination: dest_url.to_string(),
            },
            _ => Open::Other,
        });
    }

    /// The innermost open tag closes here, and marks what it covered.
    fn end(&mut self) {
        let to = self.text.len();
        match self.open.pop() {
            Some(Open::Emphasis(from)) => self.mark(italic(&self.face), from..to),
            Some(Open::Strong(from)) => {
                self.mark(pango::AttrInt::new_weight(pango::Weight::Bold), from..to);
            }
            Some(Open::Strike(from)) => {
                self.mark(pango::AttrInt::new_strikethrough(true), from..to);
            }
            Some(Open::Link { from, destination }) => self.links.push(Link {
                at: from..to,
                destination,
            }),
            Some(Open::Other) | None => {}
        }
    }

    /// An inline code span: the code face at the code size, and a range for
    /// the widget to lay the Well ground under.
    fn code_span(&mut self, code: &str) {
        let from = self.text.len();
        self.text.push_str(code);
        let at = from..self.text.len();
        self.mark(
            pango::AttrString::new_family(&self.code_face.family),
            at.clone(),
        );
        self.mark(
            pango::AttrSize::new_size_absolute(units(self.code_size)),
            at.clone(),
        );
        self.code.push(at);
    }

    /// `attribute` over `at`, in the run's own bytes.
    fn mark(&self, attribute: impl Into<pango::Attribute>, at: Range<usize>) {
        let mut attribute = attribute.into();
        attribute.set_start_index(index(at.start));
        attribute.set_end_index(index(at.end));
        self.attributes.insert(attribute);
    }
}

/// The heading numbers, level by level.
#[derive(Default)]
struct Numbering {
    counters: [u32; 7],
}

impl Numbering {
    /// The number a heading at `level` takes, or `None` for the title: an H1 is
    /// the Document's own name and stands bare, and the numbering starts under
    /// it at H2.
    fn next(&mut self, level: u8) -> Option<String> {
        let level = usize::from(level);
        if !(2..self.counters.len()).contains(&level) {
            return None;
        }
        self.counters[level] += 1;
        for deeper in &mut self.counters[level + 1..] {
            *deeper = 0;
        }
        Some(
            (2..=level)
                .map(|level| self.counters[level].to_string())
                .collect::<Vec<_>>()
                .join("."),
        )
    }
}

/// Whether the block holds an image, which is what makes a paragraph one of the
/// blocks laid out as its own source.
fn holds_an_image(events: &[(Event<'_>, Range<usize>)]) -> bool {
    events
        .iter()
        .any(|(event, _)| matches!(event, Event::Start(Tag::Image { .. })))
}

/// Whether the item beginning these events is a task, and whether it is done.
///
/// The marker is the first event inside the item, after the paragraph a loose
/// list wraps it in.
fn task(events: &[(Event<'_>, Range<usize>)]) -> Option<bool> {
    for (event, _) in events {
        match event {
            Event::Start(Tag::Paragraph) => {}
            Event::TaskListMarker(done) => return Some(*done),
            _ => return None,
        }
    }
    None
}

/// The marker an item stands behind, and the counter moved on.
fn marker(counters: &mut [Option<u64>], task: Option<bool>) -> String {
    match task {
        Some(true) => "\u{2611} ".to_string(),
        Some(false) => "\u{2610} ".to_string(),
        None => match counters.last_mut() {
            Some(Some(number)) => {
                let standing = *number;
                *number = number.saturating_add(1);
                format!("{standing}. ")
            }
            _ => "\u{2022} ".to_string(),
        },
    }
}

/// How italic is asked for in `face`: by the family that carries it where the
/// face is cut that way, and by the style where the family holds its own.
fn italic(face: &Face) -> pango::Attribute {
    match &face.italic {
        Some(family) => pango::AttrString::new_family(family).into(),
        None => pango::AttrInt::new_style(pango::Style::Italic).into(),
    }
}

/// The resolution `context` draws at, in dots per inch.
///
/// The app asks too: the pane measures its own widest line off the context it
/// will draw the page on (`quill::preview`), and a measure read at one
/// resolution and a page laid out at another would be a page that wraps
/// somewhere else.
#[must_use]
pub fn resolution(context: &pango::Context) -> f64 {
    let dpi = pangocairo::functions::context_get_resolution(context);
    if dpi > 0.0 { dpi } else { DPI }
}

/// The multiplier a Template's point sizes are drawn at, at `zoom` on a screen
/// of `dpi` dots to the inch.
///
/// The one home for the arithmetic the pass sizes every heading and every
/// paragraph by and the pane sizes its measure by (`quill::preview`'s
/// `widest`), and so the one place `zoom` is held to the percentages a writer
/// may ask for ([`crate::settings::preview_zooms`], which the settings file and
/// the three zoom Commands are read against as well).
#[must_use]
pub fn scale(zoom: u32, dpi: f64) -> f64 {
    let zooms = crate::settings::preview_zooms();
    f64::from(zoom.clamp(*zooms.start(), *zooms.end())) / 100.0 * dpi / 72.0
}

/// `pixels` in the units Pango counts in. A float-to-integer cast saturates in
/// Rust, so a page too tall to count clamps rather than wrapping.
///
/// Public because the pane asks Pango which byte a pointer landed on, in the
/// units Pango takes (`quill::preview`), and a truncation there and a rounding
/// here would be two answers about one page.
#[must_use]
pub fn units(pixels: f64) -> i32 {
    (pixels * f64::from(pango::SCALE)).round() as i32
}

/// `units` of Pango's, back in the units the layout was made in — the points a
/// page is measured in, or the pixels a sheet is drawn in.
///
/// [`units`] the other way round, and public for the same reason: everything
/// that reads a Pango layout back reads it back through this one division,
/// rather than through a copy of it each. The paginator's rows and the drawer's
/// runs are in points ([`crate::paginate`], [`crate::draw`]), and the Preview
/// pane's sheet and page column are in the pixels they paint in
/// (`quill::preview`, `quill::column`).
#[must_use]
pub fn back(units: i32) -> f64 {
    f64::from(units) / f64::from(pango::SCALE)
}

/// A byte offset as Pango counts one.
fn index(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}

/// The weight a Template names, as the nearest weight Pango has a name for.
///
/// Crate-wide because the drawer sets the furniture itself ([`crate::draw`])
/// and a Template names its heading weight the same way there. Nothing outside
/// the engine reads it.
#[must_use]
pub(crate) fn weight(value: u16) -> pango::Weight {
    match value {
        ..150 => pango::Weight::Thin,
        150..250 => pango::Weight::Ultralight,
        250..325 => pango::Weight::Light,
        325..365 => pango::Weight::Semilight,
        365..390 => pango::Weight::Book,
        390..450 => pango::Weight::Normal,
        450..550 => pango::Weight::Medium,
        550..650 => pango::Weight::Semibold,
        650..750 => pango::Weight::Bold,
        750..850 => pango::Weight::Ultrabold,
        850..950 => pango::Weight::Heavy,
        950.. => pango::Weight::Ultraheavy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template;

    /// A Pango context with no display behind it, which is what makes the pass
    /// testable in the engine at all.
    ///
    /// Neither Inter nor the Quill Faces need be installed for any of this:
    /// fontconfig answers with something, and what is asserted here — the
    /// kinds, the text, the order, the ranges, the alignment, the numbering
    /// and one size against another — holds whatever family it answers with.
    fn context() -> pango::Context {
        use pango::prelude::FontMapExt;

        pangocairo::FontMap::default().create_context()
    }

    /// `text` rendered under the built-in `id`.
    fn page(id: &str, text: &str, toggles: Toggles, measure: f64, zoom: u32) -> Page {
        let mut document = Document::untitled();
        document.reload(text.to_string());
        let template = template::built_in(id).expect("a built-in Template");
        render(&document, &template, toggles, measure, zoom, &context())
    }

    /// Every kind of block the first version lays out, in one Document.
    const FIXTURE: &str = "\
# Title

A paragraph with *emphasis*, **strong**, ~~strike~~, `code` and a [link](https://example.org).

## Second

- one
- two

1. first
2. second

- [ ] todo
- [x] done

> A quotation.

```rust
let x = 1;
```

---

| a | b |
| - | - |
| 1 | 2 |

![alt](picture.png)

[^1]: A footnote.
";

    /// The text of every layout in `block`, one string per layout.
    fn said(block: &Block) -> Vec<String> {
        block
            .layouts
            .iter()
            .map(|placed| placed.layout.text().to_string())
            .collect()
    }

    #[test]
    fn the_fixture_renders_to_the_blocks_it_is_written_in() {
        let page = page("modern", FIXTURE, Toggles::default(), 600.0, 100);
        let shape: Vec<(Kind, Vec<String>)> = page
            .blocks
            .iter()
            .map(|block| (block.kind, said(block)))
            .collect();
        let expected: Vec<(Kind, Vec<String>)> = vec![
            (Kind::Heading { level: 1 }, vec!["Title".to_string()]),
            (
                Kind::Paragraph,
                vec!["A paragraph with emphasis, strong, strike, code and a link.".to_string()],
            ),
            (Kind::Heading { level: 2 }, vec!["Second".to_string()]),
            (
                Kind::List,
                vec!["\u{2022} one".to_string(), "\u{2022} two".to_string()],
            ),
            (
                Kind::List,
                vec!["1. first".to_string(), "2. second".to_string()],
            ),
            (
                Kind::List,
                vec!["\u{2610} todo".to_string(), "\u{2611} done".to_string()],
            ),
            (Kind::Quote, vec!["A quotation.".to_string()]),
            (Kind::Code, vec!["let x = 1;".to_string()]),
            (Kind::Rule, Vec::new()),
            (
                Kind::Source,
                vec!["| a | b |\n| - | - |\n| 1 | 2 |".to_string()],
            ),
            (Kind::Source, vec!["![alt](picture.png)".to_string()]),
            (Kind::Source, vec!["[^1]: A footnote.".to_string()]),
        ];
        assert_eq!(shape, expected);
    }

    #[test]
    fn no_marker_survives_into_a_typeset_block() {
        let page = page("modern", FIXTURE, Toggles::default(), 600.0, 100);
        for block in &page.blocks {
            if matches!(block.kind, Kind::Source | Kind::Code) {
                continue;
            }
            for said in said(block) {
                assert!(
                    !said.contains(['#', '*', '`', '_', '>'])
                        && !said.contains("](")
                        && !said.contains("[ ]"),
                    "{:?} kept a marker in {said:?}",
                    block.kind
                );
            }
        }
    }

    #[test]
    fn the_blocks_cover_the_source_in_order() {
        let page = page("modern", FIXTURE, Toggles::default(), 600.0, 100);
        let mut at = 0;
        for block in &page.blocks {
            assert!(
                block.source.start >= at && block.source.end > block.source.start,
                "{:?} at {:?} runs back over {at}",
                block.kind,
                block.source
            );
            assert!(
                !FIXTURE[block.source.clone()].trim().is_empty(),
                "{:?} names bytes that are nothing but space",
                block.kind
            );
            at = block.source.end;
        }
        // The keys are the Document's block indices, so they ascend with the
        // blocks and skip the gaps between them.
        let keys: Vec<usize> = page.blocks.iter().map(|block| block.key).collect();
        assert!(
            keys.windows(2).all(|pair| pair[0] < pair[1]),
            "the keys ascend: {keys:?}"
        );
        assert!(
            page.blocks
                .iter()
                .all(|block| block.top >= 0.0 && block.top + block.height <= page.height + 0.5),
            "every block stands inside the page"
        );
    }

    #[test]
    fn center_headings_alone_sets_heading_alignment() {
        let centred = Toggles {
            center_headings: true,
            ..Toggles::default()
        };
        let alignment = |id: &str, toggles: Toggles| {
            page(id, "# Title\n", toggles, 600.0, 100).blocks[0].layouts[0]
                .layout
                .alignment()
        };
        assert_eq!(
            alignment("classic", Toggles::default()),
            pango::Alignment::Left,
            "Classic ranges its headings left"
        );
        assert_eq!(
            alignment("classic", centred),
            pango::Alignment::Center,
            "Center Headings centres them anyway"
        );
        assert_eq!(
            alignment("modern", Toggles::default()),
            pango::Alignment::Left,
            "Modern ranges them left with the toggle off"
        );
    }

    #[test]
    fn number_headings_numbers_from_the_second_level_down() {
        let toggles = Toggles {
            number_headings: true,
            ..Toggles::default()
        };
        let text = "# Title\n\n## One\n\n### Under\n\n#### Deeper\n\n## Two\n\n### Under\n";
        let page = page("modern", text, toggles, 600.0, 100);
        let said: Vec<String> = page.blocks.iter().flat_map(said).collect();
        assert_eq!(
            said,
            vec![
                "Title".to_string(),
                "1 One".to_string(),
                "1.1 Under".to_string(),
                "1.1.1 Deeper".to_string(),
                "2 Two".to_string(),
                "2.1 Under".to_string(),
            ]
        );
    }

    #[test]
    fn indent_paragraphs_indents_every_paragraph_but_the_first_after_a_heading() {
        let toggles = Toggles {
            indent_paragraphs: true,
            ..Toggles::default()
        };
        let text = "# Title\n\nFirst.\n\nSecond.\n\nThird.\n";
        let indented = page("modern", text, toggles, 600.0, 100);
        let indents: Vec<i32> = indented
            .blocks
            .iter()
            .filter(|block| block.kind == Kind::Paragraph)
            .map(|block| block.layouts[0].layout.indent())
            .collect();
        assert_eq!(indents.len(), 3);
        assert_eq!(
            indents[0], 0,
            "the first paragraph after the heading is not"
        );
        assert!(
            indents[1] > 0 && indents[2] > 0,
            "the rest are: {indents:?}"
        );
        // And the space between two paragraphs goes, the heading's staying.
        let spaced = page("modern", text, Toggles::default(), 600.0, 100);
        let gap = |page: &Page| page.blocks[2].top - (page.blocks[1].top + page.blocks[1].height);
        assert_eq!(
            gap(&indented),
            0.0,
            "indented paragraphs are not also spaced"
        );
        assert!(gap(&spaced) > 0.0, "spaced ones are");
    }

    #[test]
    fn a_link_carries_its_destination_over_its_own_words() {
        let page = page(
            "modern",
            "See [the site](https://example.org) for more.\n",
            Toggles::default(),
            600.0,
            100,
        );
        let placed = &page.blocks[0].layouts[0];
        let said = placed.layout.text().to_string();
        assert_eq!(placed.links.len(), 1, "one link: {:?}", placed.links);
        assert_eq!(placed.links[0].destination, "https://example.org");
        assert_eq!(&said[placed.links[0].at.clone()], "the site");
    }

    #[test]
    fn a_table_an_image_and_a_footnote_come_back_as_source_on_the_well_ground() {
        let page = page("modern", FIXTURE, Toggles::default(), 600.0, 100);
        let source: Vec<String> = page
            .blocks
            .iter()
            .filter(|block| block.kind == Kind::Source)
            .inspect(|block| assert!(block.ground, "source stands on the Well ground"))
            .flat_map(said)
            .collect();
        assert_eq!(
            source,
            vec![
                "| a | b |\n| - | - |\n| 1 | 2 |".to_string(),
                "![alt](picture.png)".to_string(),
                "[^1]: A footnote.".to_string(),
            ]
        );
        let code = page
            .blocks
            .iter()
            .find(|block| block.kind == Kind::Code)
            .expect("the fenced block");
        assert!(code.ground, "so does a code block");
    }

    #[test]
    fn zoom_scales_every_size_the_template_names() {
        let text = "# Title\n\nA line.\n";
        let sizes = |zoom| {
            let page = page("modern", text, Toggles::default(), 2000.0, zoom);
            let heights: Vec<f64> = page
                .blocks
                .iter()
                .map(|block| f64::from(block.layouts[0].layout.pixel_size().1))
                .collect();
            (heights, page.blocks[1].top)
        };
        let (single, one_top) = sizes(100);
        let (double, two_top) = sizes(200);
        for (small, large) in single.iter().zip(&double) {
            let ratio = large / small;
            assert!(
                (1.8..=2.2).contains(&ratio),
                "200% is twice the type: {small} to {large}"
            );
        }
        let ratio = two_top / one_top;
        assert!(
            (1.8..=2.2).contains(&ratio),
            "and twice the rhythm: {one_top} to {two_top}"
        );
    }

    #[test]
    fn the_measure_changes_the_wrapping_and_nothing_else() {
        let text = "# Title\n\nA paragraph long enough that it has to wrap somewhere, and \
            somewhere else again once the measure it is set in is halved.\n";
        let shape = |measure| {
            let page = page("modern", text, Toggles::default(), measure, 100);
            let said: Vec<Vec<String>> = page.blocks.iter().map(said).collect();
            let kinds: Vec<Kind> = page.blocks.iter().map(|block| block.kind).collect();
            let lines: i32 = page
                .blocks
                .iter()
                .flat_map(|block| &block.layouts)
                .map(|placed| placed.layout.line_count())
                .sum();
            (kinds, said, lines)
        };
        let (wide_kinds, wide_said, wide_lines) = shape(600.0);
        let (narrow_kinds, narrow_said, narrow_lines) = shape(300.0);
        assert_eq!(wide_kinds, narrow_kinds, "the same blocks");
        assert_eq!(wide_said, narrow_said, "saying the same words");
        assert!(
            narrow_lines > wide_lines,
            "over more lines: {wide_lines} to {narrow_lines}"
        );
    }

    #[test]
    fn an_empty_document_is_an_empty_page() {
        let page = page("modern", "", Toggles::default(), 600.0, 100);
        assert!(page.blocks.is_empty() && page.height == 0.0);
    }

    #[test]
    fn a_zoom_off_the_range_is_clamped_rather_than_believed() {
        let text = "A line.\n";
        let height = |zoom| page("modern", text, Toggles::default(), 600.0, zoom).height;
        let zooms = crate::settings::preview_zooms();
        assert_eq!(height(0), height(*zooms.start()));
        assert_eq!(height(1000), height(*zooms.end()));
    }
}
