//! Documents: text, the block index, and what one keystroke changes.
//!
//! A Document is one Markdown file on disk, and the file is the only source of
//! truth. The engine keeps its own `String` copy of the text, spliced from the
//! buffer's edit signals before any Annotator runs, and keeps beside it the
//! three things the splice has to keep in step with the text: the line-start
//! table, the block index, and the Markup spans flattened into runs.
//!
//! The **block index** is the list of top-level blocks, each a byte range and
//! the [`Kind`] it is. It tiles the Document — the stretches no block covers,
//! the blank lines between two blocks and the odd indent an indented block
//! leaves outside itself, are [`Kind::Gap`] entries — so binary search finds
//! the block containing any offset. An edit inside one block re-parses that
//! block's bytes alone with a fresh parser and rebases every range the parser
//! yields by the slice's start offset, which is the whole trick; the blocks
//! after it shift by the edit's byte delta, an integer add rather than a parse.
//! The spans and runs are not shifted at all: each block keeps its own,
//! measured from the block's start ([`BlockMarks`]), so a keystroke in the middle
//! of a manuscript moves the block index and the line table below it and
//! nothing else, and the accessors add a block's start back on the way out.
//!
//! An edit that touches a **block boundary** — a blank line, a fence marker, a
//! list marker, a setext underline — **widens** the re-parse to the block's
//! neighbours instead. Those four are the ways a boundary moves, and a block is
//! the smallest thing that re-parses correctly alone: a paragraph continuation
//! line, a lazy blockquote and a setext underline each change the meaning of
//! the line above. Widening stops where nothing is open — at a blank line, or
//! at the Document's edge — because a [`Kind::Gap`] is by definition outside
//! every container, so a region bounded by one parses in isolation the way it
//! parses in place.
//!
//! The one thing widening cannot bound is a **container context** the edit
//! opens or closes: a fence at the top makes every block under it code until
//! the closing fence, however far down that is. That is not a whole-Document
//! parse either. The region is extended forward by a **scan over bytes** — read
//! lines until the line that closes the container again — and the re-parse
//! covers exactly what the context changed over. Nothing below where it
//! re-converges is read at all.

use std::borrow::Cow;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, Event, Tag};

use crate::annotate::{self, Look, Mark, Run, Span};
use crate::markdown;
use crate::offsets::Offsets;

/// What a window titles a Document that is not on disk yet.
pub const UNTITLED: &str = "Untitled";

/// The name a Document at `path` is shown by: its file name without its
/// extension, which is the top bar's title ([`Document::name`]) and the label
/// of a recents row ([`crate::palette::recents`]).
///
/// A path with no file name at all — a bare `/`, a path ending in `..` — is
/// shown as it was written rather than as nothing, since a Document there is
/// already a path Quill cannot make sense of and hiding it would say less.
#[must_use]
pub fn shown_name(path: &Path) -> Cow<'_, str> {
    path.file_stem()
        .map_or_else(|| path.to_string_lossy(), std::ffi::OsStr::to_string_lossy)
}

/// What the file at `path` is called, extension and all: the name the Library
/// sorts and searches by, the name a rename field opens with, and the name a
/// status notice puts in its words.
///
/// [`shown_name`] is the same name without its extension, and is what a title
/// shows. Owned, because every caller either holds it past the borrow of
/// `path` or hands it to a widget; a name that is not UTF-8 is answered as
/// `to_string_lossy` writes it, which is what a window would draw.
#[must_use]
pub fn full_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

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
    /// paragraph, and widens only for what the walk out to a blank line
    /// widens anything for.
    Other,
}

impl Kind {
    /// Whether a block of this kind holds a container open past its own bytes.
    ///
    /// A fence, a metadata block and an HTML block all run until something
    /// closes them, and what closes them can be anywhere below. One of these
    /// ending a re-parsed region is the fence flip: the region is extended by
    /// [`Fence::closes`] to the line that closes it again, which is exactly the
    /// fence a writer opens at the top of a long draft, restyling everything
    /// under it.
    const fn opens(self) -> bool {
        matches!(
            self,
            Self::Code { fenced: true } | Self::FrontMatter | Self::Html
        )
    }

    /// Whether a block of this kind begins where it begins whatever is written
    /// on the line above it.
    ///
    /// An ATX heading, a thematic break, an opening fence and a metadata block
    /// each interrupt what was being written and cannot be a lazy continuation
    /// of it. So the byte one starts at is a seam the parser reads the same way
    /// in a slice as it does in place — the same thing a blank line gives,
    /// without the blank line — and the walk in [`Document::widened`] can stop
    /// there. Without it that walk runs to the edge of a Document written with
    /// no blank lines in it at all, which is the fallback under another name.
    ///
    /// A list is not here, and that is the point of `xx- one` in the tests: a
    /// list interrupts a paragraph only on what its own line says, so the seam
    /// between them is one an edit can move.
    const fn begins_a_block(self) -> bool {
        matches!(
            self,
            Self::Heading { setext: false }
                | Self::Rule
                | Self::Code { fenced: true }
                | Self::FrontMatter
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
///
/// Both halves are evidence, and a test asserts on them rather than assuming
/// the re-parse was small. The bytes are what the budget is about — they are
/// what the parser was handed — and the blocks are where the fresh index
/// landed, which is what says the splice put it in the right place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope {
    /// The blocks the re-parse produced, by their index in the block index
    /// *after* the edit.
    pub blocks: Range<usize>,
    /// The bytes it re-parsed, as they are *after* the edit.
    pub bytes: Range<usize>,
}

/// Where an edit cut into the text.
///
/// What a reader holding offsets from before the edit needs to carry them
/// across it: [`crate::focus::rebased`] moves the tiers Focus lit before a
/// keystroke into the text after it, so that the keystroke is judged against
/// where the dim was and not where its numbers happened to point (#224).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Splice {
    /// The bytes it took out, as they lay *before* the edit. Empty for an
    /// insertion, and then the offset the text went in at.
    pub at: Range<usize>,
    /// How many bytes it put in their place. Zero for a deletion.
    pub inserted: usize,
}

/// What one edit changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    /// The lines whose runs are not what they were, and so the lines the app
    /// retags. Empty when the edit changed no line's Markup at all, which is
    /// what typing a letter into a plain paragraph does.
    pub lines: Range<usize>,
    /// What had to be re-parsed to find them.
    pub scope: Scope,
    /// Where the edit cut, for whatever held offsets from before it.
    pub splice: Splice,
}

/// One Markdown file, plus everything the engine keeps in step with its text.
#[derive(Clone, Debug, Eq)]
pub struct Document {
    path: Option<PathBuf>,
    text: String,
    generation: u64,
    /// The byte each line starts at, ascending, always beginning with 0.
    lines: Offsets<()>,
    /// The top-level blocks, in order, tiling the whole text: the byte each
    /// starts at and what it is. A block ends where the next begins, or at
    /// the end of the text; [`Document::block`] puts the two together.
    blocks: Offsets<Kind>,
    /// Each block's Markup, one entry per block of `blocks`, measured from
    /// that block's start.
    markup: Vec<BlockMarks>,
}

