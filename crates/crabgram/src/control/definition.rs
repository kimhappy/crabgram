use super::{Direction, Kind, Unit, Value, WidgetKind};

/// One control port, built by [`#[derive(Controls)]`](derive@crate::Controls).
///
/// ```
/// use crabgram::{Controls, Unit, WidgetKind};
///
/// #[derive(Controls)]
/// struct Params {
///     #[control(default = 3.0, min = 0.1, max = 12.0, unit = Hertz, logarithmic)]
///     rate: f32,
///     sync: bool,
/// }
///
/// assert_eq!(Params::RATE.as_ref().widget(), WidgetKind::Knob);
/// assert_eq!(Params::RATE.as_ref().unit(), Some(Unit::Hertz));
/// assert_eq!(Params::SYNC.as_ref().widget(), WidgetKind::Toggle);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Control {
    symbol: &'static str,
    name: &'static str,
    direction: Direction,
    kind: Kind,
    unit: Option<Unit>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    flags: Flags,
}

/// The port properties of a control besides its values, each off by default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is one port property of its own"
)]
pub struct Flags {
    /// `pprops:expensive`.
    pub expensive: bool,
    /// `pprops:notOnGUI`.
    pub hidden: bool,
    /// `dg:savedToPreset`.
    pub saved: bool,
    /// `dg:mayUpdateBlockedState`, for [`Context::set_state`](crate::Context::set_state).
    pub blockable: bool,
}

impl AsRef<Self> for Control {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Control {
    /// A control with no rule checked: [`ControlProblem::of`](super::ControlProblem::of) checks them.
    pub(super) const fn unchecked(
        symbol: &'static str,
        name: &'static str,
        direction: Direction,
        kind: Kind,
        unit: Option<Unit>,
        flags: Flags,
    ) -> Self {
        Self {
            symbol,
            name,
            direction,
            kind,
            unit,
            flags,
        }
    }

    /// The port symbol: ASCII letters, digits and underscores, unique in the plugin.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        self.symbol
    }

