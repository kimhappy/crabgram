//! How a `Choice` field reads a port value.

use crabgram::{Choice, Controls, Value};

#[derive(Debug, PartialEq, Eq, Choice)]
enum Wave {
    Sine,
    Square,
    Saw,
}

#[derive(Controls)]
struct Params {
    #[control(default = Wave::Square)]
    wave: Wave,
}

#[test]
fn every_port_value_reads_the_nearest_variant() {
    let read = |raw| Wave::from_port(raw, Params::WAVE.as_ref());

    assert_eq!([0.0, 1.0, 2.0].map(read), Wave::ALL);
    assert_eq!(
        [0.4, 0.6, 1.5, 2.4].map(read),
        [Wave::Sine, Wave::Square, Wave::Saw, Wave::Saw]
    );
    assert_eq!(
        [-1.0, 3.0, f32::NEG_INFINITY, f32::INFINITY].map(read),
        [Wave::Sine, Wave::Saw, Wave::Sine, Wave::Saw]
    );
    assert_eq!(read(f32::NAN), Wave::Square, "a NaN reads the default");
}

#[derive(Controls)]
struct Wide {
    #[control(min = 0.0, max = 100.0)]
    amount: f32,
}

#[test]
fn a_control_past_the_variants_reads_the_last() {
    assert_eq!(Wave::from_port(50.0, Wide::AMOUNT.as_ref()), Wave::Saw);
}

#[test]
fn a_variant_names_its_label() {
    assert!(
        Wave::ALL
            .iter()
            .map(|it| it.label())
            .eq(["Sine", "Square", "Saw"])
    );
}

mod without_the_trait {
    #[derive(Debug, PartialEq, Eq, crabgram::Choice)]
    enum Mode {
        Boost,
        #[choice(label = "Cut Off")]
        Cut,
    }

    const CUT: &str = Mode::Cut.label();

    #[test]
    fn the_derive_gives_the_enum_a_const_label() {
        assert_eq!(CUT, "Cut Off");
        assert_eq!(Mode::Boost.label(), "Boost");
    }
}
