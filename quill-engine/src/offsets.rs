//! Offsets: ascending byte positions kept so that an edit shifts none of them.
//!
//! The line table and the block index are lists of byte offsets into the text,
//! and an edit moves every offset below it by the edit's delta. Kept as one
//! `Vec<usize>`, that move is a walk over the rest of the manuscript on every
//! keystroke — an integer add rather than a parse, but still a cost that grows
//! with the draft, and at 55,000 words it was the last thing between a
//! keystroke in the middle of a draft and one at the end (#218).
//!
//! An [`Offsets`] is the same list as a gap buffer of positions. The entries
//! at or above the last edit are held as absolute offsets, ascending; the
//! entries below it are held as distances from the end of the text, nearest
//! the edit last. An edit at the seam then changes one number, the text's
//! length, and every entry below it has moved by exactly the delta without
//! being touched. Typing at one place — which is what typing is — pushes and
//! pops at the seam; moving the caret a long way moves that many entries
//! across the seam once, at the cost of converting each, and then typing is
//! cheap again.

use std::fmt;
use std::ops::Range;

/// Ascending byte offsets, each carrying a tag, in two halves either side of
/// the last edit.
///
/// Read as one list: [`Offsets::get`] and [`Offsets::partition_point`] see
/// the entries in order, in absolute bytes, whichever half holds them.
#[derive(Clone)]
pub(crate) struct Offsets<T> {
    /// The entries at or above the seam, in absolute bytes, ascending.
    head: Vec<(usize, T)>,
    /// The entries below the seam, each as `len - offset`, the nearest the
    /// seam at the back — so that reading them from the back is ascending and
    /// the seam moves by a push or a pop.
    tail: Vec<(usize, T)>,
    /// The length of the text the tail is measured from.
    len: usize,
}

impl<T> Offsets<T> {
    /// The offsets of `entries`, ascending and in absolute bytes, into a text
    /// `len` bytes long.
    pub(crate) fn new(entries: Vec<(usize, T)>, len: usize) -> Self {
        debug_assert!(
            entries.windows(2).all(|pair| pair[0].0 <= pair[1].0)
                && entries.last().is_none_or(|last| last.0 <= len),
            "offsets out of order or past the end of the text"
        );
        Self {
            head: entries,
            tail: Vec::new(),
            len,
        }
    }

    /// How many entries there are.
    pub(crate) fn len(&self) -> usize {
        self.head.len() + self.tail.len()
    }

    /// The entry at `index`: its offset in absolute bytes and its tag.
    pub(crate) fn get(&self, index: usize) -> Option<(usize, &T)> {
        if let Some((offset, tag)) = self.head.get(index) {
            return Some((*offset, tag));
        }
        let from_back = index - self.head.len();
        let at = self.tail.len().checked_sub(from_back + 1)?;
        let (from_end, tag) = &self.tail[at];
        Some((self.len - from_end, tag))
    }

    /// The offset at `index`, in absolute bytes.
    pub(crate) fn offset(&self, index: usize) -> Option<usize> {
        self.get(index).map(|(offset, _)| offset)
    }

    /// Every entry, in order, in absolute bytes.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &T)> + '_ {
        self.head.iter().map(|(offset, tag)| (*offset, tag)).chain(
            self.tail
                .iter()
                .rev()
                .map(|(from_end, tag)| (self.len - from_end, tag)),
        )
    }

    /// The index of the first entry whose offset fails `holds`, given that
    /// `holds` is true for a prefix of the entries and false after it — the
    /// same contract as `[T]::partition_point`, over both halves at once.
    pub(crate) fn partition_point(&self, holds: impl Fn(usize) -> bool) -> usize {
        let in_head = self.head.partition_point(|&(offset, _)| holds(offset));
        if in_head < self.head.len() {
            return in_head;
        }
        // The tail is stored back to front, so along the vector the predicate
        // fails first and then holds; the entries it holds for are the back.
        let failing = self
            .tail
            .partition_point(|&(from_end, _)| !holds(self.len - from_end));
        self.head.len() + (self.tail.len() - failing)
    }

    /// Puts `fresh` where the entries at `stale` were, and moves everything
    /// after them by `delta`, the edit's change to the text's length.
    ///
    /// `fresh` is in absolute bytes of the text as it is after the edit. The
    /// entries after `stale` are moved by changing the length they are
    /// measured from, which is the whole point of the two halves.
    pub(crate) fn splice(&mut self, stale: Range<usize>, fresh: Vec<(usize, T)>, delta: isize) {
        self.seam_to(stale.start);
        self.tail
            .truncate(self.tail.len().saturating_sub(stale.end - stale.start));
        self.len = self.len.saturating_add_signed(delta);
        self.head.extend(fresh);
    }

    /// Takes the entry at `index` out.
    pub(crate) fn remove(&mut self, index: usize) -> Option<(usize, T)> {
        self.seam_to(index);
        self.tail
            .pop()
            .map(|(from_end, tag)| (self.len - from_end, tag))
    }

    /// Moves entries across the seam until exactly `index` of them are above
    /// it.
    fn seam_to(&mut self, index: usize) {
        while self.head.len() > index {
            let (offset, tag) = self.head.pop().expect("the head is longer than index");
            self.tail.push((self.len - offset, tag));
        }
        while self.head.len() < index {
            let Some((from_end, tag)) = self.tail.pop() else {
                break;
            };
            self.head.push((self.len - from_end, tag));
        }
    }
}

