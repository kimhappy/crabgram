//! Reads controls back from the data [`serde::Serialize`] writes.

use serde::{Deserialize, Deserializer, de::Error};

use super::{Control, ControlProblem, Direction, Flags, Kind, Unit};

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

/// The labels of a list, leaked for the rest of the process.
pub(super) fn leaked_labels<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<&'static [&'static str], D::Error> {
    Vec::<String>::deserialize(deserializer)
        .map(|labels| &*Box::leak(labels.into_iter().map(leak).collect::<Box<[_]>>()))
}

/// The fields of a [`Control`] as read, before they are checked.
#[derive(Deserialize)]
pub(super) struct Unchecked {
    symbol: String,
    name: String,
    direction: Direction,
    kind: Kind,
    unit: Option<Unit>,
    #[serde(flatten)]
    flags: Flags,
}

/// Why a control read back is no port; its strings stay leaked.
#[derive(Debug, thiserror::Error)]
#[error("control {symbol}: {problem}")]
pub(super) struct Rejected {
    symbol: String,
    problem: ControlProblem,
}

impl TryFrom<Unchecked> for Control {
    type Error = Rejected;

    fn try_from(fields: Unchecked) -> Result<Self, Rejected> {
        let Unchecked {
            symbol,
            name,
            direction,
            kind,
            unit,
            flags,
        } = fields;

        let control = Self::unchecked(leak(symbol), leak(name), direction, kind, unit, flags);

        ControlProblem::of(&control).map_or(Ok(control), |problem| {
            Err(Rejected {
                symbol: control.symbol().to_owned(),
                problem,
            })
        })
    }
}

/// Reads a control and checks it: no control that no port takes comes out.
impl<'de> Deserialize<'de> for Control {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Unchecked::deserialize(deserializer)
            .and_then(|fields| Self::try_from(fields).map_err(D::Error::custom))
    }
}

#[cfg(test)]
mod tests {
    use super::{Control, ControlProblem, Direction, Flags, Kind};
    use crate::control::{make, tests::input};

    fn round_trip(control: &Control) -> serde_json::Result<Control> {
        serde_json::from_value(serde_json::to_value(control)?)
    }

    #[test]
    fn every_kind_survives_a_round_trip() {
        let controls = [
            input("level", make::float(0.5, -1.0, 2.0, false), None),
            make::control(
                "steps",
                "Steps",
                Direction::Output,
                make::integer(1, -4, 4),
                None,
                Flags::default(),
            ),
            input("sync", make::toggle(true), None),
            input("shape", make::choice(1, &["Sine", "Saw", "Square"]), None),
            make::control(
                "gain",
                "Gain",
                Direction::Input,
                make::float(0.0, -24.0, 12.0, false),
                None,
                Flags {
                    hidden: true,
                    saved: true,
                    ..Flags::default()
                },
            ),
        ];

        assert!(
            controls
                .iter()
                .all(|control| round_trip(control).is_ok_and(|read| read == *control))
        );
    }

    #[test]
    fn a_missing_flag_reads_as_off() {
        let json = r#"{"symbol":"level","name":"Level","direction":"Input","unit":null,
            "kind":{"Float":{"default":0.0,"min":0.0,"max":1.0,"logarithmic":false}}}"#;
        let control: Control = serde_json::from_str(json).unwrap();

        assert_eq!(control.flags(), Flags::default());
    }

    #[test]
    fn a_control_no_port_takes_is_not_read() {
        let json = r#"{"symbol":"shape","name":"Shape","direction":"Input","unit":null,
            "kind":{"Choice":{"default":0,"labels":["Only"]}}}"#;
        let error = serde_json::from_str::<Control>(json).unwrap_err();

        assert!(error.to_string().starts_with("control shape: a list needs"));
    }

    #[test]
    fn labels_or_a_unit_the_derive_refuses_are_not_read() {
        let read = |unit: &str, kind: &str| {
            let json = format!(
                r#"{{"symbol":"shape","name":"Shape","direction":"Input","unit":{unit},
                "kind":{kind}}}"#
            );

            serde_json::from_str::<Control>(&json).map_err(|it| it.to_string())
        };
        let refused = |unit: &str, kind: &str, problem: ControlProblem| {
            read(unit, kind).is_err_and(|it| it.ends_with(problem.message()))
        };
        let choice = |labels: &str| format!(r#"{{"Choice":{{"default":0,"labels":{labels}}}}}"#);

        assert!(read("null", &choice(r#"["Sine","Saw"]"#)).is_ok());
        assert!(refused(
            "null",
            &choice(r#"["Sine"," "]"#),
            ControlProblem::BlankLabel
        ));
        assert!(refused(
            "null",
            &choice(r#"["Saw","Saw"]"#),
            ControlProblem::RepeatedLabel
        ));
        assert!(refused(
            r#""Hertz""#,
            &choice(r#"["Sine","Saw"]"#),
            ControlProblem::UnitOnChoice
        ));
        assert!(refused(
            r#""Hertz""#,
            r#"{"Toggle":{"default":false}}"#,
            ControlProblem::UnitOnToggle
        ));
    }

    #[test]
    fn a_symbol_or_name_the_derive_refuses_is_not_read() {
        let read = |symbol: &str, name: &str| {
            let json = format!(
                r#"{{"symbol":"{symbol}","name":"{name}","direction":"Input","unit":null,
                "kind":{{"Toggle":{{"default":false}}}}}}"#
            );

            serde_json::from_str::<Control>(&json).map_err(|it| it.to_string())
        };
        let refused = |symbol, name, problem: ControlProblem| {
            read(symbol, name).is_err_and(|it| it.ends_with(problem.message()))
        };

        assert!(read("sync", "Sync").is_ok());
        assert!(refused("", "Sync", ControlProblem::InvalidSymbol));
        assert!(refused("2x", "Sync", ControlProblem::InvalidSymbol));
        assert!(refused("größe", "Sync", ControlProblem::InvalidSymbol));
        assert!(refused("a-b", "Sync", ControlProblem::InvalidSymbol));
        assert!(refused("reset", "Sync", ControlProblem::FixedPortSymbol));
        assert!(refused("lv2_bpm", "Sync", ControlProblem::FixedPortSymbol));
        assert!(refused(
            "lv2_audio_out_2",
            "Sync",
            ControlProblem::FixedPortSymbol
        ));
        assert!(read("lv2_audio_in_", "Sync").is_ok());
        assert!(refused("sync", " ", ControlProblem::BlankName));
    }

    #[test]
    fn a_kind_reads_without_a_check() {
        let kind: Kind =
            serde_json::from_str(r#"{"Choice":{"default":9,"labels":["A","B"]}}"#).unwrap();

        assert_eq!(
            kind,
            Kind::Choice {
                default: 9,
                labels: &["A", "B"]
            }
        );
    }
}
