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
//! [`measure`] answers the Gate's cold-start question — `exec` to the first
//! complete frame the compositor says it presented — and creates the file the
//! per-key capture is written into
//! ([#41](https://github.com/danielbaldwin47/Quill/issues/41) fills it).

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Starts the measurement `--measure <out.jsonl>` asks for.
///
/// Two things, both the Gate's. The file is created now, so that a bench can
/// count on it whatever the run does afterwards; the per-key capture that fills
/// it is the latency ticket's. And the cold start is printed at the first frame
/// the compositor says it presented, in milliseconds from `$QUILL_T0_NS`.
///
/// A launch with no `$QUILL_T0_NS` set has nothing to measure from and says so
/// once: `--measure` is still worth giving for the capture alone.
pub fn measure(window: &impl IsA<gtk::Widget>, out: &Path) {
    if let Err(err) = fs::write(out, "") {
        eprintln!(
            "quill: --measure: {}: cannot be written ({err})",
            out.display()
        );
    }
    let Some(t0) = t0() else {
        return;
    };
    let clocks = Clocks::now();
    window.add_tick_callback(move |_, clock| match presented(clock) {
        Some(at) => {
            println!("{}", cold_start(clocks.milliseconds(t0, at)));
            glib::ControlFlow::Break
        }
        None => glib::ControlFlow::Continue,
    });
}

/// The line `--measure` prints, once, at the first presented frame.
fn cold_start(milliseconds: f64) -> String {
    format!("cold start: {milliseconds:.3} ms")
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
        assert_eq!(cold_start(187.5), "cold start: 187.500 ms");
        assert!(!cold_start(0.0).contains('\n'));
    }
}
