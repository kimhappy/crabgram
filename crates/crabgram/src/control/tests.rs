use super::{Control, Direction, Flags, Kind, Unit, make};

pub(super) fn input(symbol: &'static str, kind: Kind, unit: Option<Unit>) -> Control {
    make::control(
        symbol,
        symbol,
        Direction::Input,
        kind,
        unit,
        Flags::default(),
    )
}

#[test]
fn denormalizing_undoes_normalizing() {
    let linear = input("level", make::float(0.0, -24.0, 12.0, false), None);
    let logarithmic = input("rate", make::float(1.0, 0.1, 20.0, true), Some(Unit::Hertz));
    let near = |a: f32, b: f32| (a - b).abs() <= 1.0e-5 * a.abs().max(1.0);

    let round_trips = |control: Control| {
        (-40..=40).map(|step| step as f32 * 0.75).all(|raw| {
            near(
                control.denormalized(control.normalized(raw)),
                control.clamp(raw),
            )
        }) && (0..=20).map(|step| step as f32 / 20.0).all(|normalized| {
            near(
                control.normalized(control.denormalized(normalized)),
                normalized,
            )
        })
    };

    assert!(round_trips(linear));
    assert!(round_trips(logarithmic));
}

#[test]
fn clamping_keeps_a_value_inside_the_range() {
    let level = input("level", make::float(0.5, -1.0, 2.0, false), None);
    let steps = input("steps", make::integer(0, -4, 4), None);

    assert_eq!(
        [
            0.25,
            -1.0,
            2.0,
            -3.0,
            5.0,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NAN
        ]
        .map(|raw| level.clamp(raw)),
        [0.25, -1.0, 2.0, -1.0, 2.0, -1.0, 2.0, 0.5]
    );
    assert_eq!(
        [-9.0, 3.5, 9.0, f32::NAN].map(|raw| steps.clamp(raw)),
        [-4.0, 3.5, 4.0, 0.0]
    );
}
