/// Which license a plugin asks for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum Licensing {
    /// No check.
    #[default]
    Off,
    /// The license of [`Info::uri`](crate::Info::uri), else the free license.
    Free,
    /// The license of [`Info::uri`](crate::Info::uri).
    Commercial,
}

impl Licensing {
    /// Whether a license is asked for; the check needs the `licensing` feature.
    #[must_use]
    pub const fn checks(self) -> bool {
        !matches!(self, Self::Off)
    }
}
