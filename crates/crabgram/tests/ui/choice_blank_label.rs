#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    #[choice(label = " ")]
    Square,
}

fn main() {}
