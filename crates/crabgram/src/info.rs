//! What a plugin is called and who makes it.

mod category;
mod definition;
mod licensing;
#[doc(hidden)]
pub mod make;
mod problem;
#[cfg(test)]
mod tests;
mod text;
mod version;

pub use self::{
    category::Category,
    definition::{Info, Maintainer},
    licensing::Licensing,
    problem::InfoProblem,
    version::Version,
};
