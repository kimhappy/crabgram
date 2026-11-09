//! A crate that forbids unsafe code still derives its controls and lists.

#![forbid(unsafe_code)]

#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

#[derive(crabgram::Controls)]
struct Params {
    level: f32,
    shape: Shape,
}

fn main() {
    let _ = (Params::LEVEL, Params::SHAPE);
}