// Generation describes edit history, not the text and indexes equality checks.
impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.text == other.text
            && self.lines == other.lines
            && self.blocks == other.blocks
            && self.markup == other.markup
    }
}

impl Default for Document {
    fn default() -> Self {
        Self {
            path: None,
            text: String::new(),
            generation: 0,
            lines: Offsets::new(vec![(0, ())], 0),
            blocks: Offsets::new(Vec::new(), 0),
            markup: Vec::new(),
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
        let lines = Offsets::new(
            line_starts(&text)
                .into_iter()
                .map(|start| (start, ()))
                .collect(),
            text.len(),
        );
        let blocks = index(&text, 0);
        let markup = distribute(&blocks, &annotate::markup(&text));
        let blocks = Offsets::new(starts_of(blocks), text.len());
        Self {
            path,
            text,
            generation: 0,
            lines,
            blocks,
            markup,
        }
    }

    /// Gives the Document the file it is: the name an untitled Document's
    /// first save derives for it, or the path a file moved to under it
    /// ([`crate::disk`]).
    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    /// Replaces the text with `text`, parsed whole, keeping the file.
    ///
    /// What a Document does when its file changed underneath it: the parse is
    /// [`Document::open`]'s, since nothing of the old text is reusable, and
    /// [`crate::disk::Filed::reload`] is what puts the caret back afterwards.
    pub fn reload(&mut self, text: String) {
        let generation = self.generation + 1;
        *self = Self::holding(text, self.path.take());
        self.generation = generation;
    }

    /// The text revision asynchronous Annotators stamp their results with.
    ///
    /// Advances on insert, delete and reload, and is excluded from equality.
    /// A new Document starts at zero; its worker and result store belong to it.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
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
    ///
    /// Assembled on the way out, block by block, so it costs the length of
    /// the index: a reader of one block on the keystroke path asks
    /// [`Document::block`] for it instead.
    #[must_use]
    pub fn blocks(&self) -> Vec<Block> {
        (0..self.blocks.len()).map(|at| self.block(at)).collect()
    }

    /// The block at `index` in the block index, as the bytes it covers and
    /// what it is.
    ///
    /// A block ends where the next one begins, and the last at the end of the
    /// text, because the index tiles the Document.
    ///
    /// # Panics
    ///
    /// When `index` is not below the count [`Document::blocks`] would give:
    /// the indices that reach here come from [`Document::block_at`] and
    /// [`Scope::blocks`], which name blocks the Document has.
    #[must_use]
    pub fn block(&self, index: usize) -> Block {
        let (start, kind) = self.indexed_start(index);
        let end = self.blocks.offset(index + 1).unwrap_or(self.text.len());
        Block {
            at: start..end,
            kind,
        }
    }

    /// Every block with its Markup, in order, from the block at `first`.
    fn indexed(&self, first: usize) -> impl Iterator<Item = (Block, &BlockMarks)> + '_ {
        (first..self.blocks.len()).map(|at| (self.block(at), &self.markup[at]))
    }

    /// The Markup spans of the whole Document, in absolute bytes.
    ///
    /// Assembled on the way out, because the spans are kept per block
    /// ([`BlockMarks`]); the readers of a whole Document — the tests, and the debug
    /// net under [`Document::edit`] — pay for the walk, and no keystroke does.
    #[must_use]
    pub fn spans(&self) -> Vec<Span> {
        self.indexed(0)
            .flat_map(|(block, markup)| markup.spans_from(block.at.start))
            .collect()
    }

    /// Those spans flattened into runs that do not overlap, in absolute bytes.
    #[must_use]
    pub fn runs(&self) -> Vec<Run> {
        self.indexed(0)
            .flat_map(|(block, markup)| markup.runs_from(block.at.start))
            .collect()
    }

    /// The runs that draw any of the bytes `at`.
    ///
    /// Found by binary search — on the block index for the block `at` starts
    /// in, then on both ends of that block's own runs — because the caller is
    /// the retag and the retag is on the keystroke path: walking the
    /// Document's runs to find a line's worth of them would put the cost of a
    /// keystroke back in proportion to the length of the manuscript, which is
    /// the one shape `docs/research/markdown-parser.md` rules out.
    #[must_use]
    pub fn runs_in(&self, at: &Range<usize>) -> Vec<Run> {
        self.over(at)
            .flat_map(|(block, markup)| markup.runs_over(block.at.start, at))
            .collect()
    }

    /// The spans that could cover any of the bytes `at`, and no others before
    /// them.
    ///
    /// Wider than [`Document::runs_in`] by construction: spans nest, so one
    /// starting well before `at` can still reach into it. What bounds the
    /// search is that a span never straddles a block — so nothing starting
    /// before the block holding `at.start` can reach `at`, and the block index
    /// finds that block by binary search. A span in the returned list may
    /// still end before `at` begins; the caller skips it.
    #[must_use]
    pub fn spans_in(&self, at: &Range<usize>) -> Vec<Span> {
        self.over(at)
            .flat_map(|(block, markup)| markup.spans_before(block.at.start, at.end))
            .collect()
    }

