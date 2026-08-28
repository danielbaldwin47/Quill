//! Documents: text, the block index, and what one keystroke changes.
//!
//! A Document is one Markdown file on disk, and the file is the only source of
//! truth. The engine keeps its own `String` copy of the text, spliced from the
//! buffer's edit signals before any Annotator runs, and keeps beside it the
//! three things the splice has to move with the text: the line-start table, the
//! block index, and the Markup spans flattened into runs.
//!
//! The **block index** is the list of top-level blocks, each a byte range and
//! the [`Kind`] it is. It tiles the Document — the stretches no block covers,
//! the blank lines between two blocks and the odd indent an indented block
//! leaves outside itself, are [`Kind::Gap`] entries — so binary search finds
//! the block containing any offset. An edit inside one block re-parses that
//! block's bytes alone with a fresh parser and rebases every range the parser
//! yields by the slice's start offset, which is the whole trick; the blocks
//! after it shift by the edit's byte delta, an integer add rather than a parse.
//!
//! An edit that touches a **block boundary** — a blank line, a fence marker, a
//! list marker, a setext underline — re-parses the whole Document instead. That
//! is correct and not yet fast: the ticket after this one replaces the fallback
//! with the widening rules and proves the keystroke budget. It is off the path
//! of ordinary typing, and it never renders the wrong thing.

use std::borrow::Cow;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, Event, Tag};

use crate::annotate::{self, Look, Mark, Run, Span};
use crate::markdown;

/// What a window titles a Document that is not on disk yet.
pub const UNTITLED: &str = "Untitled";

/// Where a byte offset is, in the two numbers a `GtkTextIter` is set from.
///
/// The app reaches a byte with `set_line` and then `set_line_index`, never by
/// counting characters from the top of the buffer, because that is O(document)
/// per lookup and there is one lookup per span.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Place {
    /// The line the byte is on, counted from 0.
    pub line: usize,
    /// How many bytes into that line it is, counted from the line's first byte.
    pub index: usize,
}

/// What a top-level block of a Document is.
///
/// The flags are not decoration: they are what says whether an edit can be
/// answered by the block alone. A fenced block has marker lines a Document's
/// meaning below it hangs on; a setext heading is a heading only because of the
/// line underneath it; an indented code block and an ATX heading have neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Bytes no block covers: the blank lines that separate two blocks, the
    /// indentation an indented code block leaves outside itself, and the link
    /// reference and footnote definitions the parser resolves rather than
    /// draws. An edit here is a boundary edit by definition.
    Gap,
    /// Prose: the block a writer is in most of the time.
    Paragraph,
    /// A heading, by either of the two ways of writing one.
    Heading {
        /// True when it is written with `===` or `---` underneath rather than
        /// with `#` in front, which makes the line below it load-bearing.
        setext: bool,
    },
    /// A `>` blockquote and everything nested inside it.
    Quote,
    /// A bulleted or numbered list and every item of it.
    List,
    /// A code block, by either of the two ways of writing one.
    Code {
        /// True when it is delimited by ``` or `~~~` rather than by four
        /// spaces of indent, which makes its first and last lines markers.
        fenced: bool,
    },
    /// A block of raw HTML.
    Html,
    /// A thematic break.
    Rule,
    /// The `---` metadata block at the head of a file.
    FrontMatter,
    /// A pipe table, its header and delimiter rows included.
    Table,
    /// A `[^1]: …` footnote definition.
    Footnote,
    /// A block this module has no rule of its own for. It re-parses like a
    /// paragraph and takes the boundary fallback for nothing else.
    Other,
}

impl Kind {
    /// Whether a block of this kind holds a container open past its own bytes.
    ///
    /// A fence, a metadata block and an HTML block all run until something
    /// closes them, and what closes them can be anywhere below. A slice cannot
    /// see that far, so an edit that makes one is the whole Document's
    /// business — which is exactly the fence a writer opens at the top of a
    /// long draft, restyling everything under it.
    const fn opens(self) -> bool {
        matches!(
            self,
            Self::Code { fenced: true } | Self::FrontMatter | Self::Html
        )
    }
}

/// One top-level block: the bytes it covers and what it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    /// Absolute UTF-8 bytes from the start of the Document.
    pub at: Range<usize>,
    /// What those bytes are.
    pub kind: Kind,
}

/// How much of a Document one edit was re-parsed over.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    /// Those blocks alone, by their index in the block index *after* the edit.
    Blocks(Range<usize>),
    /// The whole Document, because the edit touched a block boundary.
    Whole,
}

/// What one edit changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    /// The lines whose runs are not what they were, and so the lines the app
    /// retags. Empty when the edit changed no line's Markup at all, which is
    /// what typing a letter into a plain paragraph does.
    pub lines: Range<usize>,
    /// What had to be re-parsed to find them. A test asserts on this rather
    /// than assuming the scope was small.
    pub scope: Scope,
}

/// One Markdown file, plus everything the engine keeps in step with its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    path: Option<PathBuf>,
    text: String,
    /// The byte each line starts at, ascending, always beginning with 0.
    lines: Vec<usize>,
    /// The top-level blocks, in order, tiling the whole text.
    blocks: Vec<Block>,
    /// The Markup spans of the whole text, in the order [`annotate::markup`]
    /// gives them.
    spans: Vec<Span>,
    /// Those spans flattened: non-overlapping, in order.
    runs: Vec<Run>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            path: None,
            text: String::new(),
            lines: vec![0],
            blocks: Vec::new(),
            spans: Vec::new(),
            runs: Vec::new(),
        }
    }
}

impl Document {
    /// A Document with no file behind it and nothing written in it.
    #[must_use]
    pub fn untitled() -> Self {
        Self::default()
    }

