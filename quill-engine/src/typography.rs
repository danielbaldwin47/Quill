//! Typography: the pitch, the measure, the margins a page is laid out from and
//! the band the caret's row is kept in.
//!
//! The numbers, and none of the widget that reads them. Everything that has to
//! agree with a row of text — the leading, the top of the page, the caret's
//! height and width, later the Typewriter anchor — asks this module rather
//! than doing the arithmetic again.
//!
//! The type sizes are a **ladder of fourteen steps**, not a range of pixels.
//! They are the Design oracle's own, measured off iA Writer for Mac and
//! written out in [`LADDER`]: a step names an em, a line pitch and a caret
//! width that were read together, so nothing here fits a curve through them
//! ([ADR 0015](https://github.com/danielbaldwin47/Quill/blob/main/docs/adr/0015-the-design-oracle.md),
//! `docs/design.md` § Text sizes). The page around a row — the measure, the
//! gutter, the band the caret's row is kept in — is still the Parity oracle's
//! `legacy/app/css/page.css`, in its own order so the two land on the same
//! integers.

use std::ops::RangeInclusive;

use crate::settings::Face;

/// The measure, in characters: iA's default line-length limit, `--measure:
/// 64ch` in `legacy/app/css/type.css`.
pub const MEASURE: u32 = 64;

/// The cell the Quill Faces are cut on, in ems.
///
/// A `ch` is one cell, and the three Faces share one: Duo widens `m` and `w`
/// inside it and Quattro narrows `i`, `l` and the word space, but the grid the
/// measure is counted on is the same 0.6 em. At 20 px that is the 12 px cell
/// `spike/gtk4-editor/RESULTS.txt` measured out of Pango for all three.
const CELL: f64 = 0.6;

/// The gutter each side of the measure, in cells.
///
/// Seven cells is what `###### ` needs to hang out of the measure and reach
/// the container's own edge, and it is what the Design oracle leaves there
/// ([ADR 0016](../../docs/adr/0016-the-text-container-is-78-cells.md)). It
/// does not scale with the window: a window too narrow for the container keeps
/// the seven cells and gives up measure instead, so a heading hangs and a
/// selection finds its edge at every size.
pub const GUTTER: u32 = 7;

/// The text container, in cells: the [`MEASURE`] plus a [`GUTTER`] each side.
pub const CONTAINER: u32 = MEASURE + 2 * GUTTER;

/// The air below the last row of text: `--page-bottom: 30vh`.
const PAGE_BOTTOM: f64 = 0.30;

/// How much of the view is kept above the caret's row, and how much below:
/// `scroll-padding: 10vh 0 28vh` in `legacy/app/css/page.css`.
///
/// The two are not equal because a writer reads up and writes down. The room
/// that matters is the room the next line will need, so the band sits high in
/// the view and the text drifts up the page instead of crawling along the
/// bottom edge.
const BAND_ABOVE: f64 = 0.10;
const BAND_BELOW: f64 = 0.28;

/// One rung of the Design oracle's text-size ladder.
///
/// The Text Size menu steps rather than names a value, so the three numbers
/// here were measured off the app at every size it reaches, on a scale-2
/// display: `ref/ia/mac-native/NOTES.md` § 11 is the table, and this is that
/// table.
#[derive(Clone, Copy, Debug)]
struct Rung {
    /// The em in logical pixels. macOS points are logical pixels, so this is
    /// the ladder's own pt value.
    em: f64,
    /// The line pitch, in device pixels at scale 2.
    pitch: u32,
    /// The caret's width, in device pixels at scale 2.
    caret_width: u32,
}

/// How many text sizes the Design oracle offers.
pub const STEPS: u32 = 14;