    /// The blocks `at` reaches, each with its Markup: from the block holding
    /// `at.start` through every block that begins before `at.end`.
    fn over(&self, at: &Range<usize>) -> impl Iterator<Item = (Block, &BlockMarks)> + '_ {
        let first = self.block_at(at.start).unwrap_or(0);
        let end = at.end;
        self.indexed(first)
            .take_while(move |(block, _)| block.at.start < end)
    }

    /// The block `offset` is in, by its index in [`Document::blocks`].
    ///
    /// Binary search, because there is one of these per edit and a Document is
    /// as long as a manuscript. The end of the text belongs to the last block:
    /// a writer typing at the end of a draft is typing into its last block, not
    /// past it.
    #[must_use]
    pub fn block_at(&self, offset: usize) -> Option<usize> {
        let found = self.blocks.partition_point(|start| start <= offset);
        let index = found.checked_sub(1)?;
        if offset < self.block(index).at.end || offset == self.text.len() {
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
            index: offset - self.line_start(line),
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

    /// The name the top bar shows: the file name without its extension, or
    /// [`UNTITLED`] until the first save (#246, story 48).
    #[must_use]
    pub fn name(&self) -> Cow<'_, str> {
        self.path
            .as_deref()
            .map_or(Cow::Borrowed(UNTITLED), shown_name)
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
        let old = plan.at.clone();
        let first = self.line_of(old.start);
        let before = self.prints(first, self.last_line_of(&old, first));

        self.text.replace_range(at.clone(), inserted);
        self.generation += 1;
        self.splice_lines(&at, inserted, delta);

        let new = plan.at.start..shift(plan.at.end, delta);
        let spans = rebase(annotate::markup(&plan.slice), plan.at.start);
        let blocks = self.splice_index(&old, &new, delta, plan.index, &spans);
        // The net under the whole strategy, and the reason it can be trusted
        // on a writer's own files rather than only on the passages the tests
        // hold. It costs a whole-Document parse per keystroke, so a debug
        // build types like one; that is the trade, and a build that renders
        // the wrong thing quietly would be the worse half of it.
        debug_assert_eq!(
            self.spans(),
            annotate::markup(&self.text),
            "a block-scoped re-parse drew what a whole-Document parse would not"
        );

        let after = self.prints(first, self.last_line_of(&new, first));
        Edit {
            lines: changed(first, &before, &after),
            scope: Scope { blocks, bytes: new },
            splice: Splice {
                at,
                inserted: inserted.len(),
            },
        }
    }

    /// Which bytes the edit at `at` re-parses, and the index they give.
    ///
    /// Read against the text as it is, before the splice: the block index, the
    /// list markers and the setext underline it consults are all the ones the
    /// writer is editing, not the ones they are about to have.
    ///
    /// The loop is the fence flip. A region is chosen to begin and end where
    /// nothing is open, so the only way the slice can leave a container open is
    /// that the edit opened one; the scan then carries the region on to where
    /// the context re-converges, and the fresh region is read again in case
    /// what it swallowed opened another.
    fn plan(&self, at: &Range<usize>, inserted: &str) -> Plan {
        let mut region = self.widened(at, inserted);
        loop {
            let slice = self.slice(&region, at, inserted);
            let index = index(&slice, region.start);
            let Some(fence) = self.left_open(&slice, &index, &region) else {
                return Plan {
                    at: region,
                    index,
                    slice,
                };
            };
            region.end = self.reconverges(region.end, fence);
        }
    }

    /// The bytes one edit re-parses, before any container scan widens them.
    ///
    /// The block the edit is in, widened to its neighbours when the edit
    /// touched one of the four things that move a boundary, and then carried
    /// out on both sides until nothing is open at either edge. A [`Kind::Gap`]
    /// is blank lines and nothing else, so an edge at one — or at the
    /// Document's own edge — is an edge the parser reads the same way in the
    /// slice as it does in place.
    fn widened(&self, at: &Range<usize>, inserted: &str) -> Range<usize> {
        let Some(found) = self.block_at(at.start) else {
            return 0..self.text.len();
        };
        let block = self.block(found);
        // A delete that runs off the end of its block crossed a boundary to
        // get there, so it widens for the same reason the four rules do.
        let widen = at.end > block.at.end || self.at_a_boundary(at, inserted, &block);
        // The rule widens to the block's *neighbours*, not to the blank lines
        // beside it: a blank line is what a Gap is made of, so stepping only
        // that far would leave a rule that fired changing nothing.
        let (mut lo, hi) = if widen {
            (self.before(found), self.after(found))
        } else {
            (found, found)
        };
        while lo > 0 && !self.settled(lo) {
            lo -= 1;
        }
        // `at.end` is the delete that ran off the end of its block: whatever
        // else the region is, it has to hold every byte the edit names, or the
        // slice cannot be built from it.
        self.block(lo).at.start..self.out_to_a_gap(hi, at.end)
    }

    /// `region`'s bytes with the edit written into them.
    ///
    /// Built rather than borrowed, and built again on every pass of the scan
    /// in [`Document::plan`], because the text has not moved yet: the region is
    /// decided against the index the writer is editing, and the only place the
    /// edited bytes exist until the splice is here.
    fn slice(&self, region: &Range<usize>, at: &Range<usize>, inserted: &str) -> String {
        let mut slice = String::with_capacity(region.len() + inserted.len());
        slice.push_str(&self.text[region.start..at.start]);
        slice.push_str(inserted);
        slice.push_str(&self.text[at.end..region.end]);
        slice
    }

    /// The container a re-parsed slice leaves open past `region`, if it leaves
    /// one and there is any Document left for it to reach.
    ///
    /// A blank line closes every container but a fence, and a fence swallows
    /// the blank lines under it — so the last block of the slice's own index is
    /// a [`Kind::Gap`] unless the edit opened something that runs off the end
    /// of the region. That is the whole of detecting the flip.
    fn left_open(&self, slice: &str, index: &[Block], region: &Range<usize>) -> Option<Open> {
        let last = index.last()?;
        if region.end >= self.text.len() || !last.kind.opens() {
            return None;
        }
        Open::left_open(
            &slice[last.at.start - region.start..last.at.end - region.start],
            last.kind,
        )
    }

    /// Where the container `fence` re-converges past `from`.
    ///
    /// The scan is over bytes rather than a parse, which is what makes a fence
    /// opened at the top of a long draft cost the reading of its lines and not
    /// the parsing of them. The line that closes the fence is a block edge in
    /// the new parse but not in the old index, and the splice replaces whole
    /// blocks, so the answer is carried out to an old block edge nothing is
    /// open at.
    fn reconverges(&self, from: usize, open: Open) -> usize {
        let Open::Fence(fence) = open else {
            return self.text.len();
        };
        let Some(closed) = fence.closes(&self.text[from..]).map(|end| from + end) else {
            return self.text.len();
        };
        let Some(hi) = self.block_at(closed) else {
            return self.text.len();
        };
        self.out_to_a_gap(hi, closed).max(closed)
    }

    /// Whether the block at `found` is a Gap — blank lines, and so a byte
    /// nothing is open at.
    fn blank(&self, found: usize) -> bool {
        self.kind(found) == Kind::Gap
    }

    /// What the block at `found` is.
    fn kind(&self, found: usize) -> Kind {
        self.indexed_start(found).1
    }

    /// Where block `index` starts and what it is, for an `index` the
    /// Document handed out ([`Document::block`]).
    fn indexed_start(&self, index: usize) -> (usize, Kind) {
        let (start, kind) = self
            .blocks
            .get(index)
            .expect("a block index the Document handed out");
        (start, *kind)
    }

    /// The byte a region beginning at block `hi` ends at: the end of the first
    /// Gap at or under it that also reaches `past`.
    ///
    /// Over the whole of the Gap, not up to where it begins. Which block the
    /// newline before a blank line belongs to is the parser's to say, and an
    /// edit can move it — a list absorbs the line under it or leaves it to the
    /// Gap, depending on bytes a writer can type. Inside the region that is the
    /// re-parse's answer to give; on the region's edge it is a seam that slips,
    /// and the every-byte sweep over the judged passage is what says so.
    fn out_to_a_gap(&self, mut hi: usize, past: usize) -> usize {
        let last = self.blocks.len() - 1;
        // The edge is `blocks[hi].at.end`, so what settles it is this block
        // being a Gap — over the whole of it, per above — or the *next* one
        // beginning wherever it begins. A Gap at `hi + 1` settles nothing: that
        // is the edge at a Gap's start, which is the seam that slips.
        while hi < last
            && ((!self.blank(hi) && !self.kind(hi + 1).begins_a_block())
                || self.block(hi).at.end < past)
        {
            hi += 1;
        }
        self.block(hi).at.end
    }

    /// Whether the block at `found` starts at a seam a slice can be cut at: it
    /// is a Gap, or it begins wherever it begins.
    fn settled(&self, found: usize) -> bool {
        self.blank(found) || self.kind(found).begins_a_block()
    }

    /// The block over `found` that is not a Gap, or the Document's first.
    fn before(&self, found: usize) -> usize {
        (0..found).rev().find(|&at| !self.blank(at)).unwrap_or(0)
    }

    /// The block under `found` that is not a Gap, or the Document's last.
    fn after(&self, found: usize) -> usize {
        let last = self.blocks.len() - 1;
        (found + 1..=last)
            .find(|&at| !self.blank(at))
            .unwrap_or(last)
    }

    /// Whether the edit at `at` touches something a block alone cannot answer,
    /// and so widens the re-parse to the block's neighbours.
    ///
    /// The four ways a block boundary moves, as `docs/architecture.md`
    /// § Annotators and the keystroke path lists them, plus the one they all
    /// reduce to: a newline written or taken away splits or joins two blocks
    /// wherever it lands.
    ///
    /// Two blocks written hard against each other need no rule of their own.
    /// The parser decided to keep them apart by reading the lines at their
    /// seam, and an edit in either one can move that decision — but the walk in
    /// [`Document::widened`] carries the region past a neighbour with no blank
    /// line before it whether or not a rule fired, because that edge is not one
    /// a slice can be cut at.
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
        self.spans_in(&block)
            .into_iter()
            .filter(|span| matches!(span.mark, Mark::BulletMarker | Mark::OrderedMarker))
            .map(|span| span.at)
    }

    /// Moves the line-start table over the same bytes the text moved over.
    ///
    /// A line start is one byte past a newline, so the starts that go are
    /// exactly the ones whose newline was inside `at`, the starts that come are
    /// the newlines of `inserted`, and every start after the edit moves by the
    /// delta — which [`Offsets`] does without walking them. Rebuilding instead
    /// would be O(document) on every keystroke, which is the shape of cost
    /// long-form cannot carry.
    fn splice_lines(&mut self, at: &Range<usize>, inserted: &str, delta: isize) {
        let first = self.lines.partition_point(|start| start <= at.start);
        let last = self.lines.partition_point(|start| start <= at.end);
        let fresh = newlines(inserted)
            .map(|index| (at.start + index + 1, ()))
            .collect();
        self.lines.splice(first..last, fresh, delta);
    }

    /// Puts `blocks` where the blocks over `old` were, with `spans` as their
    /// Markup, and shifts the rest.
    ///
    /// The shift is the integer add the block-scoped strategy trades a
    /// re-parse for: what is below an edit is the same Markup it was, one delta
    /// further down the file — and because each block's Markup is measured
    /// from the block's own start, the add touches the block index alone.
    ///
    /// Returns where the blocks landed, which is what [`Scope::blocks`] names.
    /// A heal at the head merges the first of them into the block before it,
    /// so the range it reports is one shorter and starts one earlier; a heal
    /// at the tail merges the block after them into the last of them, and
    /// leaves the range as it was.
    fn splice_index(
        &mut self,
        old: &Range<usize>,
        new: &Range<usize>,
        delta: isize,
        blocks: Vec<Block>,
        spans: &[Span],
    ) -> Range<usize> {
        // The index is kept as starts, and a block ends where the next one
        // begins, so the re-parsed blocks have to tile `new` exactly or the
        // block before them would quietly grow over the region.
        debug_assert!(
            (blocks.is_empty() && new.is_empty())
                || (blocks
                    .first()
                    .is_some_and(|block| block.at.start == new.start)
                    && blocks.last().is_some_and(|block| block.at.end == new.end)
                    && blocks
                        .windows(2)
                        .all(|pair| pair[0].at.end == pair[1].at.start)),
            "the re-parsed blocks do not tile {new:?}: {blocks:?}"
        );
        let markup = distribute(&blocks, spans);
        let grew = blocks.len();
        let mut head = self.blocks.partition_point(|start| start < old.start);
        let stale = self.blocks.partition_point(|start| start < old.end);
        self.blocks.splice(head..stale, starts_of(blocks), delta);
        self.markup.splice(head..stale, markup);
        let mut tail = head + grew;
        // A re-parsed slice reaches exactly as far as its own bytes, so where
        // it ends in a Gap and what follows begins in one, the two are one Gap
        // that a whole-Document parse would never have split.
        self.heal(tail);
        if self.heal(head) {
            head -= 1;
            tail -= 1;
        }
        debug_assert!(
            self.blocks.offset(0).is_none_or(|start| start == 0)
                && self.blocks.len() == self.markup.len(),
            "after {new:?} the block index does not start at 0 or has a block without its marks"
        );
        head..tail
    }

    /// Joins the two blocks meeting at `seam` when both are Gaps, and says
    /// whether it did.
    fn heal(&mut self, seam: usize) -> bool {
        if seam == 0 || seam >= self.blocks.len() {
            return false;
        }
        if self.kind(seam - 1) == Kind::Gap && self.kind(seam) == Kind::Gap {
            // A Gap carries no Markup as a rule, but the merge moves whatever
            // it does carry onto the joined block's start all the same. The
            // first Gap runs on to wherever the second ended once the second's
            // start is out of the index.
            let base = self.block(seam - 1).at.len();
            let taken = self.markup.remove(seam);
            self.markup[seam - 1].spans.extend(taken.spans_from(base));
            self.markup[seam - 1].runs.extend(taken.runs_from(base));
            self.blocks.remove(seam);
            return true;
        }
        false
    }

    /// The line `offset` is on.
    fn line_of(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        self.lines.partition_point(|start| start <= offset) - 1
    }

    /// The byte `line` starts at, or the end of the text for a line past the
    /// last.
    fn line_start(&self, line: usize) -> usize {
        self.lines.offset(line).unwrap_or(self.text.len())
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
        self.line_start(line)
    }

    /// The bytes of `line`, its newline included.
    ///
    /// A line past the end of the Document is the empty range at the end,
    /// because a caller retagging the lines an [`Edit`] named must be able to
    /// ask for the line after the last one without checking first.
    #[must_use]
    pub fn line_bytes(&self, line: usize) -> Range<usize> {
        let start = self.line_start(line);
        let end = self.line_start(line + 1).max(start);
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

/// The Markup of one block, measured from the block's own start.
///
/// Kept per block rather than as one list over the Document so that an edit
/// moves none of it. A span below an edit is the same span at the same offset
/// inside its block; only the block's start moved, and the block index is
/// what resolves a start. Before this, the spans and the runs were two more
/// absolute lists shifted by every keystroke, and at 55,000 words the two
/// shifts were most of what a keystroke in the middle of a draft cost (#218).
///
/// No span straddles a block, which is what lets a block own its spans
/// outright: a span is filed under the block it starts in, and the runs are
/// the block's own spans flattened, which comes out the same as flattening the
/// whole Document because [`annotate::flatten`] resolves nothing across a
/// stretch no span covers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BlockMarks {
    /// The block's spans, in the order [`annotate::markup`] gives them.
    spans: Vec<Span>,
    /// Those spans flattened: non-overlapping, in order.
    runs: Vec<Run>,
}

