//! The caret's blink and its glide, with no widget in it.
//!
//! The Parity oracle keeps the two in `legacy/app/js/caret.js` and
//! `legacy/app/css/caret.css`, as a CSS animation, a CSS transition and a
//! `setTimeout` for the quiet before the blink comes back. None of the three
//! survives the port: the Editor paints the bar itself, below the text, and
//! the frame clock hands it a time rather than firing a callback. So all of it
//! is here as one state machine — the Editor says what happened, and asks, for
//! a given frame time, where the bar is and how opaque it is.
//!
//! Time is always the caller's, in microseconds, which is
//! `gdk::FrameClock::frame_time`'s unit. Never a timeout and never a system
//! clock: a machine driven by the times it is handed can be run through a
//! scripted timeline at whatever instant the test likes, and for timings this
//! short that is the only way they are ever checked.
//!
//! The bar's geometry is the caller's too. The Editor takes the row's top and
//! the row's height from `quill_engine::typography`'s pitch rather than from
//! the glyph, so the bar spans the split leading the way iA's does. This
//! module keeps what it is given and does two pieces of arithmetic on it: the
//! width from the type size, and the snap of x onto a whole device pixel.
//!
//! Nothing here is a widget, and nothing here is `gtk`.

use crate::flags::Flags;

/// A millisecond, in the microseconds every time in this module is counted in.
const MS: i64 = 1_000;

/// The share of the type size the bar is wide: `WIDTH` in `caret.js`.
///
/// iA's own captures measure 0.152 em — the header comment in `caret.js` — but
/// 0.155 is the number the oracle draws with, so it is the number ours draws
/// with. Between 10 px and 40 px the two disagree at 23 px and at 36 px, and
/// nowhere else.
const WIDTH: f64 = 0.155;

/// The narrowest bar, whatever the size: two whole pixels, never a hairline.
const MIN_WIDTH: u32 = 2;

/// The quiet after a move or an edit before the blink starts again: `IDLE_MS`.
const IDLE: i64 = 480 * MS;

/// A move within this of an edit never glides: `EDIT_SNAP_MS`.
const EDIT_SNAP: i64 = 150 * MS;

/// The glide along a row: `GLIDE_X`.
const GLIDE_ALONG: i64 = 34 * MS;

/// The glide between rows, longer because the way is: `GLIDE_Y`.
const GLIDE_ACROSS: i64 = 46 * MS;

/// Moves closer together than this snap: `SNAP_MS`.
///
/// A held arrow repeats faster than a glide lasts, so without this every
/// repeat would start another and the caret would be towed along behind the
/// key rather than moving with it.
const SNAP: i64 = 60 * MS;

/// The shortest hop worth following, in ems: `GLIDE_MIN`.
///
/// Under three cells there is nothing to track — about two millimetres on a
/// desktop — and a glide over it reads as the bar smearing rather than as the
/// bar travelling. The gate is skipped when the hop changed rows, because a
/// row is worth following however little the column moved.
const GLIDE_MIN: f64 = 3.0;

/// The longest hop worth following along a row, in ems.
///
/// `dx <= M.em * 14` in `placeCaret`. Past fourteen cells the eye has lost the
/// bar before it arrives, so the glide only delays the answer to where it went.
const GLIDE_FAR: f64 = 14.0;

/// The furthest a hop may fall and still be followed, in pitches.
///
/// `dy <= M.pitch * 1.2`: the next row, and no further. A jump of a screen is
/// a new place rather than the same caret moving to it.
const GLIDE_DROP: f64 = 1.2;

/// How far past the advance boundary the bar sits, in ems: `NUDGE_X`.
///
/// The oracle measured it at 0.060–0.073 em of iA's own captures and draws at
/// 0.07. The bar stands in the gap after the glyph rather than on its last
/// column, which is what makes it read as between two letters.
const NUDGE: f64 = 0.07;

/// The bar full on, at the top of the blink's cycle.
const ON: i64 = 470 * MS;

/// The fade down, slower than the fade back so the blink reads as breathing
/// rather than flashing.
const FADE_OUT: i64 = 85 * MS;

/// The dark half of the cycle.
const OFF: i64 = 445 * MS;

/// The fade back up.
const FADE_IN: i64 = 55 * MS;

/// One turn of the blink.
///
/// The four above are the ones `caret.css` names in the comment over its
/// keyframes, and the ones #106 § Decided details decided, so they are the
/// ones built here. The keyframes themselves run `1.06s` at `0%,44% {1}
/// 52%,94% {0} 100% {1}`, which works out at 466.4, 84.8, 445.2 and 63.6 for a
/// 1060 ms round: the same blink to within 9 ms, all of it in the fade back
/// up. Worth knowing if the two are ever put side by side frame by frame, and
/// not worth seeing otherwise.
const CYCLE: i64 = ON + FADE_OUT + OFF + FADE_IN;

/// The share of the pitch the band carries above the baseline: 11/16, leaving
/// 31.25 % below it.
///
/// `ABOVE` in `caret.js`, measured off iA Writer's own captures rather than
/// derived — `ref/ia/REFERENCE.md` § 4.1, re-measured there on
/// `appstore-mac-01` and `msstore-win-01`.
const ABOVE_BASELINE: f64 = 0.6875;

