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

/// The gutter the measure is never allowed to cross: `--page-gutter:
/// clamp(24px, 6vw, 96px)` in `legacy/app/css/page.css`.
const GUTTER_SHARE: f64 = 0.06;
const GUTTER_SMALLEST: f64 = 24.0;
const GUTTER_LARGEST: f64 = 96.0;

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
/// leading is *liquid*: `pitch / em` falls from 1.732 to 1.374 up the ladder,
/// so the bigger the type the tighter the leading, proportionally — the
/// linear clamp the Parity oracle fitted through three marketing stills is
/// not this curve at either end. And the caret's width quantises to 5, 6, 8
/// and 10 device pixels and stops there, so it is not a fraction of the em:
/// `caret width / em` falls from 0.172 to 0.080 across the range.
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
#[must_use]
pub fn pitch(step: u32, scale: f64) -> u32 {
    device(rung(step).pitch, scale)
}

/// The caret's width at `step` on a display of `scale`, in device pixels.
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

/// The column of text, and the margin on each side of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Column {
    /// The margin either side, in pixels: `left-margin` and `right-margin`.
    pub side: u32,
    /// What is left for the text, in pixels.
    pub width: u32,
}

/// Where a `measure`-wide column sits in a view `view` pixels wide.
///
/// Centred while it fits and narrowed by the gutter when it does not: `width:
/// min(var(--measure), calc(100% - 2 * var(--page-gutter)))` with `margin: 0
/// auto` (`legacy/app/css/page.css`). A window too narrow for 64 characters
/// gives the text the room it has, and the text never reaches an edge.
#[must_use]
pub fn column(view: u32, measure: u32) -> Column {
    let view = f64::from(view);
    let gutter = (GUTTER_SHARE * view).clamp(GUTTER_SMALLEST, GUTTER_LARGEST);
    let width = f64::from(measure).min((view - 2.0 * gutter).max(0.0));
    Column {
        side: ((view - width) / 2.0).round() as u32,
        width: width.round() as u32,
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
    use crate::settings::{Choice, default_size, type_sizes};

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
            type_sizes(),
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
        for step in type_sizes() {
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
                (cell(face, default_size()) - 12.798).abs() < 0.001,
                "{name} at the default step is not 0.6 of its 21.33 px em"
            );
            assert_eq!(
                measure(face, default_size()),
                819,
                "{name}'s 64-character measure at the default step is not 819 px"
            );
        }
    }

    #[test]
    fn the_measure_is_centred_while_it_fits() {
        let measure = measure(Face::Duo, default_size());
        assert_eq!(
            column(1440, measure),
            Column {
                side: 311,
                width: 819
            },
            "a judged 1440 px window does not centre the 64-character measure"
        );
        assert_eq!(
            column(960, measure),
            Column {
                side: 71,
                width: 819
            },
            "the `narrow` judged state is still wide enough for the whole measure"
        );
    }

    #[test]
    fn a_window_too_narrow_for_the_measure_keeps_its_gutter_instead() {
        let measure = measure(Face::Duo, default_size());
        assert_eq!(
            column(600, measure),
            Column {
                side: 36,
                width: 528
            },
            "at 600 px the gutter is 6 % of the view and the column takes what is left"
        );
        let narrow = column(200, measure);
        assert_eq!(
            narrow.side, 24,
            "the gutter never falls below 24 px, however narrow the window"
        );
        assert_eq!(narrow.width, 152, "and the text takes the rest of it");
    }

    #[test]
    fn the_page_starts_two_pitches_down_and_ends_well_clear_of_the_bottom() {
        assert_eq!(
            page_top(pitch(default_size(), 1.0)),
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
