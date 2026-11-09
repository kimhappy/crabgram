//! The mistakes the derives and `info!` refuse, with their errors, and the edge cases they take.
//!
//! After a toolchain bump, rerun with `TRYBUILD=overwrite` and review the diff.

#[test]
#[cfg_attr(miri, ignore = "trybuild runs rustc, which Miri cannot")]
fn mistakes_fail_to_compile() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}

#[test]
#[cfg_attr(miri, ignore = "trybuild runs rustc, which Miri cannot")]
fn edge_cases_compile() {
    trybuild::TestCases::new().pass("tests/pass/*.rs");
}