/// What is left of the caret when the window is not active.
///
/// iA and macOS drop it entirely; the oracle keeps a ghost, so that coming
/// back from another window your place is still where you left it.
const GHOST: f64 = 0.3;

/// The glide's easing: `cubic-bezier(.22, .61, .36, 1)` in `caret.css`, as
/// (x1, y1, x2, y2).
const EASE: (f64, f64, f64, f64) = (0.22, 0.61, 0.36, 1.0);

/// The bar the Editor paints, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    /// The left edge, snapped onto a whole device pixel when it is set.
    pub x: f64,
    /// The top of the row, not the top of the ink.
    pub y: f64,
    /// [`width`] at the type size.
    pub w: f64,
    /// The full line pitch, so the bar spans the split leading.
    pub h: f64,
}

/// What moved the caret, which is what decides whether it travels there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    /// A click or a drag.
    Pointer,
    /// An arrow, a word jump, Home, End.
    Key,
    /// The move a buffer change makes.
    ///
    /// The glyph just typed is on the glass in about 6 ms and the caret has to
    /// be beside it in the same frame; gliding there put the one mark the eye
    /// is fixated on four frames behind the letter, which the oracle measured
    /// as the largest perceptible latency in the product
    /// (`progress/latency-report.md` §5). So this one never glides.
    FollowsEdit,
}

/// What last drove the widget, which is half of what a move's kind is.
///
/// GTK says only that the insert mark moved, never what moved it, so the
/// Editor keeps the answer: a `GtkGestureClick` press sets [`Source::Pointer`]
/// and a `GtkEventControllerKey` press sets [`Source::Key`], each before GTK's
/// own handling turns it into a caret move.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Source {
    /// A click or a drag reached the widget last.
    Pointer,
    /// A key press reached the widget last.
    Key,
    /// Neither has: the app moved the caret itself.
    #[default]
    App,
}

/// The kind of a move, from what drove the widget and whether the buffer
/// changed in this frame.
///
/// The whole of the classification, as a function of the two, so that it can
/// be checked without a widget to drive. A change in the same frame wins over
/// either controller: the key that inserted the glyph is also the key that
/// moved the caret, and the caret has to be beside the glyph in that frame.
///
/// A move with no controller behind it is the app's own — a launch flag, a
/// restored position, a Command — and it is put there rather than travelled
/// to, so it takes the kind that never glides.
#[must_use]
pub fn kind(last: Source, edited: bool) -> Move {
    match last {
        _ if edited => Move::FollowsEdit,
        Source::Pointer => Move::Pointer,
        Source::Key => Move::Key,
        Source::App => Move::FollowsEdit,
    }
}

/// What the launch asked the caret to be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// A writer's launch: the blink and the glide both run.
    Live,
    /// `--deterministic`: the blink frozen on and no glide, so two shots of
    /// one state are the same pixels.
    Deterministic,
    /// `--nocaret`: no bar at all, so a shot judges the text rather than the
    /// instrument standing in it.
    Nocaret,
}

impl Mode {
    /// The mode this launch's flags asked for.
    ///
    /// `--nocaret` wins over `--deterministic`: a caret that is not drawn has
    /// nothing left to freeze.
    #[must_use]
    pub fn from_flags(flags: &Flags) -> Self {
        if flags.nocaret {
            Self::Nocaret
        } else if flags.deterministic {
            Self::Deterministic
        } else {
            Self::Live
        }
    }
}

/// A glide in flight: where the bar left from, when, and for how long.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from_x: f64,
    from_y: f64,
    at: i64,
    len: i64,
}

/// The caret, as everything but the paint.
///
/// Feed it [`Caret::moved`], [`Caret::edited`], [`Caret::focus`] and
/// [`Caret::tick`]; read [`Caret::rect`], [`Caret::alpha`] and
/// [`Caret::wants_tick`].
#[derive(Clone, Copy, Debug)]
pub struct Caret {
    mode: Mode,
    /// One em in device pixels, which is the unit both glide gates are
    /// measured in.
    ///
    /// Set with the type by [`Caret::resize`], and zero until it is: a caret
    /// with no size to measure a hop against never glides along a row, which
    /// is the safe half of the gate rather than the wrong one.
    em: f64,
    focused: bool,
    /// Where the bar is going, or already is; its x is snapped.
    to: Bar,
    /// Whether the bar has been put anywhere yet. The first placement never
    /// glides, and neither does the first after the window comes back — the
    /// oracle drops its `prev` at both.
    placed: bool,
    glide: Option<Glide>,
    /// The last move or edit: the blink is held for [`IDLE`] after it.
    active_at: Option<i64>,
    /// The last move that went anywhere: another within [`SNAP`] of it is a
    /// key repeating rather than a hand, and snaps.
    moved_at: Option<i64>,
    /// The last edit: a move within [`EDIT_SNAP`] of it snaps.
    edited_at: Option<i64>,
    /// The last time the machine was told about, which is the one
    /// [`Caret::wants_tick`] answers for.
    now: i64,
}

