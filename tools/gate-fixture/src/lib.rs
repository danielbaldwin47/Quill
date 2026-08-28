//! A crate with nothing in it but a defect slot.
//!
//! Committed clean, so `tools/gate check` on this directory passes; `selftest`
//! copies one of `defects/*.rs` over `src/defect.rs` to switch a single defect
//! on and prove the command names that step and stops there.

pub mod defect;

#[cfg(test)]
mod tests {
    #[test]
    fn the_fixture_passes_with_no_defect_switched_on() {
        assert_eq!(super::defect::wobble(1), 2);
    }
}