#[cfg(test)]
impl<T> Offsets<T> {
    /// Every offset, in order, in absolute bytes, without its tag — what a
    /// test compares with a table built fresh.
    pub(crate) fn offsets(&self) -> Vec<usize> {
        self.iter().map(|(offset, _)| offset).collect()
    }
}

impl<T: PartialEq> PartialEq for Offsets<T> {
    /// Two lists are equal when they read the same, wherever each one's seam
    /// happens to be.
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for Offsets<T> {}

impl<T: fmt::Debug> fmt::Debug for Offsets<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table of line starts with the seam moved into the middle of it.
    fn split() -> Offsets<char> {
        let mut table = Offsets::new(
            vec![(0, 'a'), (10, 'b'), (20, 'c'), (30, 'd'), (40, 'e')],
            50,
        );
        table.seam_to(2);
        table
    }

    #[test]
    fn the_two_halves_read_as_one_ascending_list() {
        let table = split();
        assert_eq!(table.head.len(), 2, "the seam did not move");
        assert_eq!(table.offsets(), vec![0, 10, 20, 30, 40]);
        assert_eq!(table.get(1), Some((10, &'b')));
        assert_eq!(table.get(2), Some((20, &'c')));
        assert_eq!(table.get(4), Some((40, &'e')));
        assert_eq!(table.get(5), None);
        assert_eq!(table.len(), 5);
    }

    #[test]
    fn a_partition_point_lands_the_same_wherever_the_seam_is() {
        let plain = Offsets::new(vec![(0, ()), (10, ()), (20, ()), (30, ()), (40, ())], 50);
        let mut moved = plain.clone();
        for seam in 0..=5 {
            moved.seam_to(seam);
            for offset in [0, 5, 10, 25, 40, 45, 60] {
                assert_eq!(
                    moved.partition_point(|start| start <= offset),
                    plain.partition_point(|start| start <= offset),
                    "offset {offset} with the seam at {seam}"
                );
            }
        }
    }

    #[test]
    fn a_splice_moves_what_is_below_it_without_touching_it() {
        let mut table = split();
        // Three bytes written at 15, on the line starting at 10: nothing
        // stale, nothing fresh, everything below moves by three.
        table.splice(2..2, Vec::new(), 3);
        assert_eq!(table.offsets(), vec![0, 10, 23, 33, 43]);
        assert_eq!(table.len, 53);
        // A newline written at 12: one fresh start at 13, and the rest move
        // by one more.
        table.splice(2..2, vec![(13, 'n')], 1);
        assert_eq!(table.offsets(), vec![0, 10, 13, 24, 34, 44]);
        assert_eq!(table.get(2), Some((13, &'n')));
        // The bytes 12..30 deleted: the starts at 13 and 24 go, the rest come
        // up by eighteen.
        table.splice(2..4, Vec::new(), -18);
        assert_eq!(table.offsets(), vec![0, 10, 16, 26]);
        assert_eq!(table.len, 36);
    }

    #[test]
    fn a_removal_closes_the_gap_and_the_rest_stay_where_they_were() {
        let mut table = split();
        assert_eq!(table.remove(3), Some((30, 'd')));
        assert_eq!(table.offsets(), vec![0, 10, 20, 40]);
        assert_eq!(table.remove(0), Some((0, 'a')));
        assert_eq!(table.offsets(), vec![10, 20, 40]);
        assert_eq!(table.remove(9), None);
    }

    #[test]
    fn equality_reads_through_the_seam() {
        let mut moved = split();
        let plain = Offsets::new(
            vec![(0, 'a'), (10, 'b'), (20, 'c'), (30, 'd'), (40, 'e')],
            50,
        );
        assert_eq!(moved, plain);
        moved.seam_to(5);
        assert_eq!(moved, plain);
        moved.seam_to(0);
        assert_eq!(moved, plain);
        assert_eq!(format!("{moved:?}"), format!("{plain:?}"));
    }
}
