//! Control ports.

mod controls;
mod definition;
mod kind;
#[doc(hidden)]
pub mod make;
mod problem;
#[cfg(test)]
mod tests;
mod unit;
mod value;
#[cfg(feature = "serde")]
mod wire;

pub use self::{
    controls::Controls,
    definition::{Control, Flags},
    kind::Kind,
    problem::ControlProblem,
    unit::{Direction, Unit, WidgetKind},
    value::{Choice, Value},
};

const EXACT_INTEGERS: i32 = 1 << f32::MANTISSA_DIGITS;
