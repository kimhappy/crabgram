#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0, max = 20_000_000)]
    count: u32,
}

fn main() {}
