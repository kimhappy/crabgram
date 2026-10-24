#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

#[derive(crabgram::Controls)]
struct Params {
    #[control(unit = Hertz)]
    shape: Shape,
}

fn main() {}
