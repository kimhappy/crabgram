use crate::{Context, Controls, Info};

/// An LV2 plugin; hand the type to [`export!`](crate::export). The crate docs show one.
///
/// [`run`](Self::run) sees every block with audio and handles [`Context::is_enabled`] and
/// [`Context::is_reset`] on each.
pub trait Plugin: Counts + Send + Sized + 'static {
    /// The controls, one port per field.
    type Controls: Controls;

    /// The plugin description.
    const INFO: Info;

    /// A fresh instance, running at [`SAMPLE_RATE`](crate::anagram::SAMPLE_RATE).
    ///
    /// Allocate everything here: [`run`](Self::run) is real-time.
    #[must_use]
    fn new() -> Self;

    /// Processes one block of [`FRAMES`](crate::anagram::FRAMES) frames on the real-time thread.
    ///
    /// No allocation, locks, I/O or panics.
    ///
    /// `controls` holds the inputs as set and the outputs as last written, both clamped to range.
    fn run(&mut self, controls: &Self::Controls, context: &mut Context<'_, Self>);
}

mod seal {
    #[diagnostic::on_unimplemented(
        message = "`Counts` comes with `Plugin`: implement `Plugin` for `{Self}` instead"
    )]
    pub trait Seal {}

    impl<P: super::Plugin> Seal for P {}
}

/// The counts of every [`Plugin`], derived from its [`INFO`](Plugin::INFO) and controls.
pub trait Counts: seal::Seal {
    /// The number of audio channels: the length of [`Context::buffers`](crate::Context::buffers).
    const CHANNELS: usize;

    /// The number of ports, audio and control.
    const PORTS: usize;
}

impl<P: Plugin> Counts for P {
    const CHANNELS: usize = P::INFO.channels() as usize;
    const PORTS: usize = crate::port::Layout::of::<P>().count() as usize;
}
