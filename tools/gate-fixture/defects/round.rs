//! The rounding defect: a float cast to an integer in a function the ROUND_ONCE
//! list in `tools/gate` does not name (CODING_STANDARDS.md § Shape). Formatted,
//! with no bare `#[allow(...)]`, so the steps before it pass.

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    (f64::from(n) * 1.5).round() as i32
}
