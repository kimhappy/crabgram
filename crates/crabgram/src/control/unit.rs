#[cfg(doc)]
use super::{Choice, Control};

/// The unit of a control.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Unit {
    /// Cents, `unit:cent`.
    Cent,
    /// Decibels, `unit:db`.
    Decibel,
    /// Hertz, `unit:hz`.
    Hertz,
    /// Kilohertz, `unit:khz`.
    KiloHertz,
    /// Megahertz, `unit:mhz`.
    MegaHertz,
    /// Milliseconds, `unit:ms`.
    Millisecond,
    /// One decimal place without a unit, `dg:oneDecimalPoint`.
    OneDecimalPoint,
    /// Percent, `unit:pc`.
    Percent,
    /// Seconds, `unit:s`.
    Second,
    /// Semitones, `unit:semitone12TET`.
    Semitone,
}

impl Unit {
    /// Whether a logarithmic control may use this unit.
    #[must_use]
    pub(crate) const fn allows_logarithmic(self) -> bool {
        matches!(self, Self::Hertz | Self::Millisecond | Self::Second)
    }

    /// The unit as a Turtle prefixed name.
    #[must_use]
    pub const fn curie(self) -> &'static str {
        match self {
            Self::Cent => "unit:cent",
            Self::Decibel => "unit:db",
            Self::Hertz => "unit:hz",
            Self::KiloHertz => "unit:khz",
            Self::MegaHertz => "unit:mhz",
            Self::Millisecond => "unit:ms",
            Self::OneDecimalPoint => "dg:oneDecimalPoint",
            Self::Percent => "unit:pc",
            Self::Second => "unit:s",
            Self::Semitone => "unit:semitone12TET",
        }
    }
}

/// Who writes a control.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    /// The plugin reads the value.
    Input,
    /// The plugin writes the value.
    Output,
}

/// The widget of a control: see [`Control::widget`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WidgetKind {
    /// An `f32` or integer input.
    Knob,
    /// A [`Choice`] input.
    List,
    /// A `bool` input, or a `bool` output the plugin moves.
    Toggle,
    /// An `f32`, integer or [`Choice`] output.
    Meter,
}

impl WidgetKind {
    /// The lowercase name the styling uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Knob => "knob",
            Self::List => "list",
            Self::Toggle => "toggle",
            Self::Meter => "meter",
        }
    }
}