impl Caret {
    /// A caret for a launch, in the mode that launch asked for.
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            em: 0.0,
            focused: true,
            to: Bar {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            placed: false,
            glide: None,
            active_at: None,
            moved_at: None,
            edited_at: None,
            now: 0,
        }
    }

    /// The type is now `em` device pixels to the em.
    ///
    /// The size the widget is set in, times the surface's scale factor, since
    /// every length this machine holds is in device pixels. Both glide gates
    /// are measured against it, so a caret whose type changed under it would
    /// otherwise judge a hop by the old size.
    pub fn resize(&mut self, em: f64) {
        self.em = em;
    }

    /// The caret is at `to`, moved by `kind`, at frame time `t`.
    ///
    /// The bar travels there if this is navigation — a click or a cursor key,
    /// with no edit behind it and not at a key's repeat rate — and is simply
    /// put there otherwise. A glide already in flight is left from where it
    /// had got to, not from where it began, so a second move mid-travel reads
    /// as one caret changing course.
    ///
    /// The same position arrives twice per keystroke in the oracle, once for
    /// the render and once for the selection, and it will here too. The second
    /// is dropped rather than reissued: reissuing it would cancel a glide
    /// already in flight and, worse, make the pair look like a hand moving
    /// fast enough to snap. It still holds the blink, which is the one thing
    /// the oracle does either way.
    pub fn moved(&mut self, to: Bar, kind: Move, t: i64) {
        self.now = t;
        self.active_at = Some(t);
        let to = Bar {
            x: snap(to.x),
            y: snap(to.y),
            ..to
        };
        if self.placed && to == self.to {
            return;
        }
        let at = self.rect(t);
        self.glide = if self.glides(at, to, kind, t) {
            Some(Glide {
                from_x: at.x,
                from_y: at.y,
                at: t,
                // `dy > 0.5` in the oracle rather than a row's height: taken
                // mid-glide the row below is a fraction away, not a pitch.
                len: if (at.y - to.y).abs() > 0.5 {
                    GLIDE_ACROSS
                } else {
                    GLIDE_ALONG
                },
            })
        } else {
            None
        };
        self.to = to;
        self.placed = true;
        self.moved_at = Some(t);
    }

    /// The buffer changed at `t`.
    ///
    /// The move the change makes arrives separately; this is what puts the
    /// caret inside the edit-snap window, and it holds the blink on its own so
    /// that a keystroke that moves nothing still stops the flicker.
    pub fn edited(&mut self, t: i64) {
        self.now = t;
        self.edited_at = Some(t);
        self.active_at = Some(t);
    }

    /// The window became active at `t`, or stopped being.
    pub fn focus(&mut self, active: bool, t: i64) {
        self.now = t;
        self.focused = active;
        if active {
            self.placed = false;
            self.glide = None;
            self.active_at = Some(t);
        }
    }

    /// A frame at `t`.
    ///
    /// Bookkeeping only: [`Caret::rect`] and [`Caret::alpha`] are functions of
    /// the time they are asked about rather than of the last tick, so the only
    /// thing a frame settles is whether a glide is still in flight.
    pub fn tick(&mut self, t: i64) {
        self.now = t;
        if let Some(g) = self.glide
            && t - g.at >= g.len
        {
            self.glide = None;
        }
    }

    /// Where the bar is at `t`.
    #[must_use]
    pub fn rect(&self, t: i64) -> Bar {
        let Some(g) = self.glide else { return self.to };
        let p = (t - g.at) as f64 / g.len as f64;
        if p >= 1.0 {
            return self.to;
        }
        if p <= 0.0 {
            return Bar {
                x: g.from_x,
                y: g.from_y,
                ..self.to
            };
        }
        // The width and the height are the new row's from the first frame: the
        // oracle sets both outright and animates the position alone.
        let e = eased(p);
        Bar {
            x: g.from_x + (self.to.x - g.from_x) * e,
            y: g.from_y + (self.to.y - g.from_y) * e,
            ..self.to
        }
    }

    /// How opaque the bar is at `t`, from 0 to 1.
    ///
    /// The three that are not the blink come first, in the order they beat
    /// each other: nothing drawn at all, then the ghost the unfocused shot
    /// judges, then the frozen bar `--deterministic` asks for.
    #[must_use]
    pub fn alpha(&self, t: i64) -> f64 {
        if self.mode == Mode::Nocaret {
            return 0.0;
        }
        if !self.focused {
            return GHOST;
        }
        if self.mode == Mode::Deterministic {
            return 1.0;
        }
        match self.active_at {
            Some(a) if t >= a + IDLE => blink(t - (a + IDLE)),
            _ => 1.0,
        }
    }

    /// Whether the Editor has to ask for another frame.
    ///
    /// True only while a blink or a glide is in progress, so a caret with
    /// nothing to animate — unfocused, frozen, not drawn, or held on by a hand
    /// that is still moving — costs the frame clock nothing. The Latency Piece
    /// rests on that (#41 § Frame clock).
    #[must_use]
    pub fn wants_tick(&self) -> bool {
        if self.mode != Mode::Live || !self.focused {
            return false;
        }
        self.glide.is_some_and(|g| self.now < g.at + g.len)
            || self.active_at.is_some_and(|a| self.now >= a + IDLE)
    }

    /// When the Editor has to come back, if not on the very next frame.
    ///
    /// [`Caret::wants_tick`] is false through the [`IDLE`] quiet a move or an
    /// edit buys, because there is nothing to animate inside it and the
    /// Latency Piece is paid for in the frames that are never asked for. The
    /// blink does come back at the end of it, though, and a widget that let
    /// its tick source go there would have nothing left to ask for the frame
    /// that resumes it. This is that instant, on the caller's own clock. It is
    /// `None` whenever `wants_tick` is already true, and whenever there is no
    /// blink coming at all.
    #[must_use]
    pub fn resumes_at(&self) -> Option<i64> {
        if self.mode != Mode::Live || !self.focused || self.wants_tick() {
            return None;
        }
        self.active_at
            .map(|a| a + IDLE)
            .filter(|&when| self.now < when)
    }

    /// Whether the move from `at` to `to`, of this kind at `t`, is one to be
    /// tracked by eye.
    ///
    /// The oracle's gate is five conditions in `placeCaret`, and all five are
    /// here now that the widget hands the machine a type size as well as a
    /// rectangle. Two are about when the move came: `EDIT_SNAP_MS`, so that
    /// nothing following an edit ever travels, and `SNAP_MS`, so that a held
    /// arrow's repeat rate does not tow the caret along behind the key. Three
    /// are about how far it went — a hop shorter than [`GLIDE_MIN`] ems along
    /// a row has nothing in it to follow, and one further than [`GLIDE_FAR`]
    /// ems or [`GLIDE_DROP`] pitches has lost the eye before the bar could
    /// arrive — and a hop that changed rows is worth following however little
    /// the column moved.
    fn glides(&self, at: Bar, to: Bar, kind: Move, t: i64) -> bool {
        if !self.placed || kind == Move::FollowsEdit || self.mode != Mode::Live {
            return false;
        }
        if self.moved_at.is_some_and(|m| t - m < SNAP) {
            return false;
        }
        if self.edited_at.is_some_and(|e| t - e < EDIT_SNAP) {
            return false;
        }
        let dx = (to.x - at.x).abs();
        let dy = (to.y - at.y).abs();
        if dx == 0.0 && dy == 0.0 {
            // Nowhere to travel. A glide of no length would still cancel one
            // in flight, and there is nothing for the eye in either.
            return false;
        }
        // `dy > 0.5` rather than a row's height, as in [`Caret::moved`]: taken
        // mid-glide the row below is a fraction of a pitch away, not a pitch.
        let across = dy > 0.5;
        if !across && dx < self.em * GLIDE_MIN {
            return false;
        }
        dy <= to.h * GLIDE_DROP && dx <= self.em * GLIDE_FAR
    }
}

