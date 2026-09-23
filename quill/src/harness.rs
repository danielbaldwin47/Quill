//! The harness: what `--deterministic` and `--measure` do to a running Quill.
//!
//! Both belong to the Gate rather than to a writer (`docs/agents/gate.md`), and
//! both have to be in place before the first frame — a shot judged against a
//! frozen opponent is only a comparison if the same command line draws the same
//! pixels twice, and a cold start measured from the second frame is not a cold
//! start.
//!
//! [`determine`] pins everything about a frame that a desktop would otherwise
//! decide: animation, the caret's blink, and the whole of font rendering, which
//! is the one thing that would make two machines disagree about a glyph.
//!
//! `--measure` is four things, and they are four functions because they
//! answer to different moments. [`capture`] opens the file the per-key capture
//! is written into, at startup, so that a bench can count on the file whatever
//! the launch goes on to do — a Document that cannot be read must not take the
//! file with it. [`watch`] stamps every key and writes one line per key once
//! the compositor has said when the frame carrying it was presented, and says
//! on stdout when the pointer leaves the window, which is a run the bench
//! refuses (#327).
//! [`cold_start`] answers the Gate's cold-start question, `exec` to the first
//! complete frame the compositor says it presented, and needs a window to hang
//! off. [`settle`] says on stdout when the launch's own work is over and when
//! nothing is left painting on its own, so a bench can start typing after the
//! one and hold its first measured key until after the other (#495).
//!
//! The per-key capture is the left-hand side of `tools/gate bench`'s join: the
//! bench knows when it wrote each key to `/dev/uinput`, this knows when the
//! frame carrying it turned into light, and `presentation_time` is on the same
//! `CLOCK_MONOTONIC` scale as both. Three things about the frame clock decide
//! the shape here, and all three were paid for in
//! `docs/research/native-harness.md` §4:
//!
//! - A key controller in the bubble phase never fires, because the Editor is a
//!   [`gtk::TextView`] and it eats the key. Only the capture phase sees them.
//! - A [`gdk::FrameTimings`] is complete only once a *later* frame has run, so
//!   a key cannot be written on the keystroke that made it. Stamps are held and
//!   drained on later frames, on a timer, and when the app is told to quit.
//! - An idle window runs no later frame, so the last key of a run would wait
//!   for ever. A stamp still waiting after [`TAIL`] asks for a frame itself:
//!   long after the pace a bench types at, so it costs nothing during a run and
//!   closes the file's tail once typing stops.

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

/// The variable the harness stamps before `exec`, in realtime nanoseconds:
/// what `date +%s%N` prints.
const T0: &str = "QUILL_T0_NS";

/// 96 dpi, in the 1024ths `gtk-xft-dpi` is written in.
const DPI: i32 = 96 * 1024;

/// On, in the tri-state (`-1` for "ask the desktop") the Xft settings use.
const ON: i32 = 1;

/// How often the stamps waiting on a frame are looked at again.
const DRAIN_EVERY: Duration = Duration::from_millis(100);

/// How long a stamp waits for its frame to complete before a frame is asked
/// for, in monotonic microseconds. Well past the 90 ms the headline regime
/// types at, so a run never pays for it and a run's last key never hangs.
const TAIL: i64 = 250_000;

/// Pins everything about a frame that the desktop would otherwise decide.
///
/// Called from `startup`, which is after GTK has a display and before the first
/// window: [`gtk::Settings`] is a property of the display, and a setting
/// changed after a widget has drawn is a setting the first frame did not have.
pub fn determine() {
    let Some(settings) = gtk::Settings::default() else {
        // No display: nothing to pin, and nothing that will draw a frame.
        return;
    };
    settings.set_gtk_enable_animations(false);
    settings.set_gtk_cursor_blink(false);
    // Manual font rendering is what makes the four Xft settings below count:
    // left automatic, GTK picks antialiasing and hinting from the display's
    // scale and the desktop's mood.
    settings.set_gtk_font_rendering(gtk::FontRendering::Manual);
    settings.set_gtk_xft_antialias(ON);
    settings.set_gtk_xft_hinting(ON);
    settings.set_gtk_xft_hintstyle(Some("hintslight"));
    settings.set_gtk_xft_rgba(Some("none"));
    settings.set_gtk_xft_dpi(DPI);
    settings.set_gtk_hint_font_metrics(true);
}