    /// Reads `path` from disk.
    ///
    /// The whole-Document parse happens here, once, which is the architecture's
    /// cold-start cost rather than a keystroke cost.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be read, so
    /// the caller decides what a writer sees.
    pub fn open(path: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::holding(text, Some(path.to_path_buf())))
    }

    /// A Document of `text`, parsed whole.
    fn holding(text: String, path: Option<PathBuf>) -> Self {
        let lines = line_starts(&text);
        let blocks = index(&text, 0);
        let spans = annotate::markup(&text);
        let runs = annotate::flatten(&spans);
        Self {
            path,
            text,
            lines,
            blocks,
            spans,
            runs,
        }
    }

    /// The file this Document is, or `None` while it is untitled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The Document's text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The top-level blocks, in order, tiling the whole text.
    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The Markup spans of the whole Document.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Those spans flattened into runs that do not overlap.
    #[must_use]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// The runs that draw any of the bytes `at`.
    ///
    /// Found by binary search on both ends, because the caller is the retag
    /// and the retag is on the keystroke path: walking the Document's runs to
    /// find a line's worth of them would put the cost of a keystroke back in
    /// proportion to the length of the manuscript, which is the one shape
    /// `docs/research/markdown-parser.md` rules out.
    #[must_use]
    pub fn runs_in(&self, at: &Range<usize>) -> &[Run] {
        let from = self.runs.partition_point(|run| run.at.end <= at.start);
        let count = self.runs[from..].partition_point(|run| run.at.start < at.end);
        &self.runs[from..from + count]
    }

    /// The spans that could cover any of the bytes `at`, and no others before
    /// them.
    ///
    /// Wider than [`Document::runs_in`] by construction: spans nest, so one
    /// starting well before `at` can still reach into it. What bounds the
    /// search is that a span never straddles a block — so nothing starting
    /// before the block holding `at.start` can reach `at`, and the block index
    /// finds that block by binary search. A span in the returned slice may
    /// still end before `at` begins; the caller skips it.
    #[must_use]
    pub fn spans_in(&self, at: &Range<usize>) -> &[Span] {
        let floor = self
            .block_at(at.start)
            .map_or(0, |block| self.blocks[block].at.start);
        let from = self.spans.partition_point(|span| span.at.start < floor);
        let to = self.spans.partition_point(|span| span.at.start < at.end);
        &self.spans[from..to.max(from)]
    }

    /// The block `offset` is in, by its index in [`Document::blocks`].
    ///
    /// Binary search, because there is one of these per edit and a Document is
    /// as long as a manuscript. The end of the text belongs to the last block:
    /// a writer typing at the end of a draft is typing into its last block, not
    /// past it.
    #[must_use]
    pub fn block_at(&self, offset: usize) -> Option<usize> {
        let found = self
            .blocks
            .partition_point(|block| block.at.start <= offset);
        let index = found.checked_sub(1)?;
        if offset < self.blocks[index].at.end || offset == self.text.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Where `offset` is, as the line and the byte index within it.
    ///
    /// An offset past the end of the text lands at the end, and an offset
    /// inside a character lands after that character: the Annotators only ever
    /// name boundaries, but `--caret` names a byte an owner chose, and neither
    /// is worth refusing a launch over.
    #[must_use]
    pub fn place(&self, offset: usize) -> Place {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset += 1;
        }
        let line = self.line_of(offset);
        Place {
            line,
            index: offset - self.lines[line],
        }
    }

    /// The name to show for this Document: its file name, or [`UNTITLED`].
    #[must_use]
    pub fn title(&self) -> Cow<'_, str> {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or(Cow::Borrowed(UNTITLED), std::ffi::OsStr::to_string_lossy)
    }

    /// Writes `text` in at `at` bytes, and says what that changed.
    ///
    /// The buffer's `insert-text` is what calls this, before any Annotator
    /// runs, so that every offset the Annotators go on to name is an offset
    /// into the text the writer can now see.
    pub fn insert(&mut self, at: usize, text: &str) -> Edit {
        self.edit(at..at, text)
    }

    /// Takes the bytes `at` out, and says what that changed.
    ///
    /// The buffer's `delete-range` is what calls this, for the reason
    /// [`Document::insert`] gives.
    pub fn delete(&mut self, at: Range<usize>) -> Edit {
        self.edit(at, "")
    }

    /// Replaces `at` with `inserted`, and re-derives what that moved.
    ///
    /// The order is the keystroke path's order and matters: the scope is
    /// decided against the text as it was, because the block index and the
    /// markers it is judged against are the old ones; the text and the line
    /// table then move together, so that no reader ever sees one without the
    /// other; and only then are the spans of the re-parsed region spliced in.
    fn edit(&mut self, at: Range<usize>, inserted: &str) -> Edit {
        debug_assert!(
            at.start <= at.end && at.end <= self.text.len(),
            "an edit names bytes the Document does not have: {at:?}"
        );
        let delta = signed(inserted.len()) - signed(at.len());
        let plan = self.plan(&at, inserted);
        let old = plan.as_ref().map_or(0..self.text.len(), |p| p.at.clone());
        let first = self.line_of(old.start);
        let before = self.prints(first, self.last_line_of(&old, first));

        self.text.replace_range(at.clone(), inserted);
        self.splice_lines(&at, inserted, delta);

        let (count, new, blocks, spans) = match plan {
            Some(plan) => {
                let at = plan.at.start..shift(plan.at.end, delta);
                let spans = rebase(annotate::markup(&plan.slice), plan.at.start);
                (Some(plan.index.len()), at, plan.index, spans)
            }
            None => {
                let at = 0..self.text.len();
                (None, at, index(&self.text, 0), annotate::markup(&self.text))
            }
        };
        let runs = annotate::flatten(&spans);
        let head = self.splice_index(&old, &new, delta, blocks);
        self.splice_spans(&old, delta, spans, runs);
        // The net under the whole strategy, and the reason it can be trusted
        // on a writer's own files rather than only on the passages the tests
        // hold. It costs a whole-Document parse per keystroke, so a debug
        // build types like one; that is the trade, and a build that renders
        // the wrong thing quietly would be the worse half of it.
        debug_assert_eq!(
            self.spans,
            annotate::markup(&self.text),
            "a block-scoped re-parse drew what a whole-Document parse would not"
        );

        let after = self.prints(first, self.last_line_of(&new, first));
        Edit {
            lines: changed(first, &before, &after),
            scope: count.map_or(Scope::Whole, |count| Scope::Blocks(head..head + count)),
        }
    }

    /// How the edit at `at` can be re-parsed, or `None` for the whole Document.
    ///
    /// Read against the text as it is, before the splice: the block index, the
    /// list markers and the setext underline it consults are all the ones the
    /// writer is editing, not the ones they are about to have.
    fn plan(&self, at: &Range<usize>, inserted: &str) -> Option<Plan> {
        let found = self.block_at(at.start)?;
        let block = &self.blocks[found];
        // A delete that runs off the end of its block is two blocks' business,
        // and the second of them is a boundary it crossed to get there.
        if at.end > block.at.end || self.at_a_boundary(at, inserted, block) || !self.apart(found) {
            return None;
        }
        let region = block.at.clone();
        let mut slice = String::with_capacity(region.len() + inserted.len());
        slice.push_str(&self.text[region.start..at.start]);
        slice.push_str(inserted);
        slice.push_str(&self.text[at.end..region.end]);
        let index = index(&slice, region.start);
        // The block has to still be the same kind of block. A paragraph the
        // edit turned into a list item joins the list under it and a heading
        // ends the paragraph over it, and either way a boundary has moved
        // somewhere the slice cannot see. What the writer typed is drawn all
        // the same — by the whole-Document parse, one keystroke later than the
        // block would have drawn it.
        if index.first().is_none_or(|first| first.kind != block.kind)
            || index.iter().any(|block| block.kind.opens())
        {
            return None;
        }
        Some(Plan {
            at: region,
            index,
            slice,
        })
    }

    /// Whether the block at `found` has a Gap, or the Document's edge, on both
    /// sides of it.
    ///
    /// Two blocks written hard against each other are two the parser decided
    /// to keep apart, and it decided that by reading the lines at their seam.
    /// An edit inside either one can move that decision, and a slice of one
    /// block cannot see far enough to know. A blank line between them settles
    /// it: nothing continues across one.
    fn apart(&self, found: usize) -> bool {
        let blank = |index: usize| {
            self.blocks
                .get(index)
                .is_none_or(|block| block.kind == Kind::Gap)
        };
        (found == 0 || blank(found - 1)) && blank(found + 1)
    }

    /// Whether the edit at `at` touches something a block alone cannot answer.
    ///
    /// The four ways a block boundary moves, as `docs/architecture.md`
    /// § Annotators and the keystroke path lists them, plus the one they all
    /// reduce to: a newline written or taken away splits or joins two blocks
    /// wherever it lands.
    fn at_a_boundary(&self, at: &Range<usize>, inserted: &str, block: &Block) -> bool {
        if inserted.contains('\n') || self.text[at.clone()].contains('\n') {
            return true;
        }
        match block.kind {
            // The blank line between two blocks is where one ends and the next
            // begins, and a link reference definition is read by the whole
            // Document or not at all.
            Kind::Gap => true,
            // Every line of one is a fence marker's business: a fence closed
            // early or opened late restyles everything below it.
            Kind::Code { fenced: true } | Kind::FrontMatter => true,
            Kind::List => self
                .markers(block.at.clone())
                .any(|marker| touches(at, &marker)),
            // The underline is the whole of what makes it a heading.
            Kind::Heading { setext: true } => at.end >= self.last_line_start(&block.at),
            _ => false,
        }
    }

    /// The list markers inside `block`, in order.
    fn markers(&self, block: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
        let from = self
            .spans
            .partition_point(|span| span.at.start < block.start);
        self.spans[from..]
            .iter()
            .take_while(move |span| span.at.start < block.end)
            .filter(|span| matches!(span.mark, Mark::BulletMarker | Mark::OrderedMarker))
            .map(|span| span.at.clone())
    }

    /// Moves the line-start table over the same bytes the text moved over.
    ///
    /// A line start is one byte past a newline, so the starts that go are
    /// exactly the ones whose newline was inside `at`, the starts that come are
    /// the newlines of `inserted`, and every start after the edit shifts by the
    /// delta. Rebuilding instead would be O(document) on every keystroke, which
    /// is the shape of cost long-form cannot carry.
    fn splice_lines(&mut self, at: &Range<usize>, inserted: &str, delta: isize) {
        let first = self.lines.partition_point(|&start| start <= at.start);
        let last = self.lines.partition_point(|&start| start <= at.end);
        let fresh: Vec<usize> = newlines(inserted)
            .map(|index| at.start + index + 1)
            .collect();
        let grew = fresh.len();
        self.lines.splice(first..last, fresh);
        for start in &mut self.lines[first + grew..] {
            *start = shift(*start, delta);
        }
    }

    /// Puts `blocks` where the blocks over `old` were, and shifts the rest.
    ///
    /// Returns the index the first of them landed at, which is what
    /// [`Scope::Blocks`] names.
    fn splice_index(
        &mut self,
        old: &Range<usize>,
        new: &Range<usize>,
        delta: isize,
        blocks: Vec<Block>,
    ) -> usize {
        let grew = blocks.len();
        let head = splice_by_start(&mut self.blocks, old, delta, blocks);
        // A re-parsed slice reaches exactly as far as its own bytes, so where
        // it ends in a Gap and what follows begins in one, the two are one Gap
        // that a whole-Document parse would never have split.
        self.heal(head + grew);
        self.heal(head);
        debug_assert!(
            self.blocks.first().is_none_or(|block| block.at.start == 0)
                && self
                    .blocks
                    .last()
                    .is_none_or(|b| b.at.end == self.text.len())
                && self
                    .blocks
                    .windows(2)
                    .all(|pair| pair[0].at.end == pair[1].at.start),
            "the block index stopped tiling the Document at {new:?}"
        );
        head
    }

    /// Joins the two blocks meeting at `seam` when both are Gaps.
    fn heal(&mut self, seam: usize) {
        if seam == 0 || seam >= self.blocks.len() {
            return;
        }
        if self.blocks[seam - 1].kind == Kind::Gap && self.blocks[seam].kind == Kind::Gap {
            self.blocks[seam - 1].at.end = self.blocks[seam].at.end;
            self.blocks.remove(seam);
        }
    }

    /// Puts `spans` and `runs` where the ones over `old` were, and shifts the
    /// rest.
    ///
    /// Both lists are sorted by where a span starts, and no span or run
    /// straddles a block boundary, so the stretch to replace is the one whose
    /// starts fall inside `old` and everything past it is an integer add.
    fn splice_spans(&mut self, old: &Range<usize>, delta: isize, spans: Vec<Span>, runs: Vec<Run>) {
        splice_by_start(&mut self.spans, old, delta, spans);
        splice_by_start(&mut self.runs, old, delta, runs);
    }

    /// The line `offset` is on.
    fn line_of(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        self.lines.partition_point(|&start| start <= offset) - 1
    }

    /// The last line `at` reaches, or `first` when `at` covers no bytes.
    fn last_line_of(&self, at: &Range<usize>, first: usize) -> usize {
        if at.end > at.start {
            self.line_of(at.end - 1)
        } else {
            first
        }
    }

    /// The byte the last line of `block` starts at.
    fn last_line_start(&self, block: &Range<usize>) -> usize {
        let line = self.last_line_of(block, self.line_of(block.start));
        self.lines[line]
    }

    /// The bytes of `line`, its newline included.
    ///
    /// A line past the end of the Document is the empty range at the end,
    /// because a caller retagging the lines an [`Edit`] named must be able to
    /// ask for the line after the last one without checking first.
    #[must_use]
    pub fn line_bytes(&self, line: usize) -> Range<usize> {
        let start = self.lines.get(line).copied().unwrap_or(self.text.len());
        let end = self
            .lines
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len())
            .max(start);
        start..end
    }

    /// What each line from `first` to `last` looks like, as the runs on it.
    ///
    /// The offsets are relative to the line's own start, which is the whole
    /// point: a line whose runs only moved because bytes were written above it
    /// looks the same, and so is not retagged.
    fn prints(&self, first: usize, last: usize) -> Vec<Print> {
        (first..=last)
            .map(|line| {
                let at = self.line_bytes(line);
                Print {
                    runs: self
                        .runs_in(&at)
                        .iter()
                        .map(|run| (clip(&run.at, &at), run.look))
                        .collect(),
                    spans: self
                        .spans_in(&at)
                        .iter()
                        .filter(|span| span.at.end > at.start)
                        .map(|span| (clip(&span.at, &at), span.mark))
                        .collect(),
                }
            })
            .collect()
    }
}