/// The ladder, step 0 to step 13.
///
/// Two things a formula would have got wrong are in these numbers. The
/// leading is *liquid*: `pitch / em` peaks at 1.732 on step 2 and falls from
/// there to 1.374 at the top of the ladder, so the bigger the type the tighter
/// the leading, proportionally — the linear clamp the Parity oracle fitted
/// through three marketing stills is not this curve at either end. The
/// wobble under the peak, 1.690 then 1.705, is whole-pixel quantisation on a
/// 29 px em rather than a shape (NOTES § 11 says so). And the caret's width
/// quantises to 5, 6, 8 and 10 device pixels and stops there, so it is not a
/// fraction of the em at all: `caret width / em` runs from 0.172 at the foot
/// of the ladder through 0.186 on step 2 to 0.080 at the top.
// A table is read down its columns, and rustfmt would give each rung five
// lines of its own.
#[rustfmt::skip]
const LADDER: [Rung; STEPS as usize] = [
    Rung { em: 14.50, pitch: 49, caret_width: 5 },
    Rung { em: 15.25, pitch: 52, caret_width: 5 },
    Rung { em: 16.17, pitch: 56, caret_width: 6 },
    Rung { em: 17.17, pitch: 59, caret_width: 6 },
    Rung { em: 19.25, pitch: 66, caret_width: 6 },
    Rung { em: 21.33, pitch: 73, caret_width: 6 },
    Rung { em: 25.58, pitch: 86, caret_width: 8 },
    Rung { em: 29.75, pitch: 98, caret_width: 8 },
    Rung { em: 33.92, pitch: 109, caret_width: 8 },
    Rung { em: 38.08, pitch: 120, caret_width: 10 },
    Rung { em: 44.25, pitch: 135, caret_width: 10 },
    Rung { em: 50.33, pitch: 149, caret_width: 10 },
    Rung { em: 56.50, pitch: 161, caret_width: 10 },
    Rung { em: 62.58, pitch: 172, caret_width: 10 },
];

/// The steps a writer may ask for: every rung of the ladder.
#[must_use]
pub fn steps() -> RangeInclusive<u32> {
    0..=(STEPS - 1)
}

/// The rung at `step`, or the top one when the ladder does not go that high.
///
/// Clamped rather than checked: [`crate::settings`] is where a step is held
/// to [`steps`], and a painter asking for type it can no longer reach should
/// draw the largest there is rather than stop drawing.
fn rung(step: u32) -> Rung {
    LADDER[step.min(STEPS - 1) as usize]
}

/// A ladder number, measured in device pixels at scale 2, in the device
/// pixels of a display at `scale`.
///
/// Never nothing: a rounded-away pitch would stack every row of a page on one
/// line, and a rounded-away caret would leave a writer with no caret at all.
fn device(at_scale_2: u32, scale: f64) -> u32 {
    ((f64::from(at_scale_2) * scale / 2.0).round().max(1.0)) as u32
}

/// The em at `step`, in logical pixels.
#[must_use]
pub fn em(step: u32) -> f64 {
    rung(step).em
}

/// The line pitch at `step` on a display of `scale`, in device pixels.
///
/// Ask it at scale 1 for logical pixels, which is what anything GTK lays the
/// page out from wants: GTK applies the surface's scale factor itself, and
/// only the caret is placed in device pixels.
#[must_use]
pub fn pitch(step: u32, scale: f64) -> u32 {
    device(rung(step).pitch, scale)
}

/// The caret's width at `step` on a display of `scale`, in device pixels —
/// logical pixels at scale 1, as [`pitch`] explains.
///
/// An odd bar's extra pixel falls right of the advance boundary the bar is
/// centred on; that is the painter's, and `docs/design.md` § Caret width says
/// so.
#[must_use]
pub fn caret_width(step: u32, scale: f64) -> u32 {
    device(rung(step).caret_width, scale)
}

/// The step an old `size` in logical pixels becomes.
///
/// The ladder replaced a free integer, so every `settings.toml` written before
/// it has a size that is not a rung. A size between two rungs takes the rung
/// **above** it, so that no writer's type is made smaller by an upgrade they
/// did not ask for; a size above the whole ladder takes the top rung. The old
/// default, 20 px, lands that way on step 5 — which is the ladder's own
/// default, and the size iA Writer opens at.
#[must_use]
pub fn step_for_size(size: u32) -> u32 {
    let size = f64::from(size);
    let step = LADDER.iter().position(|rung| rung.em >= size);
    step.unwrap_or(LADDER.len() - 1) as u32
}

