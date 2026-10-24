#![allow(
    clippy::must_use_candidate,
    reason = "the module is hidden: only the derive macros call it"
)]

use super::{Control, ControlProblem, Direction, Flags, Kind, Unit};
use crate::text::is_blank;

/// The control of a port, with every rule checked once.
///
/// # Panics
///
/// At compile time, when no port takes the control: see [`ControlProblem`](super::ControlProblem).
pub const fn control(
    symbol: &'static str,
    name: &'static str,
    direction: Direction,
    kind: Kind,
    unit: Option<Unit>,
    flags: Flags,
) -> Control {
    let control = Control::unchecked(symbol, name, direction, kind, unit, flags);

    if let Some(problem) = ControlProblem::of(&control) {
        panic!("{}", problem.message());
    }

    control
}

/// `name`, checked on its own so that a blank one fails where it is written.
///
/// # Panics
///
/// At compile time, when `name` is blank.
pub const fn name(name: &'static str) -> &'static str {
    assert!(!is_blank(name), "{}", ControlProblem::BlankName.message());

    name
}

pub const fn float(default: f32, min: f32, max: f32, logarithmic: bool) -> Kind {
    Kind::Float {
        default,
        min,
        max,
        logarithmic,
    }
}

/// Bounds are `i64` to fit any field type; they saturate to `i32`, so one past ±2^24 stays past it.
pub const fn integer(default: i64, min: i64, max: i64) -> Kind {
    Kind::Integer {
        default: saturated(default),
        min: saturated(min),
        max: saturated(max),
    }
}

const fn saturated(value: i64) -> i32 {
    if value < i32::MIN as i64 {
        i32::MIN
    } else if value > i32::MAX as i64 {
        i32::MAX
    } else {
        value as i32
    }
}

pub const fn toggle(default: bool) -> Kind {
    Kind::Toggle { default }
}

pub const fn choice(default: usize, labels: &'static [&'static str]) -> Kind {
    Kind::Choice { default, labels }
}

/// The labels of a `Choice`, checked on their own so that an enum no control uses fails too.
///
/// # Panics
///
/// At compile time, when no list takes `labels`.
pub const fn labels(labels: &'static [&'static str]) -> &'static [&'static str] {
    if let Some(problem) = super::kind::labels_problem(labels) {
        panic!("{}", problem.message());
    }

    labels
}
