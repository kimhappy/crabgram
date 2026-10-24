use core::ffi::CStr;

use super::{Category, Info, Licensing};

/// The fields of an [`Info`], which only `info!` fills through [`checked`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[allow(missing_docs)]
pub struct Fields {
    pub uri: &'static CStr,
    pub name: &'static str,
    pub description: &'static str,
    pub abbreviation: &'static str,
    pub category: Category,
    pub channels: u32,
    pub licensing: Licensing,
    pub license_document: Option<&'static str>,
    pub package: &'static str,
    pub authors: &'static str,
    pub repository: &'static str,
    pub license: &'static str,
    pub version: &'static str,
}

/// The [`Info`] of `fields`.
///
/// # Panics
///
/// At compile time, on any [`InfoProblem`](super::InfoProblem).
#[must_use]
pub const fn checked(fields: Fields) -> Info {
    let info = Info(fields);

    match info.problem() {
        None => info,
        Some(problem) => problem.fail(),
    }
}
