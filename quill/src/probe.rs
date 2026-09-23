//! Scratch (#495, not for merge): main-loop stalls and slow frames, written to `$QUILL_PROBE`.

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

thread_local! {
    static OUT: RefCell<Option<File>> = const { RefCell::new(None) };
}

pub fn line(what: &str) {
    OUT.with_borrow_mut(|out| {
        if let Some(file) = out {
            let _ = writeln!(file, "{} {what}", glib::monotonic_time());
        }
    });
}

pub fn start(widget: &gtk::Widget) {
    let Ok(path) = std::env::var("QUILL_PROBE") else {
        return;
    };
    let Ok(file) = File::create(&path) else {
        return;
    };
    OUT.with_borrow_mut(|out| *out = Some(file));
    // exec in monotonic microseconds, from $QUILL_T0_NS (realtime ns).
    if let Some(t0) = std::env::var("QUILL_T0_NS")
        .ok()
        .and_then(|s| s.trim().parse::<i128>().ok())
    {
        let real = glib::real_time() as i128 * 1000;
        let mono = glib::monotonic_time() as i128;
        line(&format!("exec_us {}", mono - (real - t0) / 1000));
    }

    fn walk(w: &gtk::Widget) {
        w.connect_notify_local(Some("opacity"), |w, _| {
            let parent = w.parent().map_or("-".into(), |p| p.type_().name().to_string());
            let grand = w
                .parent()
                .and_then(|p| p.parent())
                .map_or("-".into(), |p| p.type_().name().to_string());
            line(&format!(
                "opacity {} {} (parent {parent}, {grand}) -> {:.2}",
                w.type_().name(),
                w.css_classes().join("."),
                w.opacity()
            ));
        });
        let mut child = w.first_child();
        while let Some(c) = child {
            walk(&c);
            child = c.next_sibling();
        }
    }
    let root = widget.clone();
    glib::timeout_add_local_once(Duration::from_millis(50), move || walk(&root));

    let last = Rc::new(Cell::new(glib::monotonic_time()));
    glib::timeout_add_local_full(Duration::from_millis(1), glib::Priority::HIGH, move || {
        let now = glib::monotonic_time();
        let gap = now - last.get();
        if gap > 4000 {
            line(&format!("stall {:.2}", gap as f64 / 1000.0));
        }
        last.set(now);
        glib::ControlFlow::Continue
    });

    widget.add_tick_callback(|_, clock| {
        let at = Rc::new(Cell::new([0i64; 6]));
        let set = |i: usize, at: &Rc<Cell<[i64; 6]>>| {
            let mut a = at.get();
            a[i] = glib::monotonic_time();
            at.set(a);
        };
        let a = at.clone();
        clock.connect_before_paint(move |_| {
            a.set([0; 6]);
            set(0, &a)
        });
        let a = at.clone();
        clock.connect_update(move |_| set(1, &a));
        let a = at.clone();
        clock.connect_layout(move |_| set(2, &a));
        let a = at.clone();
        clock.connect_paint(move |_| set(3, &a));
        let a = at.clone();
        clock.connect_after_paint(move |clock: &gdk::FrameClock| {
            set(4, &a);
            let t = a.get();
            let total = t[4] - t[0];
            line(&format!("f {}", clock.frame_counter()));
            if total > 2000 {
                let d = |x: i64, y: i64| if x > 0 && y > 0 { (y - x) as f64 / 1000.0 } else { -1.0 };
                line(&format!(
                    "frame {} total {:.2} update {:.2} layout {:.2} paint {:.2}",
                    clock.frame_counter(),
                    total as f64 / 1000.0,
                    d(t[1], t[2].max(t[3]).max(t[4])),
                    d(t[2], t[3].max(t[4])),
                    d(t[3], t[4]),
                ));
            }
        });
        glib::ControlFlow::Break
    });
}
