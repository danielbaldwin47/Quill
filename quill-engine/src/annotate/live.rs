//! Live: the Annotator that renders markup in place.
//!
//! Live reads the Markup spans the keystroke already computed
//! ([`crate::annotate::markup`], kept per block by [`Document`]) and the
//! writer's caret, and says three things about the bytes: which marker bytes
//! are **folded** away, which bytes are **scaled** because they are a heading's,
//! and what **furniture** stands in a folded marker's cells. Nothing here is
//! drawn: the app maps each look to a tag and paints the furniture in its
//! snapshot.
//!
//! The block the caret is in — and every block a selection touches — is where
//! the writer is working, so nothing in it folds: the source is there to edit.
//! A block is what the Document's block index says it is, with one exception
//! the index cannot make: [`Kind::List`] is a whole list and every item of it,
//! and a writer edits one item at a time, so Live splits a list at its
//! [`Mark::BulletMarker`] and [`Mark::OrderedMarker`] spans and treats each item
//! as a block of its own. A nested item is an item, because its marker splits
//! the item around it just the same.
//!
//! A quotation's `>` is never folded and takes no furniture: a quote is a quote
//! by its marker, with no bar and no dimming (`docs/design.md` row Markers).
//! In a folded block it comes back as [`LiveLook::Quote`], which is the app's
//! instruction to set it in the body's own ink rather than the marker's. The
//! words inside it fold their own inline markers like any other prose.
//!
//! Spans come back in order, and one nesting is possible and no other: a
//! [`LiveLook::Scaled`] heading contains the folded markers and furniture
//! inside it. Everything else is disjoint, so the app may apply one tag per
//! span — `invisible` and `scale` are different properties, and overlapping
//! `GtkTextTag`s of different properties compose.
//!
//! All offsets are UTF-8 bytes from the start of the Document, as everywhere
//! else in the engine.

use std::ops::Range;

use crate::annotate::{Mark, Span};
use crate::document::{Block, Document, Kind};

/// Which kind of marker a fold took away.
///
/// The two are drawn differently: a block-leading marker hangs in the gutter,
/// where the furniture that replaces it stands, and an inline delimiter simply
/// closes up. Only a heading's `#`s reach here as [`Fold::Hanging`] — a
/// bullet, an ordered number and a task box are block-leading too, but they
/// come back as [`LiveLook::Furniture`], which already says what stands in
/// their cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fold {
    /// A marker at the head of its block, hung in the gutter.
    Hanging,
    /// A delimiter inside a line: `**`, `_`, a backtick, a link's brackets and
    /// its destination.
    Inline,
}

/// What stands in the cells of a marker Live folded away.
///
/// Every one of these but [`Furniture::Link`] replaces the marker under it, so
/// the span it arrives on is folded as well as furnished ([`Furniture::folds`]).
/// A link's words are the writer's own text and stay on the page; the furniture
/// over them is the accent rule and the destination a Ctrl+click opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Furniture {
    /// A bullet list item's dot, where its `-`, `*` or `+` stood.
    Bullet,
    /// An ordered list item's number, as the source wrote it, and the byte it
    /// wrote after it: a `1)` list draws `1)` and not `1.`.
    Number {
        /// The count the marker counts with.
        count: u32,
        /// `.` or `)`, the two CommonMark allows after a count.
        delimiter: char,
    },
    /// A task list item's box, and whether it is ticked. It covers the item's
    /// whole marker — the bullet and the box together — because one box stands
    /// where both did.
    Checkbox {
        /// True for `[x]` or `[X]`, false for `[ ]`.
        checked: bool,
    },
    /// The rule a thematic break draws, where its `---` stood.
    Hairline,
    /// The edge of a fenced code block, where its fence stood.
    Fence,
    /// A link's words, and the bytes of the destination as the source wrote it.
    /// For a reference link that is the label rather than an address: what it
    /// resolves to is a whole-document question, and the app answers it off the
    /// map [`crate::markdown::references`] builds once for the page it is
    /// furnishing.
    Link {
        /// The [`Mark::Url`] bytes inside the link.
        destination: Range<usize>,
    },
}

