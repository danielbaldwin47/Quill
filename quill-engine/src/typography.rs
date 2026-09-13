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
//! written out in [`WIDE`], [`MIDDLE`] and [`NARROWEST`] — one ladder per
//! [`SizeClass`], because the window's own width picks the class and neither
//! narrow ladder is the wide one scaled: a step names an em, a line pitch and
//! a caret width that were read together, so nothing here fits a curve
//! through them
//! ([ADR 0015](https://github.com/danielbaldwin47/Quill/blob/main/docs/adr/0015-the-design-oracle.md),
//! `docs/design.md` § Text sizes). The page around a row — the measure, the
//! gutter, the band the caret's row is kept in — is still the Parity oracle's
//! `legacy/app/css/page.css`, in its own order so the two land on the same
//! integers.

use std::ops::RangeInclusive;

use crate::settings::{Face, default_step};

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

/// The air above the first row of text, in device pixels at scale 2.
///
/// A constant rather than a multiple of the pitch, and the derivation is
/// [`page_top`].
const PAGE_TOP: u32 = 60;

/// The air below the last row of text, as a share of the **text area**: the
/// derivation is [`page_bottom`].
const PAGE_BOTTOM: f64 = 460.0 / 857.0;

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

/// The wide class's ladder, step 0 to step 13.
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
const WIDE: [Rung; STEPS as usize] = [
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

/// The middle class's ladder, step 0 to step 13.
///
/// `ref/ia/mac-native/NOTES.md` § State 25, walked at 960 pt one Text Size
/// click at a time. **It is not the wide ladder scaled.** The cell against
/// [`WIDE`]'s at the same step wanders 0.9195 … 0.8888 with a dip to 0.8211
/// at step 6, turning several times, so no rung here is derivable from the
/// one above it and the table is carried whole.
///
/// NOTES measures a **cell** rather than an em, so each `em` below is that
/// cell divided by [`CELL`] — the relation [`cell`] reads the other way, kept
/// one way round so that one number per rung is measurement and the rest is
/// arithmetic on it. The caret's width is [`WIDE`]'s at the same step: #419
/// walked the cell and the pitch and did not read a caret in either narrow
/// class, so the bar keeps the width it has until one is measured.
#[rustfmt::skip]
const MIDDLE: [Rung; STEPS as usize] = [
    Rung { em: 13.3333, pitch: 43, caret_width: 5 },
    Rung { em: 14.2600, pitch: 46, caret_width: 5 },
    Rung { em: 15.0975, pitch: 49, caret_width: 6 },
    Rung { em: 15.9308, pitch: 53, caret_width: 6 },
    Rung { em: 16.8875, pitch: 56, caret_width: 6 },
    Rung { em: 18.8775, pitch: 63, caret_width: 6 },
    Rung { em: 21.0075, pitch: 69, caret_width: 8 },
    Rung { em: 25.1958, pitch: 81, caret_width: 8 },
    Rung { em: 29.3458, pitch: 92, caret_width: 8 },
    Rung { em: 33.4000, pitch: 103, caret_width: 10 },
    Rung { em: 37.5000, pitch: 113, caret_width: 10 },
    Rung { em: 43.5250, pitch: 126, caret_width: 10 },
    Rung { em: 49.5833, pitch: 139, caret_width: 10 },
    Rung { em: 55.6250, pitch: 150, caret_width: 10 },
];

/// The narrowest class's ladder, step 0 to step 13.
///
/// NOTES § State 25 again, walked at 400 pt. Its scale against [`WIDE`]
/// wanders 0.7829 … 0.7790 with a dip to 0.7003 at step 7, so it is carried
/// whole for the reason [`MIDDLE`] is, and its `em` and `caret_width` are
/// written the way [`MIDDLE`]'s are.
#[rustfmt::skip]
const NARROWEST: [Rung; STEPS as usize] = [
    Rung { em: 11.3525, pitch: 37, caret_width: 5 },
    Rung { em: 12.4233, pitch: 40, caret_width: 5 },
    Rung { em: 13.4000, pitch: 44, caret_width: 6 },
    Rung { em: 14.4725, pitch: 47, caret_width: 6 },
    Rung { em: 15.5275, pitch: 50, caret_width: 6 },
    Rung { em: 16.5525, pitch: 53, caret_width: 6 },
    Rung { em: 18.7500, pitch: 59, caret_width: 8 },
    Rung { em: 20.8333, pitch: 65, caret_width: 8 },
    Rung { em: 24.8083, pitch: 76, caret_width: 8 },
    Rung { em: 28.8992, pitch: 86, caret_width: 10 },
    Rung { em: 32.9167, pitch: 96, caret_width: 10 },
    Rung { em: 36.8750, pitch: 105, caret_width: 10 },
    Rung { em: 42.9167, pitch: 118, caret_width: 10 },
    Rung { em: 48.7500, pitch: 129, caret_width: 10 },
];

/// Which of the three ladders a window is laid out on.
///
/// The Design oracle shrinks its type as the window narrows, and it does so
/// on the window's **own width** and nothing else: the break is in the same
/// two places at every text size and at every line-length limit, and at
/// 1200 pt a full container of the wide type fits inside the window with 400
/// px to spare and the type shrinks anyway, so container overflow is not the
/// trigger (`ref/ia/mac-native/NOTES.md` § State 22, `docs/design.md` row
/// Window limitation).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SizeClass {
    /// A window of [`NARROWEST_MAX`] logical pixels or less: [`NARROWEST`].
    Narrowest,
    /// A window between [`NARROWEST_MAX`] and [`MIDDLE_MAX`]: [`MIDDLE`].
    Middle,
    /// A window wider than [`MIDDLE_MAX`]: [`WIDE`], the ladder every judged
    /// state but `page/narrow` is shot at, and the class a window with no
    /// size yet is laid out in.
    #[default]
    Wide,
}

