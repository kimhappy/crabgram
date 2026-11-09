use super::{Control, Direction, Kind};
use crate::{
    port::{AUDIO_IN, AUDIO_OUT, ENABLED, RESET, TEMPO},
    text::{is_blank, same, starts_with},
};

/// Why no port takes a control; the derive fails with its [`message`](Self::message).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, thiserror::Error)]
#[error("{}", self.message())]
#[non_exhaustive]
pub enum ControlProblem {
    /// The symbol is empty, starts with a digit or holds another character than an ASCII letter,
    /// digit or underscore.
    InvalidSymbol,
    /// The symbol is that of a port every plugin has: `reset`, `lv2_enabled`, `lv2_bpm`,
    /// `lv2_audio_in_<n>` or `lv2_audio_out_<n>`.
    FixedPortSymbol,
    /// The name is empty or only whitespace.
    BlankName,
    /// An `f32` bound is infinite or NaN.
    NonFiniteBound,
    /// The minimum is not below the maximum.
    EmptyRange,
    /// The default lies outside the range.
    DefaultOutsideRange,
    /// A logarithmic range starts at or below 0.
    LogarithmicFromZero,
    /// An integer range passes ±2^24, where `f32` skips integers.
    PastExactIntegers,
    /// Fewer than two labels.
    TooFewLabels,
    /// A label is empty or only whitespace.
    BlankLabel,
    /// Two labels are the same.
    RepeatedLabel,
    /// The default label is past the last one.
    DefaultPastLastLabel,
    /// A `bool` control has a unit.
    UnitOnToggle,
    /// A [`Choice`](super::Choice) control has a unit.
    UnitOnChoice,
    /// A logarithmic control lacks the unit Hertz, Millisecond or Second.
    LogarithmicWithoutLogarithmicUnit,
    /// An output is expensive.
    ExpensiveOutput,
    /// An output is `saved`.
    SavedOutput,
    /// An output is blockable.
    BlockableOutput,
    /// A shown input is marked `saved`.
    SavedWithoutHidden,
}

impl ControlProblem {
    /// The problem as a sentence.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidSymbol => {
                "a symbol is ASCII letters, digits and underscores, and does not start with a digit"
            }
            Self::FixedPortSymbol => {
                "the symbol is that of a port every plugin has: reset, lv2_enabled, lv2_bpm, \
                 lv2_audio_in_<n> or lv2_audio_out_<n>"
            }
            Self::BlankName => "the name cannot be blank",
            Self::NonFiniteBound => "a bound is not finite",
            Self::EmptyRange => "the minimum is not below the maximum",
            Self::DefaultOutsideRange => "the default lies outside the range",
            Self::LogarithmicFromZero => "a logarithmic range must start above 0",
            Self::PastExactIntegers => {
                "the range reaches past ±2^24, where an f32 port no longer holds every integer"
            }
            Self::TooFewLabels => "a list needs at least two labels",
            Self::BlankLabel => "a label cannot be blank: leave `label` out for the variant name",
            Self::RepeatedLabel => "two labels are the same: change one",
            Self::DefaultPastLastLabel => "the default is past the last label",
            Self::UnitOnToggle => "a bool control takes no unit",
            Self::UnitOnChoice => "a Choice control takes no unit",
            Self::LogarithmicWithoutLogarithmicUnit => {
                "a logarithmic control needs the unit Hertz, Millisecond or Second"
            }
            Self::ExpensiveOutput => "only an input can be expensive",
            Self::SavedOutput => "only an input can be saved to presets",
            Self::BlockableOutput => "only an input can be blocked",
            Self::SavedWithoutHidden => "`saved` is for a `hidden` input",
        }
    }
}

const fn is_symbol_tail(bytes: &[u8]) -> bool {
    match bytes {
        [] => true,
        [first, rest @ ..] => {
            (first.is_ascii_alphanumeric() || *first == b'_') && is_symbol_tail(rest)
        }
    }
}

const fn is_digits(bytes: &[u8]) -> bool {
    match bytes {
        [] => true,
        [first, rest @ ..] => first.is_ascii_digit() && is_digits(rest),
    }
}

const fn is_numbered(symbol: &[u8], prefix: &[u8]) -> bool {
    starts_with(symbol, prefix)
        && symbol.len() > prefix.len()
        && is_digits(symbol.split_at(prefix.len()).1)
}

impl ControlProblem {
    /// Why no port takes `control`, if none does.
    pub(super) const fn of(control: &Control) -> Option<Self> {
        if let Some(problem) = Self::of_symbol(control.symbol()) {
            return Some(problem);
        }

        if is_blank(control.name()) {
            return Some(Self::BlankName);
        }

        let (kind, unit, flags) = (control.kind(), control.unit(), control.flags());

        match control.direction() {
            Direction::Output if flags.expensive => Some(Self::ExpensiveOutput),
            Direction::Output if flags.saved => Some(Self::SavedOutput),
            Direction::Output if flags.blockable => Some(Self::BlockableOutput),
            Direction::Input if flags.saved && !flags.hidden => Some(Self::SavedWithoutHidden),
            _ => match kind.problem() {
                None if unit.is_some() && matches!(kind, Kind::Toggle { .. }) => {
                    Some(Self::UnitOnToggle)
                }
                None if unit.is_some() && matches!(kind, Kind::Choice { .. }) => {
                    Some(Self::UnitOnChoice)
                }
                None if matches!(
                    kind,
                    Kind::Float {
                        logarithmic: true,
                        ..
                    }
                ) && !matches!(unit, Some(unit) if unit.allows_logarithmic()) =>
                {
                    Some(Self::LogarithmicWithoutLogarithmicUnit)
                }
                problem => problem,
            },
        }
    }

    /// Why no port takes the symbol `symbol`, if none does.
    const fn of_symbol(symbol: &str) -> Option<Self> {
        match symbol.as_bytes() {
            [] | [b'0'..=b'9', ..] => Some(Self::InvalidSymbol),
            bytes if !is_symbol_tail(bytes) => Some(Self::InvalidSymbol),
            bytes
                if same(symbol, RESET)
                    || same(symbol, ENABLED)
                    || same(symbol, TEMPO)
                    || is_numbered(bytes, AUDIO_IN.as_bytes())
                    || is_numbered(bytes, AUDIO_OUT.as_bytes()) =>
            {
                Some(Self::FixedPortSymbol)
            }
            _ => None,
        }
    }
}
