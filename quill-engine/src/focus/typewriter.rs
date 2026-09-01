//! Typewriter: where the caret's row is held down the window, and how the
//! page travels to keep it there.
//!
//! The rules are the Parity oracle's, `legacy/app/js/focus.js` lines 200-260,
//! ported case for case and kept free of any widget (#115). Three things are
//! decided here and the Editor applies the numbers to its vertical adjustment:
//!
//! - **Which rule holds the row** ([`hold`]). With Typewriter on the row's
//!   centre is kept at the anchor, a share of the viewport, on every move —
//!   except for [`POINTER_MS`] after a press, when it is only nudged into the
//!   [`Band::Pointer`], so that a click never throws the page across the
//!   screen. With Focus on and Typewriter off it is nudged into the wider
//!   [`Band::Edge`], so that a lit sentence is never at the foot of the
//!   window. With both off the caret ticket's band applies
//!   ([`crate::typography::band_target`]), and this module says nothing.
//! - **Where the page goes** ([`target`]): the scroll that puts the row there,
//!   in the coordinate the scroll is counted in, or nothing when the row is
//!   already held.
//! - **How it gets there** ([`Glide`]): an ease-out cubic over a length that
//!   grows with the distance, retargetable mid-way. Whether to glide at all is
//!   [`crate::theme::animated`]: `--deterministic` and a desktop asking for
//!   reduced motion make every move a jump.

/// How long after a pointer press the [`Band::Pointer`] holds the row instead
/// of the anchor: `focus.js:240`.
pub const POINTER_MS: u32 = 400;

/// A move shorter than this is no move: the oracle's `Math.abs(delta) < 0.5`
/// (`focus.js:253`), which keeps a row already held from asking for a frame.
const STILL: f64 = 0.5;

/// A band of the viewport the caret's row is nudged into, and no further than
/// its nearest edge.
///
/// Each edge is the further in of a count of rows and a share of the viewport
/// ([`Band::edges`]), so that a short window still keeps a row's worth of
/// air and a tall one keeps a share of itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Band {
    /// The band a click nudges into, with Typewriter on: `focus.js:242`.
    Pointer,
    /// The band every move nudges into with Focus on and Typewriter off, wider
    /// than the pointer's: `focus.js:249`.
    Edge,
}

impl Band {
    /// The band's edges, `(low, high)`, measured down from the top of a
    /// viewport `viewport` high whose rows are `row_height` high.
    #[must_use]
    pub fn edges(self, row_height: f64, viewport: f64) -> (f64, f64) {
        let (rows, share) = match self {
            Band::Pointer => (1.5, 0.18),
            Band::Edge => (2.0, 0.15),
        };
        let low = (row_height * rows).max(viewport * share);
        let high = (viewport - row_height * rows).min(viewport * (1.0 - share));
        (low, high)
    }
}

/// Where the caret's row is held on one move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    /// Typewriter: the row's centre at this share of the viewport.
    Anchor(f64),
    /// Nudged into a band, as far as its nearest edge, and not at all inside.
    Nudge(Band),
    /// Neither mode holds it: the caret ticket's band is the caller's.
    Free,
}

/// Which rule holds the row: the three cases of `focus.js:233-251`.
///
/// `since_press` is how long ago the pointer was last pressed, or `None` when
/// it never was. The pointer band outranks the anchor for [`POINTER_MS`] after
/// a press whatever moved the caret, which is the oracle's rule: a key pressed
/// in that window nudges too.
#[must_use]
pub fn hold(typewriter: bool, anchor: f64, focus: bool, since_press: Option<u32>) -> Hold {
    if typewriter {
        if since_press.is_some_and(|ms| ms < POINTER_MS) {
            Hold::Nudge(Band::Pointer)
        } else {
            Hold::Anchor(anchor)
        }
    } else if focus {
        Hold::Nudge(Band::Edge)
    } else {
        Hold::Free
    }
}