impl BlockMarks {
    /// The Markup of `block`, from the `spans` inside it in absolute bytes.
    fn of(block: &Block, spans: &[Span]) -> Self {
        let base = block.at.start;
        let spans: Vec<Span> = spans
            .iter()
            .map(|span| Span::new(span.at.start - base..span.at.end - base, span.mark))
            .collect();
        let runs = annotate::flatten(&spans);
        Self { spans, runs }
    }

    /// The spans, back in absolute bytes for a block starting at `base`.
    fn spans_from(&self, base: usize) -> impl Iterator<Item = Span> + '_ {
        rebased_spans(&self.spans, base)
    }

    /// The runs, back in absolute bytes for a block starting at `base`.
    fn runs_from(&self, base: usize) -> impl Iterator<Item = Run> + '_ {
        rebased_runs(&self.runs, base)
    }

    /// The spans starting before the absolute byte `end`, for a block
    /// starting at `base`.
    fn spans_before(&self, base: usize, end: usize) -> impl Iterator<Item = Span> + '_ {
        let count = self
            .spans
            .partition_point(|span| base + span.at.start < end);
        rebased_spans(&self.spans[..count], base)
    }

    /// The runs that draw any of the absolute bytes `at`, for a block starting
    /// at `base`, by binary search on both ends.
    fn runs_over(&self, base: usize, at: &Range<usize>) -> impl Iterator<Item = Run> + '_ {
        let start = at.start.saturating_sub(base);
        let end = at.end.saturating_sub(base);
        let from = self.runs.partition_point(|run| run.at.end <= start);
        let count = self.runs[from..].partition_point(|run| run.at.start < end);
        rebased_runs(&self.runs[from..from + count], base)
    }
}