thread_local! {
    /// The capture this launch is writing, if `--measure` named one. GTK is
    /// one thread, so the key handler, the frame clock and `shutdown` all
    /// reach it here rather than by being handed it through every window.
    static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

/// Opens the file `--measure <out.jsonl>` names, empty.
///
/// Called from `startup`, before any window and before any Document is read, so
/// that the file a bench was promised is there even when the launch that was
/// asked for turns out to be a launch that cannot open anything. The handle is
/// kept for the rest of the process: a JSONL file truncated from the outside
/// while its writer holds it open comes back NUL-padded.
pub fn capture(out: &Path) {
    match File::create(out) {
        Ok(file) => CAPTURE.with_borrow_mut(|held| {
            *held = Some(Capture {
                out: out.to_path_buf(),
                file: Some(file),
                pending: Vec::new(),
            });
        }),
        Err(err) => eprintln!(
            "quill: --measure: {}: cannot be written ({err})",
            out.display()
        ),
    }
}

/// Stamps every key `window` sees, writes one line per key, and prints one
/// line on stdout each time the pointer leaves the window.
///
/// The controllers are in the capture phase because the Editor consumes keys,
/// and the drain is hung on the first frame because a widget has a frame clock
/// only once it is realized.
pub fn watch(window: &impl IsA<gtk::Widget>) {
    let widget: gtk::Widget = window.as_ref().clone();

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|controller, _key, keycode, _state| {
        stamp(keycode, controller.current_event_time());
        // The Editor still gets the key: this watches, it does not handle.
        glib::Propagation::Proceed
    });
    widget.add_controller(keys);

    // The pointer leaving is the owner's mouse crossing the stage mid-run.
    // The chrome answers with its opacity transition, and while any animation
    // runs the frame clock paces every frame to the refresh grid, so a key
    // that lands just after one waits a whole refresh before it is painted —
    // 16.3 ms of the 17.97 ms one such key scored (#327). Said on stdout, one
    // line per leave, for the bench to refuse the run on rather than score.
    let pointer = gtk::EventControllerMotion::new();
    pointer.set_propagation_phase(gtk::PropagationPhase::Capture);
    pointer.connect_leave(|_| println!("{}", pointer_left_line(glib::monotonic_time())));
    widget.add_controller(pointer);

    widget.add_tick_callback(|widget, clock| {
        let painted = widget.downgrade();
        clock.connect_after_paint(move |clock| {
            // Every key stamped since the last paint was carried by this frame.
            mark(clock.frame_counter());
            if let Some(widget) = painted.upgrade() {
                drain(clock, &widget);
            }
        });

        let ticking = widget.downgrade();
        let clock = clock.clone();
        glib::timeout_add_local(DRAIN_EVERY, move || match ticking.upgrade() {
            Some(widget) => {
                drain(&clock, &widget);
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });

        // The clock is the same one for the life of the window, so once is all
        // this has to run.
        glib::ControlFlow::Break
    });
}

/// Writes what is left and lets go of the file.
///
/// Called from `shutdown`, which is the last moment a key can still be written.
/// A stamp still waiting on its frame here is written with no presentation time
/// rather than dropped: the bench's accounting is only honest if every key the
/// app saw is in the file.
///
/// This is not what makes the file whole when a bench kills the app — a killed
/// process runs no `shutdown`. Every drain writes through, so the file is whole
/// as of the last one; [`DRAIN_EVERY`] and [`TAIL`] are what a bench relies on, and
/// they are both far inside the settle it waits out before it kills anything.
pub fn flush() {
    with_capture(Capture::close);
}

/// Prints the cold start at the first frame the compositor says it presented,
/// in milliseconds from `$QUILL_T0_NS`.
///
/// A launch with no `$QUILL_T0_NS` set has nothing to measure from and says so
/// once: `--measure` is still worth giving for the capture alone.
pub fn cold_start(window: &impl IsA<gtk::Widget>) {
    let Some(t0) = t0() else {
        return;
    };
    let clocks = Clocks::now();
    window.add_tick_callback(move |_, clock| match presented(clock) {
        Some(at) => {
            println!("{}", cold_start_line(clocks.milliseconds(t0, at)));
            glib::ControlFlow::Break
        }
        None => glib::ControlFlow::Continue,
    });
}

/// The line `--measure` prints, once, at the first presented frame.
fn cold_start_line(milliseconds: f64) -> String {
    format!("cold start: {milliseconds:.3} ms")
}

/// How far a launch has got with its own work, as [`settle`] has said it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Launch {
    /// Something the window armed at launch is still under way.
    Busy,
    /// The window's own launch work is over, and said so.
    Settled,
    /// Nothing paints on its own any more either, and said so.
    Quiet,
}

