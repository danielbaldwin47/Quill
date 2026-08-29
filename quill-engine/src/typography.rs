//! Typography: the pitch, the measure, the margins a page is laid out from and
//! the band the caret's row is kept in.
//!
//! The numbers, and none of the widget that reads them. The Parity oracle
//! keeps them in `legacy/app/css/type.css` and `legacy/app/css/page.css` as
//! custom properties; here they are functions of the type size and of the view
//! the text is set in, computed in the oracle's own order so that the two land
//! on the same integers. Everything that has to agree with a row of text — the
//! leading, the top of the page, later the caret's height and the Typewriter
//! anchor — asks this module rather than doing the arithmetic again.

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

/// The line pitch at `size`, in whole pixels: iA's liquid leading.
///
/// A proportional part plus a near-constant slab of air, `1.30 × size +
/// 10.4`, held between 1.52 and 2 times the size, and only then rounded —
/// `--line-air` and `--line-pitch` in `legacy/app/css/type.css`, in that
/// order, because rounding a clamp is not the number clamping a round gives.
/// The clamp is what keeps small type readable: at 14 px and below the liquid
/// leading asks for more air than the type is tall, and the pitch stops at
/// twice the size instead.
#[must_use]
pub fn pitch(size: u32) -> u32 {
    let size = f64::from(size);
    let air = (1.30 * size + 10.4).clamp(1.52 * size, 2.0 * size);
    air.round() as u32
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

/// One cell of `face` at `size`, in pixels.
///
/// A Face is asked for rather than assumed, so that a Face cut to another grid
/// has somewhere to say so; the three shipped today share [`CELL`].
#[must_use]
pub fn cell(face: Face, size: u32) -> f64 {
    let em = match face {
        Face::Duo | Face::Quattro | Face::Mono => CELL,
    };
    em * f64::from(size)
}

/// The measure at `size` in `face`, in pixels: [`MEASURE`] cells of it.
#[must_use]
pub fn measure(face: Face, size: u32) -> u32 {
    (cell(face, size) * f64::from(MEASURE)).round() as u32
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
    use crate::settings::{Choice, type_sizes};

    /// The oracle's `--line-air` and `--line-pitch`, written out as CSS writes
    /// them, so that this disagrees with the code the moment the code stops
    /// being the oracle's arithmetic in the oracle's order.
    fn oracle_pitch(size: u32) -> u32 {
        let size = f64::from(size);
        let air = f64::max(size * 1.52, f64::min(size * 1.30 + 10.4, size * 2.0));
        air.round() as u32
    }

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
    fn the_pitch_is_the_oracles_own_arithmetic_at_every_size_a_writer_can_reach() {
        for size in type_sizes() {
            assert_eq!(
                pitch(size),
                oracle_pitch(size),
                "the pitch at {size} px is not `legacy/app/css/type.css`'s"
            );
        }
    }

    #[test]
    fn the_pitch_at_twenty_pixels_is_the_thirty_six_the_spike_measured() {
        assert_eq!(pitch(20), 36, "spike/gtk4-editor/RESULTS.txt measured 36");
    }

    #[test]
    fn small_type_stops_at_twice_its_size_and_larger_type_is_left_alone() {
        assert_eq!(pitch(10), 20, "10 px is clamped to twice its size");
        assert_eq!(pitch(14), 28, "14 px is the last size the clamp holds");
        assert_eq!(pitch(16), 31, "16 px is the liquid leading itself");
        assert_eq!(pitch(40), 62, "40 px is the liquid leading itself");
    }

    #[test]
    fn a_wrapped_row_and_a_new_paragraph_both_sit_one_pitch_below_the_last_row() {
        for size in type_sizes() {
            let pitch = pitch(size);
            // Every row of ink a Face could give at this size, since the split
            // has to hold whatever Pango measures: at 20 px the spike measured
            // a 36 px pitch over a row of 26 (`spike/gtk4-editor/RESULTS.txt`,
            // pitch 36 above 10 pixels of air), and a Face cut taller or
            // shorter than that is still a Face.
            for row in 1..=pitch {
                let air = pitch - row;
                let leading = leading(pitch, row);
                assert_eq!(
                    leading.inside_wrap, air,
                    "at {size} px over a {row} px row, a wrapped row is not one pitch \
                     below the row above it"
                );
                assert_eq!(
                    leading.below + leading.above,
                    air,
                    "at {size} px over a {row} px row, a paragraph does not start one \
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
    fn every_face_is_measured_on_the_same_twelve_pixel_cell_at_twenty_pixels() {
        for name in Face::VALUES {
            let face = Face::parse(name).expect("every Face `settings.toml` writes is a Face");
            assert!(
                (cell(face, 20) - 12.0).abs() < f64::EPSILON,
                "{name} at 20 px is not on the 12 px cell the spike measured"
            );
            assert_eq!(
                measure(face, 20),
                768,
                "{name} at 20 px is not the 768 px column `legacy/app/css/page.css` measured"
            );
        }
    }

    #[test]
    fn the_measure_is_centred_while_it_fits() {
        let measure = measure(Face::Duo, 20);
        assert_eq!(
            column(1440, measure),
            Column {
                side: 336,
                width: 768
            },
            "a judged 1440 px window does not centre the 64-character measure"
        );
        assert_eq!(
            column(960, measure),
            Column {
                side: 96,
                width: 768
            },
            "the `narrow` judged state is still wide enough for the whole measure"
        );
    }

    #[test]
    fn a_window_too_narrow_for_the_measure_keeps_its_gutter_instead() {
        let measure = measure(Face::Duo, 20);
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
            page_top(pitch(20)),
            72,
            "the first row at 20 px does not start two pitches down"
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
