//! The chrome that gets out of the way while a hand is typing, with no widget
//! in it.
//!
//! The Parity oracle keeps this in `legacy/app/js/chrome.js` as two
//! `setTimeout`s and two `data-` attributes: a keystroke puts the title bar
//! at nothing and the stats bar at a dim, 500 ms later the stats bar is back
//! and the count is taken again, 1,400 ms later the title bar is back, and a
//! pointer moving brings both back at once. Here it is the same state, in the
//! shape [`crate::caret`] has: the window says what happened and when, and
//! asks, for a given instant, how opaque each bar is and whether a recount is
//! owed. The timers are the window's, armed from [`Typing::resumes_at`]; the
//! machine never fires one.
//!
//! Time is the caller's, in microseconds, as `glib::monotonic_time` and
//! `gdk::FrameClock::frame_time` count it, so the whole of it runs through a
//! scripted timeline in a test.

use crate::flags::Flags;

/// A millisecond, in the microseconds every time here is counted in.
const MS: i64 = 1_000;

/// How long after the last keystroke the stats bar comes back and the count
/// is taken again (`chrome.js` `tA`, 500).
pub const STATS_MS: i64 = 500;
/// How long after the last keystroke the title bar comes back (`chrome.js`
/// `tB`, 1400).
pub const TITLE_MS: i64 = 1_400;
/// The title bar's opacity while typing (`[data-typing="on"] #chrome-top`).
pub const TITLE_FADED: f64 = 0.0;
/// The stats bar's opacity while typing (`[data-typing="on"] #chrome-bottom`).
pub const STATS_FADED: f64 = 0.38;

/// The typing state, as everything but the paint.
///
/// Feed it [`Typing::keystroke`] and [`Typing::pointer`]; read
/// [`Typing::title_alpha`], [`Typing::stats_alpha`], [`Typing::takes_recount`]
/// and [`Typing::resumes_at`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Typing {
    /// When the last keystroke came, while the chrome is still stepping back
    /// from it.
    since: Option<i64>,
    /// `--typing`: held inside the window whatever the clock says, until the
    /// first key or pointer move, which a judged shot never sends.
    held: bool,
    /// Whether a keystroke has been taken since the count was last taken:
    /// the recount the 500 ms timer, or a pointer move before it, owes.
    owed: bool,
}

