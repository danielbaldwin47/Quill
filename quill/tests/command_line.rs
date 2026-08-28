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
