//! A crate with a constant named `value`, which a binding of the derive must not meet.

#![allow(non_upper_case_globals)]

const value: u8 = 3;

#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

#[derive(crabgram::Controls)]
struct Params {
    #[control(default = value, min = 1, max = 8)]
    steps: u8,
    #[control(default = Shape::Square)]
    shape: Shape,
}

fn main() {}
