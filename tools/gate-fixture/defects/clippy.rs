//! The clippy defect: `return` on the last expression of a function is
//! `clippy::needless_return`, a default-set lint, so `-D warnings` turns it red.
//! Formatted, and with no bare `#[allow(...)]`, so the steps before it pass.

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    return n + 1;
}
