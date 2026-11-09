#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 5, max = 5)]
    steps: u8,
}

fn main() {}