impl Furniture {
    /// Whether the bytes this furniture stands on are folded away under it.
    ///
    /// True for every marker's furniture and false for [`Furniture::Link`],
    /// whose bytes are the writer's words.
    #[must_use]
    pub const fn folds(&self) -> bool {
        !matches!(self, Self::Link { .. })
    }
}

/// What Live says a range of bytes is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveLook {
    /// Marker bytes that are not the caret's to edit: taken off the page.
    Folded(Fold),
    /// A heading, at its level 1 to 6. The app owns the ladder the level scales
    /// by; Live owns the level.
    Scaled(u8),
    /// Cells a marker left, and what stands in them.
    Furniture(Furniture),
    /// A quotation's `>`, in a block whose markers are folded. It is not
    /// folded and takes no furniture; what it takes is the body's own ink,
    /// where the Markup Annotator rests it at [`Mark::QuoteMarker`]'s marker
    /// ink. #263's story 31: a quote is unmistakably a quote and never reads as
    /// dimmed. The caret's own block keeps the Markup look, as every other
    /// marker in it does.
    Quote,
}

/// One judgement of Live's about one byte range of a Document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSpan {
    /// Absolute UTF-8 bytes from the start of the Document.
    pub at: Range<usize>,
    /// The Document block index of the block these bytes are in. A list item is
    /// a block to Live but not to the index, so every item of one list carries
    /// the list's own index.
    pub block: usize,
    /// What Live says those bytes are.
    pub look: LiveLook,
}

/// Live's spans over the whole of `doc`, with the writer at `at`.
///
/// `at` is the selection as the buffer holds it, and an empty range is the
/// caret, exactly as [`crate::focus::tiers`] takes it.
///
/// One walk of the block index and one of the Document's spans, so it costs the
/// length of the Document, and nothing on the keystroke lane asks for it: the
/// app asks [`spans_in`] both for the bytes it is retagging and for the page it
/// is furnishing. This is the whole-page answer the bounded one is held against
/// (`spans_in_reads_the_blocks_its_range_touches_and_no_others`), by a second
/// walk that reads the Document's spans whole rather than block by block.
#[must_use]
pub fn spans(doc: &Document, at: &Range<usize>) -> Vec<LiveSpan> {
    let text = doc.text();
    let markup = doc.spans();
    let mut out = Vec::new();
    let mut cursor = 0;
    for (index, block) in doc.blocks().iter().enumerate() {
        let first = cursor;
        while cursor < markup.len() && markup[cursor].at.start < block.at.end {
            cursor += 1;
        }
        block_spans(text, index, block, &markup[first..cursor], at, &mut out);
    }
    sorted(out)
}

/// Live's spans over the blocks `over` touches, with the writer at `at`.
///
/// The unit is the block, not the byte range, because a fold is a judgement
/// about a whole block and the app retags by block anyway. Bounded by the
/// blocks `over` reaches — the block index is binary-searched for the first of
/// them — so a keystroke pays for the block it is in and not for the
/// manuscript.
#[must_use]
pub fn spans_in(doc: &Document, at: &Range<usize>, over: &Range<usize>) -> Vec<LiveSpan> {
    let text = doc.text();
    let Some(first) = doc.block_at(over.start) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut index = first;
    loop {
        let block = doc.block(index);
        let end = block.at.end;
        block_spans(text, index, &block, &doc.spans_in(&block.at), at, &mut out);
        if end >= over.end || end >= text.len() {
            break;
        }
        index += 1;
    }
    sorted(out)
}

/// `out` in the order a reader walks it: by start, the widest first, so that a
/// scaled heading arrives before what it contains.
fn sorted(mut out: Vec<LiveSpan>) -> Vec<LiveSpan> {
    out.sort_by(|a, b| a.at.start.cmp(&b.at.start).then(b.at.end.cmp(&a.at.end)));
    out
}

