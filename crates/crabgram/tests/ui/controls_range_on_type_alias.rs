type Decibels = f32;

#[derive(crabgram::Controls)]
struct Params {
    #[control(min = -24.0, max = 12.0)]
    gain: Decibels,
}

fn main() {}