/// How the air around a row of ink is divided between the three gaps GTK
/// draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Leading {
    /// `pixels-above-lines`: the air above a paragraph's first row.
    pub above: u32,
    /// `pixels-inside-wrap`: the air between the rows of one paragraph.
    pub inside_wrap: u32,
    /// `pixels-below-lines`: the air below a paragraph's last row.
    pub below: u32,
}

/// The three gaps that put every row of text one [`pitch`] below the last,
/// given the height `row` of one row of ink.
///
/// A CSS line box splits its leading half above the ink and half below. GTK
/// puts `pixels-above-lines` entirely above, and only above a *paragraph* —
/// prose wraps, and a wrapped row is not a paragraph, so `pixels-inside-wrap`
/// carries the whole of the air between the rows of one paragraph (ADR 0004).
/// Two sums are the whole of what the split has to get right, and the tests
/// hold both: `inside_wrap` alone separates two rows of one paragraph, and
/// `below` plus `above` alone separate two paragraphs. They are also why the
/// three do not sum to the air — each gap is drawn in one of those two places,
/// never in both.
#[must_use]
pub fn leading(pitch: u32, row: u32) -> Leading {
    // A row of ink taller than the pitch has no air to give; the type is then
    // as tight as the Face allows rather than overlapping.
    let air = pitch.saturating_sub(row);
    Leading {
        above: air / 2,
        inside_wrap: air,
        below: air - air / 2,
    }
}

/// One cell of `face` at `step`, in logical pixels.
///
/// The cell follows the em, and the em is the ladder's: VERDICTS 4.1.7 read
/// 0.6 em per cell off the app itself, which is the grid the Quill Faces are
/// already cut on. A Face is asked for rather than assumed, so that a Face cut
/// to another grid has somewhere to say so; the three shipped today share
/// [`CELL`].
#[must_use]
pub fn cell(face: Face, step: u32) -> f64 {
    let per_em = match face {
        Face::Duo | Face::Quattro | Face::Mono => CELL,
    };
    per_em * em(step)
}

/// The measure at `step` in `face`, in logical pixels: [`MEASURE`] cells of
/// it.
#[must_use]
pub fn measure(face: Face, step: u32) -> u32 {
    (cell(face, step) * f64::from(MEASURE)).round() as u32
}

/// The text container, and where the measure sits inside it.
///
/// The container is the whole of what the writing surface owns: the measure
/// and the gutter each side of it. A heading hangs into the left gutter and a
/// selection's rows fill the container edge to edge, so every painter that has
/// to agree with either asks here rather than measuring ink.
///
/// Every edge is in pixels from the left of the view, and all of them are
/// counted off the same two rounded lengths — the container and one gutter —
/// rather than each being rounded off the cell on its own, so that two edges
/// meant to agree cannot land a pixel apart. It is the rule
/// `quill::tags`' own hanging keeps for the same reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Column {
    /// The container's left edge.
    pub left: u32,
    /// The container's right edge.
    pub right: u32,
    /// The measure's left edge: [`left`](Self::left) plus one gutter. The app
    /// sets it as both `left-margin` and `right-margin`, which centres the
    /// measure to within whatever odd pixel the view's width leaves over.
    pub side: u32,
    /// What is left for the text, in pixels: the container less both gutters.
    pub width: u32,
}

impl Column {
    /// The gutter each side of the measure, in pixels: [`GUTTER`] cells.
    #[must_use]
    pub fn gutter(&self) -> u32 {
        self.side - self.left
    }

    /// How far a heading of `level` — one to six — hangs left of the measure,
    /// in pixels: its marker run, `level + 1` cells with the space after the
    /// last `#`.
    ///
    /// `###### ` is seven cells, so the deepest heading hangs the whole gutter
    /// and its first `#` lands on the container's left edge; every shallower
    /// one starts further in, and all six `#` columns line up on the right
    /// against the measure. This is the Design oracle's rule (`14-gutters`),
    /// and it is why the gutter is seven cells wide.
    ///
    /// Counted off [`gutter`](Self::gutter) rather than off the cell a second
    /// time, so that `hang(6)` reaches the container's edge exactly however
    /// the cell rounded. The app keeps its own copy of the `level + 1` rule in
    /// `quill::tags::marker_cells` until #167 hangs it off this container
    /// instead.
    #[must_use]
    pub fn hang(&self, level: u8) -> u32 {
        debug_assert!((1..=6).contains(&level), "a heading is level 1 to 6");
        (f64::from(self.gutter()) * (f64::from(level) + 1.0) / f64::from(GUTTER)).round() as u32
    }
}

