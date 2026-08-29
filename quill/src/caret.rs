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

#![allow(dead_code)] // The Editor reads all of this in #107; nothing calls it yet.

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
            ..to
        };
        if self.placed && to == self.to {
            return;
        }
        let at = self.rect(t);
        self.glide = if self.glides(kind, t) && (at.x != to.x || at.y != to.y) {
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

    /// Whether a move of this kind at `t` is one to be tracked by eye.
    ///
    /// The oracle's gate is five conditions in `placeCaret`, and these are the
    /// ones that survive being handed device pixels: `EDIT_SNAP_MS`, which
    /// #106 names, and `SNAP_MS`, which keeps a repeat rate from towing the
    /// caret. The two left out are `GLIDE_MIN` — hops under 3 em snap, there
    /// being nothing to follow over 2 mm — and the 14 em and 1.2 pitch caps
    /// that snap a jump too long to track. Both are measured in ems, and the
    /// machine is given a rectangle rather than a type size; #107 hands the
    /// widget both, and is where they can arrive.
    fn glides(&self, kind: Move, t: i64) -> bool {
        if !self.placed || kind == Move::FollowsEdit || self.mode != Mode::Live {
            return false;
        }
        if self.moved_at.is_some_and(|m| t - m < SNAP) {
            return false;
        }
        self.edited_at.is_none_or(|e| t - e >= EDIT_SNAP)
    }
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
/// The row's top and the pitch arrive whole from `quill_engine::typography`,
/// so x is the only one of the four that needs this — and it does need it: a
/// bar starting on a half pixel is rasterised a column wider than it was cut,
/// with a grey edge standing in for the half.
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

    /// A caret in a window that has just become active.
    fn live() -> Caret {
        let mut c = Caret::new(Mode::Live);
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

        // A jump: no edit for two cycles, so the caret travels there.
        let jump = IDLE + 2 * CYCLE;
        c.moved(bar(400.0, 0.0), Move::Pointer, jump);
        assert_eq!(c.alpha(jump), 1.0, "a move holds the blink on");
        assert_eq!(c.rect(jump).x, 100.0, "it leaves from where it was");
        assert_eq!(c.rect(jump + GLIDE_ALONG), bar(400.0, 0.0));

        // An edit and the move it makes: the bar is there in the same frame.
        let edit = jump + CYCLE;
        c.edited(edit);
        c.moved(bar(412.0, 0.0), Move::FollowsEdit, edit);
        assert_eq!(c.rect(edit), bar(412.0, 0.0));
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

        let first = 1000 * MS;
        c.moved(bar(12.0, 0.0), Move::Key, first);
        assert!(c.wants_tick(), "the first of a run travels");

        let repeat = first + 33 * MS;
        c.moved(bar(24.0, 0.0), Move::Key, repeat);
        assert_eq!(
            c.rect(repeat).x,
            24.0,
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
}
