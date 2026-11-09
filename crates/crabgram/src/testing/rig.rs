use core::{any::type_name, ffi::c_void, ptr};
use std::sync::Arc;

#[cfg(doc)]
use super::CountingAllocator;
use super::{
    Block,
    allocator::{counts_the_heap, heap_operations},
    recorder::Recorder,
};
use crate::{
    Controls, Field, Plugin, Value,
    access::Stateful,
    anagram::{FRAMES, MIN_BPM, SAMPLE_RATE},
    lv2::{self, Handle, HostFeatures},
    port::Layout,
};

/// One activated instance of `P`, processing in place.
///
/// A panic in [`Plugin::new`] or [`Plugin::run`] fails the test instead of aborting. The rig
/// records the states the plugin asks for and applies its change requests to the returned block.
///
/// ```
/// # #![feature(generic_const_exprs)]
/// use crabgram::{
///     prelude::*,
///     testing::{Block, Rig},
/// };
/// # #[global_allocator]
/// # static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;
///
/// #[derive(Controls)]
/// struct Params {
///     #[control(default = 1.0, min = 0.0, max = 2.0)]
///     gain: f32,
///     #[control(output)]
///     peak: f32,
/// }
///
/// struct Gain;
///
/// impl Plugin for Gain {
///     type Controls = Params;
///
///     const INFO: Info = info! {
///         uri: c"urn:example:gain",
///         name: "Gain",
///         description: "",
///         abbreviation: "GN",
///         category: Utility,
///         channels: 1,
///     };
///
///     fn new() -> Self {
///         Self
///     }
///
///     fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
///         let [samples] = context.buffers();
///
///         samples.iter_mut().for_each(|it| *it *= controls.gain);
///         let peak = samples.iter().fold(0.0, |peak, it| it.abs().max(peak));
///
///         context.write(Params::PEAK, peak);
///     }
/// }
///
/// let mut rig = Rig::<Gain>::new();
/// let block = rig.process(
///     Block {
///         audio: [[0.25; 16]],
///         controls: Params {
///             gain: 2.0,
///             ..Params::default()
///         },
///         ..Block::default()
///     },
/// );
///
/// assert_eq!(block.audio, [[0.5; 16]]);
/// assert_eq!(block.controls.peak, 0.5);
///
/// let mut next = block;
///
/// next.controls.gain = 1.0;
/// assert_eq!(rig.process(next).audio, [[0.5; 16]]);
///
/// rig.reactivate();
/// rig.process(Block::default());
/// ```
///
/// ```
/// # #![feature(generic_const_exprs)]
/// use crabgram::{
///     prelude::*,
///     testing::{Block, Rig},
/// };
/// # #[global_allocator]
/// # static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;
///
/// #[derive(Controls)]
/// struct Params {
///     #[control(default = 1.0, min = 0.1, max = 12.0, unit = Hertz, blockable)]
///     rate: f32,
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
///         if controls.sync {
///             let _ = context.request_change(Params::RATE, context.bpm() / 60.0);
///             let _ = context.set_state(Params::RATE, ControlState::Blocked);
///         } else {
///             let _ = context.set_state(Params::RATE, ControlState::Normal);
///         }
///     }
/// }
///
/// let mut rig = Rig::<Tremolo>::new();
///
/// assert_eq!(rig.state(Params::RATE), ControlState::Normal);
///
/// let block = rig.process(Block {
///     controls: Params {
///         sync: true,
///         ..Params::default()
///     },
///     ..Block::default()
/// });
///
/// assert_eq!(rig.state(Params::RATE), ControlState::Blocked);
/// assert_eq!(block.controls.rate, 40.0 / 60.0);
/// ```
pub struct Rig<P: Plugin>
where
    [(); P::PORTS]:,
{
    handle: Handle,
    flags: [f32; 2],
    controls: <P::Controls as Controls>::Values,
    bpm: f32,
    /// Whether the next block checks that `CountingAllocator` is the global allocator.
    checks_the_heap: bool,
    /// Dropped after `drop` cleans up the instance.
    features: HostFeatures<Recorder<P>>,
}