/// `spans`, measured from a block's start, back in absolute bytes for a block
/// starting at `base`.
fn rebased_spans(spans: &[Span], base: usize) -> impl Iterator<Item = Span> + '_ {
    spans
        .iter()
        .map(move |span| Span::new(base + span.at.start..base + span.at.end, span.mark))
}

/// `runs`, measured from a block's start, back in absolute bytes for a block
/// starting at `base`.
fn rebased_runs(runs: &[Run], base: usize) -> impl Iterator<Item = Run> + '_ {
    runs.iter().map(move |run| Run {
        at: base + run.at.start..base + run.at.end,
        look: run.look,
    })
}

/// `blocks` as the block index keeps them: the byte each starts at and what it
/// is, the end left to the block after it.
fn starts_of(blocks: Vec<Block>) -> Vec<(usize, Kind)> {
    blocks
        .into_iter()
        .map(|block| (block.at.start, block.kind))
        .collect()
}

/// `spans`, in absolute bytes and in order, filed under the block of `blocks`
/// each starts in.
///
/// `blocks` tile the stretch the spans were parsed from, so every span lands
/// in one of them; the debug build checks that none was left over.
fn distribute(blocks: &[Block], spans: &[Span]) -> Vec<BlockMarks> {
    let mut from = 0;
    let markup = blocks
        .iter()
        .map(|block| {
            let count = spans[from..].partition_point(|span| span.at.start < block.at.end);
            let markup = BlockMarks::of(block, &spans[from..from + count]);
            from += count;
            markup
        })
        .collect();
    debug_assert_eq!(
        from,
        spans.len(),
        "a span starts outside every block it was parsed with: {:?}",
        &spans[from..]
    );
    markup
}

/// A container's opening line, in the two things that close it again: the
/// character it is written in, and how many of it there are.
///
/// This is the fence flip's whole vocabulary. A fence is closed by a line of
/// its own character, as many or more, and nothing after it — a rule the
/// CommonMark spec states over lines rather than over a parse tree, which is
/// why reading for it is a scan and not a second parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fence {
    /// The byte the fence is written in: a backtick, a tilde, or the dash of a
    /// metadata block.
    byte: u8,
    /// How many of it opened the container.
    len: usize,
}

/// What a container left open past a region gives the scan to look for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Open {
    /// Read on for the line that closes this fence.
    Fence(Fence),
    /// Read on to the end of the Document, because nothing the scan can match
    /// closes this one. An HTML block runs to a closing tag rather than to a
    /// line of its own character, and which tags do it is a list CommonMark
    /// keeps rather than a rule it states. Reading one this way costs a wider
    /// re-parse on an edit inside raw HTML, which is not a keystroke this
    /// Piece is about; reading it as closed would draw the wrong thing.
    ToTheEnd,
}

impl Open {
    /// The container `text` opens and does not close again, if it leaves one
    /// open.
    ///
    /// `text` is one block of a slice's own parse, so its first line is the
    /// opening line by construction; all that is being asked is whether a line
    /// under it closes the thing.
    fn left_open(text: &str, kind: Kind) -> Option<Self> {
        if kind == Kind::Html {
            return Some(Self::ToTheEnd);
        }
        let mut lines = text.split_inclusive('\n');
        let fence = Fence::of(lines.next()?)?;
        lines
            .all(|line| !fence.closed_by(line))
            .then_some(Self::Fence(fence))
    }
}

impl Fence {
    /// The fence `line` is written in, if it is a fence line at all.
    fn of(line: &str) -> Option<Self> {
        let bare = line.trim_start_matches(' ');
        if line.len() - bare.len() > 3 {
            return None;
        }
        let byte = *bare.as_bytes().first()?;
        if !matches!(byte, b'`' | b'~' | b'-') {
            return None;
        }
        let len = bare.bytes().take_while(|it| *it == byte).count();
        (len >= 3).then_some(Self { byte, len })
    }

    /// Whether `line` closes this fence.
    fn closed_by(self, line: &str) -> bool {
        let Some(other) = Self::of(line) else {
            return false;
        };
        // `len` counts bytes of one ASCII character, so it is a boundary.
        other.byte == self.byte
            && other.len >= self.len
            && line.trim_start_matches(' ')[other.len..].trim().is_empty()
    }

