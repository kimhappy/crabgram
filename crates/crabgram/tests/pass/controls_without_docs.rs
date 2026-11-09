//! A crate that documents its controls struct but not every field.

#![deny(missing_docs)]

/// The control ports.
#[derive(crabgram::Controls)]
pub struct Params {
    #[allow(missing_docs)]
    pub level: f32,
    sync: bool,
}

fn main() {}