/// Where a row whose centre is `centre` down the viewport should be nudged
/// to: `None` inside `band`, and the nearer edge outside it.
#[must_use]
pub fn nudge(centre: f64, band: Band, row_height: f64, viewport: f64) -> Option<f64> {
    let (low, high) = band.edges(row_height, viewport);
    if centre < low {
        Some(low)
    } else if centre > high {
        Some(high)
    } else {
        None
    }
}

/// The scroll that puts the caret's row where `hold` says, or `None` when
/// nothing should move.
///
/// Everything is in the one coordinate the scroll is counted in — `scroll` is
/// the top of the viewport, `row_top` the top of the row — as
/// [`crate::typography::band_target`] has it, and the answer is in it too. It
/// is not clamped to the document: a row near either end asks for a place the
/// view cannot go, and the caller has the adjustment that knows where the ends
/// are.
#[must_use]
pub fn target(
    row_top: f64,
    row_height: f64,
    scroll: f64,
    viewport: f64,
    hold: Hold,
) -> Option<f64> {
    if viewport <= 0.0 {
        return None;
    }
    let centre = row_top + row_height / 2.0 - scroll;
    let want = match hold {
        Hold::Anchor(anchor) => viewport * anchor,
        Hold::Nudge(band) => nudge(centre, band, row_height, viewport)?,
        Hold::Free => return None,
    };
    let delta = centre - want;
    (delta.abs() >= STILL).then_some(scroll + delta)
}

/// One eased scroll from where the view was to where the row is held.
///
/// Times are milliseconds since the glide started; the caller keeps the clock,
/// and asks [`Glide::at`] for the frame. The length is the oracle's rule
/// (`focus.js:255-257`): a row's travel — the common case, a new line — stays
/// quick, and a longer one grows with the square root of the distance up to a
/// ceiling, so a jump across the page is felt and a step is not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glide {
    /// The scroll it leaves.
    pub from: f64,
    /// The scroll it lands on.
    pub to: f64,
    /// How long it takes, in milliseconds.
    pub length: f64,
}

impl Glide {
    /// A glide from `from` to `to` over a page whose rows are `row_height`
    /// high.
    #[must_use]
    pub fn new(from: f64, to: f64, row_height: f64) -> Glide {
        let travel = (to - from).abs();
        let length = if travel <= row_height * 1.6 {
            120.0
        } else {
            (110.0 + travel.sqrt() * 7.0).min(280.0)
        };
        Glide { from, to, length }
    }

    /// Where the view is `elapsed` milliseconds in.
    #[must_use]
    pub fn at(&self, elapsed: u32) -> f64 {
        let share = if self.length <= 0.0 {
            1.0
        } else {
            (f64::from(elapsed) / self.length).min(1.0)
        };
        self.from + (self.to - self.from) * ease(share)
    }

    /// Whether the glide has landed `elapsed` milliseconds in.
    #[must_use]
    pub fn arrived(&self, elapsed: u32) -> bool {
        f64::from(elapsed) >= self.length
    }

    /// A new glide to `to`, leaving from where this one is `elapsed`
    /// milliseconds in: the oracle's `cancel()` followed by a fresh `glide`
    /// from the scroll it had reached.
    #[must_use]
    pub fn retarget(&self, elapsed: u32, to: f64, row_height: f64) -> Glide {
        Glide::new(self.at(elapsed), to, row_height)
    }
}

