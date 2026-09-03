//! Scroll sync: where the following pane scrolls when the other one moves.
//!
//! The Editor and the Preview show the same Document as two different pages,
//! so a shared pixel means nothing and a shared block means everything. Both
//! rules here take each pane's blocks as vertical ranges keyed by the block
//! index they render ([`crate::document::Document::blocks`]), and answer the
//! following pane's offset in its own coordinate. The widgets apply the
//! number, and the guard that stops the follower driving the driver back is
//! theirs too.
//!
//! - **A wheel or a scrollbar** takes the top-block rule ([`follow_top_block`]):
//!   whichever block is at the driver's top edge, and however far into it the
//!   edge falls, the follower puts the same block's same fraction at its own
//!   top edge.
//! - **An edit or a caret move** takes the caret rule ([`follow_caret`]): the
//!   caret's block lands at the fraction down the Preview that the caret's row
//!   has down the Editor's viewport, so the block being written stays where
//!   the eye already is.
//!
//! The two panes need not agree on which blocks exist: the rendered page has
//! no [`crate::document::Kind::Gap`] between paragraphs, and a folded table is
//! one block in the Editor and none in the Preview. A block the follower lacks
//! anchors on the next block it has, and the fold's blank space is taken up at
//! that block's top edge rather than by scrolling to a place that is not
//! there. Every answer is clamped to the follower's scrollable range and
//! rounded to whole pixels; it is an `f64` because `gtk::Adjustment`, which the
//! widgets set it on, counts in `f64`.
//!
//! Nothing here reads a [`crate::settings::Settings`] or a widget: sync is
//! always on, and both functions are pure arithmetic over their arguments
//! (ADR 0008).

/// One block as a pane lays it out: the Document block it renders, and where
/// it stands in that pane.
///
/// A pane's blocks are given in Document order, which is also top-to-bottom
/// order, and every measurement is in the coordinate that pane's scroll offset
/// is counted in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Block {
    /// The index of the Document block this renders, the one key the two panes
    /// share.
    pub key: usize,
    /// The block's top edge.
    pub top: f64,
    /// The block's height. A block of no height is all top edge, and a
    /// fraction into it means nothing.
    pub height: f64,
}

impl Block {
    /// A block of `height` standing at `top`, rendering Document block `key`.
    #[must_use]
    pub fn new(key: usize, top: f64, height: f64) -> Self {
        Self { key, top, height }
    }
}

/// The follower's offset that puts the driver's top block at the follower's
/// top edge, at the same fraction into that block.
///
/// `offset` is the driver's own offset and `max` the follower's scrollable
/// maximum — its content height less its viewport, never below zero. Identical
/// pages answer the driver's own offset; a follower with no blocks at all has
/// nothing to anchor on and answers the driver's offset clamped.
#[must_use]
pub fn follow_top_block(driver: &[Block], offset: f64, follower: &[Block], max: f64) -> f64 {
    let Some((key, fraction)) = at_top(driver, offset) else {
        return clamp(offset, max);
    };
    clamp(place(follower, key, fraction).unwrap_or(offset), max)
}

/// The follower's offset that puts the caret's block `fraction` down the
/// follower's viewport.
///
/// `fraction` is where the caret's row stands down the driving pane's
/// viewport, `viewport` the follower's own height, and `max` its scrollable
/// maximum. At the top and the foot of the Document the answer is the range's
/// end rather than the place the rule asks for.
#[must_use]
pub fn follow_caret(
    caret: usize,
    fraction: f64,
    follower: &[Block],
    viewport: f64,
    max: f64,
) -> f64 {
    let top = place(follower, caret, 0.0).unwrap_or(0.0);
    clamp(top - viewport * fraction.clamp(0.0, 1.0), max)
}

