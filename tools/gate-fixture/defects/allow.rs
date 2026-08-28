//! The bare-allow defect: an `#[allow(...)]` with no reason on its line, which
//! `docs/agents/gate.md` forbids.

#[allow(dead_code)]
fn nobody_calls_this() {}

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    n + 1
}
