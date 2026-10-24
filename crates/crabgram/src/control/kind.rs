#[cfg(doc)]
use super::{Choice, Control};
use super::{ControlProblem, EXACT_INTEGERS};
use crate::text::{is_blank, same};

/// The values a control takes, with its default.
///
/// With `serde`, deserializing leaks the labels; only [`Control`] checks them.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs, reason = "each field holds what it names")]
pub enum Kind {
    /// An `f32` field; `logarithmic` is `pprops:logarithmic`.
    Float {
        default: f32,
        min: f32,
        max: f32,
        logarithmic: bool,
    },
    /// An integer field, `lv2:integer`.
    Integer { default: i32, min: i32, max: i32 },
    /// A `bool` field, `lv2:toggled`.
    Toggle { default: bool },
    /// A [`Choice`] field, `lv2:enumeration`, with `default` a label index.
    Choice {
        default: usize,
        /// Labels valued 0, 1, 2, and so on.
        #[cfg_attr(
            feature = "serde",
            serde(deserialize_with = "super::wire::leaked_labels")
        )]
        labels: &'static [&'static str],
    },
}

const fn holds(min: f32, value: f32, max: f32) -> bool {
    min <= value && value <= max
}

impl Kind {
    pub(super) const fn problem(&self) -> Option<ControlProblem> {
        match *self {
            Self::Float {
                default,
                min,
                max,
                logarithmic,
            } => {
                if !min.is_finite() || !max.is_finite() {
                    Some(ControlProblem::NonFiniteBound)
                } else if min >= max {
                    Some(ControlProblem::EmptyRange)
                } else if !holds(min, default, max) {
                    Some(ControlProblem::DefaultOutsideRange)
                } else if logarithmic && min <= 0.0 {
                    Some(ControlProblem::LogarithmicFromZero)
                } else {
                    None
                }
            }
            Self::Integer { default, min, max } => {
                if min < -EXACT_INTEGERS || max > EXACT_INTEGERS {
                    Some(ControlProblem::PastExactIntegers)
                } else if min >= max {
                    Some(ControlProblem::EmptyRange)
                } else if default < min || default > max {
                    Some(ControlProblem::DefaultOutsideRange)
                } else {
                    None
                }
            }
            Self::Toggle { .. } => None,
            Self::Choice { default, labels } => match labels_problem(labels) {
                None if default >= labels.len() => Some(ControlProblem::DefaultPastLastLabel),
                problem => problem,
            },
        }
    }
}

const fn is_repeated(labels: &[&str], position: usize) -> bool {
    let mut earlier = 0;

    while earlier < position {
        if same(labels[earlier], labels[position]) {
            return true;
        }

        earlier += 1;
    }

    false
}

/// Why no list takes `labels`, if none does.
pub(super) const fn labels_problem(labels: &[&str]) -> Option<ControlProblem> {
    if labels.len() < 2 {
        return Some(ControlProblem::TooFewLabels);
    }

    let mut position = 0;

    while position < labels.len() {
        if is_blank(labels[position]) {
            return Some(ControlProblem::BlankLabel);
        }

        if is_repeated(labels, position) {
            return Some(ControlProblem::RepeatedLabel);
        }

        position += 1;
    }

    None
}