/// Prints two lines on stdout as a launch finishes its own work: `settled`
/// once the window has nothing of its own under way, and `quiet` once, after
/// that, nothing is left to paint on its own either.
///
/// A launch goes on painting for seconds after its first frame, and those
/// frames carry no key: a key that lands inside the same refresh as one waits
/// for the next, which on an unchanged build read 16.25 ms in #489. The two
/// lines are two moments because typing undoes one kind of that work and not
/// the other (#495):
///
/// - `launching` says the window's own work is still under way
///   ([`crate::window::Window`]'s reveal, Annotators and Preview). The
///   Annotators' first pass is answered for the Document's generation, so a
///   key typed during it throws the whole pass away and it is done again at
///   the first pause. A bench starts typing only after `settled`.
/// - GTK shows an overlay scrollbar's indicator when its scrolled window
///   scrolls, and hides it on a timer of its own two seconds or so after the
///   last scroll, in a frame of its own. The launch's scroll to its caret is a
///   scroll, so every launch of a long Document has one to wait out, and with
///   the Preview open it has two. Typing leaves it alone as long as it does not
///   scroll, so a bench's warm-up can run over it, and its first measured key
///   waits for `quiet`.
///
/// Read at every frame painted once the compositor has presented one, and on
/// [`DRAIN_EVERY`] as well, because the last of the work need not paint: the
/// Annotators' last drain can find nothing left to tag.
pub fn settle(window: &impl IsA<gtk::Widget>, launching: impl Fn() -> bool + 'static) {
    let t0 = std::env::var(T0)
        .ok()
        .and_then(|written| written.trim().parse().ok());
    let clocks = Clocks::now();
    let late = std::env::var("QUILL_LATE_WORK_MS").ok().and_then(|v| v.parse::<u64>().ok());
    let born = glib::monotonic_time();
    if let Some(ms) = late {
        let painted = window.as_ref().clone().upcast::<gtk::Widget>();
        glib::timeout_add_local_once(Duration::from_millis(ms), move || painted.queue_draw());
    }
    if let Some(ms) = std::env::var("QUILL_FAKE_LEAVE_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
        glib::timeout_add_local_once(Duration::from_millis(ms), || {
            println!("{}", pointer_left_line(glib::monotonic_time()));
        });
    }
    let launching = Rc::new(move || {
        launching() || late.is_some_and(|ms| glib::monotonic_time() - born < (ms * 1000) as i64)
    });
    window.add_tick_callback(move |widget, clock| {
        if presented(clock).is_none() {
            return glib::ControlFlow::Continue;
        }
        let state = Rc::new(Cell::new(Launch::Busy));
        let check = {
            let widget = widget.downgrade();
            let launching = launching.clone();
            let state = state.clone();
            Rc::new(move || {
                let Some(widget) = widget.upgrade() else {
                    return;
                };
                let say = |now: Launch| {
                    state.set(now);
                    let at = glib::monotonic_time();
                    let from_exec = t0.map(|t0| clocks.milliseconds(t0, at));
                    println!("{}", launch_line(now, at, from_exec));
                };
                if state.get() == Launch::Busy && !launching() {
                    say(Launch::Settled);
                }
                if state.get() == Launch::Settled && !indicating(widget.upcast_ref()) {
                    say(Launch::Quiet);
                }
            })
        };
        // Let go of once both are out, so a run's every frame after them pays
        // nothing for lines already said.
        let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
        let (painted, on_paint, done) = (handler.clone(), check.clone(), state.clone());
        handler.replace(Some(clock.connect_after_paint(move |clock| {
            on_paint();
            if done.get() == Launch::Quiet
                && let Some(id) = painted.take()
            {
                clock.disconnect(id);
            }
        })));
        glib::timeout_add_local(DRAIN_EVERY, move || {
            check();
            if state.get() == Launch::Quiet {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        glib::ControlFlow::Break
    });
}

/// Whether any scrollbar under `widget` is an overlay indicator on the glass:
/// GTK fades one in and out by its opacity, and marks it with the
/// `overlay-indicator` style class, which a scrollbar that is always shown
/// (overlay scrolling turned off on the desktop) never has.
fn indicating(widget: &gtk::Widget) -> bool {
    if let Some(bar) = widget.downcast_ref::<gtk::Scrollbar>()
        && bar.has_css_class("overlay-indicator")
        && bar.is_mapped()
        && bar.opacity() > 0.0
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if indicating(&widget) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}

/// A line [`settle`] prints: which moment, when in monotonic microseconds, so
/// a bench can put it beside the keys it wrote, and how long after `exec` when
/// the harness stamped `$QUILL_T0_NS`.
fn launch_line(now: Launch, at_us: i64, from_exec_ms: Option<f64>) -> String {
    let what = match now {
        Launch::Busy => "busy",
        Launch::Settled => "settled",
        Launch::Quiet => "quiet",
    };
    match from_exec_ms {
        Some(ms) => format!("launch {what} at {at_us} us, {ms:.3} ms from exec"),
        None => format!("launch {what} at {at_us} us"),
    }
}

/// The line `--measure` prints each time the pointer leaves the window, in
/// monotonic microseconds, so a bench can put the leave beside its keys.
fn pointer_left_line(at_us: i64) -> String {
    format!("pointer left the window at {at_us} us")
}

/// `$QUILL_T0_NS`, in realtime nanoseconds.
fn t0() -> Option<i128> {
    let Ok(written) = std::env::var(T0) else {
        eprintln!("quill: --measure: ${T0} is not set, so there is no cold start to print");
        return None;
    };
    match written.trim().parse() {
        Ok(t0) => Some(t0),
        Err(_) => {
            eprintln!("quill: --measure: ${T0}: \"{written}\" is not a time in nanoseconds");
            None
        }
    }
}

/// The first frame the compositor said it presented, in monotonic microseconds.
///
/// A `GdkFrameTimings` is filled in after its frame is drawn, so it is worth
/// reading only once it is complete; a complete one still says `0` when the
/// compositor reported no presentation time at all. So this looks for the
/// Gate's own words: the first complete frame with a non-zero presentation
/// time.
fn presented(clock: &gdk::FrameClock) -> Option<i64> {
    (clock.history_start()..=clock.frame_counter())
        .filter_map(|counter| clock.timings(counter))
        .filter(gdk::FrameTimings::is_complete)
        .map(|timings| timings.presentation_time())
        .find(|time| *time != 0)
}

/// When the pixels painted in `frame` first reached the glass, with the
/// refresh interval of the frame that carried them: `frame`'s own presentation
/// when the compositor showed it, otherwise the first later frame's that was
/// presented. `None` while every frame from `frame` on is incomplete or
/// discarded ([`Capture::drain`]).
///
/// Not unit-tested: a `gdk::FrameClock` and its timings exist only under a
/// display. The tests below cover the line the result is written as.
fn presented_from(clock: &gdk::FrameClock, frame: i64) -> Option<(i64, i64)> {
    (frame..=clock.frame_counter())
        .filter_map(|counter| clock.timings(counter))
        .filter(gdk::FrameTimings::is_complete)
        .map(|timings| (timings.presentation_time(), timings.refresh_interval()))
        .find(|(time, _)| *time != 0)
}

/// Does something to the capture, if this launch has one.
///
/// Every one of the three moments below reaches it the same way, and a launch
/// without `--measure` has nothing for any of them to do.
fn with_capture<T: Default>(what: impl FnOnce(&mut Capture) -> T) -> T {
    CAPTURE.with_borrow_mut(|held| held.as_mut().map(what).unwrap_or_default())
}

/// Holds one key until the frame that carried it has been presented.
fn stamp(keycode: u32, evdev_ms: u32) {
    with_capture(|capture| capture.stamp(keycode, evdev_ms));
}

/// Gives every key stamped since the last paint the frame that has just been
/// painted.
fn mark(frame: i64) {
    with_capture(|capture| capture.mark(frame));
}

/// Writes every key whose frame is now complete, and asks for a frame when one
/// has been waiting longer than [`TAIL`] — an idle window runs no later frame,
/// and a frame is complete only once a later one has run.
fn drain(clock: &gdk::FrameClock, widget: &gtk::Widget) {
    if with_capture(|capture| capture.drain(clock, glib::monotonic_time())) {
        widget.queue_draw();
    }
}

/// One key, from the moment it was seen to the moment its frame was presented.
#[derive(Clone, Copy, Debug)]
struct Stamp {
    /// The GDK keycode, which is the evdev code the bench wrote plus 8.
    keycode: u32,
    /// The event's own time, from libinput, in milliseconds. GDK quantises it
    /// to the millisecond, so it is a cross-check on the join rather than the
    /// measurement.
    evdev_ms: u32,
    /// Monotonic microseconds when the first handler saw the key.
    handler_us: i64,
    /// The frame that carried it, once one has been painted.
    frame: Option<i64>,
}

/// The file `--measure` writes, and the keys not yet in it.
struct Capture {
    /// What the file is called, for the one message a write failure earns.
    out: PathBuf,
    /// The open file, until a write to it fails.
    file: Option<File>,
    /// Keys stamped but not yet written, oldest first.
    pending: Vec<Stamp>,
}

impl Capture {
    /// Takes one key down, to be written once its frame has been presented.
    fn stamp(&mut self, keycode: u32, evdev_ms: u32) {
        self.pending.push(Stamp {
            keycode,
            evdev_ms,
            handler_us: glib::monotonic_time(),
            frame: None,
        });
    }

    /// Gives every key not yet on a frame the frame just painted. Keys that
    /// share a frame share its presentation time, which is what a burst faster
    /// than the refresh interval is and is recorded as such.
    fn mark(&mut self, frame: i64) {
        for stamp in self.pending.iter_mut().filter(|s| s.frame.is_none()) {
            stamp.frame = Some(frame);
        }
    }

    /// Writes every key whose frame has been presented, and says whether a key
    /// is still waiting on a frame that only a later frame will complete.
    ///
    /// A key's frame is the one painted after it, but the compositor does not
    /// show every frame it is handed: two painted inside one refresh, which a
    /// stall followed by a catch-up produces at saturation, reach it together
    /// and only the later is presented; the earlier is discarded, and its
    /// timings never complete (or complete saying zero). Its pixels were first
    /// on the glass when the next presented frame was, and presentation is in
    /// order, so the first presented frame at or after a key's own is the
    /// time the key is written with. Until one completes the key waits, and
    /// if none ever comes it is written with none when its frame leaves the
    /// clock's history or the capture closes.
    fn drain(&mut self, clock: &gdk::FrameClock, now: i64) -> bool {
        let mut lines = String::new();
        let mut stale = false;
        let history = clock.history_start();
        self.pending.retain(|stamp| {
            let Some(frame) = stamp.frame else {
                // Stamped between two paints: the next one is its frame.
                return true;
            };
            if let Some(presented) = presented_from(clock, frame) {
                lines.push_str(&line(stamp, Some(presented)));
                lines.push('\n');
                return false;
            }
            if frame < history {
                // Fallen out of the clock's history with nothing presented
                // after it: it never will be now, and holding it would hold
                // the whole buffer.
                lines.push_str(&line(stamp, None));
                lines.push('\n');
                return false;
            }
            stale |= now - stamp.handler_us > TAIL;
            true
        });
        self.write(&lines);
        stale
    }

    /// Writes what is still waiting, with no presentation time, and closes.
    fn close(&mut self) {
        let mut lines = String::new();
        for stamp in &self.pending {
            lines.push_str(&line(stamp, None));
            lines.push('\n');
        }
        self.pending.clear();
        self.write(&lines);
        self.file = None;
    }

    /// Appends `lines`, and says so once if it cannot.
    fn write(&mut self, lines: &str) {
        if lines.is_empty() {
            return;
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if let Err(err) = file.write_all(lines.as_bytes()).and_then(|()| file.flush()) {
            eprintln!(
                "quill: --measure: {}: cannot be written ({err})",
                self.out.display()
            );
            self.file = None;
        }
    }
}

/// One key as the capture file records it: one JSON object on one line.
///
/// A key with no presentation time is written with `null` rather than left out,
/// so that the bench's accounting can say how many keys it is short of and the
/// owner can see which ones.
fn line(stamp: &Stamp, presented: Option<(i64, i64)>) -> String {
    let frame = stamp
        .frame
        .map_or_else(|| "null".to_owned(), |n| n.to_string());
    let (present_us, refresh_us) = presented.map_or_else(
        || ("null".to_owned(), "null".to_owned()),
        |(present, refresh)| (present.to_string(), refresh.to_string()),
    );
    format!(
        "{{\"keycode\":{},\"evdev_ms\":{},\"handler_us\":{},\"frame\":{frame},\
         \"present_us\":{present_us},\"refresh_us\":{refresh_us}}}",
        stamp.keycode, stamp.evdev_ms, stamp.handler_us
    )
}

/// The two clocks a cold start is measured across.
///
/// `$QUILL_T0_NS` is realtime — what `date +%s%N` prints — and a presentation
/// time is the frame clock's monotonic microseconds, whose zero is the boot and
/// not the epoch. Neither can be read on the other's scale, so this is the
/// offset between them, read once. Both advance at the same rate, so *when* it
/// is read does not matter; a step of the realtime clock landing inside the
/// measurement is the price, and a launch measured in milliseconds is not where
/// NTP steps.
#[derive(Clone, Copy, Debug)]
struct Clocks {
    /// Monotonic microseconds when the offset was read.
    monotonic: i64,
    /// Realtime microseconds at the same moment.
    realtime: i64,
}

impl Clocks {
    /// Reads both clocks, as close together as two calls allow.
    fn now() -> Self {
        let realtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| i64::try_from(since.as_micros()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        Self {
            monotonic: glib::monotonic_time(),
            realtime,
        }
    }

    /// The milliseconds from `t0`, in realtime nanoseconds, to `presented`, a
    /// presentation time in monotonic microseconds.
    fn milliseconds(self, t0: i128, presented: i64) -> f64 {
        let started = i128::from(self.monotonic) - (i128::from(self.realtime) - t0 / 1_000);
        (i128::from(presented) - started) as f64 / 1_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cold_start_is_the_time_from_the_harnesss_t0_to_the_presented_frame() {
        // The offset was read when the epoch clock said 1.2 s and the frame
        // clock said 5.0 s, so the harness's `exec` at 1.0 s past the epoch is
        // 4.8 s on the frame clock.
        let clocks = Clocks {
            monotonic: 5_000_000,
            realtime: 1_200_000,
        };
        let ms = clocks.milliseconds(1_000_000_000, 4_987_500);
        assert!((ms - 187.5).abs() < 1e-9, "{ms} ms");
    }

    #[test]
    fn a_frame_presented_before_t0_would_be_negative_rather_than_wrap() {
        let clocks = Clocks {
            monotonic: 5_000_000,
            realtime: 1_200_000,
        };
        let ms = clocks.milliseconds(1_000_000_000, 4_799_000);
        assert!(ms < 0.0, "{ms} ms");
    }

    #[test]
    fn the_cold_start_line_is_one_line_of_milliseconds() {
        assert_eq!(cold_start_line(187.5), "cold start: 187.500 ms");
        assert!(!cold_start_line(0.0).contains('\n'));
    }

    #[test]
    fn a_pointer_leave_is_one_line_the_bench_reads_by_its_opening_words() {
        assert_eq!(
            pointer_left_line(6_516_887_400),
            "pointer left the window at 6516887400 us"
        );
        assert!(!pointer_left_line(0).contains('\n'));
    }

    #[test]
    fn a_launch_moment_is_one_line_with_its_monotonic_time_and_its_time_from_exec() {
        assert_eq!(
            launch_line(Launch::Settled, 6_516_887_400, Some(1_040.5)),
            "launch settled at 6516887400 us, 1040.500 ms from exec"
        );
        assert_eq!(
            launch_line(Launch::Quiet, 6_516_887_400, Some(2_680.5)),
            "launch quiet at 6516887400 us, 2680.500 ms from exec"
        );
        assert_eq!(
            launch_line(Launch::Quiet, 6_516_887_400, None),
            "launch quiet at 6516887400 us"
        );
        assert!(!launch_line(Launch::Settled, 0, Some(0.0)).contains('\n'));
    }

    /// A stamp with everything the join needs, as a presented key.
    fn presented_key() -> Stamp {
        Stamp {
            keycode: 30,
            evdev_ms: 169_983_352,
            handler_us: 169_962_975_970,
            frame: Some(412),
        }
    }

    #[test]
    fn a_presented_key_is_one_line_of_json_with_every_field_the_join_needs() {
        assert_eq!(
            line(&presented_key(), Some((169_962_979_465, 16_666))),
            "{\"keycode\":30,\"evdev_ms\":169983352,\"handler_us\":169962975970,\
             \"frame\":412,\"present_us\":169962979465,\"refresh_us\":16666}"
        );
    }

    #[test]
    fn a_key_the_compositor_never_presented_says_so_rather_than_saying_zero() {
        let written = line(&presented_key(), None);
        assert!(written.contains("\"present_us\":null"), "{written}");
        assert!(written.contains("\"refresh_us\":null"), "{written}");
        assert!(written.contains("\"frame\":412"), "{written}");
    }

    #[test]
    fn a_key_no_frame_ever_carried_says_so_too() {
        let mut stamp = presented_key();
        stamp.frame = None;
        let written = line(&stamp, None);
        assert!(written.contains("\"frame\":null"), "{written}");
    }

    #[test]
    fn every_key_is_one_line() {
        assert!(!line(&presented_key(), Some((1, 2))).contains('\n'));
        assert!(!line(&presented_key(), None).contains('\n'));
    }
}
