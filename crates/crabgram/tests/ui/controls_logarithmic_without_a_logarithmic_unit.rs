#[derive(crabgram::Controls)]
struct Params {
    #[control(default = 1.0, min = 0.5, max = 2.0, unit = Percent, logarithmic)]
    speed: f32,
}

fn main() {}
