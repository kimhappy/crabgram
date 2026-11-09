/// The LV2 version of the plugin.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Version {
    pub(super) minor: u32,
    pub(super) micro: u32,
}

impl Version {
    /// `lv2:minorVersion`.
    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }

    /// `lv2:microVersion`.
    #[must_use]
    pub const fn micro(self) -> u32 {
        self.micro
    }
}