/// Live's spans over one block of the index, whose Markup spans are `markup`.
fn block_spans(
    text: &str,
    block: usize,
    at: &Block,
    markup: &[Span],
    caret: &Range<usize>,
    out: &mut Vec<LiveSpan>,
) {
    for part in parts(at, markup) {
        let open = touches(&part, caret, text.len());
        emit(text, block, &part, markup, open, out);
    }
}

/// The part of its block that the writer at `offset` is in: the unit Live
/// folds by, as the module's own lines describe it.
///
/// The app asks so that a caret move can be answered with the fold's own unit
/// rather than the index's: a move from one item of a list to the next leaves
/// the Document's block where it was and moves this. `None` where the Document
/// has no block at `offset`.
///
/// One block's parts and no more: the walk is [`parts`] over the block
/// [`Document::block_at`] names, and the part is picked with [`touches`], the
/// predicate the fold itself uses, so the answer is the range the fold would
/// leave open — the end of the text among it.
#[must_use]
pub fn part_at(doc: &Document, offset: usize) -> Option<Range<usize>> {
    let block = doc.block(doc.block_at(offset)?);
    let caret = offset..offset;
    let len = doc.text().len();
    parts(&block, &doc.spans_in(&block.at))
        .into_iter()
        .find(|part| touches(part, &caret, len))
}

/// The blocks Live sees in one block of the index.
///
/// One, for everything but a list: a list is split at each of its items'
/// markers, because a writer edits one item at a time and the index has no
/// smaller block to offer.
fn parts(at: &Block, markup: &[Span]) -> Vec<Range<usize>> {
    if at.kind != Kind::List {
        return vec![at.at.clone()];
    }
    let mut parts = Vec::new();
    let mut start = at.at.start;
    for span in markup {
        if !matches!(span.mark, Mark::BulletMarker | Mark::OrderedMarker) {
            continue;
        }
        if span.at.start > start {
            parts.push(start..span.at.start);
            start = span.at.start;
        }
    }
    parts.push(start..at.at.end);
    parts
}

/// Whether the writer is in `part`, and so whether it stays unfolded.
///
/// An empty `caret` is the caret and anything else is a selection, which
/// unfolds every block it reaches. The end of the text belongs to the last
/// part, as it belongs to the last block ([`Document::block_at`]).
fn touches(part: &Range<usize>, caret: &Range<usize>, len: usize) -> bool {
    if caret.start == caret.end {
        part.contains(&caret.start) || (caret.start >= len && part.end >= len)
    } else {
        caret.start < part.end && part.start < caret.end
    }
}

/// Live's spans over one of a block's parts, `open` when the writer is in it.
fn emit(
    text: &str,
    block: usize,
    part: &Range<usize>,
    markup: &[Span],
    open: bool,
    out: &mut Vec<LiveSpan>,
) {
    // The task box a bullet swallowed, so that it is not furnished twice.
    let mut swallowed = None;
    for (index, span) in markup.iter().enumerate() {
        if !part.contains(&span.at.start) {
            continue;
        }
        if let Mark::Heading(level) = span.mark {
            // A heading is the size it is wherever the caret stands: what the
            // caret changes is whether its `#`s are on the page.
            out.push(LiveSpan {
                at: span.at.clone(),
                block,
                look: LiveLook::Scaled(level),
            });
            continue;
        }
        if open {
            continue;
        }
        let found = match span.mark {
            Mark::Markup => Some((span.at.clone(), LiveLook::Folded(fold(span, markup)))),
            Mark::LinkMark | Mark::Url | Mark::InfoString => {
                Some((span.at.clone(), LiveLook::Folded(Fold::Inline)))
            }
            Mark::Fence => Some((span.at.clone(), furnished(Furniture::Fence))),
            Mark::ThematicBreak => Some((span.at.clone(), furnished(Furniture::Hairline))),
            Mark::OrderedMarker => {
                let marker = &text[span.at.clone()];
                Some((
                    span.at.clone(),
                    furnished(Furniture::Number {
                        count: number(marker),
                        delimiter: delimiter(marker),
                    }),
                ))
            }
            Mark::BulletMarker => match task_box(markup, index) {
                Some((at, task)) => {
                    swallowed = Some(at);
                    Some((span.at.start..task.at.end, checkbox(text, task)))
                }
                None => Some((span.at.clone(), furnished(Furniture::Bullet))),
            },
            Mark::TaskBox if swallowed != Some(index) => {
                Some((span.at.clone(), checkbox(text, span)))
            }
            Mark::QuoteMarker => Some((span.at.clone(), LiveLook::Quote)),
            Mark::Link => {
                words(span, markup, block, out);
                None
            }
            _ => None,
        };
        if let Some((at, look)) = found {
            out.push(LiveSpan { at, block, look });
        }
    }
}

