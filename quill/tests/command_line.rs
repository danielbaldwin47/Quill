//! What a command line Quill cannot read does to the process.
//!
//! The parsing itself is held by the unit tests beside it in `flags.rs`; this
//! is the half of the promise those cannot make, because it is about `main`
//! and not about a struct. A harness that misspelled a flag has to *know* — a
//! `tools/gate judge` that shot the default state and called it the judged
//! state would be worse than one that failed — so the exit code is the
//! contract, and it is checked here by running the binary.
//!
//! Both of these run with no display, and neither reaches GTK: the command
//! line is read before fontconfig, before the settings, and before any window.

use std::process::Command;

/// The binary this test was built alongside.
fn quill() -> Command {
    Command::new(env!("CARGO_BIN_EXE_quill"))
}

#[test]
fn a_flag_quill_does_not_know_is_a_non_zero_exit_and_one_line_naming_it() {
    let refused = quill()
        .arg("--frobnicate")
        .output()
        .expect("the binary runs");
    assert!(
        !refused.status.success(),
        "a command line Quill cannot read must fail: {:?}",
        refused.status
    );
    let said = String::from_utf8_lossy(&refused.stderr);
    assert_eq!(
        said.trim_end(),
        "quill: --frobnicate: not a flag Quill knows"
    );
    assert_eq!(said.lines().count(), 1, "one line, not a stack: {said}");
    assert!(refused.stdout.is_empty(), "nothing on stdout: {refused:?}");
}

#[test]
fn help_is_the_usage_on_stdout_and_a_zero_exit() {
    let asked = quill().arg("--help").output().expect("the binary runs");
    assert!(asked.status.success(), "{:?}", asked.status);
    let printed = String::from_utf8_lossy(&asked.stdout);
    assert!(printed.starts_with("Usage: quill"), "{printed}");
    for flag in ["--text", "--deterministic", "--measure"] {
        assert!(printed.contains(flag), "no {flag} in:\n{printed}");
    }
}

/// A launch whose session bus is not there waits for nobody.
///
/// The settings portal is asked for the desktop's ground before the first
/// frame, synchronously, because a ground resolved after that frame is a flash
/// of the other one — so the one thing that must never happen is a launch
/// hanging on a bus that is not answering. This points the address at a socket
/// that does not exist and times the whole process: it gets past the portal,
/// past the fonts and into GTK, which is where a run with no display ends.
///
/// A launch of the harness's, because it is the one that writes nothing: this
/// test must not put a `settings.toml` in front of whoever ran `cargo test`.
/// The bound is loose on purpose — this is a hang detector, and the number
/// that matters is the cold start on the ticket, measured by `tools/gate
/// bench` on a machine that has a display.
#[test]
fn a_session_bus_that_is_not_there_does_not_hold_the_launch_up() {
    let began = std::time::Instant::now();
    let launched = quill()
        .arg("--deterministic")
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/quill/bus",
        )
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("the binary runs");
    let took = began.elapsed();
    assert!(
        !launched.status.success(),
        "a launch with no display has nothing to paint on: {:?}",
        launched.status
    );
    assert!(
        took < std::time::Duration::from_secs(2),
        "the launch waited {took:?} on a bus that is not there"
    );
}
