#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0.0)]
    #[control(min = 5.0, max = 10.0)]
    level: f32,
}

fn main() {}