    /// The byte just past the line that closes this fence in `text`, or `None`
    /// when no line of it does.
    fn closes(self, text: &str) -> Option<usize> {
        let mut at = 0;
        for line in text.split_inclusive('\n') {
            at += line.len();
            if self.closed_by(line) {
                return Some(at);
            }
        }
        None
    }
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
            document.lines.offsets(),
            line_starts(&document.text),
            "the line table drifted from the text"
        );
        assert_eq!(
            document.blocks(),
            index(&document.text, 0),
            "the block index drifted from the text"
        );
        assert_eq!(
            document.spans(),
            annotate::markup(&document.text),
            "the spans are not the ones a whole-Document parse gives"
        );
        assert_eq!(
            document.runs(),
            annotate::flatten(&document.spans()),
            "the runs drifted from the spans"
        );
        assert_eq!(
            document.markup,
            distribute(&document.blocks(), &annotate::markup(&document.text)),
            "a span is filed under a block it does not start in"
        );
    }

    /// Every block's spans and runs as the accessors give them, in absolute
    /// bytes, which is what a rebase has to agree with a fresh parse on.
    fn read_back(document: &Document) -> Vec<(Vec<Span>, Vec<Run>)> {
        document
            .blocks()
            .iter()
            .map(|block| (document.spans_in(&block.at), document.runs_in(&block.at)))
            .collect()
    }

    /// The block index as `(bytes, kind)` pairs, for reading in a failure.
    fn shape(document: &Document) -> Vec<(Range<usize>, Kind)> {
        document
            .blocks()
            .into_iter()
            .map(|block| (block.at, block.kind))
            .collect()
    }

    /// `subject` with a paragraph and a heading over it and under it, and the
    /// bytes a rule firing inside it widens over.
    ///
    /// A rule widens to the block's neighbours, so the two paragraphs are what
    /// it reaches and the two headings are what it must not: a rule that
    /// widened to the whole Document passes no byte assertion written from
    /// this.
    fn padded(subject: &str) -> (String, Range<usize>) {
        let text = format!("# Top\n\nAlpha.\n\n{subject}\n\nBeta.\n\n## End\n");
        let region = "# Top\n".len()..text.find("## End").expect("the padding under it");
        (text, region)
    }

    /// What each span of `document` covers and what it marks, which is what a
    /// widening test names when it says what the spans are afterwards.
    fn marked(document: &Document) -> Vec<(&str, Mark)> {
        document
            .spans()
            .into_iter()
            .map(|span| (&document.text[span.at], span.mark))
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
        let doc = Document::open(Path::new("../dev/ref/sample.md"))
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
        let doc = Document::open(Path::new("../dev/shots/oracle/markup.md"))
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
        let doc = Document::open(Path::new("../dev/shots/oracle/markup.md"))
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
        let doc = Document::open(Path::new("../dev/shots/oracle/markup.md"))
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
        let mut doc = Document::open(Path::new("../dev/ref/sample.md"))
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
    fn an_edit_says_where_it_cut() {
        let mut doc = opened("One two.\n\nThree four.\n");
        let edit = doc.insert(4, "\nand ");
        assert_eq!(
            edit.splice,
            Splice {
                at: 4..4,
                inserted: 5
            },
            "an insertion cuts nothing out and says how much went in"
        );
        let edit = doc.delete(4..9);
        assert_eq!(
            edit.splice,
            Splice {
                at: 4..9,
                inserted: 0
            },
            "a deletion names the bytes it took, as they lay before it"
        );
    }

    #[test]
    fn a_newline_written_and_taken_away_leaves_the_line_table_as_it_found_it() {
        let mut doc = opened("One two.\n\nThree four.\n");
        let table = doc.lines.offsets();
        doc.insert(4, "\nand ");
        assert_eq!(doc.lines.offsets(), line_starts(doc.text()));
        as_if_opened(&doc);
        doc.delete(4..9);
        assert_eq!(doc.lines.offsets(), table, "the table did not come back");
        as_if_opened(&doc);
    }

    // The block-scoped re-parse.

    #[test]
    fn an_insert_inside_a_paragraph_re_parses_that_block_alone() {
        let mut doc = opened("# Title\n\nThe lamp *was* lit.\n\n## After\n");
        let edit = doc.insert(13, "old ");
        assert_eq!(
            edit.scope.blocks,
            1..4,
            "only the paragraph the writer is in re-parses, and the blank lines \
             around it, which are what says where it ends"
        );
        assert_eq!(
            edit.scope.bytes,
            8..34,
            "and the parser saw neither heading"
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
            edit.scope.blocks,
            1..3,
            "a blank line over it means nothing above it can be ended"
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
        assert_eq!(edit.scope.blocks, 0..1);
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
            edit.scope.blocks,
            1..3,
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

    // The four widening rules. Each fixture is written with a block outside
    // the widened region on both sides, so that a rule which widened to the
    // whole Document would fail the byte assertion rather than pass it.

    #[test]
    fn an_edit_on_a_blank_line_widens_to_the_blocks_on_either_side_of_it() {
        // The blank line is where one block ends and the next begins, and a
        // letter written on it joins the two: what was One, a blank and Two is
        // one paragraph of three lines.
        let (text, _) = padded("One.\n\nTwo.");
        let mut doc = opened(&text);
        let edit = doc.insert(text.find("Two.").expect("the block under it") - 1, "x");
        assert_eq!(
            edit.scope.bytes,
            text.find("One.").expect("the block over it") - 1
                ..text.find("Beta.").expect("the padding under it") + 1,
            "the two paragraphs and their own blank lines, and nothing past them"
        );
        assert_eq!(
            &shape(&doc)[4..6],
            [(15..27, Kind::Paragraph), (27..28, Kind::Gap)],
            "the blank line is gone and the two are one block: {:?}",
            shape(&doc)
        );
        assert_eq!(
            marked(&doc),
            [
                ("# Top", Mark::Heading(1)),
                ("# ", Mark::Markup),
                ("## End", Mark::Heading(2)),
                ("## ", Mark::Markup),
            ],
            "the joined prose carries no mark, and the padding kept its own"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_on_a_fence_marker_widens_to_the_blocks_on_either_side_of_it() {
        // Every line of a fenced block is a marker's business: a fence closed
        // early or opened late restyles what is under it.
        let (text, region) = padded("```rust\nlet lamp = 1;\n```");
        let mut doc = opened(&text);
        let edit = doc.insert(text.find("\nlet lamp").expect("the info string"), "y");
        assert_eq!(edit.scope.blocks, 1..8);
        assert_eq!(
            edit.scope.bytes,
            region.start..region.end + 1,
            "the fence and the blocks beside it, and nothing past them"
        );
        assert_eq!(
            marked(&doc),
            [
                ("# Top", Mark::Heading(1)),
                ("# ", Mark::Markup),
                ("```rusty\nlet lamp = 1;\n```", Mark::CodeBlock),
                ("```", Mark::Fence),
                ("rusty", Mark::InfoString),
                ("```", Mark::Fence),
                ("## End", Mark::Heading(2)),
                ("## ", Mark::Markup),
            ],
            "the info string took the letter and the block is still closed"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_on_a_list_marker_widens_to_the_blocks_on_either_side_of_it() {
        // `1` in front of the bullet is not a marker of any kind, so the item
        // stops being one and the list under it interrupts the paragraph it
        // became. That seam is what a slice of the list could not have seen.
        let (text, region) = padded("- one\n- two");
        let mut doc = opened(&text);
        let marker = doc
            .markers(region.clone())
            .next()
            .expect("the list's first bullet");
        let edit = doc.insert(marker.start, "1");
        assert_eq!(
            edit.scope.bytes,
            region.start..region.end + 1,
            "the list and the blocks beside it, and nothing past them"
        );
        assert_eq!(
            marked(&doc),
            [
                ("# Top", Mark::Heading(1)),
                ("# ", Mark::Markup),
                ("- ", Mark::BulletMarker),
                ("## End", Mark::Heading(2)),
                ("## ", Mark::Markup),
            ],
            "one bullet left, and it is the item the writer did not touch"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_edit_on_a_setext_underline_widens_to_the_blocks_on_either_side_of_it() {
        // The underline is the whole of what makes the line over it a heading.
        let (text, region) = padded("Title\n=====");
        let mut doc = opened(&text);
        let edit = doc.insert(text.find("=====").expect("the underline"), "=");
        assert_eq!(
            edit.scope.bytes,
            region.start..region.end + 1,
            "the heading and the blocks beside it, and nothing past them"
        );
        assert_eq!(
            marked(&doc),
            [
                ("# Top", Mark::Heading(1)),
                ("# ", Mark::Markup),
                ("## End", Mark::Heading(2)),
                ("## ", Mark::Markup),
            ],
            "a setext heading carries no span of its own — it has no marker to \
             hang, which `annotate` decided and tests — so what this names is \
             that the widening left the padding's spans exactly as they were"
        );
        assert_eq!(
            doc.blocks()[4].kind,
            Kind::Heading { setext: true },
            "and the longer underline is the same heading it was: {:?}",
            shape(&doc)
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_block_with_a_neighbour_against_it_widens_to_take_the_neighbour_in() {
        // A list can interrupt a paragraph, so these two are written hard
        // against each other and the parser decided where they part by reading
        // the seam. An edit in the paragraph can move that decision — deleting
        // the `x` here makes the first line a list item and the two blocks one
        // list — and a slice of the paragraph cannot see far enough to know.
        // No rule fires here: it is the walk out to a blank line that takes the
        // neighbour in, because the seam between them is not an edge a slice
        // can be cut at.
        let mut doc = opened("Alpha.\n\nxx- one\n- two\n");
        assert_eq!(
            shape(&doc)[2..],
            [(8..16, Kind::Paragraph), (16..22, Kind::List)],
            "the fixture must be two blocks with no blank line between them"
        );
        let edit = doc.delete(8..9);
        assert_eq!(edit.scope.blocks, 1..4);
        assert_eq!(
            edit.scope.bytes,
            7..21,
            "the paragraph and the list under it, and not the block above"
        );
        as_if_opened(&doc);

        // The same edit with a blank line under the paragraph is the block's
        // own business, because nothing continues across a blank line.
        let mut doc = opened("Alpha.\n\nxx- one\n\n- two\n");
        let edit = doc.delete(8..9);
        assert_eq!(edit.scope.blocks, 1..4);
        assert_eq!(
            edit.scope.bytes,
            7..16,
            "the paragraph and the blank lines around it, and not the list"
        );
        as_if_opened(&doc);

        // And the seam really does move: one byte further off the front makes
        // the paragraph a list item, and the two blocks one list.
        let mut doc = opened("Alpha.\n\nx- one\n- two\n");
        let edit = doc.delete(8..9);
        assert_eq!(edit.scope.blocks, 1..3);
        assert_eq!(
            shape(&doc)[2..],
            [(8..20, Kind::List)],
            "the two of them are now one list"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn the_runs_and_spans_of_a_line_are_found_without_reading_the_document() {
        let doc = Document::open(Path::new("../dev/shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        for line in 0..doc.lines.len() {
            let at = doc.line_bytes(line);
            let runs: Vec<Run> = doc
                .runs()
                .into_iter()
                .filter(|run| run.at.start < at.end && run.at.end > at.start)
                .collect();
            assert_eq!(
                doc.runs_in(&at),
                runs,
                "line {line}'s runs are not the ones a walk of the whole Document finds"
            );
            let spans: Vec<Span> = doc
                .spans()
                .into_iter()
                .filter(|span| span.at.start < at.end && span.at.end > at.start)
                .collect();
            let found: Vec<Span> = doc
                .spans_in(&at)
                .into_iter()
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
        // The user story: an unclosed fence at the top makes every block below
        // it code, in the keystroke that opened it.
        let mut doc = opened("Alpha.\n\nBeta.\n\nGamma.\n");
        let edit = doc.insert(0, "```\n");
        assert_eq!(
            doc.blocks()[0],
            Block {
                at: 0..doc.text().len(),
                kind: Kind::Code { fenced: true }
            },
            "the rest of the Document is inside the fence: {:?}",
            shape(&doc)
        );
        assert_eq!(
            edit.scope.blocks,
            0..1,
            "and it is one block now, however many it was"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_draft_with_no_blank_line_in_it_still_bounds_the_walk() {
        // The walk out of `widened` stops where nothing is open, and a writer
        // can hand it a Document with no blank line anywhere. An ATX heading
        // begins wherever it begins, so it is the other thing that stops the
        // walk; without it one keystroke here re-parses every byte, which is
        // the fallback under another name.
        let mut doc = opened(&"# One\n".repeat(5_000));
        let edit = doc.insert(3, "x");
        assert!(
            edit.scope.bytes.len() < 64,
            "one heading of 5,000 re-parsed {:?} of {} bytes",
            edit.scope.bytes,
            doc.text().len()
        );
        as_if_opened(&doc);
    }

    #[test]
    fn an_unclosed_fence_at_the_top_is_the_one_flip_that_reads_the_whole_draft() {
        // The scan finds no line to re-converge at, so the stretch that becomes
        // code is the rest of the Document — and this ticket *parses* that
        // stretch rather than deriving it. A fenced block's spans are
        // derivable (a ground, its opening fence, its info string), so the
        // parse is a cost the Piece could still pay off; it is an order of
        // magnitude inside the Gate's <= 16 ms worst, and this test is here to
        // say what it does rather than to bless it.
        let para = "Alpha *one*.\n\nBeta **two**.\n\n";
        let mut doc = opened(&format!("# Top\n\n{}", para.repeat(400)));
        let edit = doc.insert(7, "```\n");
        assert_eq!(
            edit.scope.bytes.end,
            doc.text().len(),
            "the scan ran to the end because nothing closed the fence"
        );
        assert_eq!(
            doc.blocks()[2],
            Block {
                at: 7..doc.text().len(),
                kind: Kind::Code { fenced: true }
            },
            "and every block under it is one block of code: {:?}",
            &shape(&doc)[..4]
        );
        as_if_opened(&doc);
    }

    #[test]
    fn the_fence_flip_stops_where_the_context_re_converges() {
        // A long Document with a fenced block partway down. A fence opened at
        // the top pairs with that block's *closing* fence — a closer may carry
        // no info string, so `\u{60}\u{60}\u{60}rust` is not one — and the scan
        // stops there. Everything below it is prose before the keystroke and
        // prose after it, and is never handed to the parser.
        let para = "Alpha *one*.\n\nBeta **two**.\n\n";
        let text = format!(
            "# Top\n\n{}```rust\nlet lamp = 1;\n```\n\n{}",
            para.repeat(100),
            para.repeat(300)
        );
        let mut doc = opened(&text);
        // The fenced block's own end is where the closing fence is: the opening
        // one carries an info string and a closer may not, so that closing
        // fence is the line the scan will stop at.
        let fenced = doc
            .blocks()
            .iter()
            .position(|block| block.kind == Kind::Code { fenced: true })
            .expect("the fenced block partway down");
        let close = doc.blocks()[fenced].at.end;
        let under = doc.blocks()[fenced + 1].at.end;
        let moved = "```\n".len();
        let tail = shape(&doc)
            .into_iter()
            .filter(|(at, _)| at.start >= close)
            .collect::<Vec<_>>();

        let edit = doc.insert(7, "```\n");
        assert_eq!(
            edit.scope.bytes.end,
            under + moved,
            "the re-parse stopped at the blank line under the fence that closed \
             the context again"
        );
        assert!(
            edit.scope.bytes.len() * 2 < doc.text().len(),
            "and that is under half the Document: {:?} of {}",
            edit.scope.bytes,
            doc.text().len()
        );
        assert_eq!(
            doc.blocks()[2],
            Block {
                at: 7..close + moved,
                kind: Kind::Code { fenced: true }
            },
            "everything from the new fence to the old one is code: {:?}",
            &shape(&doc)[..4]
        );
        assert_eq!(
            shape(&doc)
                .into_iter()
                .filter(|(at, _)| at.start >= close + moved)
                .map(|(at, kind)| (at.start - moved..at.end - moved, kind))
                .collect::<Vec<_>>(),
            tail,
            "and the prose past it is the blocks it always was, only moved"
        );
        as_if_opened(&doc);

        // Taking the fence away again restores them, and is bounded the same
        // way: the stretch that stops being code is what re-parses, and the
        // prose below the old fence is still never read.
        let edit = doc.delete(7..11);
        assert!(
            edit.scope.bytes.len() * 2 < doc.text().len(),
            "unfencing read {:?} of {}",
            edit.scope.bytes,
            doc.text().len()
        );
        assert_eq!(doc.text(), text, "the text is back to what it was");
        assert_eq!(shape(&doc), shape(&opened(&text)), "and so is every block");
        as_if_opened(&doc);
    }

    // Only the lines whose runs changed.

    #[test]
    fn a_letter_typed_into_plain_prose_changes_no_line() {
        let mut doc = opened("Alpha.\n\nOne two three.\nFour five six.\n");
        let edit = doc.insert(12, "x");
        assert_eq!(edit.scope.blocks, 1..3);
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
    fn a_widened_re_parse_still_hands_back_only_the_lines_that_changed() {
        let mut doc = opened("Alpha.\n\n*Gamma* and more.\n\nDelta.\n");
        // A blank line, so the re-parse widens over the emphasis under it; that
        // emphasis is re-parsed, but it comes out looking exactly as it did.
        let edit = doc.insert(7, "# New");
        assert!(edit.scope.bytes.contains(&10), "{:?}", edit.scope);
        assert_eq!(
            edit.lines,
            1..2,
            "a re-parse over a line is not a retag of it"
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
        let source = std::fs::read_to_string("../dev/shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo");
        for offset in 0..=source.len() {
            if !source.is_char_boundary(offset) {
                continue;
            }
            let mut doc = opened(&source);
            let edit = doc.insert(offset, "x");
            assert_eq!(
                doc.blocks(),
                index(doc.text(), 0),
                "an `x` written at byte {offset} indexed as {:?}",
                edit.scope
            );
            as_if_opened(&doc);
            let edit = doc.delete(offset..offset + 1);
            assert_eq!(doc.text(), source, "byte {offset} did not come back");
            assert_eq!(
                doc.blocks(),
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
        let source = std::fs::read_to_string("../dev/ref/sample.md")
            .expect("the shared test passage is here");
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
    fn typing_at_the_end_of_a_long_draft_is_one_block_whatever_the_draft_costs() {
        // The regime the Gate benches: a writer at the end of a draft, typing
        // prose. Every key of it has to be one block's work at either size, or
        // a keystroke in a long Document does not cost what one in a short
        // Document costs — which is the whole of what this Piece owes.
        for name in ["doc10k.md", "doc52k.md"] {
            let mut doc = Document::open(&Path::new("../dev/shots/latency").join(name))
                .expect("the bench's documents are in the repo");
            let last = doc.blocks().len() - 1;
            for written in ["T", "h", "e", " ", "l", "a", "m", "p"] {
                let edit = doc.insert(doc.text().len(), written);
                assert_eq!(
                    edit.scope.blocks,
                    last - 1..last + 1,
                    "{name}: typing `{written}` at the end of the draft left its block"
                );
                assert!(
                    edit.scope.bytes.len() < 1024,
                    "{name}: typing `{written}` handed the parser {} bytes",
                    edit.scope.bytes.len()
                );
            }
            assert!(doc.text().ends_with("The lamp"), "{name}");
        }
    }

    /// Five blocks with Markup in each, so an edit in the first has four
    /// blocks below it whose spans and runs only moved.
    const STACKED: &str = "# Title with *emphasis*\n\nSome **strong** prose here.\n\n\
                           - one `code`\n- two\n\n> a quote with a [link](x)\n\n\
                           Last paragraph, _still_ marked.\n";

    #[test]
    fn an_insert_in_the_first_block_leaves_every_block_below_it_reading_as_a_fresh_parse() {
        let mut doc = opened(STACKED);
        assert!(
            doc.blocks().len() >= 5,
            "the passage stacks blocks: {:?}",
            shape(&doc)
        );
        doc.insert("# Ti".len(), "xyz");
        let fresh = opened(doc.text());
        assert_eq!(
            read_back(&doc),
            read_back(&fresh),
            "a block below the edit reads differently from a fresh parse of the same text"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_delete_in_the_first_block_leaves_every_block_below_it_reading_as_a_fresh_parse() {
        let mut doc = opened(STACKED);
        doc.delete("# Title".len().."# Title with".len());
        let fresh = opened(doc.text());
        assert_eq!(
            read_back(&doc),
            read_back(&fresh),
            "a block below the edit reads differently from a fresh parse of the same text"
        );
        as_if_opened(&doc);
    }

    #[test]
    fn a_block_keeps_its_markup_from_its_own_start_so_an_edit_above_it_moves_none_of_it() {
        let mut doc = opened(STACKED);
        let last = doc.blocks().len() - 1;
        let before = doc.markup[last].clone();
        assert!(
            !before.spans.is_empty()
                && before
                    .spans
                    .iter()
                    .all(|span| span.at.end <= doc.blocks()[last].at.len()),
            "the last block's spans are not measured from its own start: {before:?}"
        );
        doc.insert(0, "Words written above everything.\n\n");
        let moved = doc.blocks().len() - 1;
        assert!(
            moved > last,
            "the insert put blocks above: {:?}",
            shape(&doc)
        );
        assert_eq!(
            doc.markup[moved], before,
            "an edit above a block rewrote the block's own Markup"
        );
        assert_eq!(
            doc.spans_in(&doc.blocks()[moved].at),
            opened(doc.text()).spans_in(&doc.blocks()[moved].at),
            "the accessor did not add the moved start back"
        );
    }

    fn write_temp(stem: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("quill-engine-{stem}.md"));
        std::fs::write(&path, text).expect("writes to the temp directory");
        path
    }
}
