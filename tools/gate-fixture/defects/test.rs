//! The test defect: a test that fails. Formatted, no bare `#[allow(...)]`, and
//! nothing clippy objects to, so the first three steps pass and this one does not.

/// Something for the fixture's own test to call, so the clean crate is not empty.
pub fn wobble(n: i32) -> i32 {
    n + 1
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_fixture_fails_on_purpose() {
        assert_eq!(super::wobble(1), 3, "the fixture's failing test");
    }
}