/// What one line draws as, measured from the line's own start.
///
/// Two lines with the same print draw alike wherever in the Document they have
/// ended up, which is what lets an edit hand back the lines that changed
/// rather than every line it moved: writing into a paragraph slides every run
/// below it along, and a run that only slid has the same print it had.
///
/// The spans are in it as well as the runs, and that is the whole reason it is
/// a type rather than a list of runs. A run carries the colour, the cut and
/// the ground; a heading's hanging indent and a bullet's hang are not in one,
/// because they belong to the line rather than to the bytes. A line whose runs
/// come out identical can still have to be drawn again — a setext heading that
/// changed level is exactly that line — so what the app reads a paragraph
/// property off has to be part of what says the line changed.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Print {
    /// The runs on the line, clipped to it.
    runs: Vec<(Range<usize>, Look)>,
    /// The spans reaching the line, clipped to it.
    spans: Vec<(Range<usize>, Mark)>,
}

/// `at` cut down to `line`, and measured from the line's start.
fn clip(at: &Range<usize>, line: &Range<usize>) -> Range<usize> {
    at.start.max(line.start) - line.start..at.end.min(line.end) - line.start
}

/// One entry of a list the splice moves: the blocks, the spans and the runs.
///
/// All three are ordered by where an entry starts and none of them straddles a
/// block boundary, which is what makes one splice serve all three.
trait Ranged {
    /// Where the entry is, in absolute bytes.
    fn at(&self) -> &Range<usize>;
    /// The same, to move.
    fn at_mut(&mut self) -> &mut Range<usize>;
}

