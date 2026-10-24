#[derive(crabgram::Controls)]
struct Params {
    #[control(min = 4_294_967_286, max = 4_294_967_295)]
    count: u32,
}

fn main() {}
