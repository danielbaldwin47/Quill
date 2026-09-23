//! The caret's blink and its glide, with no widget in it.
//!
//! The Parity oracle keeps the two in `dev/legacy/app/js/caret.js` and
//! `dev/legacy/app/css/caret.css`, as a CSS animation, a CSS transition and a
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
//! the glyph, so the bar spans the split leading the way iA's does, and its
//! width from that module's ladder. This module keeps what it is given and
//! does two pieces of arithmetic on it: the bar's left edge from the advance
//! boundary it is centred on, and the snap of an edge onto a whole device
//! pixel.
//!
//! Nothing here is a widget, and nothing here is `gtk`.

use crate::flags::Flags;

/// A millisecond, in the microseconds every time in this module is counted in.
const MS: i64 = 1_000;

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

/// The bar full on, at the top of the blink's cycle — and the whole of the
/// hold a key buys.
///
/// Every edit and every move puts the cycle back to its top
/// ([`Caret::alpha`]), so a hand typing at any pace quicker than this never
/// sees the bar leave full strength, and a hand that stops sees it fade one
/// ramp after this runs out. That is the machine
/// `dev/ref/ia/mac-native/NOTES.md` § State 5 describes — "the caret is held on
/// for one full on-phase after the last key before the cadence starts again"
/// — and it is why there is no suppression constant beside this one.
///
/// `docs/design.md` § Blink says the same thing from outside, as "resumes
/// 0.633 s after the last key": `blink-typing.tsv` marks the last key at
/// 5.432 s, holds full strength to 5.96 and is dark from 6.06, so last key to
/// dark is 0.63 — this 0.516 plus one [`FADE_OUT`]. A hold of 0.633 *before*
/// the cycle restarted would keep the bar solid to 1.15 s, which is not what
/// the trace shows.
const ON: i64 = 516 * MS;

/// The fade down, one of the two ~0.09 s ramps between the two thresholds
/// `dev/ref/ia/mac-native/NOTES.md` § State 4 reads the trace at.
const FADE_OUT: i64 = 90 * MS;

/// The dark half of the cycle: 0.305 s measured, rounded to keep the turn
/// exactly 1.000 s.
const OFF: i64 = 304 * MS;

/// The fade back up, the ramp down's twin.
const FADE_IN: i64 = 90 * MS;

/// One turn of the blink: 1.000 s.
///
/// The Design oracle's, measured under a hand. `dev/ref/ia/mac-native/blink-idle.tsv`
/// samples the bar's accent pixels at 103 Hz over twelve seconds of a window
/// nobody is touching, and `dev/ref/ia/mac-native/NOTES.md` § State 4 reads it at
/// two thresholds: at **full strength** the bar is on 0.516 s and off 0.484 s
/// to a 1.000 s period, and at **any accent pixel at all** it is on 0.691 s
/// and off 0.305 s. Those two rows are what fix all four phases. The gap
/// between them is the fade — "about 0.09 s at each edge", and the waveform
/// is 426 → 356 → 0 and back rather than a hard switch — so the 0.516 is
/// [`ON`] alone and the 0.484 is the ramp down, the 0.305 the bar is truly
/// dark, and the ramp back. `docs/design.md` § Blink carries the
/// full-strength row.
///
/// Reading the 0.516 as lit-to-dark instead — [`ON`] plus the ramp — would
/// put the full-strength plateau at 0.426 and the dark at 0.394, and the
/// trace shows 0.511 and 0.303.
///
/// The Parity oracle's `caret.css` runs a 1.06 s keyframe at `0%,44% {1}
/// 52%,94% {0} 100% {1}` — 466.4, 84.8, 445.2 and 63.6 — which is where these
/// four stood until #169: a turn 55 ms longer, 46 ms less of it at full
/// strength and 141 ms more of it dark. The oracle that owns the row is the
/// one measured.
const CYCLE: i64 = ON + FADE_OUT + OFF + FADE_IN;

