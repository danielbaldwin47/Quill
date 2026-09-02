//! What a command line Quill cannot read does to the process.
//!
//! The parsing itself is held by the unit tests beside it in `flags.rs`; this
//! is the half of the promise those cannot make, because it is about `main`
//! and not about a struct. A harness that misspelled a flag has to *know* — a
//! `tools/gate judge` that shot the default state and called it the judged
//! state would be worse than one that failed — so the exit code is the
//! contract, and it is checked here by running the binary.
//!
//! Every test here runs with no display. The first two never reach GTK at all:
//! the command line is read before fontconfig, before the settings, and before
//! any window. The two below them do reach it, and die there — which is after
//! the settings have been read and written, so what a launch did to a file is
//! observable even though nothing was ever painted.

use std::process::Command;

/// The binary this test was built alongside.
fn quill() -> Command {
    Command::new(env!("CARGO_BIN_EXE_quill"))
}

/// A launch that gets as far as GTK and no further, with both XDG base
/// directories pointed at `home` so that nothing it writes is the writer's.
///
/// The bus is pointed at a socket that is not there for the reason
/// [`a_session_bus_that_is_not_there_does_not_hold_the_launch_up`] gives.
fn launch(home: &std::path::Path) -> Command {
    let mut quill = quill();
    quill
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/quill/bus",
        )
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    quill
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
    let home = scratch("bus");
    let began = std::time::Instant::now();
    let launched = launch(&home)
        .arg("--deterministic")
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
    std::fs::remove_dir_all(&home).ok();
}

/// A directory of this test run's own, named for the process because
/// worktrees test concurrently.
fn scratch(what: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("quill-{what}-{}", std::process::id()));
    std::fs::remove_dir_all(&home).ok();
    std::fs::create_dir_all(&home).expect("a scratch directory");
    home
}

/// `--settings <file>` moves both halves of the settings file: the launch
/// reads that file and writes it, and the writer's own is never opened.
///
/// The launch has no display and dies at GTK, which is well after the settings
/// are read and after a missing file has been written — the two things this
/// asserts. What the app then does with what it read is the engine's to test
/// (`quill_engine::settings`), and a window's to show.
#[test]
fn the_settings_flag_moves_both_the_reading_and_the_writing() {
    let home = scratch("settings");
    let fixture = home.join("fixture.toml");

    // Writing: the file the flag names is the one a first launch creates.
    let launched = launch(&home)
        .arg("--settings")
        .arg(&fixture)
        .output()
        .expect("the binary runs");
    assert!(
        !launched.status.success(),
        "a launch with no display has nothing to paint on: {:?}",
        launched.status
    );
    let written =
        std::fs::read_to_string(&fixture).expect("the launch wrote the file it was given");
    assert!(written.contains("theme = "), "the defaults: {written}");
    assert!(
        !home.join("config").join("quill").exists(),
        "the writer's own settings.toml was touched"
    );
    assert!(
        !home.join("state").join("quill").exists(),
        "state.toml was touched"
    );

    // Reading: a file the flag names that cannot be read is the file named on
    // stderr, one line, as the writer's own would have been.
    std::fs::write(&fixture, "this is not TOML {\n").expect("the fixture is written");
    let said = launch(&home)
        .arg("--settings")
        .arg(&fixture)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&said.stderr);
    assert!(
        said.contains(&format!("quill: {}: is not TOML", fixture.display())),
        "the fixture is what was read: {said}"
    );
    assert_eq!(
        std::fs::read_to_string(&fixture).expect("the fixture is still there"),
        "this is not TOML {\n",
        "a file Quill misunderstands is left as the writer left it"
    );
    std::fs::remove_dir_all(&home).ok();
}