impl Default for Caret {
    /// A writer's caret, which is what a widget built before its launch's
    /// flags have been read has to be until [`Mode::from_flags`] answers.
    fn default() -> Self {
        Self::new(Mode::Live)
    }
}

/// How far past the glyph's advance boundary the bar's left edge sits at type
/// size `size`, in the pixels the widget lays out in.
///
/// `M.em * NUDGE_X` in `placeCaret`, added before the snap rather than after
/// it, so that the nudge decides which whole pixel the bar lands on rather
/// than pushing it off one. Logical pixels, because the advance the Editor
/// reads out of the layout is in logical pixels; the scale factor is applied
/// to their sum, on the way into [`Bar`].
#[must_use]
pub fn nudge(size: u32) -> f64 {
    f64::from(size) * NUDGE
}

/// Where the bar's top sits, given the row's baseline and the pitch, in the
/// pixels the widget lays out in.
///
/// `snap(b.top + M.base - ABOVE * M.pitch)` in `caret.js`, and the third of
/// the numbers measured off iA's own captures in `ref/ia/REFERENCE.md` § 4.1:
/// the band is [`ABOVE_BASELINE`] of the pitch above the baseline and the rest
/// below it.
///
/// The baseline, and not the top of the line box, is the anchor. A CSS line
/// box puts its baseline at about 73 % of the pitch, which is lower than iA
/// puts it: iA centres the band on the middle of the *ink*, cap height to
/// descender, rather than on the middle of the font's em box, so the bar
/// reads as balanced against the letters instead of riding up toward the line
/// above. Anchoring to the baseline is also what makes the rule hold for any
/// Face, size or leading the Type piece chooses, since every one of those
/// moves the ink inside the box but none of them moves the baseline out from
/// under it.
#[must_use]
pub fn band_top(baseline: f64, pitch: f64) -> f64 {
    baseline - ABOVE_BASELINE * pitch
}

/// The bar's width at type size `size`, in whole device pixels.
///
/// `Math.max(2, Math.round(M.em * WIDTH))` in `caret.js`, and whole pixels for
/// the reason the comment beside it gives: a rasteriser snaps a painted box's
/// two edges on its own, so a 4.5 px stem comes out 5 px wide with a grey
/// column down one side however hard its position is snapped. Asking for the
/// integer keeps both edges hard at any scale. 3 px at 20 px, 5 px at 31 px.
///
/// The sibling of `quill_engine::typography::pitch`, and read with it: the
/// Editor builds a [`Bar`] from the two, one across the row and one down it.
#[must_use]
pub fn width(size: u32) -> u32 {
    ((f64::from(size) * WIDTH).round() as u32).max(MIN_WIDTH)
}

/// The left edge, on a whole device pixel.
///
/// The pitch arrives whole from `quill_engine::typography`, but neither edge
/// the bar is placed by does: x is nudged off the advance boundary and y hangs
/// from a baseline at [`ABOVE_BASELINE`] of the pitch, and both land wherever
/// that arithmetic leaves them. Both need this: a bar starting on a half pixel
/// is rasterised a row or a column wider than it was cut, with a grey edge
/// standing in for the half. `caret.js` snaps its top and its left for the
/// same reason.
///
/// Device pixels are the caller's: the widget applies the surface's scale
/// factor on the way in, which is where the oracle's `Math.round(v * dpr) /
/// dpr` went. Handing this logical pixels on a scale-2 output would snap to
/// every second device pixel and leave the fault it exists to prevent.
fn snap(x: f64) -> f64 {
    x.round()
}

