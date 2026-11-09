#[derive(crabgram::Controls)]
struct Params {
    #[control(default = Self::FEW, min = Self::FEW, max = Self::MANY)]
    steps: u8,
    #[control(min = 0, max = 16_000_000)]
    count: u32,
}

impl Params {
    const FEW: u8 = 2;
    const MANY: u8 = 200;
}

fn main() {
    assert_eq!(Params::STEPS.as_ref().min(), 2.0);
    assert_eq!(Params::STEPS.as_ref().max(), 200.0);
    assert_eq!(Params::COUNT.as_ref().max(), 16_000_000.0);
}