impl SizeClass {
    /// Every class there is, narrowest first: what a caller that has to hold
    /// for all three walks, so that a fourth class is added in one place.
    pub const ALL: [Self; 3] = [Self::Narrowest, Self::Middle, Self::Wide];
}

/// The widest window in the narrowest class, in logical pixels.
///
/// The two breaks are exact and have no hysteresis: approached from either
/// side, 440 gives the smaller type and 441 the larger, every time. They are
/// recorded in the points the window is set in; whether they are points or
/// device pixels wants a display at backing scale 1, which the capture rig
/// has not got (NOTES § State 25 § What the run could not settle).
const NARROWEST_MAX: u32 = 440;

/// The widest window in the middle class, in logical pixels.
const MIDDLE_MAX: u32 = 1250;

/// The size class a window `width` logical pixels across is laid out in.
///
/// Pure, and a step function of the width alone: nothing here remembers which
/// way the window was dragged.
#[must_use]
pub fn size_class(width: u32) -> SizeClass {
    if width <= NARROWEST_MAX {
        SizeClass::Narrowest
    } else if width <= MIDDLE_MAX {
        SizeClass::Middle
    } else {
        SizeClass::Wide
    }
}

/// The steps a writer may ask for: every rung of the ladder.
#[must_use]
pub fn steps() -> RangeInclusive<u32> {
    0..=(STEPS - 1)
}

/// A size of type: which of the three ladders, and which rung of it.
///
/// One value because everything a step names — the em, the pitch, the caret's
/// width, the cell — is named by the pair and not by the step alone, so the
/// two travel together or not at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Size {
    class: SizeClass,
    step: u32,
}

impl Size {
    /// `step` of `class`'s ladder, or its top rung when the ladder does not go
    /// that high.
    ///
    /// Clamped rather than checked: [`crate::settings`] is where a step is
    /// held to [`steps`], and a painter asking for type it can no longer reach
    /// should draw the largest there is rather than stop drawing.
    #[must_use]
    pub fn new(class: SizeClass, step: u32) -> Self {
        Self {
            class,
            step: step.min(STEPS - 1),
        }
    }

    /// The ladder this size is on.
    #[must_use]
    pub fn class(self) -> SizeClass {
        self.class
    }

    /// The rung of it, always one of [`steps`].
    #[must_use]
    pub fn step(self) -> u32 {
        self.step
    }
}

impl Default for Size {
    /// The default step of the class a window with no size yet is laid out in.
    fn default() -> Self {
        Self::new(SizeClass::default(), default_step())
    }
}

/// The rung `size` names.
fn rung(size: Size) -> Rung {
    let ladder = match size.class {
        SizeClass::Narrowest => &NARROWEST,
        SizeClass::Middle => &MIDDLE,
        SizeClass::Wide => &WIDE,
    };
    ladder[size.step as usize]
}

/// A ladder number, measured in device pixels at scale 2, in the device
/// pixels of a display at `scale`.
///
/// Never nothing: a rounded-away pitch would stack every row of a page on one
/// line, and a rounded-away caret would leave a writer with no caret at all.
fn device(at_scale_2: u32, scale: f64) -> u32 {
    ((f64::from(at_scale_2) * scale / 2.0).round().max(1.0)) as u32
}

