#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

impl Default for Shape {
    fn default() -> Self {
        Self::Sine
    }
}

impl crabgram::Controls for Shape {
    const CONTROLS: &'static [crabgram::Control] = &[];
    const QUICK_POT: Option<&'static crabgram::Control> = None;

    type Values = [f32; 0];

    fn from_ports(_: &[f32; 0]) -> Self {
        Self::Square
    }

    fn to_ports(&self) -> [f32; 0] {
        []
    }
}

fn main() {}