/// The share of the pitch the band carries above the baseline: 11/16, leaving
/// 31.25 % below it.
///
/// `ABOVE` in `dev/legacy/app/js/caret.js`, which is where this number is of
/// record: its header takes the caret's geometry from `dev/ref/ia/REFERENCE.md`
/// § 4.1 plus a re-measurement of its own on `appstore-mac-01` and
/// `msstore-win-01`, and the share is one of the numbers that re-measurement
/// added — § 4.1 itself records only the width, the height and that the bar
/// sits flush after the last glyph.
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
    /// The ladder's width at the session's step, in device pixels:
    /// `quill_engine::typography::caret_width`.
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
    /// (`dev/progress/latency-report.md` §5). So this one never glides.
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

/// Whether a move made by `source` while `held` says a button is down owes
/// its follow to the release rather than paying it now.
///
/// Nothing the writer can see moves under a held button. GTK's text view runs
/// a selection drag from the press, and after a scroll re-lays the view out
/// the window hands it a synthetic motion at the pointer's unmoved glass
/// position, which now sits over different text — so a row glided into the
/// band under a held button reads as a drag across whatever slid past. The
/// fold already stands still for the same reason
/// ([`crate::editor::Editor::released`]), and this is that rule for the
/// caret's band (#263, the third Hand test round).
///
/// [`Source::App`] is out, as it is out of every band rule: what the app put
/// there is not something the pointer is dragging.
#[must_use]
pub fn waits_for_release(source: Source, held: bool) -> bool {
    held && source != Source::App
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
    /// Whether the buffer holds a selection.
    ///
    /// While it does there is no caret at all: the two bars at the selection's
    /// ends are the instrument, and a third bar blinking somewhere inside the
    /// held cells would read as a second cursor. `place()` in
    /// `dev/legacy/app/js/caret.js` hides it outright for the same reason.
    selected: bool,
    /// Where the bar is going, or already is; its x is snapped.
    to: Bar,
    /// Whether the bar has been put anywhere yet. The first placement never
    /// glides, and neither does the first after the window comes back — the
    /// oracle drops its `prev` at both.
    placed: bool,
    glide: Option<Glide>,
    /// The last move or edit, which is the top of the blink's cycle: the bar
    /// is at full strength for [`ON`] after it.
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
            selected: false,
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

    /// The buffer holds a selection at `t`, or has stopped holding one.
    ///
    /// The caret is not drawn while it does, and asks for no frames: the
    /// selection's own two bars do not blink, so there is nothing left to
    /// animate.
    ///
    /// Releasing one holds the blink on, the way a move or an edit does. A
    /// selection usually collapses by the insert mark moving, which would hold
    /// it through [`Caret::moved`] anyway — but it can collapse by the other
    /// end moving onto the insert mark instead, which is a shift-arrow back to
    /// the anchor, and a caret that came back mid-cycle would flash on the
    /// keystroke that released it.
    pub fn selected(&mut self, yes: bool, t: i64) {
        self.now = t;
        if self.selected && !yes {
            self.active_at = Some(t);
        }
        self.selected = yes;
    }

    /// Whether the window this caret is in is active.
    ///
    /// The selection's two end bars go ink when it is not, where the caret
    /// goes to a blue ghost, so the paint has to ask.
    #[must_use]
    pub fn focused(&self) -> bool {
        self.focused
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
    /// The four that are not the blink come first, in the order they beat each
    /// other: nothing drawn at all, then the selection that replaces the caret
    /// with its own two bars, then the ghost the unfocused shot judges, then
    /// the frozen bar `--deterministic` asks for. The selection beats the
    /// ghost because an unfocused selection is drawn as a held block with ink
    /// ends, and a blue ghost floating in it would be a third mark.
    #[must_use]
    pub fn alpha(&self, t: i64) -> f64 {
        if self.mode == Mode::Nocaret {
            return 0.0;
        }
        if self.selected {
            return 0.0;
        }
        if !self.focused {
            return GHOST;
        }
        if self.mode == Mode::Deterministic {
            return 1.0;
        }
        // The last edit or move is the top of the cycle, which is the whole of
        // how the blink is held while a hand types: keys closer together than
        // [`ON`] keep putting it back before it has begun to fade. A caret
        // that has not moved at all has no cycle to be in and is simply lit.
        match self.active_at {
            Some(a) => blink(t - a),
            None => 1.0,
        }
    }

    /// Whether the Editor has to ask for another frame.
    ///
    /// True only while something is actually moving: a glide in flight, or one
    /// of the blink's two ramps ([`ramping`]). A caret with nothing to animate
    /// — unfocused, frozen, not drawn, held on by a hand that is still moving,
    /// or sitting on either of the blink's plateaus — costs the frame clock
    /// nothing. The Latency Piece rests on that (#41 § Frame clock), and on
    /// more than the quiet after a key: a tick callback keeps GDK's frame clock
    /// requesting a phase, so a redraw asked for while one is attached joins
    /// the next slot on the refresh grid rather than painting at once (#327).
    /// Off the ramps there is no callback to pace it.
    #[must_use]
    pub fn wants_tick(&self) -> bool {
        if self.mode != Mode::Live || !self.focused || self.selected {
            return false;
        }
        self.glide.is_some_and(|g| self.now < g.at + g.len)
            || self.active_at.is_some_and(|a| ramping(self.now - a))
    }

    /// When the Editor has to come back, if not on the very next frame.
    ///
    /// [`Caret::wants_tick`] is false on both of the blink's plateaus — the
    /// [`ON`] a move or an edit puts the cycle back to and every later one,
    /// and the [`OFF`] the bar is dark for — because the alpha does not change
    /// inside either, and the Latency Piece is paid for in the frames that are
    /// never asked for. A ramp does come after each of them, though, and a
    /// widget that let its tick source go with nothing to ask for the frame
    /// that resumes it would blink no further. This is that instant
    /// ([`next_ramp`]), on the caller's own clock. It is `None` whenever
    /// `wants_tick` is already true, and whenever there is no blink coming at
    /// all.
    ///
    /// The last tick of a ramp draws the plateau's own alpha — 0.0 at the foot
    /// of [`FADE_OUT`], 1.0 at the head of [`FADE_IN`] — so nothing is missed
    /// between the source going and coming back.
    #[must_use]
    pub fn resumes_at(&self) -> Option<i64> {
        if self.mode != Mode::Live || !self.focused || self.selected || self.wants_tick() {
            return None;
        }
        self.active_at.map(|a| a + next_ramp(self.now - a))
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

/// Where the bar's top sits, given the row's baseline and the pitch, in the
/// pixels the widget lays out in.
///
/// `snap(b.top + M.base - ABOVE * M.pitch)` in `caret.js`: the band is
/// [`ABOVE_BASELINE`] of the pitch above the baseline and the rest below it.
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

/// The bar's left edge, from the advance boundary it stands on and its width.
///
/// The bar is **centred** on the boundary. iA Writer for Mac puts 3 px of its
/// 6 px bar each side of it, at three offsets and in both themes
/// (`dev/ref/ia/mac-native/VERDICTS.md` 0013.1–0013.3), and `docs/design.md` row
/// Caret column takes that over the Parity oracle, whose left edge sits on the
/// boundary and whose bar therefore reads as standing on the glyph that
/// follows — which is what
/// [#147](https://github.com/danielbaldwin47/Quill/issues/147) reports.
///
/// A width that will not split evenly gives the extra pixel to the right of
/// the boundary, so the boundary is always inside the bar and never its left
/// edge. Device pixels, like everything else in a [`Bar`]: the caller asked
/// `quill_engine::typography::caret_width` for the width in them already, and
/// at scale 2 every width on the ladder is even.
#[must_use]
pub fn left(boundary: f64, w: f64) -> f64 {
    boundary - (w / 2.0).floor()
}

/// One edge of the bar, onto a whole device pixel.
///
/// The pitch arrives whole from `quill_engine::typography`, but neither edge
/// the bar is placed by does: x is half a bar's width left of the advance
/// boundary ([`left`]) and y hangs from a baseline at [`ABOVE_BASELINE`] of
/// the pitch, and both land wherever that arithmetic leaves them. Both need
/// this: a bar starting on a half pixel is rasterised a row or a column wider
/// than it was cut, with a grey edge standing in for the half. `caret.js`
/// snaps its top and its left for the same reason.
///
/// Device pixels are the caller's: the widget applies the surface's scale
/// factor on the way in, which is where the oracle's `Math.round(v * dpr) /
/// dpr` went. Handing this logical pixels on a scale-2 output would snap to
/// every second device pixel and leave the fault it exists to prevent.
#[must_use]
pub fn snap(x: f64) -> f64 {
    x.round()
}

/// The blink's alpha, `elapsed` microseconds into a cycle that began with the
/// bar on.
fn blink(elapsed: i64) -> f64 {
    let p = elapsed.rem_euclid(CYCLE);
    match Phase::at(p) {
        Phase::Lit => 1.0,
        Phase::FadeOut => 1.0 - (p - Phase::Lit.ends_at()) as f64 / FADE_OUT as f64,
        Phase::Dark => 0.0,
        Phase::FadeIn => (p - Phase::Dark.ends_at()) as f64 / FADE_IN as f64,
    }
}

/// The four phases one turn of the blink runs, in the order it runs them.
///
/// The turn is [`ON`], [`FADE_OUT`], [`OFF`], [`FADE_IN`], and the three things
/// asked of it — how opaque the bar is, whether that is changing, and where the
/// phase ends — are all read off this one walk, so that a boundary moved in
/// [`CYCLE`]'s four constants moves in all three answers at once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    /// The bar solid, at the top of the turn.
    Lit,
    /// The ramp down.
    FadeOut,
    /// The bar dark.
    Dark,
    /// The ramp back up.
    FadeIn,
}

impl Phase {
    /// The phase at `p`, an offset inside one [`CYCLE`].
    fn at(p: i64) -> Self {
        if p < ON {
            Self::Lit
        } else if p < ON + FADE_OUT {
            Self::FadeOut
        } else if p < ON + FADE_OUT + OFF {
            Self::Dark
        } else {
            Self::FadeIn
        }
    }

    /// Whether the bar's alpha changes through it, which is the whole of what
    /// a blink has to be drawn for: the two ramps, and neither plateau.
    fn changes(self) -> bool {
        matches!(self, Self::FadeOut | Self::FadeIn)
    }

    /// Where it ends, as an offset into the turn — which is where the next
    /// phase begins.
    fn ends_at(self) -> i64 {
        match self {
            Self::Lit => ON,
            Self::FadeOut => ON + FADE_OUT,
            Self::Dark => ON + FADE_OUT + OFF,
            Self::FadeIn => CYCLE,
        }
    }
}

/// Whether the bar's alpha is changing `elapsed` microseconds into a cycle
/// that began with the bar on.
///
/// The predicate [`Caret::wants_tick`] answers with. A negative `elapsed` is a
/// caller whose clock is behind the move that put the cycle back to its top:
/// the top has not been reached, so nothing is changing yet.
fn ramping(elapsed: i64) -> bool {
    elapsed >= 0 && Phase::at(elapsed.rem_euclid(CYCLE)).changes()
}

/// The start of the next ramp after `elapsed`, as an elapsed time of its own.
///
/// Asked only where [`ramping`] is false, which is on one of the two plateaus,
/// and the end of the plateau is the start of the ramp. Strictly later than
/// `elapsed` on either — a phase ends after every offset inside it — so the
/// instant [`Caret::resumes_at`] hands back is always still to come. A
/// negative `elapsed` has the whole of the first [`ON`] still ahead of it.
fn next_ramp(elapsed: i64) -> i64 {
    if elapsed < 0 {
        return ON;
    }
    let p = elapsed.rem_euclid(CYCLE);
    elapsed - p + Phase::at(p).ends_at()
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

    /// A bar at 20 px type, which is a size off the ladder rather than a step
    /// of it: these are the geometry's own numbers, and the pitch and width
    /// that go with them are 36 and 3. Nothing here reads the width — these
    /// tests are the blink's and the glide's — so it is written out rather
    /// than asked of the ladder, which measures a step and not a size.
    fn bar(x: f64, y: f64) -> Bar {
        Bar {
            x,
            y,
            w: 3.0,
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

    /// The timeline #38 § Testing Decisions asks for: type, wait out the
    /// on-phase, wait a microsecond more, jump, edit and then move.
    #[test]
    fn the_scripted_timeline() {
        let mut c = live();

        // Type. The glyph is on the glass this frame, so the bar is beside it
        // this frame, and the blink is held while the hand is moving.
        c.edited(0);
        c.moved(bar(100.0, 0.0), Move::FollowsEdit, 0);
        assert_eq!(c.rect(0), bar(100.0, 0.0));
        assert_eq!(c.alpha(0), 1.0);

        // A microsecond short of the on-phase's end, nothing has begun to
        // move, so the frame clock is still owed nothing.
        c.tick(ON - 1);
        assert_eq!(c.alpha(ON - 1), 1.0);
        assert!(!c.wants_tick(), "the bar is still at full strength");

        // The end of it, and the fade is under way.
        c.tick(ON);
        assert!(c.wants_tick(), "the blink is running again at 516 ms");
        assert_eq!(c.alpha(ON), 1.0, "the fade has not moved yet");
        assert_eq!(c.alpha(ON + FADE_OUT / 2), 0.5, "half way down");
        assert_eq!(c.alpha(ON + FADE_OUT), 0.0, "dark");
        assert_eq!(
            c.alpha(ON + FADE_OUT + OFF),
            0.0,
            "the fade back starts here"
        );
        assert_eq!(
            c.alpha(ON + FADE_OUT + OFF + FADE_IN / 2),
            0.5,
            "half way up"
        );
        assert_eq!(c.alpha(CYCLE), 1.0, "and round again");

        // A jump: no edit for two cycles, so the caret travels there. Thirteen
        // ems of it, inside both the [`GLIDE_MIN`] floor and the [`GLIDE_FAR`]
        // cap, which is a hop the eye can follow.
        let jump = 2 * CYCLE;
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

    /// The turn of the blink the Design oracle was measured at: 1.000 s, of
    /// which the bar is at full strength for 0.516 s and truly dark for
    /// 0.305 s, with a fade of about 0.09 s between them each way.
    ///
    /// `dev/ref/ia/mac-native/blink-idle.tsv` is the trace it is read off — the
    /// bar's accent pixels at 103 Hz over twelve seconds of a window nobody
    /// is touching — and `dev/ref/ia/mac-native/NOTES.md` § State 4 is the reading
    /// of it, at the two thresholds that fix all four phases. The cadence is a
    /// pure function of the time since the cycle began, so this is the whole
    /// of it: no widget, no clock and no caret.
    #[test]
    fn the_cadence_the_design_oracle_measured() {
        assert_eq!(CYCLE, 1_000 * MS, "NOTES § State 4: a 1.000 s turn");
        assert_eq!(ON, 516 * MS, "at full strength, on 0.516 s");
        assert_eq!(FADE_OUT + OFF + FADE_IN, 484 * MS, "and off 0.484 s");
        assert_eq!(OFF, 304 * MS, "of which 0.305 s is no accent pixel at all");

        assert_eq!(blink(0), 1.0, "on at 0");
        assert_eq!(blink(515 * MS), 1.0, "full strength to the last of the on");
        assert_eq!(blink(ON), 1.0, "0.516 s in, where the fade down begins");
        assert_eq!(blink(561 * MS), 0.5, "half way down");
        assert_eq!(blink(606 * MS), 0.0, "dark once the fade has run");
        assert_eq!(blink(909 * MS), 0.0, "and dark to the last of the 0.305");
        assert_eq!(blink(955 * MS), 0.5, "half way back up");
        assert_eq!(blink(1_000 * MS), 1.0, "on again at 1.000 s");

        // The turn after it, and the one before: the cycle is `rem_euclid`, so
        // it runs backwards from a caret whose cycle began after the frame.
        assert_eq!(blink(CYCLE + 606 * MS), 0.0, "the second turn's dark");
        assert_eq!(blink(-CYCLE), 1.0, "and a turn the other way");
    }

    /// The bar is held solid while a hand is typing, and dark 0.633 s after
    /// the last key.
    ///
    /// `dev/ref/ia/mac-native/NOTES.md` § State 5 is the reading of
    /// `blink-typing.tsv`: through the typing window the bar is "solid on for
    /// 3.448 s — no blink at all", and after the last key it is "held on for
    /// one full on-phase before the cadence starts again". So the hold is not
    /// a constant of its own — every key puts the cycle back to its top, and
    /// keys closer together than [`ON`] never let it start to fade. What that
    /// looks like from outside is the 0.633 s `docs/design.md` § Blink names:
    /// the trace's last key is at 5.432 s and its first dark sample at 6.065.
    #[test]
    fn the_blink_is_held_while_a_hand_types() {
        let mut c = live();

        // Ten keys 180 ms apart, which is the pace NOTES § State 5 measured
        // the oracle at. The bar never leaves full strength between them.
        let mut last = 0;
        for i in 0..10 {
            last = i * 180 * MS;
            c.edited(last);
            assert_eq!(c.alpha(last), 1.0, "lit as the key lands");
            assert_eq!(c.alpha(last + 179 * MS), 1.0, "and lit until the next");
        }

        // After the last one: one full on-phase, then the fade, then dark —
        // 0.606 s from key to paper, which is the 0.63 the trace shows to
        // within the 10 ms it was sampled at and the 180 ms the keys came at.
        assert_eq!(c.alpha(last + ON - 1), 1.0, "full strength through the on");
        assert_eq!(c.alpha(last + ON), 1.0, "and at the instant it runs out");
        assert_eq!(c.alpha(last + 561 * MS), 0.5, "fading a fade later");
        assert_eq!(c.alpha(last + 606 * MS), 0.0, "dark 0.606 s after the key");
        assert_eq!(c.alpha(last + CYCLE), 1.0, "and lit again a turn on");

        // Where a 0.633 s hold *before* the cycle restarted would differ: it
        // would still be at full strength here, three quarters of a second
        // after the hand stopped, and the oracle is dark.
        assert_eq!(c.alpha(last + 750 * MS), 0.0, "dark at 0.75 s, not held");
    }

    /// The bar is centred on the advance boundary, which is what the Design
    /// oracle measures and what `docs/design.md` row Caret column decides. The
    /// Mac app's own numbers are the first case: a 6 px bar on the boundary at
    /// 819.0 runs 816…821, so its left edge is 3 px before the boundary.
    #[test]
    fn the_bar_is_centred_on_the_advance_boundary() {
        assert_eq!(
            left(819.0, 6.0),
            816.0,
            "VERDICTS 0013.1, state 01 mid-word"
        );
        assert_eq!(left(691.0, 6.0), 688.0, "the offset-00 frame");
        assert_eq!(left(947.0, 6.0), 944.0, "the offset-10 frame");
        assert_eq!(left(1203.0, 6.0), 1200.0, "the offset-20 frame");
    }

    /// An odd width cannot split evenly, so the bar takes the fewer pixels on
    /// the left and the extra one falls right of the boundary. The ladder is
    /// even at scale 2 all the way up, so this is the odd scale-1 case — 5 px
    /// at scale 2 is 3 px at scale 1 — and `quill_engine::typography`'s own
    /// tests are where those halvings are pinned.
    #[test]
    fn an_odd_bars_extra_pixel_falls_right_of_the_boundary() {
        // The left edge a bar of each odd width takes on the boundary at 100,
        // written out rather than worked out: 1, 2 and 3 px of the bar fall
        // left of the boundary and 2, 3 and 4 px right of it.
        const ODD: [(f64, f64); 3] = [(3.0, 99.0), (5.0, 98.0), (7.0, 97.0)];
        for (w, x) in ODD {
            assert_eq!(left(100.0, w), x, "the {w} px bar's left edge");
            assert!(100.0 - x < w - (100.0 - x), "the {w} px bar leans right");
        }
    }

    /// Whatever the width, the bar covers the boundary itself rather than
    /// starting past it — the whole of what #147 reports.
    #[test]
    fn the_boundary_is_always_inside_the_bar() {
        for w in 2..=12 {
            let w = f64::from(w);
            let x = left(1896.0, w);
            assert!(x < 1896.0, "{w} px starts at {x}, on or past the boundary");
            assert!(x + w > 1896.0, "{w} px ends at {} px, short of it", x + w);
        }
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
        c.tick(ON);
        assert!(c.wants_tick(), "the blink is running again");

        c.moved(bar(60.0, 0.0), Move::Key, ON);
        c.tick(ON + GLIDE_ALONG);
        assert!(!c.wants_tick(), "a move put the cycle back to its top");

        c.focus(false, ON + 2 * CYCLE);
        c.tick(ON + 3 * CYCLE);
        assert!(!c.wants_tick(), "an unfocused caret is a still ghost");
        assert_eq!(c.alpha(ON + 3 * CYCLE), GHOST);
    }

    /// A selection takes the caret away entirely, and the frames with it.
    #[test]
    fn a_selection_puts_the_caret_out() {
        let mut c = live();
        c.moved(bar(60.0, 0.0), Move::Key, 0);
        assert_eq!(c.alpha(0), 1.0);

        c.selected(true, 0);
        assert_eq!(c.alpha(0), 0.0, "the two end bars are the instrument now");
        c.tick(ON + CYCLE);
        assert_eq!(c.alpha(ON + CYCLE), 0.0, "and it does not blink back");
        assert!(!c.wants_tick(), "nothing left to animate");
        assert_eq!(c.resumes_at(), None, "nor a blink to come back for");

        // Unfocused, it is still out: a ghost floating inside the held cells
        // would read as a second cursor beside the ends.
        c.focus(false, ON + CYCLE);
        assert_eq!(c.alpha(ON + CYCLE), 0.0);

        c.focus(true, 2 * CYCLE);
        c.selected(false, 2 * CYCLE);
        assert_eq!(c.alpha(2 * CYCLE), 1.0, "and comes back when it collapses");
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
        assert_eq!(c.resumes_at(), Some(ON), "and the blink comes back here");

        c.tick(ON - MS);
        assert_eq!(c.resumes_at(), Some(ON), "still, a millisecond short");

        c.tick(ON);
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

    /// A ramp asks for frames, a plateau asks for none and names the ramp that
    /// ends it, and each boundary between them belongs to the phase it opens.
    ///
    /// The quiet a key buys is the first plateau and
    /// [`the_quiet_asks_for_one_frame_at_its_end`] is its case; this is every
    /// plateau after it — the dark one, and each later lit one — because the
    /// bar is as still on those, and a tick source held across them paces the
    /// next key by the refresh grid rather than painting it at once. The move
    /// is at an hour of no significance, so that a plateau's answer is read
    /// against the cycle's own top rather than against zero.
    #[test]
    fn a_plateau_asks_for_no_frames_and_names_the_ramp_that_ends_it() {
        let top = 3 * CYCLE + 137 * MS;
        let mut c = live();
        c.edited(top);
        c.moved(bar(100.0, 0.0), Move::FollowsEdit, top);

        // Down the first ramp: frames, and nothing to come back for.
        c.tick(top + ON + FADE_OUT / 2);
        assert!(c.wants_tick(), "the fade down is drawn");
        assert_eq!(c.resumes_at(), None, "it is already being drawn");

        // Its foot is the dark plateau's first instant, not the ramp's last.
        c.tick(top + ON + FADE_OUT);
        assert!(!c.wants_tick(), "the fade down is over at its foot");
        assert_eq!(
            c.resumes_at(),
            Some(top + ON + FADE_OUT + OFF),
            "and the fade back up is where it comes back"
        );

        // The dark plateau, which the bar sits out at alpha 0.
        c.tick(top + ON + FADE_OUT + OFF / 2);
        assert!(!c.wants_tick(), "a dark bar has nothing to draw");
        assert_eq!(
            c.resumes_at(),
            Some(top + ON + FADE_OUT + OFF),
            "still the fade back up, from the middle of the dark"
        );

        // Up the second ramp.
        c.tick(top + ON + FADE_OUT + OFF + FADE_IN / 2);
        assert!(c.wants_tick(), "the fade up is drawn");
        assert_eq!(c.resumes_at(), None, "it is already being drawn");

        // Its head is the second lit plateau's first instant: as still as the
        // hold a key buys, and as free.
        c.tick(top + CYCLE);
        assert!(!c.wants_tick(), "the fade up is over at its head");
        assert_eq!(
            c.resumes_at(),
            Some(top + CYCLE + ON),
            "and the next fade down is where it comes back"
        );

        c.tick(top + CYCLE + ON / 2);
        assert!(!c.wants_tick(), "a solid bar has nothing to draw");
        assert_eq!(
            c.resumes_at(),
            Some(top + CYCLE + ON),
            "still that fade down, from the middle of the lit"
        );

        // The ramps' end states are drawn by the ramps themselves, so the
        // plateaus the source is dropped over begin already correct.
        assert_eq!(
            c.alpha(top + ON + FADE_OUT),
            0.0,
            "the foot of the fade down"
        );
        assert_eq!(c.alpha(top + CYCLE), 1.0, "the head of the fade up");

        // A move is told a frame time the caret has not been ticked to yet, so
        // the cycle's top can still be ahead of the machine's own clock. The
        // whole of the first on-phase is ahead of it there.
        let mut ahead = live();
        ahead.tick(2 * CYCLE);
        ahead.edited(3 * CYCLE);
        ahead.moved(bar(100.0, 0.0), Move::FollowsEdit, 3 * CYCLE);
        assert!(!ahead.wants_tick(), "the top has not been reached");
        assert_eq!(ahead.resumes_at(), Some(3 * CYCLE + ON));
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

    /// The press moves the caret before the button comes up, and the band
    /// owes that move its follow until then (#263).
    #[test]
    fn a_move_under_a_held_button_waits_for_the_release() {
        assert!(waits_for_release(Source::Pointer, true));
        assert!(waits_for_release(Source::Key, true));
    }

    /// With no button down there is no drag to disturb, so the band follows
    /// where it always did; and the app's own moves never follow at all.
    #[test]
    fn a_key_move_with_no_button_down_does_not() {
        assert!(!waits_for_release(Source::Key, false));
        assert!(!waits_for_release(Source::Pointer, false));
        assert!(!waits_for_release(Source::App, true));
    }
}