/// The em at `size`, in logical pixels.
#[must_use]
pub fn em(size: Size) -> f64 {
    rung(size).em
}

/// The line pitch at `size` on a display of `scale`, in device pixels.
///
/// Ask it at scale 1 for logical pixels, which is what anything GTK lays the
/// page out from wants: GTK applies the surface's scale factor itself, and
/// only the caret is placed in device pixels.
#[must_use]
pub fn pitch(size: Size, scale: f64) -> u32 {
    device(rung(size).pitch, scale)
}

/// The caret's width at `size` on a display of `scale`, in device pixels —
/// logical pixels at scale 1, as [`pitch`] explains.
///
/// An odd bar's extra pixel falls right of the advance boundary the bar is
/// centred on; that is the painter's, and `docs/design.md` § Caret width says
/// so.
#[must_use]
pub fn caret_width(size: Size, scale: f64) -> u32 {
    device(rung(size).caret_width, scale)
}

/// The step an old `size` in logical pixels becomes.
///
/// The ladder replaced a free integer, so every `settings.toml` written before
/// it has a size that is not a rung. A size between two rungs takes the rung
/// **above** it, so that no writer's type is made smaller by an upgrade they
/// did not ask for; a size above the whole ladder takes the top rung. The old
/// default, 20 px, lands that way on step 5 — which is the ladder's own
/// default, and the size iA Writer opens at.
///
/// Read off the wide ladder, which is the one a `settings.toml` written
/// before the ladder was laid out on: the size classes came later, and a
/// stored size is a size the writer chose at whatever width, not at one.
#[must_use]
pub fn step_for_size(size: u32) -> u32 {
    let size = f64::from(size);
    let step = WIDE.iter().position(|rung| rung.em >= size);
    step.unwrap_or(WIDE.len() - 1) as u32
}

/// How the air around a row of ink is divided between the three gaps GTK
/// draws.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Leading {
    /// The upper half of the air between two paragraphs.
    pub above: u32,
    /// `pixels-inside-wrap`: the air between the rows of one paragraph.
    pub inside_wrap: u32,
    /// The lower half of the air between two paragraphs.
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
///
/// `above` and `below` are halves of one gap rather than two properties. The
/// Editor hands GTK their **sum** as `pixels-above-lines` and leaves
/// `pixels-below-lines` at zero (`quill::editor`'s `restyle`), because a
/// non-zero bottom band is where GTK aborts on a paragraph holding invisible
/// bytes (#279); `below` is given back to the code well's boundary rows
/// through the tags `quill::tags`'s `well_leading` sets, which is what keeps
/// the well's rectangle on the pixels it was on.
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

/// One cell of `face` at `size`, in logical pixels.
///
/// The cell follows the em, and the em is the class's ladder's: VERDICTS
/// 4.1.7 read 0.6 em per cell off the app itself, which is the grid the Quill
/// Faces are already cut on. A Face is asked for rather than assumed, so that
/// a Face cut to another grid has somewhere to say so; the three shipped today
/// share [`CELL`].
#[must_use]
pub fn cell(face: Face, size: Size) -> f64 {
    let per_em = match face {
        Face::Duo | Face::Quattro | Face::Mono => CELL,
    };
    per_em * em(size)
}

