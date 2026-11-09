#[derive(Default)]
struct Params;

impl crabgram::Controls for Params {
    const CONTROLS: &'static [crabgram::Control] = &[];
    const QUICK_POT: Option<&'static crabgram::Control> = None;

    type Values = [f32; 0];

    fn from_ports(_: &[f32; 0]) -> Self {
        Self
    }

    fn to_ports(&self) -> [f32; 0] {
        []
    }
}

fn main() {}