impl<P: Plugin> Rig<P>
where
    [(); P::PORTS]:,
{
    const LAYOUT: Layout = Layout::of::<P>();

    /// An activated instance at [`SAMPLE_RATE`]; the first block resets.
    #[must_use]
    #[allow(
        clippy::new_without_default,
        reason = "a rig instantiates a plugin, which `new` names better than `default`"
    )]
    pub fn new() -> Self {
        let features = HostFeatures::new(Arc::new(Recorder::new()));
        // SAFETY: the descriptor lives through the call, and the features outlive the instance.
        let handle = unsafe {
            lv2::instantiate::<P>(
                ptr::from_ref(&lv2::descriptor::<P>()),
                SAMPLE_RATE,
                c"".as_ptr(),
                features.list(),
            )
        };

        // SAFETY: the handle is live.
        unsafe { lv2::activate::<P>(handle) };

        Self {
            handle,
            flags: [0.0; 2],
            controls: P::Controls::default().to_ports(),
            bpm: MIN_BPM,
            checks_the_heap: true,
            features,
        }
    }

    /// This rig, without the check that [`CountingAllocator`] is the global allocator.
    #[must_use]
    pub const fn without_heap_check(mut self) -> Self {
        self.checks_the_heap = false;
        self
    }

    /// Activates the instance again: the next block resets and
    /// [`Context::is_activated`](crate::Context::is_activated) holds.
    pub fn reactivate(&mut self) {
        // SAFETY: the handle is live.
        unsafe { lv2::activate::<P>(self.handle) };
    }

    fn recorder(&self) -> &Recorder<P> {
        self.features.host()
    }

    /// The state the plugin last put `field` in; the default state until then.
    #[must_use]
    pub fn state<A: Stateful, V>(&self, field: Field<P::Controls, A, V>) -> A::State {
        A::state_of(self.recorder().states[field.position()].get())
    }

    /// Runs the plugin on `block` and returns it with the outputs.
    ///
    /// # Panics
    ///
    /// When `run` touches the heap, or when [`CountingAllocator`] is not the global allocator of a
    /// rig that checks it.
    #[track_caller]
    #[allow(
        clippy::must_use_candidate,
        reason = "a test may run a block only to advance the plugin"
    )]
    pub fn process(&mut self, block: Block<P>) -> Block<P>
    where
        [(); P::CHANNELS]:,
    {
        self.run(block, FRAMES)
    }

    /// Runs `block` with zero frames.
    ///
    /// # Panics
    ///
    /// As [`process`](Self::process).
    #[track_caller]
    #[allow(
        clippy::must_use_candidate,
        reason = "a test may run a block only to advance the plugin"
    )]
    pub fn process_empty(&mut self, block: Block<P>) -> Block<P>
    where
        [(); P::CHANNELS]:,
    {
        self.run(block, 0)
    }

    #[track_caller]
    fn run(&mut self, mut block: Block<P>, frames: usize) -> Block<P>
    where
        [(); P::CHANNELS]:,
    {
        self.flags = [block.enabled.to_port(), block.reset.to_port()];
        self.take_inputs(&block.controls);
        self.bpm = block.bpm;
        self.connect(&mut block.audio);

        let operations = heap_operations(|| {
            // SAFETY: every port is connected to a buffer of `FRAMES` samples or one value.
            unsafe { lv2::run::<P>(self.handle, frames as u32) };
        });

        if self.checks_the_heap {
            assert!(
                counts_the_heap(),
                "`Rig` needs `CountingAllocator` as the global allocator, or \
                 `Rig::without_heap_check`"
            );
            self.checks_the_heap = false;
        }
        assert!(
            operations == 0,
            "`{}::run` allocated, reallocated or freed {operations} times; it must not touch the \
             heap",
            type_name::<P>()
        );

        self.features
            .host()
            .requests
            .iter()
            .zip(self.controls.as_mut())
            .for_each(|(request, value)| *value = request.take().unwrap_or(*value));
        block.controls = P::Controls::from_ports(&self.controls);
        block
    }

    /// Copies the inputs of `controls` into the ports; the outputs keep what `run` last wrote.
    fn take_inputs(&mut self, controls: &P::Controls) {
        P::Controls::CONTROLS
            .iter()
            .zip(controls.to_ports().as_ref())
            .zip(self.controls.as_mut())
            .filter(|((control, _), _)| !control.is_output())
            .for_each(|((_, new), port)| *port = *new);
    }

    fn connect(&mut self, audio: &mut [[f32; FRAMES]]) {
        let layout = Self::LAYOUT;
        let audio = audio
            .iter_mut()
            .map(|channel| channel.as_mut_ptr())
            .zip(0..)
            .flat_map(|(data, channel)| {
                [
                    (layout.audio_in(channel), data),
                    (layout.audio_out(channel), data),
                ]
            });
        let values = [layout.enabled(), layout.reset()]
            .into_iter()
            .chain((0..layout.controls()).map(|position| layout.control(position)))
            .zip(
                self.flags
                    .iter_mut()
                    .chain(self.controls.as_mut())
                    .map(ptr::from_mut),
            );
        let tempo = [(layout.tempo(), ptr::from_mut(&mut self.bpm))];

        audio.chain(values).chain(tempo).for_each(|(port, data)| {
            // SAFETY: the handle is live, and the buffer outlives the run that follows.
            unsafe { lv2::connect_port::<P>(self.handle, port, data.cast::<c_void>()) };
        });
    }
}

impl<P: Plugin> Drop for Rig<P>
where
    [(); P::PORTS]:,
{
    fn drop(&mut self) {
        // SAFETY: the handle is live and never used again.
        unsafe { lv2::cleanup::<P>(self.handle) };
    }
}
