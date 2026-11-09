#[derive(crabgram::Controls)]
struct Params {
    #[control(min = -12, max = 12)]
    semitones: u8,
}

fn main() {}
