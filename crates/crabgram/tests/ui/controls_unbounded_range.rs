#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0.0, max = f32::INFINITY)]
    level: f32,
}

fn main() {}
