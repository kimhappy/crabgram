#![allow(overflowing_literals)]

#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0, max = 300)]
    steps: u8,
}

fn main() {}