/// The measure at `size` in `face`, in logical pixels: [`MEASURE`] cells of
/// it.
#[must_use]
pub fn measure(face: Face, size: Size) -> u32 {
    (cell(face, size) * f64::from(MEASURE)).round() as u32
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

/// The margin the middle and the wide class keep outside the container, each
/// side, in logical pixels.
///
/// 5 pt, and points are logical pixels: NOTES § State 25 reads the rule in
/// points throughout, which is the one departure from the device pixels the
/// rest of that file is measured in.
const MARGIN: f64 = 5.0;

/// The widest window the narrowest class's tighter margin constant covers, in
/// logical pixels.
///
/// A fourth break, inside the narrowest class, which moves that class's
/// margin and not its type (NOTES § State 25 § The narrowest class's margin is
/// a rule, and it hides a fourth break). It was bisected at steps 0, 5 and 8
/// and falls between the same two widths each time, so like the class breaks
/// it is the window's width alone.
const NARROWEST_TIGHT_MAX: u32 = 390;

/// The narrowest class's margin constant up to [`NARROWEST_TIGHT_MAX`], in
/// logical pixels.
const NARROWEST_K_TIGHT: f64 = 17.5;

/// The narrowest class's margin constant above it.
const NARROWEST_K_WIDE: f64 = 22.5;

/// The margin outside the container, each side, in a view `view` pixels wide
/// laid out in `class` on a `cell` this many pixels across.
///
/// `max(5, round(K − the cell))` in the narrowest class and a flat [`MARGIN`]
/// in the other two. The narrowest class's rule fits all 56 rows #419
/// measured — four widths across fourteen steps — and it is why § State 22
/// read a 440 pt container 32 device pixels inside what a flat 5 pt margin
/// expects: at the default step the app keeps 13.
fn margin(view: u32, class: SizeClass, cell: f64) -> f64 {
    match class {
        SizeClass::Narrowest => {
            let k = if view <= NARROWEST_TIGHT_MAX {
                NARROWEST_K_TIGHT
            } else {
                NARROWEST_K_WIDE
            };
            (k - cell).round().max(MARGIN)
        }
        SizeClass::Middle | SizeClass::Wide => MARGIN,
    }
}

/// Where the container sits in a view `view` pixels wide laid out in `class`,
/// for `face` at `step`.
///
/// The class and the step pick the cell ([`cell`]) and the margin
/// ([`margin`]), and the container is the smaller of [`CONTAINER`] cells and
/// the view less a margin each side — the Design oracle's rule,
/// `docs/design.md` row Window limitation. Asking for a class rather than for
/// a cell is what keeps the two from disagreeing: a cell from one class and a
/// margin from another is not a page anything draws.
///
/// The class is the caller's rather than `size_class(view)`'s because the two
/// are not always the same view: the class is the **window's**, and the view
/// the container is centred in is whatever room the window left the text — the
/// window less the Library pane, where one is shown (NOTES § State 22,
/// [`size_class`]).
///
/// The container is centred with an odd leftover's extra pixel on the
/// **right** — NOTES § State 25 read 20 px left against 22 right, and 74
/// against 76, where the leftover was an odd number of points — and the
/// gutters hold at [`GUTTER`] cells each
/// while the measure takes what they leave, so the text never reaches an edge
/// and a heading never hangs off the window (ADR 0016). The oracle lets its
/// own gutters narrow to about 6 cells where the window wins and gives the
/// measure the difference; that is the one part of its rule this does not
/// take, because `###### ` is sized to seven. Where the window cannot even
/// seat the two gutters the measure is nothing rather than negative, and the
/// caller has a column it can still lay out.
#[must_use]
pub fn column(view: u32, face: Face, size: Size) -> Column {
    let advance = cell(face, size);
    let room = f64::from(view) - 2.0 * margin(view, size.class, advance);
    let view = f64::from(view);
    // The three lengths every edge below is counted off, rounded here and only
    // here; each edge is then whole-pixel arithmetic on them.
    let gutter = (advance * f64::from(GUTTER)).round().max(0.0);
    let container = (advance * f64::from(CONTAINER)).min(room).max(0.0).round();
    let left = ((view - container) / 2.0).floor();
    Column {
        left: left as u32,
        right: (left + container) as u32,
        side: (left + gutter) as u32,
        width: (container - 2.0 * gutter).max(0.0) as u32,
    }
}

/// The air above the first row of text on a display of `scale`: a constant
/// [`PAGE_TOP`] device pixels at scale 2, 30 points.
///
/// Ask it at scale 1 for logical pixels, which is what the widget lays out in,
/// the way [`pitch`] is asked.
///
/// The Design oracle opens an empty document's caret box **164 device pixels**
/// below its window's top edge (#227). #231 re-shot that window on the original
/// 14-inch M1 MacBook Pro and settled the two things one point could not say
/// (`ref/ia/mac-native/CAPTURE-ORIGINAL-MBP.md` § #231 — the page top,
/// `VERDICTS.md` § Found here):
///
/// - **It does not scale with the pitch.** The box top is 164 at steps 0, 5 and
///   13 alike — pitches 49, 73 and 172 — and 164 again on the default re-shot
///   after the excursion. Three points that far apart tell a constant from a
///   multiple of the pitch.
/// - **104 of the 164 belongs to the title bar.** Shown, the bar is a real
///   opaque band: `#222222` over rows 2 … 103, a `#292929` separator at 104,
///   paper from 105, and AX puts the toolbar's bottom 52 points — 104 pixels —
///   below the window's top edge. The text does not move when the bar is
///   shown, so that band is room the editor leaves for chrome drawn over it.
///
/// What is left, 164 − 104 = **60 device pixels at scale 2 = 30 points**, is
/// the editor's own page top: the half of the figure a window whose chrome is
/// opaque, as Quill's is, has a counterpart to. ADR 0015 gives the row to the
/// Design oracle, and `docs/design.md` row Page top carries the derivation.
///
/// This replaces the two pitches the Parity oracle held, which came from an 85
/// point reading of iA of unrecorded provenance.
#[must_use]
pub fn page_top(scale: f64) -> u32 {
    device(PAGE_TOP, scale)
}

/// The air below the last row of text in a text area `view` pixels tall, so
/// that the end of a draft stops well clear of the bottom edge rather than
/// against it.
///
/// The Design oracle scrolled to the end of `ref/sample.md` keeps **460 pt**
/// of air under the last row of a 949 pt window — 48.5 % of the whole window —
/// where the Parity oracle keeps 30 % (`page.css` `--page-bottom`).
/// `ref/ia/mac-native/NOTES.md` § State 27 is the measurement and
/// `docs/design.md` row Page bottom carries the derivation. A share rather
/// than a constant, because that is what both oracles hold it as.
///
/// The share is taken against the **text area** and not the window, because
/// `view` is what the Editor was allocated and Quill's own bars are already
/// off it. The oracle's window is 949 pt tall with a 52 pt title bar and its
/// 40 pt Toolbar under the text (NOTES § The grid at the default text size for
/// the one, § State 27 § The bar for the other), so the
/// 460 pt sits in an **857 pt** text area: [`PAGE_BOTTOM`] is `460/857`, and
/// asking it for 857 gives the 460 the capture read.
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
            let size = Size::new(SizeClass::Wide, step);
            assert!(
                (em(size) - em_px).abs() < 0.005,
                "step {step}'s em is {} logical px, not NOTES § 11's {em_px}",
                em(size)
            );
            assert_eq!(
                pitch(size, 2.0),
                pitch_px,
                "step {step}'s pitch at scale 2 is not NOTES § 11's"
            );
            assert_eq!(
                caret_width(size, 2.0),
                width_px,
                "step {step}'s caret is not NOTES § 11's width at scale 2"
            );
        }
    }

    /// `ref/ia/mac-native/NOTES.md` § State 25's table, written out again: the
    /// step, then the cell in device pixels at scale 2 and the line pitch in
    /// the same, for the narrowest class and the middle one.
    ///
    /// A second copy for the reason [`NOTES`] is one. The wide cell closes
    /// each row, to the one decimal that table prints it to, so all three
    /// classes are read off the same table rather than two of them off it and
    /// one off the ladder they are being compared with.
    const NOTES_25: [(u32, f64, u32, f64, u32, f64); STEPS as usize] = [
        (0, 13.623, 37, 16.000, 43, 17.4),
        (1, 14.908, 40, 17.112, 46, 18.3),
        (2, 16.080, 44, 18.117, 49, 19.4),
        (3, 17.367, 47, 19.117, 53, 20.6),
        (4, 18.633, 50, 20.265, 56, 23.1),
        (5, 19.863, 53, 22.653, 63, 25.6),
        (6, 22.500, 59, 25.209, 69, 30.7),
        (7, 25.000, 65, 30.235, 81, 35.7),
        (8, 29.770, 76, 35.215, 92, 40.7),
        (9, 34.679, 86, 40.080, 103, 45.7),
        (10, 39.500, 96, 45.000, 113, 53.1),
        (11, 44.250, 105, 52.230, 126, 60.4),
        (12, 51.500, 118, 59.500, 139, 67.8),
        (13, 58.500, 129, 66.750, 150, 75.1),
    ];

    #[test]
    fn the_windows_width_picks_the_class_at_both_breaks_from_either_side() {
        for (width, want) in [
            (0, SizeClass::Narrowest),
            (240, SizeClass::Narrowest),
            (440, SizeClass::Narrowest),
            (441, SizeClass::Middle),
            (960, SizeClass::Middle),
            (1250, SizeClass::Middle),
            (1251, SizeClass::Wide),
            (1440, SizeClass::Wide),
            (10_000, SizeClass::Wide),
        ] {
            assert_eq!(
                size_class(width),
                want,
                "a {width} px window is not laid out in the class NOTES § State 22 measured"
            );
        }
        // The two breaks are the only two, and each is one pixel wide: a
        // window dragged from 240 px to 2000 px changes class exactly twice,
        // which is the whole of "exact, and no hysteresis".
        let breaks: Vec<u32> = (1..=2000u32)
            .filter(|&width| size_class(width) != size_class(width - 1))
            .collect();
        assert_eq!(
            breaks,
            vec![NARROWEST_MAX + 1, MIDDLE_MAX + 1],
            "the class changes somewhere other than the two breaks NOTES § State 22 bisected"
        );
    }

    #[test]
    fn each_class_is_the_cell_and_the_pitch_its_own_ladder_was_measured_at() {
        for (step, narrowest_cell, narrowest_pitch, middle_cell, middle_pitch, wide_cell) in
            NOTES_25
        {
            for (class, want_cell, want_pitch, tolerance) in [
                (SizeClass::Narrowest, narrowest_cell, narrowest_pitch, 0.001),
                (SizeClass::Middle, middle_cell, middle_pitch, 0.001),
                // The wide row is printed to one decimal, so it is read to
                // half of the last one; the other two carry three.
                (SizeClass::Wide, wide_cell, NOTES[step as usize].2, 0.05),
            ] {
                // The table is in the device pixels it was read in, and the
                // ladder is in logical ones.
                let got = 2.0 * cell(Face::Mono, Size::new(class, step));
                assert!(
                    (got - want_cell).abs() < tolerance,
                    "{class:?} at step {step} is a {got} px cell at scale 2, not NOTES § State 25's {want_cell}"
                );
                assert_eq!(
                    pitch(Size::new(class, step), 2.0),
                    want_pitch,
                    "{class:?} at step {step} is not NOTES § State 25's pitch at scale 2"
                );
            }
        }
        // The three readings § State 22 names at the default step, which are
        // the row the judged `page/narrow` state is shot on.
        for (class, want) in [
            (SizeClass::Narrowest, 19.863),
            (SizeClass::Middle, 22.653),
            (SizeClass::Wide, 25.596),
        ] {
            let got = 2.0 * cell(Face::Mono, Size::new(class, default_step()));
            assert!(
                (got - want).abs() < 0.001,
                "{class:?} opens on a {got} px cell at scale 2, not {want}"
            );
        }
    }

    #[test]
    fn a_display_at_another_scale_gets_the_ladder_scaled_and_never_nothing() {
        // The ladder was measured at scale 2, so scale 1 is half of it and
        // scale 3 half again as much, each rounded once.
        let wide = |step| Size::new(SizeClass::Wide, step);
        assert_eq!(pitch(wide(5), 1.0), 37, "step 5 is 73 device px at scale 2");
        assert_eq!(pitch(wide(5), 3.0), 110, "and 109.5 at scale 3, rounded up");
        assert_eq!(
            caret_width(wide(0), 1.0),
            3,
            "5 device px at scale 2 is 2.5"
        );
        assert_eq!(
            caret_width(wide(13), 4.0),
            20,
            "and 10 at scale 2 is 20 at 4"
        );
        assert_eq!(
            caret_width(wide(0), 0.25),
            1,
            "a caret rounded away is a writer with no caret"
        );
        assert_eq!(
            pitch(wide(0), 0.01),
            1,
            "and a pitch rounded away is one row"
        );
    }

    #[test]
    fn the_leading_is_liquid_rather_than_a_constant_multiple_of_the_em() {
        // What the linear clamp could not do: the bigger the type, the
        // tighter the leading, proportionally. `docs/design.md` § Line pitch
        // reads the curve off the ends of the ladder.
        let ratio = |step| {
            let size = Size::new(SizeClass::Wide, step);
            f64::from(pitch(size, 2.0)) / (2.0 * em(size))
        };
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
        for class in SizeClass::ALL {
            let top = Size::new(class, STEPS - 1);
            assert_eq!(Size::new(class, STEPS), top);
            assert_eq!(Size::new(class, 99).step(), STEPS - 1);
            assert!((em(Size::new(class, STEPS)) - em(top)).abs() < f64::EPSILON);
            assert_eq!(pitch(Size::new(class, 99), 2.0), pitch(top, 2.0));
        }
    }

    #[test]
    fn a_size_with_nothing_asked_of_it_is_the_default_step_of_the_wide_class() {
        assert_eq!(
            Size::default(),
            Size::new(SizeClass::Wide, default_step()),
            "a window with no size yet is not laid out at the ladder's default"
        );
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
            let pitch = pitch(Size::new(SizeClass::Wide, step), 2.0);
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
            for class in SizeClass::ALL {
                for step in steps() {
                    let size = Size::new(class, step);
                    assert!(
                        (cell(face, size) - 0.6 * em(size)).abs() < f64::EPSILON,
                        "{name} at step {step} of {class:?} is not on the 0.6 em cell VERDICTS 4.1.7 read"
                    );
                }
            }
            assert!(
                (cell(face, Size::default()) - 12.798).abs() < 0.001,
                "{name} at the default step is not 0.6 of its 21.33 px em"
            );
            assert_eq!(
                measure(face, Size::default()),
                819,
                "{name}'s 64-character measure at the default step is not 819 px"
            );
        }
    }

    #[test]
    fn the_container_is_centred_while_it_fits() {
        // Both judged widths, at the ladder's default step. Each is in its
        // own size class, so each is 78 cells of its own ladder: 998 px of the
        // wide class's 12.798 in the 1440 px window, and 883 px of the middle
        // class's 11.3265 in the 960 px `narrow` state — which seats a full
        // measure at the smaller type where the wide type did not.
        assert_eq!(
            column(1440, Face::Duo, Size::default()),
            Column {
                left: 221,
                right: 1219,
                side: 311,
                width: 818
            },
            "a judged 1440 px window does not centre the 78-cell container with the measure a gutter inside it"
        );
        assert_eq!(
            column(960, Face::Duo, Size::new(SizeClass::Middle, default_step())),
            Column {
                left: 38,
                right: 921,
                side: 117,
                width: 725
            },
            "the `narrow` judged state does not centre the middle class's 78 cells"
        );
        // 960 less an 883 px container leaves 77, and the odd one falls on
        // the right: 38 against 39, the way NOTES § State 25 read 20 against
        // 22 and 74 against 76.
        let odd = column(960, Face::Duo, Size::new(SizeClass::Middle, default_step()));
        assert_eq!(
            (odd.left, 960 - odd.right),
            (38, 39),
            "an odd leftover's extra pixel does not fall on the right of the container"
        );
    }

    #[test]
    fn a_pane_beside_the_text_narrows_the_page_and_leaves_the_windows_class_on_it() {
        // The Design oracle picks the class on the window's own width and
        // nothing else (NOTES § State 22), so a 1440 px window showing the
        // 368 px Library lays its remaining 1072 px out in the **wide** class
        // and not in the middle one that width alone reads as.
        // `quill::editor`'s `lay_out` is the caller that keeps the two apart:
        // the window's width here, the Editor's own below.
        assert_eq!(
            size_class(1072),
            SizeClass::Middle,
            "the room left beside the Library is not in a class of its own, so this proves nothing"
        );
        let beside = column(1072, Face::Duo, Size::new(size_class(1440), default_step()));
        assert_eq!(
            beside.gutter(),
            column(1440, Face::Duo, Size::default()).gutter(),
            "opening the Library took the type off the class its window is in"
        );
        assert_eq!(
            beside.left + beside.right,
            1072,
            "the container is not centred in the room the Editor was actually left"
        );
    }

    #[test]
    fn a_window_too_narrow_for_the_container_holds_its_gutters_and_shrinks_the_measure() {
        assert_eq!(
            column(600, Face::Duo, Size::new(SizeClass::Middle, default_step())),
            Column {
                left: 5,
                right: 595,
                side: 84,
                width: 432
            },
            "a window narrower than 78 cells is not the window less its margins, with its gutters held at 7 cells and the measure giving up the difference"
        );
    }

    #[test]
    fn the_container_is_the_smaller_of_78_cells_and_the_window_less_its_margins() {
        // Where the first term wins the measure is the whole limit and the
        // gutter is 7 cells: 960 pt at step 5 holds all 64 characters on the
        // middle class's cell, as NOTES § State 22 reads off the app.
        let middle = Size::new(SizeClass::Middle, 5);
        let full = column(960, Face::Mono, middle);
        assert_eq!(
            full.gutter(),
            79,
            "the gutter is not 7 of the class's cells"
        );
        assert_eq!(
            full.width,
            measure(Face::Mono, middle),
            "960 pt at step 5 does not hold a full 64-character measure"
        );
        // Where the window wins, the container is the window less 5 px each
        // side. The four widths are § State 22's own, at the step where 78
        // cells no longer fit any of them.
        for view in [960, 1040, 1200, 1250] {
            let clipped = column(view, Face::Mono, Size::new(size_class(view), 8));
            assert_eq!(
                clipped.right - clipped.left,
                view - 2 * (MARGIN as u32),
                "a {view} px window at step 8 does not give the container the whole window less its margins"
            );
        }
        // 440 pt is the class break, and the class's own margin — 13 px at
        // this step, not 5 — is why § State 22 read a container 32 device
        // pixels inside what it expected: 414 logical px is its 828.
        let narrowest = column(440, Face::Mono, Size::new(SizeClass::Narrowest, 5));
        assert_eq!(
            narrowest.right - narrowest.left,
            414,
            "the narrowest class's container is not the window less the margin #419 measured"
        );
    }

    #[test]
    fn the_narrowest_class_keeps_the_margin_419_measured_at_every_step() {
        // `ref/ia/mac-native/narrow-419-margins-steps.json`, halved: the side
        // margin in device pixels at scale 2, at the narrowest window the app
        // allows and at the widest window of the class. The two columns are
        // the two constants of the margin rule, and the tail of each is the
        // 5 px floor.
        const MEASURED: [(u32, f64, f64); STEPS as usize] = [
            (0, 11.0, 16.0),
            (1, 10.0, 15.0),
            (2, 9.0, 14.0),
            (3, 9.0, 14.0),
            (4, 8.0, 13.0),
            (5, 8.0, 13.0),
            (6, 6.0, 11.0),
            (7, 5.0, 10.0),
            (8, 5.0, 8.0),
            (9, 5.0, 5.0),
            (10, 5.0, 5.0),
            (11, 5.0, 5.0),
            (12, 5.0, 5.0),
            (13, 5.0, 5.0),
        ];
        for (step, tight, wide) in MEASURED {
            let cell = cell(Face::Mono, Size::new(SizeClass::Narrowest, step));
            for (view, want) in [(240, tight), (320, tight), (440, wide)] {
                assert!(
                    (margin(view, SizeClass::Narrowest, cell) - want).abs() < f64::EPSILON,
                    "a {view} px window at step {step} does not keep the {want} px #419 measured"
                );
            }
        }
        // The fourth break, bisected at steps 0, 5 and 8: it moves the margin
        // and nothing else, and it is the window's width alone.
        for (step, below, above) in [(0, 11.0, 16.0), (5, 8.0, 13.0), (8, 5.0, 8.0)] {
            let cell = cell(Face::Mono, Size::new(SizeClass::Narrowest, step));
            assert!(
                (margin(390, SizeClass::Narrowest, cell) - below).abs() < f64::EPSILON
                    && (margin(391, SizeClass::Narrowest, cell) - above).abs() < f64::EPSILON,
                "the margin at step {step} does not change between 390 and 391 px"
            );
        }
        // Neither other class has the rule: 5 px each side at every step.
        for class in [SizeClass::Middle, SizeClass::Wide] {
            for step in steps() {
                assert!(
                    (margin(1440, class, cell(Face::Mono, Size::new(class, step))) - MARGIN).abs()
                        < f64::EPSILON,
                    "{class:?} at step {step} does not keep the flat 5 px margin"
                );
            }
        }
    }

    #[test]
    fn the_deepest_heading_hangs_to_the_container_edge() {
        let column = column(1440, Face::Duo, Size::default());
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
        let column = column(1440, Face::Duo, Size::new(SizeClass::Wide, 4));
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
    fn the_page_opens_on_a_constant_and_ends_well_clear_of_the_bottom() {
        assert_eq!(
            page_top(2.0),
            60,
            "the page does not open on the oracle's 164 px less its 104 px title bar"
        );
        assert_eq!(
            page_top(1.0),
            30,
            "the constant does not come back through the scale as 30 logical px"
        );
        assert_eq!(
            page_bottom(857),
            460,
            "the text area of the oracle's 949 pt window — its 52 pt title bar and its 40 pt \
             Toolbar off it — does not leave the measured 460 pt of air"
        );
        assert_eq!(
            page_bottom(900),
            483,
            "a judged 900 px text area does not leave the same share below the last row"
        );
    }

    /// The Editor's top margin is [`page_top`] less the paragraph's `below`
    /// (`quill::editor`'s `restyle`), on a `u32`. Two pitches used to make
    /// that safe by construction and a constant 30 does not, so the claim the
    /// saturating subtraction rests on is asserted here over the whole ladder
    /// rather than measured once.
    ///
    /// `below` is half the air a pitch leaves around a row of ink, so the most
    /// of it any Face can produce is at the shortest row that Face draws. A
    /// row of ink is never shorter than the em on the Quill Faces — Duo, Mono
    /// and Quattro measure 19 logical px at step 0's em of 14.50 and 83 at
    /// step 13's 62.58, the same three to the pixel — so the em is the floor,
    /// and a step tested at it is tested for every Face that clears it.
    #[test]
    fn no_steps_leading_eats_the_page_top() {
        let top = page_top(1.0);
        for class in SizeClass::ALL {
            for step in steps() {
                let size = Size::new(class, step);
                let shortest_row = em(size).ceil() as u32;
                let below = leading(pitch(size, 1.0), shortest_row).below;
                assert!(
                    below < top,
                    "step {step} of {class:?} leaves {below} px of air under a row an em tall, \
                     against a {top} px page top: the Editor's top margin saturates \
                     to nothing and the page opens hard against the window"
                );
            }
        }
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