/// The blink's alpha, `elapsed` microseconds into a cycle that began with the
/// bar on.
fn blink(elapsed: i64) -> f64 {
    let p = elapsed.rem_euclid(CYCLE);
    if p < ON {
        1.0
    } else if p < ON + FADE_OUT {
        1.0 - (p - ON) as f64 / FADE_OUT as f64
    } else if p < ON + FADE_OUT + OFF {
        0.0
    } else {
        (p - (ON + FADE_OUT + OFF)) as f64 / FADE_IN as f64
    }
}

/// The eased share of a glide at `p`, its share of the way through in time.
///
/// A `cubic-bezier` is a curve in x as well as in y: the browser solves
/// x(u) = p for the curve's own parameter and reads y(u) back, and so does
/// this. Newton from u = p converges in a few steps on a curve this gentle;
/// the bisection is there for the ones it would walk off.
fn eased(p: f64) -> f64 {
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let (x1, y1, x2, y2) = EASE;
    let mut u = p;
    for _ in 0..8 {
        let dx = bezier(x1, x2, u) - p;
        if dx.abs() < 1e-7 {
            return bezier(y1, y2, u);
        }
        let slope = slope(x1, x2, u);
        if slope.abs() < 1e-7 {
            break;
        }
        u = (u - dx / slope).clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    u = p;
    for _ in 0..40 {
        let dx = bezier(x1, x2, u) - p;
        if dx.abs() < 1e-7 {
            break;
        }
        if dx < 0.0 {
            lo = u;
        } else {
            hi = u;
        }
        u = f64::midpoint(lo, hi);
    }
    bezier(y1, y2, u)
}

/// One axis of a cubic Bézier from 0 to 1 through control points `a` and `b`.
fn bezier(a: f64, b: f64, u: f64) -> f64 {
    let v = 1.0 - u;
    3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
}

/// That curve's first derivative, for Newton's next step.
fn slope(a: f64, b: f64, u: f64) -> f64 {
    let v = 1.0 - u;
    3.0 * v * v * a + 6.0 * v * u * (b - a) + 3.0 * u * u * (1.0 - b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bar at 20 px type: `quill_engine::typography::pitch(20)` is 36, and
    /// the width at that size is 3.
    fn bar(x: f64, y: f64) -> Bar {
        Bar {
            x,
            y,
            w: f64::from(width(20)),
            h: 36.0,
        }
    }

    /// A caret in a window that has just become active, set at the 20 px type
    /// [`bar`] measures: one em is 20 device pixels, so a hop glides between
    /// 60 and 280 of them along a row.
    fn live() -> Caret {
        let mut c = Caret::new(Mode::Live);
        c.resize(20.0);
        c.focus(true, 0);
        c
    }

    /// The timeline #38 § Testing Decisions asks for: type, wait 479 ms, wait
    /// 1 ms more, jump, edit and then move.
    #[test]
    fn the_scripted_timeline() {
        let mut c = live();

        // Type. The glyph is on the glass this frame, so the bar is beside it
        // this frame, and the blink is held while the hand is moving.
        c.edited(0);
        c.moved(bar(100.0, 0.0), Move::FollowsEdit, 0);
        assert_eq!(c.rect(0), bar(100.0, 0.0));
        assert_eq!(c.alpha(0), 1.0);

        // 479 ms of quiet is not yet quiet enough.
        c.tick(479 * MS);
        assert_eq!(c.alpha(479 * MS), 1.0);
        assert!(!c.wants_tick(), "the blink is still held at 479 ms");

        // 1 ms more, and it is back at the top of its cycle.
        c.tick(IDLE);
        assert!(c.wants_tick(), "the blink is running again at 480 ms");
        assert_eq!(c.alpha(IDLE), 1.0);
        assert_eq!(c.alpha(IDLE + ON), 1.0, "the fade has not started");
        assert_eq!(c.alpha(IDLE + ON + FADE_OUT / 2), 0.5, "half way down");
        assert_eq!(c.alpha(IDLE + ON + FADE_OUT), 0.0, "dark");
        assert_eq!(
            c.alpha(IDLE + ON + FADE_OUT + OFF),
            0.0,
            "the fade back starts here"
        );
        assert_eq!(
            c.alpha(IDLE + ON + FADE_OUT + OFF + FADE_IN / 2),
            0.5,
            "half way up"
        );
        assert_eq!(c.alpha(IDLE + CYCLE), 1.0, "and round again");

        // A jump: no edit for two cycles, so the caret travels there. Thirteen
        // ems of it, inside both the [`GLIDE_MIN`] floor and the [`GLIDE_FAR`]
        // cap, which is a hop the eye can follow.
        let jump = IDLE + 2 * CYCLE;
        c.moved(bar(360.0, 0.0), Move::Pointer, jump);
        assert_eq!(c.alpha(jump), 1.0, "a move holds the blink on");
        assert_eq!(c.rect(jump).x, 100.0, "it leaves from where it was");
        assert_eq!(c.rect(jump + GLIDE_ALONG), bar(360.0, 0.0));

        // An edit and the move it makes: the bar is there in the same frame.
        let edit = jump + CYCLE;
        c.edited(edit);
        c.moved(bar(372.0, 0.0), Move::FollowsEdit, edit);
        assert_eq!(c.rect(edit), bar(372.0, 0.0));
        assert_eq!(c.alpha(edit), 1.0);
    }

    /// The width at every size the Type Piece offers, and the two iA measured
    /// by name. 0.152 would give 3 px at 23 px and 5 px at 36 px; the oracle's
    /// 0.155 gives 4 and 6, and the oracle's is what is drawn.
    #[test]
    fn the_width_is_whole_pixels_and_never_a_hairline() {
        const WIDTHS: [u32; 31] = [
            2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 6, 6, 6,
            6, 6,
        ];
        for (i, expected) in WIDTHS.iter().enumerate() {
            let size = 10 + i as u32;
            assert_eq!(width(size), *expected, "the bar at {size} px");
        }
        assert_eq!(width(20), 3, "iA's 3 px at 20 px");
        assert_eq!(width(31), 5, "iA's 5 px at 31 px");
        assert_eq!(width(23), 4, "0.155, not the 0.152 in the header comment");
        assert_eq!(width(36), 6, "the other size the two disagree at");
        assert_eq!(width(10), MIN_WIDTH, "never a hairline");
    }

    /// x lands on a whole device pixel; the other three are kept as they came.
    #[test]
    fn x_is_snapped_and_the_rest_is_kept() {
        let mut c = live();
        c.moved(
            Bar {
                x: 100.4,
                y: 36.0,
                w: 3.0,
                h: 36.0,
            },
            Move::Pointer,
            0,
        );
        assert_eq!(
            c.rect(0),
            Bar {
                x: 100.0,
                y: 36.0,
                w: 3.0,
                h: 36.0
            }
        );
        c.moved(
            Bar {
                x: 100.5,
                y: 36.0,
                w: 3.0,
                h: 36.0,
            },
            Move::FollowsEdit,
            MS,
        );
        assert_eq!(c.rect(MS).x, 101.0, "the half pixel rounds up");
    }

    /// The edit-snap rule, in the two places it decides: a move 100 ms after
    /// an edit is still the typing hand's, one 200 ms after it is not.
    #[test]
    fn a_move_that_follows_an_edit_snaps() {
        let mut c = live();
        c.moved(bar(0.0, 0.0), Move::Key, 0);
        c.edited(1000 * MS);
        c.moved(bar(50.0, 0.0), Move::FollowsEdit, 1000 * MS);

        let soon = 1100 * MS;
        c.moved(bar(200.0, 0.0), Move::Pointer, soon);
        assert_eq!(
            c.rect(soon).x,
            200.0,
            "100 ms after an edit it is put there"
        );
        assert!(!c.wants_tick(), "a snap asks for no frames");

        let later = 1200 * MS;
        c.moved(bar(400.0, 0.0), Move::Pointer, later);
        assert_eq!(c.rect(later).x, 200.0, "200 ms after an edit it travels");
        assert!(c.wants_tick(), "and asks for the frames to travel in");
        let half = c.rect(later + GLIDE_ALONG / 2).x;
        assert!(half > 200.0 && half < 400.0, "mid-glide at {half}");
        assert_eq!(c.rect(later + GLIDE_ALONG).x, 400.0);
        c.tick(later + GLIDE_ALONG);
        assert!(
            !c.wants_tick(),
            "the glide is over and the blink is still held"
        );
    }

    /// 34 ms along a row and 46 ms between rows, the oracle's `GLIDE_X` and
    /// `GLIDE_Y`.
    #[test]
    fn a_key_move_glides_along_a_row_and_across_rows() {
        assert_eq!(GLIDE_ALONG, 34 * MS);
        assert_eq!(GLIDE_ACROSS, 46 * MS);

        let mut c = live();
        c.moved(bar(0.0, 0.0), Move::Key, 0);

        let along = 1000 * MS;
        c.moved(bar(120.0, 0.0), Move::Key, along);
        assert!(
            c.rect(along + 33 * MS).x < 120.0,
            "still travelling at 33 ms"
        );
        assert_eq!(c.rect(along + GLIDE_ALONG), bar(120.0, 0.0));

        let across = 2000 * MS;
        c.moved(bar(10.0, 36.0), Move::Key, across);
        assert!(
            c.rect(across + GLIDE_ALONG).y < 36.0,
            "a row apart takes longer"
        );
        assert_eq!(c.rect(across + GLIDE_ACROSS), bar(10.0, 36.0));
    }

    /// A caret with nothing to animate asks the frame clock for nothing.
    #[test]
    fn an_idle_caret_asks_for_no_frames() {
        let mut c = live();
        c.moved(bar(0.0, 0.0), Move::Key, 0);
        c.tick(IDLE);
        assert!(c.wants_tick(), "the blink is running again");

        c.moved(bar(60.0, 0.0), Move::Key, IDLE);
        c.tick(IDLE + GLIDE_ALONG);
        assert!(!c.wants_tick(), "a move stopped the cycle");

        c.focus(false, IDLE + 2 * CYCLE);
        c.tick(IDLE + 3 * CYCLE);
        assert!(!c.wants_tick(), "an unfocused caret is a still ghost");
        assert_eq!(c.alpha(IDLE + 3 * CYCLE), GHOST);
    }

    /// `--deterministic` freezes the blink on and takes the glide away, so two
    /// shots of one state are the same pixels — except in the state that is
    /// about the window being away, where the ghost is what is judged.
    #[test]
    fn deterministic_is_frozen_on_and_never_travels() {
        let mut d = Caret::new(Mode::Deterministic);
        d.moved(bar(0.0, 0.0), Move::Key, 0);
        d.tick(3 * CYCLE);
        assert_eq!(d.alpha(3 * CYCLE), 1.0);
        assert!(!d.wants_tick());

        d.moved(bar(500.0, 0.0), Move::Pointer, 3 * CYCLE);
        assert_eq!(
            d.rect(3 * CYCLE),
            bar(500.0, 0.0),
            "put there, not walked there"
        );

        d.focus(false, 4 * CYCLE);
        assert_eq!(
            d.alpha(4 * CYCLE),
            GHOST,
            "the ghost the unfocused shot judges"
        );
    }

    /// `--nocaret` is nothing at all, in every state and at every time.
    #[test]
    fn nocaret_is_never_drawn() {
        let mut n = Caret::new(Mode::Nocaret);
        n.moved(bar(0.0, 0.0), Move::Key, 0);
        assert_eq!(n.alpha(0), 0.0);
        n.moved(bar(400.0, 0.0), Move::Pointer, CYCLE);
        assert_eq!(n.alpha(CYCLE), 0.0);
        assert_eq!(n.rect(CYCLE), bar(400.0, 0.0), "and never travels");
        assert!(!n.wants_tick());
        n.tick(4 * CYCLE);
        assert_eq!(n.alpha(4 * CYCLE), 0.0);
        assert!(!n.wants_tick());
    }

    /// The same position arrives twice per keystroke, once for the render and
    /// once for the selection. The second must not restart the first's glide.
    #[test]
    fn the_second_of_a_pair_is_dropped() {
        let mut c = live();
        c.moved(bar(0.0, 0.0), Move::Key, 0);

        let jump = 1000 * MS;
        c.moved(bar(200.0, 0.0), Move::Key, jump);
        let mid = jump + GLIDE_ALONG / 2;
        let travelled = c.rect(mid).x;
        assert!(travelled < 200.0, "still on its way at {travelled}");

        c.moved(bar(200.0, 0.0), Move::Key, mid);
        assert_eq!(c.rect(mid).x, travelled, "it carried on from where it was");
        assert_eq!(
            c.rect(jump + GLIDE_ALONG).x,
            200.0,
            "and lands when it would have"
        );
        assert_eq!(c.alpha(mid), 1.0, "and the blink is held either way");
    }

    /// A held arrow repeats faster than a glide lasts, so the repeats snap:
    /// `SNAP_MS`, or the caret is towed along behind the key.
    #[test]
    fn a_key_at_its_repeat_rate_snaps() {
        let mut c = live();
        c.moved(bar(0.0, 0.0), Move::Key, 0);

        // Four ems a hop, so that what is being tested is the repeat rate and
        // not [`GLIDE_MIN`]: a word jump rather than a single cell.
        let first = 1000 * MS;
        c.moved(bar(80.0, 0.0), Move::Key, first);
        assert!(c.wants_tick(), "the first of a run travels");

        let repeat = first + 33 * MS;
        c.moved(bar(160.0, 0.0), Move::Key, repeat);
        assert_eq!(
            c.rect(repeat).x,
            160.0,
            "the next is a repeat, and is put there"
        );
        assert!(!c.wants_tick());
    }

    /// `--nocaret` wins over `--deterministic`: a caret that is not drawn has
    /// nothing left to freeze.
    #[test]
    fn the_flags_pick_the_mode() {
        let mut f = Flags::default();
        assert_eq!(Mode::from_flags(&f), Mode::Live);
        f.deterministic = true;
        assert_eq!(Mode::from_flags(&f), Mode::Deterministic);
        f.nocaret = true;
        assert_eq!(Mode::from_flags(&f), Mode::Nocaret);
    }

    /// The glide is the oracle's curve rather than a straight line: fast away,
    /// soft into the landing.
    #[test]
    fn the_glide_is_eased_as_the_oracle_eases() {
        assert_eq!(eased(0.0), 0.0);
        assert_eq!(eased(1.0), 1.0);

        let mut last = 0.0;
        for i in 1..=100 {
            let p = f64::from(i) / 100.0;
            let e = eased(p);
            assert!(e > last, "the curve only ever goes forward, at {i}%");
            assert!(e <= 1.0, "and never overshoots, at {i}%");
            last = e;
        }

        let half = eased(0.5);
        assert!(
            half > 0.8 && half < 0.95,
            "most of the way at half the time, at {half}"
        );
    }

    /// The whole of the classification, with no widget to drive: what last
    /// reached the Editor, and whether the buffer changed in this frame.
    #[test]
    fn the_kind_is_the_controller_and_the_frame() {
        // A change in this frame is the whole answer, whichever key or button
        // made it: the glyph is on the glass and the bar has to be beside it.
        for last in [Source::Pointer, Source::Key, Source::App] {
            assert_eq!(kind(last, true), Move::FollowsEdit, "{last:?} and an edit");
        }
        assert_eq!(kind(Source::Pointer, false), Move::Pointer);
        assert_eq!(kind(Source::Key, false), Move::Key);
        // Nothing drove it: the app placed the caret itself — a launch flag, a
        // restored position — and it is put there rather than travelled to.
        assert_eq!(kind(Source::App, false), Move::FollowsEdit);
        assert_eq!(
            Source::default(),
            Source::App,
            "and that is where it starts"
        );
    }

    /// A typed character: the bar is beside the glyph in the same frame, and
    /// there is nothing left to animate.
    #[test]
    fn a_typed_character_snaps_beside_the_glyph() {
        let mut c = live();
        c.moved(bar(100.0, 0.0), Move::Pointer, 0);

        let typed = 1000 * MS;
        c.edited(typed);
        c.moved(bar(112.0, 0.0), kind(Source::Key, true), typed);
        assert_eq!(c.rect(typed), bar(112.0, 0.0), "beside it, this frame");
        assert!(!c.wants_tick(), "and no frame is asked for to get it there");
    }

    /// The three gates the type size buys, at the 20 px [`bar`] measures: one
    /// em is 20 device pixels and one pitch is 36 of them.
    #[test]
    fn a_hop_glides_only_when_it_can_be_followed() {
        // A caret that has been still for a second, so that neither the
        // repeat rate nor an edit is what is being measured.
        let hop = |to: Bar| {
            let mut c = live();
            c.moved(bar(400.0, 0.0), Move::Pointer, 0);
            c.moved(to, Move::Pointer, 1000 * MS);
            c.wants_tick()
        };

        assert!(!hop(bar(459.0, 0.0)), "59 px is under three ems");
        assert!(hop(bar(460.0, 0.0)), "60 px is three ems exactly");
        assert!(hop(bar(680.0, 0.0)), "280 px is fourteen ems exactly");
        assert!(!hop(bar(681.0, 0.0)), "281 px is a jump, not a move");

        // Down the page a row is worth following however little the column
        // moved, and more than 1.2 pitches is a new place rather than a move.
        assert!(hop(bar(401.0, 36.0)), "the next row, one pixel across");
        assert!(hop(bar(400.0, 43.0)), "43 px is inside 1.2 pitches");
        assert!(!hop(bar(400.0, 44.0)), "44 px is past them");

        // A caret that has not been told its size cannot measure a hop along
        // a row, and does not guess.
        let mut unsized_caret = Caret::new(Mode::Live);
        unsized_caret.focus(true, 0);
        unsized_caret.moved(bar(400.0, 0.0), Move::Pointer, 0);
        unsized_caret.moved(bar(460.0, 0.0), Move::Pointer, 1000 * MS);
        assert!(!unsized_caret.wants_tick());
    }

    /// The quiet asks for no frames, and for exactly one at the end of it.
    ///
    /// The two together are what the Editor's tick source is added and
    /// dropped on: a caret with neither costs the frame clock nothing.
    #[test]
    fn the_quiet_asks_for_one_frame_at_its_end() {
        let mut c = live();
        c.edited(0);
        c.moved(bar(100.0, 0.0), Move::FollowsEdit, 0);
        assert!(!c.wants_tick(), "nothing to animate inside the quiet");
        assert_eq!(c.resumes_at(), Some(IDLE), "and the blink comes back here");

        c.tick(479 * MS);
        assert_eq!(c.resumes_at(), Some(IDLE), "still, a millisecond short");

        c.tick(IDLE);
        assert!(c.wants_tick(), "the blink is running now");
        assert_eq!(c.resumes_at(), None, "so there is nothing to come back for");

        // A frozen caret and an undrawn one have neither: `--deterministic`
        // and `--nocaret` idle without asking the frame clock for anything.
        for mode in [Mode::Deterministic, Mode::Nocaret] {
            let mut still = Caret::new(mode);
            still.edited(0);
            still.moved(bar(100.0, 0.0), Move::FollowsEdit, 0);
            assert!(!still.wants_tick(), "{mode:?} wants no frame");
            assert_eq!(still.resumes_at(), None, "{mode:?} wants none later");
        }
    }

    /// The bar stands in the gap after the glyph rather than on its last
    /// column, which is what makes it read as being between two letters.
    #[test]
    fn the_bar_is_nudged_off_the_advance_boundary() {
        assert!((nudge(20) - 1.4).abs() < 1e-9, "1.4 px at 20 px type");
        assert!((nudge(40) - 2.8).abs() < 1e-9, "2.8 px at 40 px type");
    }

    /// The band hangs from the baseline at iA's own share, which is what keeps
    /// it centred on the ink rather than on the font's em box.
    #[test]
    fn the_band_carries_eleven_sixteenths_of_the_pitch_above_the_baseline() {
        // 0.6875 * 36 = 24.75 above the baseline at the judged size, leaving
        // 11.25 below it.
        assert!(
            (band_top(100.0, 36.0) - 75.25).abs() < 1e-9,
            "a 36 px pitch on a baseline at 100 does not start at 75.25"
        );
        for pitch in [26.0, 36.0, 52.0, 92.5] {
            let above = 100.0 - band_top(100.0, pitch);
            assert!(
                (above / pitch - 0.6875).abs() < 1e-9,
                "a {pitch} px pitch puts {above} px above the baseline, not 11/16 of itself"
            );
        }
    }
}