/// The block at `offset` and how far into it the offset falls, or `None` when
/// the pane has no blocks. An offset above the first block or below the last
/// takes that block's near end.
fn at_top(blocks: &[Block], offset: f64) -> Option<(usize, f64)> {
    let block = blocks
        .iter()
        .rev()
        .find(|block| block.top <= offset)
        .or_else(|| blocks.first())?;
    let fraction = if block.height > 0.0 {
        ((offset - block.top) / block.height).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((block.key, fraction))
}

/// Where `fraction` into block `key` stands in a pane, `None` when the pane has
/// no blocks. A key the pane lacks anchors on the top of the next block it has,
/// or, past the last of them, on the foot of the page.
fn place(blocks: &[Block], key: usize, fraction: f64) -> Option<f64> {
    if let Some(block) = blocks.iter().find(|block| block.key == key) {
        return Some(block.top + block.height * fraction);
    }
    if let Some(next) = blocks.iter().find(|block| block.key > key) {
        return Some(next.top);
    }
    let last = blocks.last()?;
    Some(last.top + last.height)
}

/// An offset inside the follower's range, in whole pixels.
fn clamp(offset: f64, max: f64) -> f64 {
    offset.clamp(0.0, max.max(0.0)).round()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page of four blocks, the third of which the other page will lack.
    fn page(scale: f64) -> Vec<Block> {
        [
            (0, 0.0, 100.0),
            (1, 100.0, 200.0),
            (2, 300.0, 60.0),
            (3, 360.0, 240.0),
        ]
        .into_iter()
        .map(|(key, top, height): (usize, f64, f64)| Block::new(key, top * scale, height * scale))
        .collect()
    }

    /// Far more room than any of these tests asks for, so that a clamp is
    /// never the reason for an answer.
    const ROOM: f64 = 10_000.0;

    // The top-block rule.

    #[test]
    fn identical_pages_answer_the_drivers_own_offset() {
        let blocks = page(1.0);
        for offset in [0.0, 50.0, 100.0, 237.0, 599.0] {
            assert_eq!(
                follow_top_block(&blocks, offset, &blocks, ROOM),
                offset,
                "at {offset}"
            );
        }
    }

    #[test]
    fn a_page_of_twice_the_height_travels_twice_as_far_into_the_block() {
        let driver = page(1.0);
        let follower = page(2.0);
        // Half into block 1, which stands at 100 and is 200 tall.
        assert_eq!(follow_top_block(&driver, 200.0, &follower, ROOM), 400.0);
        // Its top edge, and the top edge of the page.
        assert_eq!(follow_top_block(&driver, 100.0, &follower, ROOM), 200.0);
        assert_eq!(follow_top_block(&driver, 0.0, &follower, ROOM), 0.0);
    }

    #[test]
    fn a_block_the_follower_lacks_anchors_on_the_next_block_it_has() {
        let driver = page(1.0);
        let folded: Vec<Block> = driver
            .iter()
            .copied()
            .filter(|block| block.key != 2)
            .collect();
        // Anywhere in block 2 answers the top of block 3, wherever it stands.
        for offset in [300.0, 330.0, 359.0] {
            assert_eq!(
                follow_top_block(&driver, offset, &folded, ROOM),
                360.0,
                "at {offset}"
            );
        }
        // A block past the follower's last one anchors on the foot of it.
        let short: Vec<Block> = driver.iter().copied().take(2).collect();
        assert_eq!(follow_top_block(&driver, 400.0, &short, ROOM), 300.0);
    }

    #[test]
    fn the_answer_is_inside_the_followers_range() {
        let driver = page(1.0);
        let follower = page(2.0);
        assert_eq!(follow_top_block(&driver, 500.0, &follower, 700.0), 700.0);
        assert_eq!(follow_top_block(&driver, -80.0, &follower, 700.0), 0.0);
        // A pane with nothing to scroll stays where it is.
        assert_eq!(follow_top_block(&driver, 500.0, &follower, 0.0), 0.0);
    }

    #[test]
    fn a_follower_with_no_blocks_keeps_the_drivers_offset() {
        assert_eq!(follow_top_block(&page(1.0), 250.0, &[], ROOM), 250.0);
        assert_eq!(follow_top_block(&[], 250.0, &page(1.0), ROOM), 250.0);
    }

    // The caret rule.

    #[test]
    fn the_carets_block_lands_at_its_fraction_down_the_pane() {
        let follower = page(1.0);
        // Block 3 stands at 360; a caret a third down a 600-tall viewport
        // leaves 200 of the pane above it.
        assert_eq!(follow_caret(3, 1.0 / 3.0, &follower, 600.0, ROOM), 160.0);
        assert_eq!(follow_caret(1, 0.5, &follower, 600.0, ROOM), 0.0);
        // A caret at the very top of the viewport puts the block at the edge.
        assert_eq!(follow_caret(3, 0.0, &follower, 600.0, ROOM), 360.0);
    }

    #[test]
    fn the_caret_rule_clamps_at_the_documents_top_and_foot() {
        let follower = page(1.0);
        // The first block cannot be pushed down the pane.
        assert_eq!(follow_caret(0, 0.75, &follower, 600.0, ROOM), 0.0);
        // The last cannot be pulled past the foot.
        assert_eq!(follow_caret(3, 0.0, &follower, 600.0, 200.0), 200.0);
    }

    #[test]
    fn a_caret_block_the_preview_lacks_takes_the_next_one() {
        let folded: Vec<Block> = page(1.0)
            .into_iter()
            .filter(|block| block.key != 2)
            .collect();
        assert_eq!(follow_caret(2, 0.0, &folded, 600.0, ROOM), 360.0);
        assert_eq!(follow_caret(9, 0.0, &folded, 600.0, ROOM), 600.0);
        assert_eq!(follow_caret(0, 0.5, &[], 600.0, ROOM), 0.0);
    }
}
