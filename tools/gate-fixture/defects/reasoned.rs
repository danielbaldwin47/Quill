//! Not a defect: the same `#[allow(...)]` as `allow.rs`, with its reason on the
//! line, which is what `docs/agents/gate.md` asks for. Every step passes.

#[allow(dead_code)] // the fixture's own unused function, on purpose.
fn nobody_calls_this() {}

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    n + 1
}
