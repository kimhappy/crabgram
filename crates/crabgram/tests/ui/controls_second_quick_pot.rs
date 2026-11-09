#[derive(crabgram::Controls)]
struct Params {
    #[control(quick_pot)]
    level: f32,
    #[control(quick_pot)]
    tone: f32,
}

fn main() {}
