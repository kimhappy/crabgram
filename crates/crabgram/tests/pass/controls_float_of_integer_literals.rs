#[derive(crabgram::Controls)]
struct Params {
    #[control(default = 0, min = -24, max = 12)]
    gain: f32,
}

fn main() {
    assert_eq!(Params::GAIN.as_ref().min(), -24.0);
    assert_eq!(Params::GAIN.as_ref().max(), 12.0);
    assert_eq!(Params::default().gain, 0.0);
}
