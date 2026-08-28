//! The defect slot: clean, until `selftest` copies one of `defects/*.rs` here.

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    n + 1
}
