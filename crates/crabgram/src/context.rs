use core::marker::PhantomData;

use crate::{
    Field, Plugin, Refusal, Value,
    access::{Requestable, Stateful, Writable},
    anagram::FRAMES,
    channels::Outputs,
    port::Layout,
    runtime::host::Requests,
};

/// Whether a block is enabled, reset or activated, and its tempo.
#[derive(Clone, Copy)]
pub struct Status {
    pub enabled: bool,
    pub reset: bool,
    pub activated: bool,
    pub bpm: f32,
}

/// One block for [`Plugin::run`]: its audio, status and tempo.
///
/// ```
/// # #![feature(generic_const_exprs)]
/// use crabgram::prelude::*;
///
/// # #[derive(Controls)] struct Params { level: f32 }
/// struct Swap;
///
/// impl Plugin for Swap {
///     # type Controls = Params;
///     const INFO: Info = info! {
///         uri: c"urn:example:swap",
///         name: "Swap",
///         description: "Swaps the channels.",
///         abbreviation: "SWP",
///         category: Utility,
///         channels: 2,
///     };
///
///     fn new() -> Self {
///         Self
///     }
///
///     fn run(&mut self, _: &Params, context: &mut Context<'_, Self>) {
///         let [left, right] = context.buffers();
///
///         core::mem::swap(left, right);
///     }
/// }
/// ```
pub struct Context<'a, P: Plugin> {
    outputs: Outputs<'a>,
    status: Status,
    requests: &'a Requests,
    values: &'a mut [f32],
    plugin: PhantomData<P>,
}

impl<'a, P: Plugin> Context<'a, P> {
    /// # Safety
    ///
    /// `outputs` must hold [`P::CHANNELS`](crate::Counts::CHANNELS) channels.
    pub(crate) unsafe fn new(
        outputs: Outputs<'a>,
        status: Status,
        requests: &'a Requests,
        values: &'a mut [f32],
    ) -> Self {
        debug_assert_eq!(outputs.len(), P::CHANNELS, "one output per channel");

        Self {
            outputs,
            status,
            requests,
            values,
            plugin: PhantomData,
        }
    }

    /// Whether the block is on.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.status.enabled
    }

    /// Whether to clear state before this block: after activation and when the reset trigger rises.
    #[must_use]
    pub const fn is_reset(&self) -> bool {
        self.status.reset
    }

    /// Whether this is the first block since activation, which also [resets](Self::is_reset).
    #[must_use]
    pub const fn is_activated(&self) -> bool {
        self.status.activated
    }

    /// The host tempo in BPM, between [`MIN_BPM`](crate::anagram::MIN_BPM) and
    /// [`MAX_BPM`](crate::anagram::MAX_BPM); `MIN_BPM` if NaN.
    #[must_use]
    pub const fn bpm(&self) -> f32 {
        self.status.bpm
    }

    const fn port(position: usize) -> u32 {
        Layout::of::<P>().control(position as u32)
    }

    /// Writes the output `field`, clamped to its range; the next block's `controls` hold it.
    pub fn write<A: Writable, V: Value>(&mut self, field: Field<P::Controls, A, V>, value: V) {
        self.values[field.position()] = field.control().clamp(value.to_port());
    }

    /// Asks the host to put the input `field` in `state`, an [`Activity`](crate::Activity) or, for
    /// a `blockable` input, a [`ControlState`](crate::ControlState). Only a change or a refused
    /// state is sent, so call it on every block.
    ///
    /// # Errors
    ///
    /// When the host refuses the state or lacks the feature ([`Refusal::Unsupported`]).
    ///
    /// ```
    /// # #![feature(generic_const_exprs)]
    /// use crabgram::prelude::*;
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     #[control(default = 3.0, min = 0.1, max = 12.0, unit = Hertz)]
    ///     rate: f32,
    ///     #[control(default = 0.5, blockable)]
    ///     depth: f32,
    ///     sync: bool,
    /// }
    ///
    /// struct Tremolo;
    ///
    /// impl Plugin for Tremolo {
    ///     type Controls = Params;
    ///
    ///     const INFO: Info = info! {
    ///         uri: c"urn:example:tremolo",
    ///         name: "Tremolo",
    ///         description: "",
    ///         abbreviation: "TRM",
    ///         category: Modulator,
    ///         channels: 1,
    ///     };
    ///
    ///     fn new() -> Self {
    ///         Self
    ///     }
    ///
    ///     fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
    ///         let (rate, depth) = if controls.sync {
    ///             (Activity::Inactive, ControlState::Blocked)
    ///         } else {
    ///             (Activity::Active, ControlState::Normal)
    ///         };
    ///         let _ = context.set_state(Params::RATE, rate);
    ///         let _ = context.set_state(Params::DEPTH, depth);
    ///     }
    /// }
    /// ```
    pub fn set_state<A: Stateful, V>(
        &self,
        field: Field<P::Controls, A, V>,
        state: A::State,
    ) -> Result<(), Refusal> {
        let position = field.position();

        self.requests
            .update_state(position, Self::port(position), state.into())
    }

    /// Asks the host to set the input `field` to `value`, clamped to its range. A taken request is
    /// sent again only once the input moves.
    ///
    /// # Errors
    ///
    /// When the host refuses the change or lacks the feature ([`Refusal::Unsupported`]).
    pub fn request_change<A: Requestable, V: Value>(
        &self,
        field: Field<P::Controls, A, V>,
        value: V,
    ) -> Result<(), Refusal> {
        let position = field.position();

        self.requests.request_change(
            position,
            Self::port(position),
            field.control().quantize(value.to_port()),
            self.values[position],
        )
    }
}

impl<P: Plugin> Context<'_, P>
where
    [(); P::CHANNELS]:,
{
    /// One [`FRAMES`]-sample buffer per channel: the input, non-finite samples silenced, to be
    /// overwritten with the output.
    pub fn buffers(&mut self) -> [&mut [f32; FRAMES]; P::CHANNELS] {
        // SAFETY: `new` takes one output per channel.
        unsafe { self.outputs.exact() }
    }
}
