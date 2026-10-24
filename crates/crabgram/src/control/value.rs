use super::Control;

/// A control field type: `f32`, `bool`, integers up to 32 bits or a
/// [`#[derive(Choice)]`](derive@crate::Choice) enum.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a control type",
    note = "a control field is `f32`, `bool`, an integer up to 32 bits or an enum deriving `Choice`",
    note = "the derive knows types by name and takes any other name, a type alias included, for a \
            Choice enum: write the type an alias names"
)]
pub trait Value: Copy + crate::sealed::SealedValue + 'static {
    /// The value of a port, clamped to the range of `control`.
    fn from_port(raw: f32, control: &Control) -> Self;

    /// The port value.
    fn to_port(self) -> f32;
}

impl crate::sealed::SealedValue for f32 {}

impl Value for f32 {
    fn from_port(raw: f32, control: &Control) -> Self {
        control.clamp(raw)
    }

    fn to_port(self) -> f32 {
        self
    }
}

impl crate::sealed::SealedValue for bool {}

impl Value for bool {
    fn from_port(raw: f32, control: &Control) -> Self {
        control.clamp(raw) > 0.0
    }

    fn to_port(self) -> f32 {
        f32::from(u8::from(self))
    }
}

macro_rules! integer_values {
    ($($integer:ty),*) => {$(
        impl crate::sealed::SealedValue for $integer {}

        impl Value for $integer {
            fn from_port(raw: f32, control: &Control) -> Self {
                control.clamp(raw).round() as Self
            }

            fn to_port(self) -> f32 {
                f64::from(self) as f32
            }
        }
    )*};
}

integer_values!(i8, i16, i32, u8, u16, u32);

/// A fieldless enum a control selects by label.
///
/// Derive it: variants are valued in order and labelled by name or `#[choice(label = "...")]`.
/// The derive adds `Clone`, `Copy`, `const fn index` and `const fn label`.
///
/// ```
/// use crabgram::Choice;
///
/// #[derive(Debug, PartialEq, Choice)]
/// enum Shape {
///     Sine,
///     #[choice(label = "Saw Up")]
///     SawUp,
/// }
///
/// assert_eq!(Shape::LABELS, ["Sine", "Saw Up"]);
/// assert_eq!(Shape::ALL[1], Shape::SawUp);
/// assert_eq!(Shape::SawUp.index(), 1);
/// assert_eq!(Shape::SawUp.label(), "Saw Up");
/// ```
///
/// Fewer than two variants, generics, and a blank or repeated label fail to compile.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a Choice enum",
    note = "a control field is `f32`, `bool`, an integer up to 32 bits or an enum deriving `Choice`",
    note = "the derive knows types by name and takes any other name, a type alias included, for a \
            Choice enum: write the type an alias names"
)]
pub trait Choice: Value {
    /// The variants, in value order.
    const ALL: &'static [Self];

    /// The labels, in value order.
    const LABELS: &'static [&'static str];
}
