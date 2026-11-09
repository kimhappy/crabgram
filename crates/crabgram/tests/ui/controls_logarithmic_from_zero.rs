#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0.0, max = 1.0, logarithmic)]
    gain: f32,
}

fn main() {}