/// Where the container sits in a view `view` pixels wide, on a `cell` this
/// many pixels across.
///
/// [`CONTAINER`] cells centred while they fit. A window narrower than that is
/// the container: the gutters hold at [`GUTTER`] cells each and the measure
/// takes what they leave, so the text never reaches an edge and a heading
/// never hangs off the window (ADR 0016). Where the window cannot even seat
/// the two gutters the measure is nothing rather than negative, and the
/// caller has a column it can still lay out.
#[must_use]
pub fn column(view: u32, cell: f64) -> Column {
    let view = f64::from(view);
    // The three lengths every edge below is counted off, rounded here and only
    // here; each edge is then whole-pixel arithmetic on them.
    let gutter = (cell * f64::from(GUTTER)).round().max(0.0);
    let container = (cell * f64::from(CONTAINER)).clamp(0.0, view).round();
    let left = ((view - container) / 2.0).round();
    Column {
        left: left as u32,
        right: (left + container) as u32,
        side: (left + gutter) as u32,
        width: (container - 2.0 * gutter).max(0.0) as u32,
    }
}

/// The air above the first row of text: two pitches (`--page-top`).
///
/// The oracle measured the iA window's first ink 85 logical pixels below the
/// top of its editor area, and settled on two pitches as the rule that lands
/// nearest it at every size.
#[must_use]
pub fn page_top(pitch: u32) -> u32 {
    2 * pitch
}

/// The air below the last row of text in a view `view` pixels tall: 30 % of it
/// (`--page-bottom`), so that the end of a draft stops well clear of the
/// bottom edge rather than against it.
#[must_use]
pub fn page_bottom(view: u32) -> u32 {
    (PAGE_BOTTOM * f64::from(view)).round() as u32
}