/// The oracle's ease-out cubic (`focus.js:219`): leaves fast, lands soft.
#[must_use]
pub fn ease(share: f64) -> f64 {
    1.0 - (1.0 - share).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The judged window at scale 1, and a row of the type it is shot in.
    const VIEW: f64 = 900.0;
    const ROW: f64 = 26.0;
    /// A view partway down a page, so that the answer is not the delta.
    const SCROLL: f64 = 1000.0;

    /// The scroll that puts a row whose centre is `centre` down the viewport
    /// where `hold` says.
    fn moves_to(centre: f64, hold: Hold) -> Option<f64> {
        target(SCROLL + centre - ROW / 2.0, ROW, SCROLL, VIEW, hold)
    }

    /// The oracle's own arithmetic for a row at `centre` held at `anchor`:
    /// `scrollTop + (y - anchor * h)`, `focus.js:238` and `:254`.
    fn oracle_anchor(centre: f64, anchor: f64) -> f64 {
        SCROLL + (centre - VIEW * anchor)
    }

    // The target.

    #[test]
    fn the_anchor_puts_the_rows_centre_at_its_share_of_the_viewport() {
        for anchor in [0.2, 0.5, 0.8] {
            for centre in [40.0, 450.0, 860.0] {
                let want = oracle_anchor(centre, anchor);
                let held = (centre - VIEW * anchor).abs() < STILL;
                assert_eq!(
                    moves_to(centre, Hold::Anchor(anchor)),
                    (!held).then_some(want),
                    "anchor {anchor} with the row at {centre}"
                );
            }
        }
    }

    #[test]
    fn a_row_already_at_the_anchor_asks_for_nothing() {
        assert_eq!(moves_to(VIEW * 0.5, Hold::Anchor(0.5)), None);
        assert_eq!(moves_to(VIEW * 0.5 + 0.4, Hold::Anchor(0.5)), None);
        assert_eq!(
            moves_to(VIEW * 0.5 + 0.5, Hold::Anchor(0.5)),
            Some(SCROLL + 0.5)
        );
    }

    #[test]
    fn a_free_hold_and_no_viewport_move_nothing() {
        assert_eq!(moves_to(880.0, Hold::Free), None);
        assert_eq!(target(0.0, ROW, 0.0, 0.0, Hold::Anchor(0.5)), None);
    }

    // The two bands.

    #[test]
    fn each_band_is_the_further_in_of_its_rows_and_its_share() {
        // A tall window: the share wins at both edges.
        assert_eq!(Band::Pointer.edges(ROW, VIEW), (VIEW * 0.18, VIEW * 0.82));
        assert_eq!(Band::Edge.edges(ROW, VIEW), (VIEW * 0.15, VIEW * 0.85));
        // A short window: the rows win.
        let short = 200.0;
        assert_eq!(
            Band::Pointer.edges(ROW, short),
            (ROW * 1.5, short - ROW * 1.5)
        );
        assert_eq!(Band::Edge.edges(ROW, short), (ROW * 2.0, short - ROW * 2.0));
    }

    #[test]
    fn a_nudge_moves_nothing_inside_a_band_and_to_the_nearer_edge_outside_it() {
        for band in [Band::Pointer, Band::Edge] {
            for row_height in [ROW, ROW * 2.0] {
                for viewport in [VIEW, 300.0] {
                    let (low, high) = band.edges(row_height, viewport);
                    let inside = [low, (low + high) / 2.0, high];
                    for centre in inside {
                        assert_eq!(
                            nudge(centre, band, row_height, viewport),
                            None,
                            "{band:?} at {centre} of {viewport} with rows {row_height}"
                        );
                    }
                    assert_eq!(nudge(low - 1.0, band, row_height, viewport), Some(low));
                    assert_eq!(nudge(0.0, band, row_height, viewport), Some(low));
                    assert_eq!(nudge(high + 1.0, band, row_height, viewport), Some(high));
                    assert_eq!(nudge(viewport, band, row_height, viewport), Some(high));
                }
            }
        }
    }

    #[test]
    fn a_nudged_target_is_the_scroll_that_puts_the_row_on_the_edge() {
        let (low, high) = Band::Pointer.edges(ROW, VIEW);
        assert_eq!(
            moves_to(low - 40.0, Hold::Nudge(Band::Pointer)),
            Some(SCROLL - 40.0)
        );
        assert_eq!(
            moves_to(high + 40.0, Hold::Nudge(Band::Pointer)),
            Some(SCROLL + 40.0)
        );
        assert_eq!(moves_to(VIEW * 0.5, Hold::Nudge(Band::Pointer)), None);
        let (low, high) = Band::Edge.edges(ROW, VIEW);
        assert_eq!(
            moves_to(low - 40.0, Hold::Nudge(Band::Edge)),
            Some(SCROLL - 40.0)
        );
        assert_eq!(
            moves_to(high + 40.0, Hold::Nudge(Band::Edge)),
            Some(SCROLL + 40.0)
        );
    }

    // Which rule holds the row.

    #[test]
    fn within_the_press_window_the_pointer_band_holds_and_after_it_the_anchor() {
        assert_eq!(hold(true, 0.5, false, Some(0)), Hold::Nudge(Band::Pointer));
        assert_eq!(
            hold(true, 0.5, true, Some(POINTER_MS - 1)),
            Hold::Nudge(Band::Pointer)
        );
        assert_eq!(hold(true, 0.5, false, Some(POINTER_MS)), Hold::Anchor(0.5));
        assert_eq!(hold(true, 0.2, true, Some(5_000)), Hold::Anchor(0.2));
        assert_eq!(hold(true, 0.8, false, None), Hold::Anchor(0.8));
    }

    #[test]
    fn with_focus_on_and_typewriter_off_the_edge_band_holds() {
        assert_eq!(hold(false, 0.5, true, None), Hold::Nudge(Band::Edge));
        assert_eq!(hold(false, 0.5, true, Some(0)), Hold::Nudge(Band::Edge));
        assert_eq!(hold(false, 0.5, false, Some(0)), Hold::Free);
        assert_eq!(hold(false, 0.5, false, None), Hold::Free);
    }

    // The ease.

    #[test]
    fn the_ease_leaves_fast_and_lands_soft() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!(ease(0.5) > 0.5);
        let (mut last, mut gain) = (0.0, f64::INFINITY);
        for step in 1..=10 {
            let now = ease(f64::from(step) / 10.0);
            assert!(now > last, "rises at step {step}");
            assert!(now - last < gain, "slows at step {step}");
            gain = now - last;
            last = now;
        }
    }

    #[test]
    fn a_rows_travel_is_quick_and_a_longer_one_grows_to_a_ceiling() {
        assert_eq!(Glide::new(0.0, ROW, ROW).length, 120.0);
        assert_eq!(Glide::new(0.0, ROW * 1.6, ROW).length, 120.0);
        let two = Glide::new(0.0, ROW * 2.0, ROW).length;
        assert!(two > 120.0 && two < 280.0, "two rows take {two}");
        assert_eq!(Glide::new(0.0, 600.0, ROW).length, 280.0);
        assert_eq!(Glide::new(600.0, 0.0, ROW).length, 280.0);
    }

    #[test]
    fn a_glide_reaches_its_target_and_not_before() {
        let glide = Glide::new(100.0, 700.0, ROW);
        assert_eq!(glide.length, 280.0);
        assert_eq!(glide.at(0), 100.0);
        assert!(!glide.arrived(0));
        let half = glide.at(140);
        assert!(half > 100.0 && half < 700.0, "midway is {half}");
        assert!(!glide.arrived(279));
        assert_eq!(glide.at(280), 700.0);
        assert!(glide.arrived(280));
        assert_eq!(glide.at(1_000), 700.0);
        assert!(glide.arrived(1_000));
    }

    #[test]
    fn a_glide_retargeted_midway_leaves_from_where_it_had_got_to() {
        let glide = Glide::new(100.0, 400.0, ROW);
        let reached = glide.at(100);
        let again = glide.retarget(100, 50.0, ROW);
        assert_eq!(again, Glide::new(reached, 50.0, ROW));
        assert_eq!(again.at(0), reached);
        assert_eq!(again.at(1_000), 50.0);
    }

    #[test]
    fn a_glide_of_no_length_is_already_there() {
        let glide = Glide {
            from: 0.0,
            to: 10.0,
            length: 0.0,
        };
        assert_eq!(glide.at(0), 10.0);
        assert!(glide.arrived(0));
    }
}
