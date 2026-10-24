#[derive(crabgram::Choice)]
enum Shape {
    #[choice(label = "Wave")]
    Sine,
    #[choice(label = "Wave")]
    Square,
}

fn main() {}