/// Where the view has to go to keep the caret's row inside the band, or `None`
/// when the row is already in it and nothing should move.
///
/// The band is the viewport less [`BAND_ABOVE`] at the top and [`BAND_BELOW`]
/// at the foot. A row above the band is put at the top of it and a row below
/// it at the foot, which is the oracle's `scroll-padding` under a
/// `scrollIntoView` of `block: nearest`: the shorter of the two moves, so that
/// a caret leaving the band by a line does not jump the page.
///
/// Everything is in the one coordinate the scroll is counted in — `scroll` is
/// the top of the viewport, `row_top` the top of the row's band of one pitch —
/// and the answer is in it too. It is not clamped to the document: a row near
/// either end asks for a place the view cannot go, and the caller has the
/// adjustment that knows where the ends are.
///
/// A row taller than the band takes the top rule, because a row whose start is
/// off the screen cannot be read at all.
#[must_use]
pub fn band_target(row_top: f64, row_height: f64, scroll: f64, viewport: f64) -> Option<f64> {
    if viewport <= 0.0 {
        return None;
    }
    let head_room = viewport * BAND_ABOVE;
    let foot = viewport * (1.0 - BAND_BELOW);
    if row_top < scroll + head_room {
        Some(row_top - head_room)
    } else if row_top + row_height > scroll + foot {
        Some(row_top + row_height - foot)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Choice, default_step, type_steps};

    /// `ref/ia/mac-native/NOTES.md` § 11, written out again: step, em in
    /// logical pixels, and pitch and caret width in device pixels at scale 2.
    ///
    /// A second copy on purpose. The ladder is measurement rather than
    /// arithmetic, so there is nothing to re-derive it from; what a test can
    /// hold is that the table in the code is still the table in the notes, and
    /// it can only do that by having read the notes itself.
    const NOTES: [(u32, f64, u32, u32); STEPS as usize] = [
        (0, 14.50, 49, 5),
        (1, 15.25, 52, 5),
        (2, 16.17, 56, 6),
        (3, 17.17, 59, 6),
        (4, 19.25, 66, 6),
        (5, 21.33, 73, 6),
        (6, 25.58, 86, 8),
        (7, 29.75, 98, 8),
        (8, 33.92, 109, 8),
        (9, 38.08, 120, 10),
        (10, 44.25, 135, 10),
        (11, 50.33, 149, 10),
        (12, 56.50, 161, 10),
        (13, 62.58, 172, 10),
    ];

    /// A pitch at the judged size, so that the band's rows are the rows a
    /// judged shot has.
    const ROW: f64 = 36.0;

    /// The view is a thousand pixels down every time, so that a band that
    /// answered in the viewport's own coordinates rather than the scroll's
    /// would be off by exactly that and could not pass by accident.
    const SCROLL: f64 = 1000.0;

    fn moves_to(row_top: f64, viewport: f64, want: f64, what: &str) {
        let Some(got) = band_target(row_top, ROW, SCROLL, viewport) else {
            panic!("{what}: the band asked for no move at all");
        };
        assert!(
            (got - want).abs() < 1e-9,
            "{what}: the band put the view at {got}, not {want}"
        );
    }

    #[test]
    fn every_step_is_the_em_the_pitch_and_the_width_the_notes_measured() {
        assert_eq!(
            type_steps(),
            steps(),
            "the steps a writer may ask for are not the ladder's own"
        );
        for (step, em_px, pitch_px, width_px) in NOTES {
            assert!(
                (em(step) - em_px).abs() < 0.005,
                "step {step}'s em is {} logical px, not NOTES § 11's {em_px}",
                em(step)
            );
            assert_eq!(
                pitch(step, 2.0),
                pitch_px,
                "step {step}'s pitch at scale 2 is not NOTES § 11's"
            );
            assert_eq!(
                caret_width(step, 2.0),
                width_px,
                "step {step}'s caret is not NOTES § 11's width at scale 2"
            );
        }
    }

    #[test]
    fn a_display_at_another_scale_gets_the_ladder_scaled_and_never_nothing() {
        // The ladder was measured at scale 2, so scale 1 is half of it and
        // scale 3 half again as much, each rounded once.
        assert_eq!(pitch(5, 1.0), 37, "step 5 is 73 device px at scale 2");
        assert_eq!(pitch(5, 3.0), 110, "and 109.5 at scale 3, rounded up");
        assert_eq!(caret_width(0, 1.0), 3, "5 device px at scale 2 is 2.5");
        assert_eq!(caret_width(13, 4.0), 20, "and 10 at scale 2 is 20 at 4");
        assert_eq!(
            caret_width(0, 0.25),
            1,
            "a caret rounded away is a writer with no caret"
        );
        assert_eq!(pitch(0, 0.01), 1, "and a pitch rounded away is one row");
    }

    #[test]
    fn the_leading_is_liquid_rather_than_a_constant_multiple_of_the_em() {
        // What the linear clamp could not do: the bigger the type, the
        // tighter the leading, proportionally. `docs/design.md` § Line pitch
        // reads the curve off the ends of the ladder.
        let ratio = |step| f64::from(pitch(step, 2.0)) / (2.0 * em(step));
        assert!((ratio(2) - 1.732).abs() < 0.001, "{}", ratio(2));
        assert!((ratio(5) - 1.711).abs() < 0.001, "{}", ratio(5));
        assert!((ratio(13) - 1.374).abs() < 0.001, "{}", ratio(13));
        // From step 2, and not from step 0: the ratio *rises* over steps 0 to
        // 2 — 1.690, 1.705, 1.732 — which NOTES § 11 reads as whole-pixel
        // quantisation on a 29 px em rather than as part of the curve.
        assert!(
            ratio(0) < ratio(2) && ratio(1) < ratio(2),
            "step 2 is the peak"
        );
        for step in 2..STEPS - 1 {
            assert!(
                ratio(step) > ratio(step + 1),
                "the leading stopped tightening between steps {step} and {}",
                step + 1
            );
        }
    }

    #[test]
    fn a_step_past_the_top_of_the_ladder_draws_the_largest_type_there_is() {
        assert!((em(STEPS) - em(STEPS - 1)).abs() < f64::EPSILON);
        assert_eq!(pitch(99, 2.0), pitch(STEPS - 1, 2.0));
    }

    #[test]
    fn an_old_size_in_pixels_takes_the_rung_above_it_and_never_shrinks() {
        assert_eq!(step_for_size(10), 0, "10 px is below the whole ladder");
        assert_eq!(step_for_size(20), 5, "the old default is the ladder's");
        assert_eq!(step_for_size(40), 10, "40 px is between steps 9 and 10");
        assert_eq!(step_for_size(21), 5, "a rung's own size is that rung");
        assert_eq!(step_for_size(1000), STEPS - 1, "and nothing goes higher");
        for (step, em_px, _, _) in NOTES {
            assert_eq!(
                step_for_size(em_px.floor() as u32),
                step,
                "the last whole pixel step {step}'s em covers did not take it"
            );
        }
    }

    #[test]
    fn a_wrapped_row_and_a_new_paragraph_both_sit_one_pitch_below_the_last_row() {
        for step in type_steps() {
            let pitch = pitch(step, 2.0);
            // Every row of ink a Face could give at this step, since the split
            // has to hold whatever Pango measures: at 20 px the spike measured
            // a 36 px pitch over a row of 26 (`spike/gtk4-editor/RESULTS.txt`,
            // pitch 36 above 10 pixels of air), and a Face cut taller or
            // shorter than that is still a Face.
            for row in 1..=pitch {
                let air = pitch - row;
                let leading = leading(pitch, row);
                assert_eq!(
                    leading.inside_wrap, air,
                    "at step {step} over a {row} px row, a wrapped row is not one pitch \
                     below the row above it"
                );
                assert_eq!(
                    leading.below + leading.above,
                    air,
                    "at step {step} over a {row} px row, a paragraph does not start one \
                     pitch below the one before it"
                );
            }
        }
    }

    #[test]
    fn a_row_of_ink_taller_than_the_pitch_is_set_tight_rather_than_overlapping() {
        assert_eq!(
            leading(20, 30),
            Leading {
                above: 0,
                inside_wrap: 0,
                below: 0
            },
            "there is no air to give, and none may be taken"
        );
    }

    #[test]
    fn every_face_is_measured_on_the_same_cell_and_it_follows_the_ladders_em() {
        for name in Face::VALUES {
            let face = Face::parse(name).expect("every Face `settings.toml` writes is a Face");
            for step in steps() {
                assert!(
                    (cell(face, step) - 0.6 * em(step)).abs() < f64::EPSILON,
                    "{name} at step {step} is not on the 0.6 em cell VERDICTS 4.1.7 read"
                );
            }
            assert!(
                (cell(face, default_step()) - 12.798).abs() < 0.001,
                "{name} at the default step is not 0.6 of its 21.33 px em"
            );
            assert_eq!(
                measure(face, default_step()),
                819,
                "{name}'s 64-character measure at the default step is not 819 px"
            );
        }
    }

    #[test]
    fn the_container_is_centred_while_it_fits() {
        // Both judged widths, at the ladder's default step. The container is
        // 78 cells of 12.798, which is 998 px: the 1440 px window seats it
        // and the 960 px `narrow` state no longer does, so `narrow` is the
        // container itself with its gutters held and the measure giving up
        // the difference.
        let cell = cell(Face::Duo, default_step());
        assert_eq!(
            column(1440, cell),
            Column {
                left: 221,
                right: 1219,
                side: 311,
                width: 818
            },
            "a judged 1440 px window does not centre the 78-cell container with the measure a gutter inside it"
        );
        assert_eq!(
            column(960, cell),
            Column {
                left: 0,
                right: 960,
                side: 90,
                width: 780
            },
            "the `narrow` judged state is narrower than the container at the default step"
        );
    }

    #[test]
    fn a_window_too_narrow_for_the_container_holds_its_gutters_and_shrinks_the_measure() {
        assert_eq!(
            column(600, cell(Face::Duo, default_step())),
            Column {
                left: 0,
                right: 600,
                side: 90,
                width: 420
            },
            "a window narrower than 78 cells is not the container itself, with its gutters held at 7 cells and the measure giving up the difference"
        );
    }

    #[test]
    fn the_deepest_heading_hangs_to_the_container_edge() {
        let column = column(1440, cell(Face::Duo, default_step()));
        assert_eq!(
            column.hang(6),
            column.gutter(),
            "`###### ` does not hang the whole gutter, out to the container's left edge"
        );
        assert_eq!(
            column.hang(1),
            26,
            "`# ` does not hang its two cells of the default step's 21.33 px em"
        );
    }

    #[test]
    fn the_two_gutters_stay_equal_where_the_cell_is_fractional() {
        // Step 4 is a 19.25 px em and so an 11.55 px cell, which is what
        // every length here rounds off. Counting them off one rounded gutter
        // is what keeps the measure the same air on each side, and keeps the
        // deepest heading on the container's edge. Every rung of the ladder
        // is fractional this way — that is what a ladder measured off an app
        // gives, where a range of whole pixels did not.
        let column = column(1440, cell(Face::Duo, 4));
        assert_eq!(
            column.right - column.side - column.width,
            column.gutter(),
            "the gutter right of the measure is not the one left of it"
        );
        assert_eq!(
            column.hang(6),
            column.gutter(),
            "`###### ` misses the container's edge once the cell rounds"
        );
    }

    #[test]
    fn the_page_starts_two_pitches_down_and_ends_well_clear_of_the_bottom() {
        assert_eq!(
            page_top(pitch(default_step(), 1.0)),
            74,
            "the first row at the default step does not start two pitches down"
        );
        assert_eq!(
            page_bottom(900),
            270,
            "a judged 900 px window leaves 30 % of itself below the last row"
        );
    }

    #[test]
    fn a_row_already_inside_the_band_is_left_where_it_is_at_every_viewport() {
        for (viewport, row_top) in [(900.0, 1200.0), (600.0, 1200.0), (1200.0, 1300.0)] {
            assert!(
                band_target(row_top, ROW, SCROLL, viewport).is_none(),
                "a row {row_top} in a {viewport} px view is inside the band and asked for a move"
            );
        }
    }

    #[test]
    fn a_row_above_the_band_comes_down_to_a_tenth_of_the_view_and_no_further() {
        moves_to(1000.0, 900.0, 910.0, "a 900 px view");
        moves_to(1000.0, 600.0, 940.0, "a 600 px view");
        moves_to(1000.0, 1200.0, 880.0, "a 1200 px view");
    }

    #[test]
    fn a_row_below_the_band_comes_up_to_seventy_two_per_cent_of_the_view() {
        moves_to(1700.0, 900.0, 1088.0, "a 900 px view");
        moves_to(1500.0, 600.0, 1104.0, "a 600 px view");
        moves_to(1900.0, 1200.0, 1072.0, "a 1200 px view");
    }

    #[test]
    fn the_band_keeps_ten_per_cent_above_the_row_and_twenty_eight_below_it() {
        // The row at the very top of the view, and the row whose foot is at
        // the very bottom of it: the two moves are the two numbers of
        // `scroll-padding: 10vh 0 28vh`, which is the whole of the rule.
        moves_to(SCROLL, 900.0, SCROLL - 90.0, "a row at the top of the view");
        moves_to(
            SCROLL + 900.0 - ROW,
            900.0,
            SCROLL + 252.0,
            "a row at the foot of the view",
        );
    }

    #[test]
    fn the_bands_edges_belong_to_the_band() {
        assert!(
            band_target(SCROLL + 90.0, ROW, SCROLL, 900.0).is_none(),
            "a row starting exactly on the band's top edge was moved"
        );
        assert!(
            band_target(SCROLL + 648.0 - ROW, ROW, SCROLL, 900.0).is_none(),
            "a row ending exactly on the band's bottom edge was moved"
        );
        assert!(
            band_target(SCROLL + 89.0, ROW, SCROLL, 900.0).is_some(),
            "a row one pixel above the band's top edge was left there"
        );
    }

    #[test]
    fn a_view_with_no_height_yet_has_no_band_to_keep_anything_in() {
        assert!(
            band_target(1700.0, ROW, SCROLL, 0.0).is_none(),
            "a view of no height asked for a scroll"
        );
    }
}
