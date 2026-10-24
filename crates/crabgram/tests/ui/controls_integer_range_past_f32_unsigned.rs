#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 0, max = 3_000_000_000)]
    count: u32,
}

fn main() {}