impl Ranged for Block {
    fn at(&self) -> &Range<usize> {
        &self.at
    }
    fn at_mut(&mut self) -> &mut Range<usize> {
        &mut self.at
    }
}

impl Ranged for Span {
    fn at(&self) -> &Range<usize> {
        &self.at
    }
    fn at_mut(&mut self) -> &mut Range<usize> {
        &mut self.at
    }
}

impl Ranged for Run {
    fn at(&self) -> &Range<usize> {
        &self.at
    }
    fn at_mut(&mut self) -> &mut Range<usize> {
        &mut self.at
    }
}

/// Puts `fresh` where the entries starting inside `old` were, and moves every
/// entry after them by `delta`.
///
/// Returns the index the first of `fresh` landed at. The shift is the integer
/// add the block-scoped strategy trades a re-parse for: what is below an edit
/// is the same Markup it was, one delta further down the file.
fn splice_by_start<T: Ranged>(
    list: &mut Vec<T>,
    old: &Range<usize>,
    delta: isize,
    fresh: Vec<T>,
) -> usize {
    let head = list.partition_point(|entry| entry.at().start < old.start);
    let tail = list.partition_point(|entry| entry.at().start < old.end);
    let grew = fresh.len();
    list.splice(head..tail, fresh);
    for entry in &mut list[head + grew..] {
        let at = entry.at_mut();
        *at = shift(at.start, delta)..shift(at.end, delta);
    }
    head
}

/// One edit's block-scoped re-parse, worked out before the text moves.
struct Plan {
    /// The bytes the blocks it replaces covered, before the edit.
    at: Range<usize>,
    /// The block index of the slice, already rebased onto the Document.
    index: Vec<Block>,
    /// Those bytes with the edit written into them.
    slice: String,
}

