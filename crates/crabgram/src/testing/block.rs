use core::fmt;

use crate::{
    Plugin,
    anagram::{FRAMES, MIN_BPM},
};

/// One block of a [`Rig`](super::Rig): silence, default controls, enabled, no reset, `MIN_BPM` by
/// default.
///
/// It is [`Clone`], [`Debug`](fmt::Debug) and [`PartialEq`] when `P::Controls` is.
///
/// ```
/// # #![feature(generic_const_exprs)]
/// use crabgram::{prelude::*, testing::Block};
///
/// #[derive(Controls, Clone, Debug, PartialEq)]
/// struct Params {
///     level: f32,
/// }
///
/// # struct Silence;
/// # impl Plugin for Silence {
/// #     type Controls = Params;
/// #     const INFO: Info = info! {
/// #         uri: c"urn:example:silence",
/// #         name: "Silence",
/// #         description: "",
/// #         abbreviation: "SIL",
/// #         category: Utility,
/// #         channels: 1,
/// #     };
/// #     fn new() -> Self { Self }
/// #     fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
/// # }
/// let block = Block::<Silence> {
///     audio: [[0.5; 16]],
///     controls: Params { level: 0.25 },
///     ..Block::default()
/// };
///
/// assert_eq!(block.clone(), block);
/// ```
pub struct Block<P: Plugin>
where
    [(); P::CHANNELS]:,
{
    /// The audio, [`FRAMES`] samples per channel: input before the block, output after it.
    pub audio: [[f32; FRAMES]; P::CHANNELS],
    /// Inputs moved to any requested value and outputs as written, clamped to range. Outputs
    /// passed in are ignored.
    pub controls: P::Controls,
    /// The enabled port: whether the block is on.
    pub enabled: bool,
    /// Whether the reset trigger is raised.
    pub reset: bool,
    /// The tempo in beats per minute, read by [`Context::bpm`](crate::Context::bpm).
    pub bpm: f32,
}

impl<P: Plugin> Default for Block<P>
where
    [(); P::CHANNELS]:,
{
    fn default() -> Self {
        Self {
            audio: [[0.0; FRAMES]; P::CHANNELS],
            controls: P::Controls::default(),
            enabled: true,
            reset: false,
            bpm: MIN_BPM,
        }
    }
}

impl<P: Plugin> Clone for Block<P>
where
    [(); P::CHANNELS]:,
    P::Controls: Clone,
{
    fn clone(&self) -> Self {
        Self {
            audio: self.audio,
            controls: self.controls.clone(),
            enabled: self.enabled,
            reset: self.reset,
            bpm: self.bpm,
        }
    }
}

impl<P: Plugin> fmt::Debug for Block<P>
where
    [(); P::CHANNELS]:,
    P::Controls: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Block")
            .field("audio", &self.audio)
            .field("controls", &self.controls)
            .field("enabled", &self.enabled)
            .field("reset", &self.reset)
            .field("bpm", &self.bpm)
            .finish()
    }
}

impl<P: Plugin> PartialEq for Block<P>
where
    [(); P::CHANNELS]:,
    P::Controls: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.audio == other.audio
            && self.controls == other.controls
            && self.enabled == other.enabled
            && self.reset == other.reset
            && self.bpm == other.bpm
    }
}
