#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 1, max = 16, unit = Hertz, logarithmic)]
    steps: u8,
}

fn main() {}
