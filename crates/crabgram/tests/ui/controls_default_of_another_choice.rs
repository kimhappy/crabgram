#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

#[derive(crabgram::Choice)]
enum Mode {
    Fast,
    Slow,
}

#[derive(crabgram::Controls)]
struct Params {
    #[control(default = Mode::Slow)]
    shape: Shape,
}

fn main() {}
