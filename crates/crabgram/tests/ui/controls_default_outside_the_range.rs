#[derive(crabgram::Controls)]
struct Params {
    #[control(default = 20.0, min = 0.0, max = 10.0)]
    level: f32,
}

fn main() {}