/// `furniture` as a look, for the marker cells it stands in.
const fn furnished(furniture: Furniture) -> LiveLook {
    LiveLook::Furniture(furniture)
}

/// Which kind of marker `span` is, of the ones marked [`Mark::Markup`].
///
/// A heading's `#`s open the heading, so they are the Markup span that starts
/// where the heading does; a closing run of hashes, an emphasis pair and a code
/// span's backticks all start later, inside a line.
fn fold(span: &Span, markup: &[Span]) -> Fold {
    let heading = markup.iter().any(|other| {
        matches!(other.mark, Mark::Heading(_))
            && other.at.start == span.at.start
            && other.at.end >= span.at.end
    });
    if heading { Fold::Hanging } else { Fold::Inline }
}

/// The task box the bullet marker at `index` carries, if it carries one.
///
/// A task item's marker cells are the bullet and the box together, with no
/// bytes between them (`a_task_box_is_marked_beside_the_bullet_that_carries_it`
/// in [`crate::annotate`]), so one checkbox stands in both.
fn task_box(markup: &[Span], index: usize) -> Option<(usize, &Span)> {
    let bullet = &markup[index];
    markup
        .iter()
        .enumerate()
        .skip(index + 1)
        .take_while(|(_, span)| span.at.start <= bullet.at.end)
        .find(|(_, span)| span.mark == Mark::TaskBox && span.at.start == bullet.at.end)
}

/// The box `span` covers as a look, ticked or not.
fn checkbox(text: &str, span: &Span) -> LiveLook {
    let checked = text[span.at.clone()].contains(['x', 'X']);
    furnished(Furniture::Checkbox { checked })
}

/// The byte an ordered marker closes its count with.
///
/// `.` or `)`, the two CommonMark allows; the fallback is the commoner of the
/// two and is unreachable for a marker the parser marked, standing here for the
/// reason [`number`]'s does.
fn delimiter(marker: &str) -> char {
    if marker.trim_end().ends_with(')') {
        ')'
    } else {
        '.'
    }
}

/// The number an ordered marker counts with.
///
/// The parser only marks a run of digits as one, so the fallback is unreachable
/// and is there to keep a malformed marker folded rather than half-drawn.
fn number(marker: &str) -> u32 {
    let digits: String = marker
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap_or(1)
}