/// The top-level blocks of `text`, tiling it, rebased onto `base`.
///
/// Depth is what makes a block top level: the parser nests, and everything
/// inside a list, a quote or a table belongs to the block that opened it. What
/// no block covers becomes a [`Kind::Gap`], so that the index tiles and a
/// binary search over it can never fall between two entries.
fn index(text: &str, base: usize) -> Vec<Block> {
    let mut found: Vec<Block> = Vec::new();
    let mut depth = 0usize;
    let mut push = |at: Range<usize>, kind| {
        found.push(Block {
            at: base + at.start..base + at.end,
            kind,
        });
    };
    for (event, at) in markdown::events(text) {
        match &event {
            Event::Start(tag) => {
                if depth == 0 {
                    push(at.clone(), kind_of(tag, &text[at]));
                }
                depth += 1;
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            // The one block that is a leaf event rather than a tag. Every
            // other event at depth 0 is inline, and inline is a block's own
            // business.
            Event::Rule if depth == 0 => push(at, Kind::Rule),
            _ => {}
        }
    }
    let mut blocks = Vec::with_capacity(found.len() * 2 + 1);
    let mut at = base;
    for block in found {
        if block.at.start > at {
            blocks.push(Block {
                at: at..block.at.start,
                kind: Kind::Gap,
            });
        }
        at = block.at.end;
        blocks.push(block);
    }
    if at < base + text.len() {
        blocks.push(Block {
            at: at..base + text.len(),
            kind: Kind::Gap,
        });
    }
    blocks
}

/// What kind of block `tag` opens, given `text`, the bytes it covers.
fn kind_of(tag: &Tag<'_>, text: &str) -> Kind {
    match tag {
        Tag::Paragraph => Kind::Paragraph,
        Tag::Heading { .. } => Kind::Heading {
            setext: !text.trim_start_matches([' ', '\t']).starts_with('#'),
        },
        Tag::BlockQuote(_) => Kind::Quote,
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => Kind::Code { fenced: true },
        Tag::CodeBlock(CodeBlockKind::Indented) => Kind::Code { fenced: false },
        Tag::List(_) => Kind::List,
        Tag::HtmlBlock => Kind::Html,
        Tag::MetadataBlock(_) => Kind::FrontMatter,
        Tag::Table(_) => Kind::Table,
        Tag::FootnoteDefinition(_) => Kind::Footnote,
        _ => Kind::Other,
    }
}

/// `spans` moved from a slice's own offsets onto the Document's.
///
/// The rebase is the whole trick the block-scoped strategy turns on, and it is
/// an integer add because the parser's offset iterator yields plain byte
/// ranges into whatever it was handed.
fn rebase(spans: Vec<Span>, base: usize) -> Vec<Span> {
    spans
        .into_iter()
        .map(|span| Span::new(base + span.at.start..base + span.at.end, span.mark))
        .collect()
}

/// The lines from `first` whose runs are not what they were.
///
/// The common head and the common tail come off, which is what leaves only the
/// lines that actually changed: writing into the middle of a paragraph moves
/// no run on the lines above it and moves every run on the lines below it by
/// the same delta, and a run that only moved is a run that looks the same.
fn changed<T: PartialEq>(first: usize, before: &[T], after: &[T]) -> Range<usize> {
    let head = before
        .iter()
        .zip(after)
        .take_while(|(one, other)| one == other)
        .count();
    let tail = before[head..]
        .iter()
        .rev()
        .zip(after[head..].iter().rev())
        .take_while(|(one, other)| one == other)
        .count();
    first + head..first + after.len() - tail
}

/// Whether the edit at `at` reaches into `marker`.
///
/// An insert is a point, and a point at a marker's far edge is past it: what a
/// writer types where a bullet ends is the item's text, not the bullet. A
/// delete is a stretch, and any overlap at all takes bytes out of the marker.
fn touches(at: &Range<usize>, marker: &Range<usize>) -> bool {
    if at.is_empty() {
        marker.contains(&at.start)
    } else {
        at.start < marker.end && marker.start < at.end
    }
}

/// Where each newline of `text` is.
fn newlines(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.bytes()
        .enumerate()
        .filter(|&(_, byte)| byte == b'\n')
        .map(|(at, _)| at)
}

/// `offset` moved by an edit's byte delta.
fn shift(offset: usize, delta: isize) -> usize {
    offset.saturating_add_signed(delta)
}

/// `count` as the signed number a byte delta is measured in.
fn signed(count: usize) -> isize {
    isize::try_from(count).unwrap_or(isize::MAX)
}

/// The byte each line of `text` starts at.
///
/// Lines are split on `\n` alone, which covers `\n` and `\r\n` both: the `\r`
/// of a CRLF file belongs to the line it ends, and `GtkTextIter` counts it
/// there too, so the two agree on every offset a writer can reach.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(newlines(text).map(|at| at + 1));
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Document of `text` with no file behind it.
    fn opened(text: &str) -> Document {
        Document::holding(text.to_string(), None)
    }

    /// Everything the splice kept in step, checked against building it fresh.
    ///
    /// This is the invariant the whole module exists to hold: whatever an edit
    /// re-parsed, the Document afterwards must be the Document a reader who had
    /// never seen the edit would have built from the same bytes.
    fn as_if_opened(document: &Document) {
        assert_eq!(
            document.lines,
            line_starts(&document.text),
            "the line table drifted from the text"
        );
        assert_eq!(
            document.blocks,
            index(&document.text, 0),
            "the block index drifted from the text"
        );
        assert_eq!(
            document.spans,
            annotate::markup(&document.text),
            "the spans are not the ones a whole-Document parse gives"
        );
        assert_eq!(
            document.runs,
            annotate::flatten(&document.spans),
            "the runs drifted from the spans"
        );
    }

    /// The block index as `(bytes, kind)` pairs, for reading in a failure.
    fn shape(document: &Document) -> Vec<(Range<usize>, Kind)> {
        document
            .blocks
            .iter()
            .map(|block| (block.at.clone(), block.kind))
            .collect()
    }

    #[test]
    fn untitled_has_no_path_and_no_text() {
        let doc = Document::untitled();
        assert_eq!(doc.path(), None);
        assert_eq!(doc.text(), "");
        assert_eq!(doc.title(), UNTITLED);
    }

    #[test]
    fn open_reads_the_file_and_remembers_its_path() {
        let path = write_temp("open_reads", "# The Lighthouse\n\nThe lamp had been lit.\n");
        let doc = Document::open(&path).expect("reads the file just written");
        assert_eq!(doc.text(), "# The Lighthouse\n\nThe lamp had been lit.\n");
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(doc.title(), "quill-engine-open_reads.md");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_of_a_missing_file_is_an_error_not_an_empty_document() {
        let path = std::env::temp_dir().join("quill-no-such-document.md");
        std::fs::remove_file(&path).ok();
        let err = Document::open(&path).expect_err("the file does not exist");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn an_empty_file_is_a_document_with_a_title() {
        let path = write_temp("empty_file", "");
        let doc = Document::open(&path).expect("reads the empty file");
        assert_eq!(doc.text(), "");
        assert_eq!(doc.title(), "quill-engine-empty_file.md");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_untitled_document_has_one_empty_line() {
        let doc = Document::untitled();
        assert_eq!(doc.place(0), Place { line: 0, index: 0 });
    }

    #[test]
    fn every_byte_of_a_short_document_lands_on_the_line_it_is_written_on() {
        let doc = opened("# One\ntwo\n\nfour\n");
        for (offset, line, index) in [
            (0, 0, 0),
            (2, 0, 2),
            (5, 0, 5),
            (6, 1, 0),
            (9, 1, 3),
            (10, 2, 0),
            (11, 3, 0),
            (15, 3, 4),
            (16, 4, 0),
        ] {
            assert_eq!(
                doc.place(offset),
                Place { line, index },
                "byte {offset} is not on line {line} at index {index}"
            );
        }
    }

    #[test]
    fn an_offset_past_the_end_lands_at_the_end_rather_than_panicking() {
        let doc = opened("abc\n");
        assert_eq!(doc.place(9_999), doc.place(doc.text().len()));
    }

    #[test]
    fn the_em_dash_in_the_sample_passage_shifts_bytes_without_shifting_lines() {
        let doc = Document::open(Path::new("../ref/sample.md"))
            .expect("the shared test passage is in the repo");
        let text = doc.text();
        let dash = text.find('—').expect("the sample passage has an em dash");
        let place = doc.place(dash);

        // The em dash is three UTF-8 bytes, so every byte after it on the line
        // sits three further along than its character count would suggest.
        let line = text
            .lines()
            .nth(place.line)
            .expect("the line the dash is on");
        assert_eq!(&line[place.index..place.index + 3], "—");
        assert!(
            line.chars().count() < line.len(),
            "the line must be the multi-byte case, or this test proves nothing"
        );
        assert_eq!(
            doc.place(dash + 3),
            Place {
                line: place.line,
                index: place.index + 3
            },
            "the byte after the dash is still on the same line"
        );
        for inside in 1..3 {
            assert_eq!(
                doc.place(dash + inside),
                doc.place(dash + 3),
                "a byte inside the dash lands after it, never inside it"
            );
        }
    }

    #[test]
    fn a_heading_span_reaches_the_line_and_index_the_app_sets_an_iter_from() {
        let doc = Document::open(Path::new("../shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        let spans = doc.spans();
        let first = spans.first().expect("the passage opens with a heading");
        assert_eq!(
            doc.place(first.at.start),
            Place { line: 0, index: 0 },
            "the first heading's marker starts the file"
        );
        let second = spans
            .iter()
            .find(|span| span.mark == Mark::Heading(2))
            .expect("the passage's second heading");
        assert_eq!(
            doc.place(second.at.start).line,
            4,
            "`## What the sea keeps` is the fifth line"
        );
    }

    // The block index.

    #[test]
    fn the_judged_passage_indexes_as_the_blocks_it_is_written_in() {
        let doc = Document::open(Path::new("../shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        assert_eq!(
            shape(&doc),
            [
                (0..17, Kind::Heading { setext: false }),
                (17..18, Kind::Gap),
                (18..207, Kind::Paragraph),
                (207..208, Kind::Gap),
                (208..230, Kind::Heading { setext: false }),
                (230..231, Kind::Gap),
                (231..352, Kind::Quote),
                (352..353, Kind::Gap),
                (353..429, Kind::List),
                (429..485, Kind::List),
                (485..562, Kind::Paragraph),
                (562..563, Kind::Gap),
                (563..615, Kind::Code { fenced: true }),
                (615..617, Kind::Gap),
                (617..745, Kind::Paragraph),
            ],
            "the index must name every top-level block of the judged passage"
        );
    }

    #[test]
    fn the_index_tiles_the_document_so_binary_search_finds_every_byte() {
        let doc = Document::open(Path::new("../shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        for offset in 0..=doc.text().len() {
            let found = doc
                .block_at(offset)
                .unwrap_or_else(|| panic!("byte {offset} is in no block"));
            let block = &doc.blocks()[found];
            assert!(
                block.at.contains(&offset) || offset == doc.text().len(),
                "byte {offset} was found in {:?}, which does not hold it",
                block.at
            );
        }
    }

    #[test]
    fn the_shapes_a_boundary_rule_is_written_for_are_the_kinds_they_index_as() {
        let doc = opened(concat!(
            "---\ntitle: The Lighthouse\n---\n\n",
            "Setext\n======\n\n",
            "- one\n- two\n\n",
            "```rust\nlet lamp = 1;\n```\n\n",
            "> quoted\n\n",
            "<div>\nhi\n</div>\n\n",
            "---\n\n",
            "| a | b |\n|---|---|\n| 1 | 2 |\n\n",
            "Text[^1].\n\n[^1]: A note.\n",
        ));
        let kinds: Vec<Kind> = doc
            .blocks()
            .iter()
            .map(|block| block.kind)
            .filter(|kind| *kind != Kind::Gap)
            .collect();
        assert_eq!(
            kinds,
            [
                Kind::FrontMatter,
                Kind::Heading { setext: true },
                Kind::List,
                Kind::Code { fenced: true },
                Kind::Quote,
                Kind::Html,
                Kind::Rule,
                Kind::Table,
                Kind::Paragraph,
                Kind::Footnote,
            ]
        );
    }

    // The splice.

    #[test]
    fn an_insert_and_a_delete_keep_the_text_and_the_line_table_together() {
        let mut doc = Document::open(Path::new("../ref/sample.md"))
            .expect("the shared test passage is in the repo");
        let dash = doc
            .text()
            .find('—')
            .expect("the sample passage has an em dash");
        let mut expected = doc.text().to_string();

        doc.insert(dash, "—");
        expected.insert(dash, '—');
        assert_eq!(doc.text(), expected, "the insert wrote the wrong bytes");
        as_if_opened(&doc);

        doc.delete(dash..dash + 3);
        expected.replace_range(dash..dash + 3, "");
        assert_eq!(doc.text(), expected, "the delete took the wrong bytes");
        as_if_opened(&doc);

        // And across a line, where the table has starts to move as well.
        let stop = doc.text().find(".\n").expect("the passage has a sentence");
        doc.insert(stop, " again");
        expected.insert_str(stop, " again");
        assert_eq!(doc.text(), expected);
        as_if_opened(&doc);
    }

    #[test]
    fn a_newline_written_and_taken_away_leaves_the_line_table_as_it_found_it() {
        let mut doc = opened("One two.\n\nThree four.\n");
        let table = doc.lines.clone();
        doc.insert(4, "\nand ");
        assert_eq!(doc.lines, line_starts(doc.text()));
        as_if_opened(&doc);
        doc.delete(4..9);
        assert_eq!(doc.lines, table, "the table did not come back");
        as_if_opened(&doc);
    }

    // The block-scoped re-parse.

    #[test]
    fn an_insert_inside_a_paragraph_re_parses_that_block_alone() {
        let mut doc = opened("# Title\n\nThe lamp *was* lit.\n\n## After\n");
        let edit = doc.insert(13, "old ");
        assert_eq!(
            edit.scope,
            Scope::Blocks(2..3),
            "only the paragraph the writer is in re-parses"
        );
        assert_eq!(
            doc.text(),
            "# Title\n\nThe old lamp *was* lit.\n\n## After\n"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn typing_a_hash_at_the_head_of_a_paragraph_makes_it_a_heading() {
        let mut doc = opened("Alpha.\n\nThe lamp was lit.\n");
        let edit = doc.insert(8, "# ");
        assert_eq!(
            edit.scope,
            Scope::Whole,
            "a paragraph that became a heading ends the block above it"
        );
        assert_eq!(doc.blocks()[2].kind, Kind::Heading { setext: false });
        assert!(
            doc.spans()
                .iter()
                .any(|span| span.mark == Mark::Heading(1) && span.at.start == 8),
            "the paragraph is a heading in this keystroke: {:?}",
            doc.spans()
        );
        assert_eq!(
            edit.lines,
            2..3,
            "one line changed, and it is the heading's"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn typing_the_closing_marker_styles_the_emphasis() {
        let mut doc = opened("The lamp *was lit.\n");
        assert!(
            !doc.spans().iter().any(|span| span.mark == Mark::Emphasis),
            "an unclosed marker is not emphasis"
        );
        let edit = doc.insert(13, "*");
        assert_eq!(edit.scope, Scope::Blocks(0..1));
        assert!(
            doc.spans().iter().any(|span| span.mark == Mark::Emphasis),
            "the closing marker styles the run"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_at_the_end_of_a_draft_is_the_last_block_and_not_the_whole_document() {
        let mut doc = opened("# Title\n\nThe lamp was lit.");
        let edit = doc.insert(doc.text().len(), " again.");
        assert_eq!(
            edit.scope,
            Scope::Blocks(2..3),
            "typing at the end of a draft types into its last block"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn the_blocks_after_an_edit_are_rebased_by_its_delta() {
        let passage = "# Title\n\nThe lamp *was* lit.\n\n## After\n\nAnd more.\n";
        for (name, at, written) in [
            ("before", 2..2, "New "),
            ("inside", 20..20, "very "),
            ("after", 41..41, "So "),
        ] {
            let mut doc = opened(passage);
            let before = shape(&doc);
            let delta = signed(written.len());
            doc.insert(at.start, written);
            let after = shape(&doc);
            assert_eq!(after.len(), before.len(), "{name}: a block came or went");
            for (one, other) in before.iter().zip(&after) {
                let moved = one.0.start >= at.start;
                let by = if moved { delta } else { 0 };
                assert_eq!(
                    other.0.start,
                    shift(one.0.start, by),
                    "{name}: {:?} did not shift by {by}",
                    one.0
                );
            }
            as_if_opened(&doc);

            // And the delete puts every one of them back where it was.
            doc.delete(at.start..at.start + written.len());
            assert_eq!(
                shape(&doc),
                before,
                "{name}: the delete did not rebase back"
            );
            as_if_opened(&doc);
        }
    }

    // The boundary fallback.

    #[test]
    fn an_edit_on_a_blank_line_re_parses_the_whole_document() {
        let mut doc = opened("Alpha.\n\nBeta.\n");
        let edit = doc.insert(7, "x");
        assert_eq!(edit.scope, Scope::Whole);
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_inside_a_fenced_block_re_parses_the_whole_document() {
        let mut doc = opened("Alpha.\n\n```rust\nlet lamp = 1;\n```\n\nBeta.\n");
        let edit = doc.insert(20, "very_");
        assert_eq!(edit.scope, Scope::Whole);
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_on_a_list_marker_re_parses_the_whole_document_but_one_beside_it_does_not() {
        let mut doc = opened("Alpha.\n\n- one\n- two\n");
        let marker = doc.markers(8..20).next().expect("the list's first bullet");
        assert_eq!(doc.insert(marker.start, "1").scope, Scope::Whole);

        let mut doc = opened("Alpha.\n\n- one\n- two\n");
        assert_eq!(
            doc.insert(marker.end, "x").scope,
            Scope::Blocks(2..3),
            "what a writer types where the bullet ends is the item's text"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_on_a_setext_underline_re_parses_the_whole_document() {
        let mut doc = opened("Alpha.\n\nTitle\n=====\n\nBeta.\n");
        let underline = doc.text().find("=====").expect("the setext underline");
        assert_eq!(doc.insert(underline, "=").scope, Scope::Whole);
        as_if_opened(&doc);

        let mut doc = opened("Alpha.\n\nTitle\n=====\n\nBeta.\n");
        assert_eq!(
            doc.insert(underline - 3, "x").scope,
            Scope::Blocks(2..3),
            "the heading's own words are the block's business"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_block_with_a_neighbour_against_it_re_parses_the_whole_document() {
        // A list can interrupt a paragraph, so these two are written hard
        // against each other and the parser decided where they part by reading
        // the seam. An edit in the paragraph can move that decision — deleting
        // the `x` here makes the first line a list item and the two blocks one
        // list — and a slice of the paragraph cannot see far enough to know.
        let mut doc = opened("Alpha.\n\nxx- one\n- two\n");
        assert_eq!(
            shape(&doc)[2..],
            [(8..16, Kind::Paragraph), (16..22, Kind::List)],
            "the fixture must be two blocks with no blank line between them"
        );
        assert_eq!(
            doc.delete(8..9).scope,
            Scope::Whole,
            "the paragraph is still a paragraph, so only its neighbour forces this"
        );
        as_if_opened(&doc);

        // The same edit with a blank line under the paragraph is the block's
        // own business, because nothing continues across a blank line.
        let mut doc = opened("Alpha.\n\nxx- one\n\n- two\n");
        assert_eq!(doc.delete(8..9).scope, Scope::Blocks(2..3));
        as_if_opened(&doc);

        // And the seam really does move: one byte further off the front makes
        // the paragraph a list item, and the two blocks one list.
        let mut doc = opened("Alpha.\n\nx- one\n- two\n");
        assert_eq!(doc.delete(8..9).scope, Scope::Whole);
        assert_eq!(
            shape(&doc)[2..],
            [(8..20, Kind::List)],
            "the two of them are now one list"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn the_runs_and_spans_of_a_line_are_found_without_reading_the_document() {
        let doc = Document::open(Path::new("../shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        for line in 0..doc.lines.len() {
            let at = doc.line_bytes(line);
            let runs: Vec<&Run> = doc
                .runs()
                .iter()
                .filter(|run| run.at.start < at.end && run.at.end > at.start)
                .collect();
            assert_eq!(
                doc.runs_in(&at).iter().collect::<Vec<_>>(),
                runs,
                "line {line}'s runs are not the ones a walk of the whole Document finds"
            );
            let spans: Vec<&Span> = doc
                .spans()
                .iter()
                .filter(|span| span.at.start < at.end && span.at.end > at.start)
                .collect();
            let found: Vec<&Span> = doc
                .spans_in(&at)
                .iter()
                .filter(|span| span.at.end > at.start)
                .collect();
            assert_eq!(
                found, spans,
                "line {line}'s spans are not the ones a walk of the whole Document finds"
            );
        }
    }

    #[test]
    fn a_fence_opened_at_the_top_restyles_everything_under_it() {
        let mut doc = opened("Alpha.\n\nBeta.\n\nGamma.\n");
        let edit = doc.insert(0, "```");
        assert_eq!(
            edit.scope,
            Scope::Whole,
            "a fence holds a container open past its own block"
        );
        assert_eq!(
            doc.blocks()[0],
            Block {
                at: 0..doc.text().len(),
                kind: Kind::Code { fenced: true }
            },
            "the rest of the Document is inside the fence: {:?}",
            shape(&doc)
        );
        as_if_opened(&doc);
    }

    // Only the lines whose runs changed.

    #[test]
    fn a_letter_typed_into_plain_prose_changes_no_line() {
        let mut doc = opened("Alpha.\n\nOne two three.\nFour five six.\n");
        let edit = doc.insert(12, "x");
        assert_eq!(edit.scope, Scope::Blocks(2..3));
        assert!(
            edit.lines.is_empty(),
            "plain prose carries no run, so no line has to be retagged: {:?}",
            edit.lines
        );
    }

    #[test]
    fn only_the_line_whose_runs_moved_is_handed_to_the_retag() {
        // One paragraph over two lines: the marked-up line and a plain one
        // under it, which the edit shifts but does not restyle.
        let mut doc = opened("Alpha.\n\nOne *two* three.\nFour five six.\n");
        let edit = doc.insert(8, "New ");
        assert_eq!(
            edit.lines,
            2..3,
            "the line under it only moved, and a run that moved looks the same"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_whole_document_fallback_still_hands_back_only_the_lines_that_changed() {
        let mut doc = opened("Alpha.\n\n*Gamma* and more.\n\nDelta.\n");
        // A blank line, so the fallback runs; the emphasis below it only moves.
        let edit = doc.insert(7, "# New");
        assert_eq!(edit.scope, Scope::Whole);
        assert_eq!(
            edit.lines,
            1..2,
            "a whole-Document parse is not a whole-Document retag"
        );
        assert!(
            doc.spans().iter().any(|span| span.mark == Mark::Heading(1)),
            "the new line is a heading"
        );
        as_if_opened(&doc);
    }

    // Every construct, edited, still renders what a whole parse would.

    #[test]
    fn every_byte_of_the_judged_passage_survives_an_edit_at_it() {
        let source = std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo");
        for offset in 0..=source.len() {
            if !source.is_char_boundary(offset) {
                continue;
            }
            let mut doc = opened(&source);
            let edit = doc.insert(offset, "x");
            assert_eq!(
                doc.blocks,
                index(doc.text(), 0),
                "an `x` written at byte {offset} indexed as {:?}",
                edit.scope
            );
            as_if_opened(&doc);
            let edit = doc.delete(offset..offset + 1);
            assert_eq!(doc.text(), source, "byte {offset} did not come back");
            assert_eq!(
                doc.blocks,
                index(doc.text(), 0),
                "taking the `x` at byte {offset} back out again indexed as {:?}",
                edit.scope
            );
            as_if_opened(&doc);
        }
    }

    #[test]
    fn the_shared_passage_survives_a_multi_byte_character_written_at_every_byte() {
        // The em dash again, but written *in* rather than read: three bytes at
        // a time, at every boundary of a passage that already has one.
        let source =
            std::fs::read_to_string("../ref/sample.md").expect("the shared test passage is here");
        for offset in 0..=source.len() {
            if !source.is_char_boundary(offset) {
                continue;
            }
            let mut doc = opened(&source);
            doc.insert(offset, "—");
            as_if_opened(&doc);
            doc.delete(offset..offset + 3);
            as_if_opened(&doc);
            assert_eq!(doc.text(), source, "byte {offset} did not come back");
        }
    }

    #[test]
    fn typing_at_the_end_of_a_long_draft_stays_off_the_fallback() {
        // The regime the Gate benches: a writer at the end of a 10,000-word
        // draft, typing prose. Every key of it has to be one block's work, or
        // the fallback is not off the path of ordinary typing at all.
        let mut doc = Document::open(Path::new("../shots/latency/doc10k.md"))
            .expect("the bench's document is in the repo");
        let last = doc.blocks().len() - 1;
        for written in ["T", "h", "e", " ", "l", "a", "m", "p"] {
            let edit = doc.insert(doc.text().len(), written);
            assert_eq!(
                edit.scope,
                Scope::Blocks(last..last + 1),
                "typing `{written}` at the end of the draft left its block"
            );
        }
        assert!(doc.text().ends_with("The lamp"));
    }

    fn write_temp(stem: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("quill-engine-{stem}.md"));
        std::fs::write(&path, text).expect("writes to the temp directory");
        path
    }
}