    /// The display name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) const fn direction(&self) -> Direction {
        self.direction
    }

    /// The values it takes.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The unit, if any.
    #[must_use]
    pub const fn unit(&self) -> Option<Unit> {
        self.unit
    }

    pub(crate) const fn flags(&self) -> Flags {
        self.flags
    }

    /// Whether it is `pprops:expensive`.
    #[must_use]
    pub const fn is_expensive(&self) -> bool {
        self.flags.expensive
    }

    /// Whether it is `pprops:notOnGUI`.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.flags.hidden
    }

    /// Whether it is `dg:savedToPreset`.
    #[must_use]
    pub const fn is_saved(&self) -> bool {
        self.flags.saved
    }

    /// Whether it is `dg:mayUpdateBlockedState`, for
    /// [`Context::set_state`](crate::Context::set_state).
    #[must_use]
    pub const fn is_blockable(&self) -> bool {
        self.flags.blockable
    }

    /// Whether the plugin writes the value.
    #[must_use]
    pub const fn is_output(&self) -> bool {
        matches!(self.direction, Direction::Output)
    }

    /// Whether the control is neither [expensive](Self::is_expensive),
    /// [blockable](Self::is_blockable) nor an output.
    #[must_use]
    pub const fn follows_scenes(&self) -> bool {
        !self.is_output() && !self.is_expensive() && !self.is_blockable()
    }

    pub(crate) const fn is_logarithmic(&self) -> bool {
        matches!(
            self.kind,
            Kind::Float {
                logarithmic: true,
                ..
            }
        )
    }

    /// The labels of a list, empty otherwise.
    #[must_use]
    pub const fn labels(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Choice { labels, .. } => labels,
            _ => &[],
        }
    }

    /// The widget of this control.
    #[must_use]
    pub const fn widget(&self) -> WidgetKind {
        match (self.kind, self.direction) {
            (Kind::Toggle { .. }, _) => WidgetKind::Toggle,
            (_, Direction::Output) => WidgetKind::Meter,
            (Kind::Choice { .. }, Direction::Input) => WidgetKind::List,
            (Kind::Float { .. } | Kind::Integer { .. }, Direction::Input) => WidgetKind::Knob,
        }
    }

    /// The default as a port value.
    #[must_use]
    pub const fn default_value(&self) -> f32 {
        match self.kind {
            Kind::Float { default, .. } => default,
            Kind::Integer { default, .. } => default as f32,
            Kind::Toggle { default } => default as u8 as f32,
            Kind::Choice { default, .. } => default as f32,
        }
    }

    /// The smallest port value.
    #[must_use]
    pub const fn min(&self) -> f32 {
        match self.kind {
            Kind::Float { min, .. } => min,
            Kind::Integer { min, .. } => min as f32,
            Kind::Toggle { .. } | Kind::Choice { .. } => 0.0,
        }
    }

    /// The largest port value.
    #[must_use]
    pub const fn max(&self) -> f32 {
        match self.kind {
            Kind::Float { max, .. } => max,
            Kind::Integer { max, .. } => max as f32,
            Kind::Toggle { .. } => 1.0,
            Kind::Choice { labels, .. } => labels.len().saturating_sub(1) as f32,
        }
    }

    /// The number of values; `None` for `f32`.
    ///
    /// ```
    /// use crabgram::Controls;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(min = 0, max = 16)]
    ///     steps: u32,
    /// }
    ///
    /// assert_eq!(Params::STEPS.as_ref().value_count(), Some(17));
    /// ```
    #[must_use]
    pub const fn value_count(&self) -> Option<u32> {
        match self.kind {
            Kind::Float { .. } => None,
            Kind::Integer { min, max, .. } => Some(max.abs_diff(min).saturating_add(1)),
            Kind::Toggle { .. } => Some(2),
            Kind::Choice { labels, .. } => Some(labels.len() as u32),
        }
    }

    /// Keeps a port value inside the range; a NaN becomes the default.
    ///
    /// ```
    /// use crabgram::Controls;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(default = 0.5)]
    ///     level: f32,
    /// }
    ///
    /// assert_eq!(Params::LEVEL.as_ref().clamp(f32::NAN), 0.5);
    /// assert_eq!(Params::LEVEL.as_ref().clamp(2.0), 1.0);
    /// ```
    #[must_use]
    pub const fn clamp(&self, raw: f32) -> f32 {
        if raw.is_nan() {
            self.default_value()
        } else {
            raw.max(self.min()).min(self.max())
        }
    }

    /// Clamps a port value and, unless `f32`, rounds it; a toggle is on above 0.
    ///
    /// ```
    /// use crabgram::Controls;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(min = 0, max = 4)]
    ///     steps: u8,
    ///     sync: bool,
    /// }
    ///
    /// assert_eq!(Params::STEPS.as_ref().quantize(2.6), 3.0);
    /// assert_eq!(Params::STEPS.as_ref().quantize(9.0), 4.0);
    /// assert_eq!(Params::SYNC.as_ref().quantize(0.1), 1.0);
    /// ```
    #[must_use]
    pub fn quantize(&self, raw: f32) -> f32 {
        let value = self.clamp(raw);

        match self.kind {
            Kind::Float { .. } => value,
            Kind::Toggle { .. } => bool::from_port(value, self).to_port(),
            Kind::Integer { .. } | Kind::Choice { .. } => value.round(),
        }
    }

    /// The label index of a port value.
    #[must_use]
    pub const fn label_index(&self, raw: f32) -> usize {
        self.clamp(raw).round() as usize
    }

    /// Where a port value sits in the range, 0 to 1.
    ///
    /// ```
    /// use crabgram::Controls;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(default = 1.0, min = 1.0, max = 100.0, unit = Hertz, logarithmic)]
    ///     rate: f32,
    /// }
    ///
    /// let rate = Params::RATE.as_ref();
    /// assert!((rate.normalized(10.0) - 0.5).abs() < 1.0e-6);
    /// assert_eq!(rate.normalized(1000.0), 1.0);
    /// ```
    #[must_use]
    pub fn normalized(&self, raw: f32) -> f32 {
        let (min, max) = self.scaled_range();
        let span = max - min;

        if span > 0.0 {
            (self.scaled(self.clamp(raw)) - min) / span
        } else {
            0.0
        }
    }

    /// The clamped inverse of [`normalized`](Self::normalized); a NaN becomes the default.
    ///
    /// ```
    /// use crabgram::Controls;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(default = 1.0, min = 1.0, max = 100.0, unit = Hertz, logarithmic)]
    ///     rate: f32,
    /// }
    ///
    /// let rate = Params::RATE.as_ref();
    /// assert!((rate.denormalized(0.5) - 10.0).abs() < 1.0e-4);
    /// assert_eq!(rate.denormalized(f32::NAN), 1.0);
    /// ```
    #[must_use]
    pub fn denormalized(&self, normalized: f32) -> f32 {
        let (min, max) = self.scaled_range();

        self.clamp(self.unscaled(min + normalized.clamp(0.0, 1.0) * (max - min)))
    }

    fn scaled_range(&self) -> (f32, f32) {
        (self.scaled(self.min()), self.scaled(self.max()))
    }

    fn scaled(&self, raw: f32) -> f32 {
        if self.is_logarithmic() { raw.ln() } else { raw }
    }

    fn unscaled(&self, scaled: f32) -> f32 {
        if self.is_logarithmic() {
            scaled.exp()
        } else {
            scaled
        }
    }
}