/// The words of the link `span`, each carrying the destination it stands for.
///
/// The words are what is left of the link once its brackets and its destination
/// are taken out, the same subtraction the Markup Annotator makes to find them.
/// A link the source wrote no destination for — a shortcut reference — has no
/// words to open, so it gets none.
fn words(span: &Span, markup: &[Span], block: usize, out: &mut Vec<LiveSpan>) {
    let plumbing: Vec<&Span> = markup
        .iter()
        .filter(|other| {
            matches!(other.mark, Mark::LinkMark | Mark::Url)
                && other.at.start >= span.at.start
                && other.at.end <= span.at.end
        })
        .collect();
    let Some(url) = plumbing.iter().find(|other| other.mark == Mark::Url) else {
        return;
    };
    let destination = url.at.clone();
    let mut cursor = span.at.start;
    for other in &plumbing {
        if other.at.start > cursor {
            out.push(LiveSpan {
                at: cursor..other.at.start,
                block,
                look: furnished(Furniture::Link {
                    destination: destination.clone(),
                }),
            });
        }
        cursor = cursor.max(other.at.end);
    }
    if cursor < span.at.end {
        out.push(LiveSpan {
            at: cursor..span.at.end,
            block,
            look: furnished(Furniture::Link { destination }),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A passage carrying every construct Live has a rule for, which the shared
    /// sample passage does not: a quotation, an ordered list, a task list, a
    /// link, a code span, a thematic break and a fenced block.
    const PAGE: &str = "# The Lighthouse\n\nThe lamp was *lit* since dusk, and the keeper's **book** lay open.\n\n## What the sea keeps\n\n> A quote is a quote by its marker.\n\n- the rope\n- the good knife\n\n1. Untie the skiff.\n2. Row out.\n\n- [ ] scrub the lens\n- [x] wind the clock\n\nA [link to the book](https://example.org/book) and `code`.\n\n---\n\n```rust\nlet x = 1;\n```\n";

    /// The marks that are markers: the bytes Live takes off the page when their
    /// block is not the writer's. [`Mark::QuoteMarker`] is deliberately not one
    /// of them, and neither is a construct's own span.
    const MARKERS: [Mark; 9] = [
        Mark::Markup,
        Mark::LinkMark,
        Mark::Url,
        Mark::InfoString,
        Mark::Fence,
        Mark::ThematicBreak,
        Mark::BulletMarker,
        Mark::OrderedMarker,
        Mark::TaskBox,
    ];

    /// The passage the judged states are shot against.
    fn sample() -> Document {
        Document::open(Path::new("../dev/ref/sample.md"))
            .expect("the shared test passage is in the repo")
    }

    /// A Document holding `text`.
    fn document(text: &str) -> Document {
        let mut doc = Document::untitled();
        doc.insert(0, text);
        doc
    }

    /// The bytes `live` takes off the page: the folded markers, and the
    /// furniture that stands in a marker's cells rather than over the writer's
    /// own words.
    fn folded(live: &[LiveSpan]) -> Vec<Range<usize>> {
        live.iter()
            .filter(|span| match &span.look {
                LiveLook::Folded(_) => true,
                LiveLook::Furniture(furniture) => furniture.folds(),
                LiveLook::Scaled(_) | LiveLook::Quote => false,
            })
            .map(|span| span.at.clone())
            .collect()
    }

    /// Those bytes as the source text they cover, with the writer at `at`.
    fn folded_text<'a>(doc: &'a Document, at: &Range<usize>) -> Vec<&'a str> {
        folded(&spans(doc, at))
            .into_iter()
            .map(|at| &doc.text()[at])
            .collect()
    }

    /// The ranges of `doc`'s spans marked `mark`.
    fn marked(doc: &Document, mark: Mark) -> Vec<Range<usize>> {
        doc.spans()
            .into_iter()
            .filter(|span| span.mark == mark)
            .map(|span| span.at)
            .collect()
    }

    #[test]
    fn part_at_answers_the_item_the_offset_is_in_and_the_whole_block_elsewhere() {
        let text = concat!(
            "A paragraph\nthat runs on.\n\n",
            "- one\n- two\n  - nested\n\n",
            "```rust\nlet x = 1;\n```\n"
        );
        let doc = document(text);
        let at = |needle: &str| text.find(needle).expect("the passage carries it");
        let part = |offset: usize| part_at(&doc, offset).expect("every offset is in a block");

        // Each item of the list is its own part, the nested one included, and
        // one item ends where the next one's marker starts.
        let (one, two, nested) = (part(at("- one")), part(at("- two")), part(at("  - nested")));
        assert_eq!(one, at("- one")..at("- two"));
        assert_eq!(two, at("- two")..at("  - nested"));
        assert_eq!(nested.start, at("  - nested"));
        assert!(nested.end > at("nested"));

        // The paragraph and the fenced block are one part each, over both of
        // their lines, and the end of the text belongs to the last part.
        let paragraph = part(at("A paragraph"));
        assert_eq!(part(at("that runs on")), paragraph);
        assert!(!paragraph.contains(&at("- one")));
        let fence = part(at("```rust"));
        assert_eq!(part(at("let x")), fence);
        assert_eq!(
            part(text.len()).end,
            text.len(),
            "the end of the text belongs to the last part"
        );

        // And a part is exactly what the fold leaves open: with the writer in
        // the second item, that item's spans are gone and every other span is
        // the one it was with the writer in the paragraph.
        let far = spans(&doc, &(0..0));
        let here = spans(&doc, &(two.start..two.start));
        let elsewhere =
            |span: &&LiveSpan| !two.contains(&span.at.start) && !paragraph.contains(&span.at.start);
        assert_eq!(
            here.iter().filter(elsewhere).collect::<Vec<_>>(),
            far.iter().filter(elsewhere).collect::<Vec<_>>()
        );
        assert!(
            here.iter().all(|span| !two.contains(&span.at.start)),
            "nothing inside the writer's own item is folded: {here:?}"
        );
        assert!(
            far.iter().any(|span| two.contains(&span.at.start)),
            "with the writer elsewhere the item's marker folds: {far:?}"
        );
        assert!(
            here.iter().any(|span| one.contains(&span.at.start)),
            "the item the writer left folds while they stand in the next one: {here:?}"
        );
    }

    #[test]
    fn with_the_caret_in_each_block_every_marker_outside_it_folds_and_none_inside_it() {
        let doc = sample();
        let markers: Vec<Range<usize>> = doc
            .spans()
            .into_iter()
            .filter(|span| MARKERS.contains(&span.mark))
            .map(|span| span.at)
            .collect();
        assert_eq!(
            markers.len(),
            9,
            "the passage's two headings, its emphasis and strong pairs and its three bullets"
        );
        for (index, block) in doc.blocks().into_iter().enumerate() {
            let caret = block.at.start;
            let gone = folded(&spans(&doc, &(caret..caret)));
            for marker in &markers {
                let inside = marker.start >= block.at.start && marker.end <= block.at.end;
                if !inside {
                    assert!(
                        gone.contains(marker),
                        "with the caret in block {index} the marker at {marker:?} is outside it \
                         and folds: {gone:?}"
                    );
                } else if block.kind != Kind::List {
                    assert!(
                        !gone.contains(marker),
                        "the marker at {marker:?} is in the caret's own block {index} and stays"
                    );
                }
            }
        }
    }

    #[test]
    fn the_judged_caret_folds_every_marker_the_passage_has() {
        // 403, in the second paragraph: the caret the `live/folded` state is
        // shot at (`dev/shots/oracle/states.json`). That block carries no marker of
        // its own, so the whole passage folds.
        let doc = sample();
        assert_eq!(
            folded_text(&doc, &(403..403)),
            ["# ", "*", "*", "## ", "**", "**", "- ", "- ", "- "]
        );
    }

    #[test]
    fn a_list_folds_item_by_item_and_the_carets_own_item_alone_unfolds() {
        let doc = sample();
        let bullets = marked(&doc, Mark::BulletMarker);
        let caret = bullets[1].end;
        let gone = folded(&spans(&doc, &(caret..caret)));
        assert!(
            !gone.contains(&bullets[1]),
            "the writer is in the second item, so its bullet is there to edit: {gone:?}"
        );
        assert!(
            gone.contains(&bullets[0]) && gone.contains(&bullets[2]),
            "the items either side of it are not, so their bullets fold: {gone:?}"
        );
    }

    #[test]
    fn a_selection_spanning_two_blocks_unfolds_both() {
        let doc = sample();
        let blocks = doc.blocks();
        // From inside the first heading to inside the paragraph under it.
        let at = blocks[0].at.start + 2..blocks[2].at.start + 2;
        assert_eq!(
            folded_text(&doc, &at),
            ["## ", "**", "**", "- ", "- ", "- "],
            "the heading's `#` and the first paragraph's emphasis are both the \
             selection's, and nothing in the two blocks it reaches folds"
        );
    }

    #[test]
    fn a_quotations_marker_is_never_folded_and_takes_the_bodys_ink_when_its_block_is() {
        let doc = document(PAGE);
        let quote = marked(&doc, Mark::QuoteMarker);
        assert_eq!(quote.len(), 1, "the passage quotes one line");
        let block = doc
            .block_at(quote[0].start)
            .map(|at| doc.block(at).at)
            .expect("the quotation is a block of the index");
        for caret in [0, quote[0].start, quote[0].end, doc.text().len()] {
            let live = spans(&doc, &(caret..caret));
            let over: Vec<(Range<usize>, LiveLook)> = live
                .iter()
                .filter(|span| span.at.start < quote[0].end && quote[0].start < span.at.end)
                .map(|span| (span.at.clone(), span.look.clone()))
                .collect();
            let wanted = if block.contains(&caret) {
                Vec::new()
            } else {
                vec![(quote[0].clone(), LiveLook::Quote)]
            };
            assert_eq!(
                over, wanted,
                "with the caret at {caret} the `>` is neither folded nor \
                 furnished, and it is the body's own ink in every block but \
                 the writer's"
            );
        }
    }

    #[test]
    fn a_headings_text_is_scaled_by_its_level_wherever_the_caret_stands() {
        for level in 1..=6u8 {
            let text = format!(
                "{} Deep\n\nAnd prose below.\n",
                "#".repeat(usize::from(level))
            );
            let doc = document(&text);
            let heading = marked(&doc, Mark::Heading(level));
            assert_eq!(heading.len(), 1, "level {level} is one heading");
            for caret in [0, doc.text().len() - 2] {
                assert!(
                    spans(&doc, &(caret..caret)).contains(&LiveSpan {
                        at: heading[0].clone(),
                        block: 0,
                        look: LiveLook::Scaled(level),
                    }),
                    "level {level} is scaled with the caret at {caret}"
                );
            }
        }
    }

    #[test]
    fn a_headings_hashes_hang_folded_until_the_caret_is_in_the_heading() {
        let doc = document(PAGE);
        let hashes = 0..2;
        assert!(
            spans(&doc, &(30..30)).contains(&LiveSpan {
                at: hashes.clone(),
                block: 0,
                look: LiveLook::Folded(Fold::Hanging),
            }),
            "with the caret in the paragraph below, the `# ` hangs folded"
        );
        assert!(
            !folded(&spans(&doc, &(3..3))).contains(&hashes),
            "with the caret in the heading it is back on the page"
        );
    }

    #[test]
    fn an_inline_delimiter_folds_as_one_and_a_heading_opener_as_a_hang() {
        let doc = document(PAGE);
        let live = spans(&doc, &(0..0));
        let inline: Vec<&str> = live
            .iter()
            .filter(|span| span.look == LiveLook::Folded(Fold::Inline))
            .map(|span| &doc.text()[span.at.clone()])
            .collect();
        assert_eq!(
            inline,
            [
                "*",
                "*",
                "**",
                "**",
                "[",
                "](",
                "https://example.org/book",
                ")",
                "`",
                "`",
                "rust",
            ],
            "every delimiter that closes up inside its line; the two headings' \
             `#`s hang instead, and are not here"
        );
    }

    #[test]
    fn furniture_stands_in_the_cells_of_every_marker_it_replaces() {
        let doc = document(PAGE);
        // The caret in the first heading, so every block below it folds.
        let live = spans(&doc, &(0..0));
        let furniture: Vec<(&str, &Furniture)> = live
            .iter()
            .filter_map(|span| match &span.look {
                LiveLook::Furniture(furniture) => Some((&doc.text()[span.at.clone()], furniture)),
                _ => None,
            })
            .collect();
        assert_eq!(
            furniture,
            [
                ("- ", &Furniture::Bullet),
                ("- ", &Furniture::Bullet),
                (
                    "1. ",
                    &Furniture::Number {
                        count: 1,
                        delimiter: '.',
                    },
                ),
                (
                    "2. ",
                    &Furniture::Number {
                        count: 2,
                        delimiter: '.',
                    },
                ),
                ("- [ ]", &Furniture::Checkbox { checked: false }),
                ("- [x]", &Furniture::Checkbox { checked: true }),
                (
                    "link to the book",
                    &Furniture::Link {
                        destination: 272..296
                    }
                ),
                ("---", &Furniture::Hairline),
                ("```", &Furniture::Fence),
                ("```", &Furniture::Fence),
            ],
            "a task item's box stands in the bullet's cells and its own together, \
             and a link's words carry the destination a Ctrl+click opens"
        );
        assert_eq!(
            &doc.text()[272..296],
            "https://example.org/book",
            "the destination is the bytes the source wrote"
        );
    }

    #[test]
    fn a_task_box_reads_an_x_of_either_case_as_ticked() {
        let doc = document("Prose.\n\n- [X] wind the clock\n");
        let live = spans(&doc, &(0..0));
        assert!(
            live.iter().any(
                |span| span.look == LiveLook::Furniture(Furniture::Checkbox { checked: true })
            ),
            "`[X]` is a ticked box as much as `[x]` is: {live:?}"
        );
    }

    #[test]
    fn a_folded_fenced_blocks_fences_and_info_string_leave_the_page() {
        let doc = document(PAGE);
        let fences = marked(&doc, Mark::Fence);
        let info = marked(&doc, Mark::InfoString);
        let away = folded(&spans(&doc, &(0..0)));
        assert!(
            fences.iter().all(|at| away.contains(at)) && away.contains(&info[0]),
            "the fences and the `rust` after the opening one are the block's plumbing: {away:?}"
        );
        let caret = info[0].end + 2;
        let inside = folded(&spans(&doc, &(caret..caret)));
        assert!(
            fences.iter().all(|at| !inside.contains(at)),
            "with the caret in the block they are back: {inside:?}"
        );
    }

    #[test]
    fn live_spans_run_in_order_and_only_a_scaled_heading_holds_another() {
        let doc = document(PAGE);
        for caret in 0..=doc.text().len() {
            let live = spans(&doc, &(caret..caret));
            for (index, first) in live.iter().enumerate() {
                for second in &live[index + 1..] {
                    assert!(
                        first.at.start <= second.at.start,
                        "spans run in order: {first:?} before {second:?}"
                    );
                    let disjoint = second.at.start >= first.at.end;
                    let nested =
                        second.at.end <= first.at.end && matches!(first.look, LiveLook::Scaled(_));
                    assert!(
                        disjoint || nested,
                        "at caret {caret}, {second:?} overlaps {first:?} and the one \
                         nesting Live allows is a scaled heading holding what is inside it"
                    );
                }
            }
        }
    }

    #[test]
    fn every_span_carries_the_document_block_its_bytes_are_in() {
        let doc = document(PAGE);
        for span in spans(&doc, &(0..0)) {
            assert_eq!(
                doc.block_at(span.at.start),
                Some(span.block),
                "{span:?} names the block the index puts it in — a list's three \
                 items are three blocks to Live and one to the index"
            );
        }
    }

    #[test]
    fn spans_in_reads_the_blocks_its_range_touches_and_no_others() {
        let doc = document(PAGE);
        let at = 0..0;
        let whole = spans(&doc, &at);
        let blocks = doc.blocks();
        for block in &blocks {
            let expected: Vec<LiveSpan> = whole
                .iter()
                .filter(|span| block.at.contains(&span.at.start))
                .cloned()
                .collect();
            assert_eq!(
                spans_in(&doc, &at, &block.at),
                expected,
                "the retag of {:?} reads that block and nothing else",
                block.at
            );
        }
        let two = blocks[8].at.start..blocks[9].at.end;
        assert_eq!(
            spans_in(&doc, &at, &two),
            whole
                .iter()
                .filter(|span| two.contains(&span.at.start))
                .cloned()
                .collect::<Vec<_>>(),
            "a range over two blocks reads both"
        );
    }
}
