# wire-late.py: #495's scratch switches in the late-495 worktree (not for merge).
#   QUILL_LATE_WORK_MS  the launch has work of its own until then, and paints a frame when it ends
#   QUILL_FAKE_LEAVE_MS the app says the pointer left the window then (#327's trap; a scripted
#                       cursor cannot reach the headless stage on Hyprland 0.56)
#   BENCH_NO_WAIT       the bench neither waits for `settled` nor tops up for `quiet`
#   BENCH_PAUSE_MS      the pause regimes pause this long (#349's trap puts back 1400)
W = '/home/diggle/repos/quill/.claude/worktrees/late-495/'
def rep(p, a, b):
    s = open(W + p).read(); assert s.count(a) == 1, (p, a); open(W + p, 'w').write(s.replace(a, b))
rep('quill/src/harness.rs', """    let clocks = Clocks::now();
    let launching = Rc::new(launching);
""", """    let clocks = Clocks::now();
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
""")
rep('tools/bench.mjs', """    const settledAt = await saidBy(ours, 'settled', LAUNCH_WAIT_MS);""",
    """    const settledAt = process.env.BENCH_NO_WAIT ? { from_exec_ms: null } : await saidBy(ours, 'settled', LAUNCH_WAIT_MS);""")
rep('tools/bench.mjs', """      { enough: { from: first.keys, step: 2, met: () => launchSaid(ours.said(), 'quiet') !== null } });""",
    """      { enough: { from: first.keys, step: 2, met: () => Boolean(process.env.BENCH_NO_WAIT) || launchSaid(ours.said(), 'quiet') !== null } });""")
rep('tools/regimes.mjs', "export const PAUSE_MS = 1700;", "export const PAUSE_MS = Number(process.env.BENCH_PAUSE_MS || 1700);")