impl Typing {
    /// At rest: both bars at full strength, nothing owed.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            since: None,
            held: false,
            owed: false,
        }
    }

    /// The state this launch's flags asked for: `--typing` is the chrome as
    /// it is inside the 500 ms after a keystroke, held there so that the
    /// first frame shows it however long the first frame takes.
    #[must_use]
    pub const fn from_flags(flags: &Flags) -> Self {
        Self {
            since: None,
            held: flags.typing,
            owed: false,
        }
    }

    /// A real keystroke at `t`: an edit the writer made, never a load or a
    /// switch (`chrome.js:243`, `source === 'input'`). Both timers start
    /// again from here.
    pub const fn keystroke(&mut self, t: i64) {
        self.since = Some(t);
        self.held = false;
        self.owed = true;
    }

    /// The pointer moved: both bars come back at once (`chrome.js` `wake`),
    /// whenever it was, and a count still owed is taken on idle.
    pub const fn pointer(&mut self) {
        self.since = None;
        self.held = false;
    }

    /// Whether a bar that comes back `ms` after the last keystroke is still
    /// stepped back at `t`: held, or inside that window.
    fn stepped_back(&self, t: i64, ms: i64) -> bool {
        self.held || self.since.is_some_and(|since| t < since + ms * MS)
    }

    /// Whether a bar is stepped back at all at `t`, so that a caller with
    /// nothing to change can stop reading.
    #[must_use]
    pub fn typing(&self, t: i64) -> bool {
        self.stepped_back(t, TITLE_MS)
    }

    /// The title bar's opacity at `t`.
    #[must_use]
    pub fn title_alpha(&self, t: i64) -> f64 {
        if self.stepped_back(t, TITLE_MS) {
            TITLE_FADED
        } else {
            1.0
        }
    }

    /// The stats bar's opacity at `t`.
    #[must_use]
    pub fn stats_alpha(&self, t: i64) -> f64 {
        if self.stepped_back(t, STATS_MS) {
            STATS_FADED
        } else {
            1.0
        }
    }

    /// Whether the count is to be taken again now, at `t`: once per run of
    /// keystrokes, when the 500 ms after the last of them has passed or the
    /// pointer cut the run short. Taking it settles the debt, so the caller
    /// asks once per timer and counts on idle when the answer is yes.
    pub fn takes_recount(&mut self, t: i64) -> bool {
        if !self.owed {
            return false;
        }
        let due = match self.since {
            None => true,
            Some(since) => t >= since + STATS_MS * MS,
        };
        if due {
            self.owed = false;
        }
        due
    }

    /// When the caller has to come back after `t`, if anything is still to
    /// change on its own: the stats bar's return, then the title bar's. `None`
    /// at rest, and `None` while held, because held is what does not end.
    #[must_use]
    pub fn resumes_at(&self, t: i64) -> Option<i64> {
        if self.held {
            return None;
        }
        let since = self.since?;
        [STATS_MS, TITLE_MS]
            .into_iter()
            .map(|ms| since + ms * MS)
            .find(|&when| t < when)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The acceptance's timeline: a keystroke, the stats bar back at 500 ms
    /// with a recount owed, the title bar back at 1,400 ms.
    #[test]
    fn a_keystroke_steps_the_chrome_back_and_the_timers_bring_it_forward() {
        let mut typing = Typing::new();
        assert_eq!(typing.title_alpha(0), 1.0);
        assert_eq!(typing.stats_alpha(0), 1.0);
        assert_eq!(typing.resumes_at(0), None);

        typing.keystroke(10 * MS);
        assert_eq!(typing.title_alpha(10 * MS), 0.0);
        assert_eq!(typing.stats_alpha(10 * MS), 0.38);
        assert!(!typing.takes_recount(10 * MS), "not on the keystroke");
        assert_eq!(typing.resumes_at(10 * MS), Some(510 * MS));

        assert_eq!(typing.stats_alpha(509 * MS), 0.38);
        assert!(!typing.takes_recount(509 * MS));
        assert_eq!(typing.stats_alpha(510 * MS), 1.0);
        assert_eq!(typing.title_alpha(510 * MS), 0.0);
        assert!(typing.takes_recount(510 * MS));
        assert!(!typing.takes_recount(510 * MS), "once");
        assert_eq!(typing.resumes_at(510 * MS), Some(1_410 * MS));

        assert_eq!(typing.title_alpha(1_409 * MS), 0.0);
        assert_eq!(typing.title_alpha(1_410 * MS), 1.0);
        assert_eq!(typing.stats_alpha(1_410 * MS), 1.0);
        assert_eq!(typing.resumes_at(1_410 * MS), None);
        assert!(!typing.typing(1_410 * MS));
    }

    /// Pointer motion at 100 ms restores both bars at once and the count
    /// still owed is taken then, not at 500 ms.
    #[test]
    fn pointer_motion_restores_both_bars_at_once() {
        let mut typing = Typing::new();
        typing.keystroke(0);
        typing.pointer();
        assert_eq!(typing.title_alpha(100 * MS), 1.0);
        assert_eq!(typing.stats_alpha(100 * MS), 1.0);
        assert_eq!(typing.resumes_at(100 * MS), None);
        assert!(typing.takes_recount(100 * MS));
        assert!(!typing.takes_recount(500 * MS));
    }

    /// A run of keystrokes asks for one recount, from the timer after the
    /// last of them: three keys, one count.
    #[test]
    fn a_run_of_keystrokes_is_counted_once_from_the_timer() {
        let mut typing = Typing::new();
        let mut counted = 0;
        let mut t = 0;
        for _ in 0..3 {
            typing.keystroke(t);
            if typing.takes_recount(t) {
                counted += 1;
            }
            t += 200 * MS;
        }
        assert_eq!(counted, 0, "the keystroke path counts nothing");
        // The timer the first key armed, at 500 ms, has been re-armed twice.
        assert_eq!(typing.resumes_at(400 * MS), Some(900 * MS));
        assert_eq!(typing.stats_alpha(899 * MS), 0.38);
        assert!(!typing.takes_recount(899 * MS));
        assert!(typing.takes_recount(900 * MS));
        assert!(!typing.takes_recount(1_000 * MS));
        assert_eq!(counted, 0);
    }

    /// A pointer move with nothing owed and nothing stepped back changes
    /// nothing, so the motion handler has nothing to do at rest.
    #[test]
    fn a_pointer_at_rest_is_nothing() {
        let mut typing = Typing::new();
        typing.pointer();
        assert_eq!(typing, Typing::new());
        assert!(!typing.takes_recount(5 * MS));
    }

    /// `--typing` holds the chrome inside the window with no timer to end
    /// it; the first key or pointer move releases it.
    #[test]
    fn the_typing_flag_holds_the_state_until_a_hand_moves() {
        let flags = Flags {
            typing: true,
            ..Flags::default()
        };
        let mut typing = Typing::from_flags(&flags);
        assert!(typing.typing(0));
        assert_eq!(typing.title_alpha(10_000 * MS), 0.0);
        assert_eq!(typing.stats_alpha(10_000 * MS), 0.38);
        assert_eq!(typing.resumes_at(10_000 * MS), None);
        assert!(!typing.takes_recount(10_000 * MS), "a flag owes no count");

        typing.pointer();
        assert_eq!(typing, Typing::new());

        let mut typing = Typing::from_flags(&flags);
        typing.keystroke(20 * MS);
        assert_eq!(typing.resumes_at(20 * MS), Some(520 * MS));
        assert_eq!(Typing::from_flags(&Flags::default()), Typing::new());
    }
}
