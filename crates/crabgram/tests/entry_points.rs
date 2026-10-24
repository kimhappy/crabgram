//! The LV2 entry points crabgram builds, driven through the raw descriptor.

#![feature(generic_const_exprs)]

use core::{cell::Cell, ffi::c_void, marker::PhantomData};

use crabgram::{
    Choice, Context, Controls, Info, Plugin,
    anagram::FRAMES,
    lv2, port,
    testing::{Block, Rig},
};

#[derive(Debug, PartialEq, Choice)]
enum Order {
    Straight,
    Swapped,
}

#[derive(Controls)]
struct Params {
    #[control(default = 1.0, min = 0.0, max = 2.0)]
    gain: f32,
    #[control(default = Order::Swapped)]
    order: Order,
    #[control(output, min = 0, max = 1000)]
    resets: u32,
    #[control(output, min = 0, max = 16)]
    blocks: u32,
}

/// Swaps or keeps the channels, scales them, and counts resets and blocks (clamped at 16).
struct Probe {
    resets: u32,
    blocks: u32,
}

impl Plugin for Probe {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:probe",
        name: "Probe",
        description: "Counts what the host does.",
        abbreviation: "PRB",
        category: Utility,
        channels: 2,
    };

    fn new() -> Self {
        Self {
            resets: 0,
            blocks: 0,
        }
    }

    fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
        self.resets += u32::from(context.is_reset());
        self.blocks += 1;

        if context.is_enabled() {
            let [left, right] = context.buffers();

            if controls.order == Order::Swapped {
                left.swap_with_slice(right);
            }

            left.iter_mut()
                .chain(right.iter_mut())
                .for_each(|sample| *sample *= controls.gain);
        }

        context.write(Params::RESETS, self.resets);
        context.write(Params::BLOCKS, self.blocks);
    }
}

crabgram::export!(Probe);

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

/// Three channels it leaves alone, to lay out more ports than stereo has.
struct Trio;

impl Plugin for Trio {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:trio",
        name: "Trio",
        description: "Passes three channels through.",
        abbreviation: "TRI",
        category: Utility,
        channels: 3,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _controls: &Params, _context: &mut Context<'_, Self>) {}
}

/// On, no reset, a gain of 2, the channels straight, both counters at 0, and the tempo at 40.
const STRAIGHT_DOUBLED: [f32; 7] = [1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 40.0];

fn ramp(channel: usize) -> [f32; FRAMES] {
    core::array::from_fn(|frame| (channel * 100 + frame) as f32)
}

fn scaled(channel: usize, gain: f32) -> [f32; FRAMES] {
    ramp(channel).map(|sample| sample * gain)
}

/// The exported descriptor, [`Probe`]'s.
fn exported() -> &'static lv2::Descriptor {
    // SAFETY: index 0 is the exported descriptor, a `static`.
    unsafe { &*lv2_descriptor(0) }
}

#[test]
fn the_descriptor_names_the_plugin() {
    let descriptor = exported();
    // SAFETY: the descriptor URI is a static C string.
    let uri = unsafe { core::ffi::CStr::from_ptr(descriptor.uri) };

    assert_eq!(uri.to_str(), Ok(Probe::INFO.uri()));
    assert!(lv2_descriptor(1).is_null());
}

#[test]
fn the_ports_are_listed_in_index_order() {
    let indices = |ports: Vec<port::Port>| ports.into_iter().map(|port| port.index());

    assert!(indices(port::ports::<Probe>()).eq(0..11));
    assert!(indices(port::ports::<Trio>()).eq(0..13));
    assert!(indices(port::ports::<Metronome>()).eq(0..port::Layout::of::<Metronome>().count()));
}

fn ramps(controls: Params) -> Block<Probe> {
    Block {
        audio: [ramp(0), ramp(1)],
        controls,
        ..Block::default()
    }
}

#[test]
fn processing_swaps_and_scales_the_channels() {
    let block = Rig::new().process(ramps(Params {
        gain: 2.0,
        ..Params::default()
    }));

    assert_eq!(block.audio, [scaled(1, 2.0), scaled(0, 2.0)]);
}

#[test]
fn inputs_are_clamped_to_their_range() {
    let block = Rig::new().process(ramps(Params {
        gain: 50.0,
        ..Params::default()
    }));

    assert_eq!(block.audio[1], scaled(0, 2.0));
    assert_eq!(block.controls.gain, 2.0);
}

#[test]
fn only_the_first_block_resets() {
    let mut rig = Rig::<Probe>::new();

    let first = rig.process(Block::default());
    let block = rig.process(first);

    assert_eq!((block.controls.resets, block.controls.blocks), (1, 2));
}

#[test]
fn a_held_reset_trigger_resets_once() {
    let mut rig = Rig::<Probe>::new();
    let mut process = |reset| {
        rig.process(Block {
            reset,
            ..Block::default()
        })
        .controls
        .resets
    };

    process(false);
    process(true);
    assert_eq!(process(true), 2);

    process(false);
    assert_eq!(process(true), 3);
}

#[test]
fn an_empty_block_runs_nothing_and_its_reset_resets_the_next() {
    let mut rig = Rig::<Probe>::new();
    let first = rig.process(Block::default());
    let empty = rig.process_empty(Block {
        reset: true,
        ..first
    });
    let blocks_after_empty = empty.controls.blocks;
    let next = rig.process(Block::default());

    assert_eq!(
        (
            blocks_after_empty,
            next.controls.resets,
            next.controls.blocks
        ),
        (1, 2, 2)
    );
}

#[test]
fn an_empty_block_keeps_the_outputs_last_written() {
    let mut rig = Rig::<Probe>::new();

    rig.process(Block::default());

    let empty = rig.process_empty(Block::default());

    assert_eq!((empty.controls.resets, empty.controls.blocks), (1, 1));
}

#[test]
fn a_disabled_block_is_left_to_the_plugin() {
    let block = Rig::new().process(Block {
        enabled: false,
        ..ramps(Params {
            gain: 2.0,
            ..Params::default()
        })
    });

    assert_eq!(block.audio, [ramp(0), ramp(1)]);
}

/// Views a buffer as shared cells, which the instance may write through while the test holds them.
fn cells<T, const N: usize>(array: &mut [T; N]) -> &[Cell<T>; N] {
    Cell::from_mut(array).as_array_of_cells()
}

fn instantiate(descriptor: &'static lv2::Descriptor, sample_rate: f64) -> lv2::Handle {
    static NO_FEATURES: [usize; 1] = [0];

    // SAFETY: the descriptor lives through the call, and the empty feature list is static.
    unsafe {
        (descriptor.instantiate.unwrap())(
            descriptor,
            sample_rate,
            c"".as_ptr(),
            NO_FEATURES.as_ptr().cast(),
        )
    }
}

/// An instance driven through the raw descriptor, connected to buffers that live for `'a`.
struct Host<'a> {
    descriptor: &'static lv2::Descriptor,
    handle: lv2::Handle,
    buffers: PhantomData<Cell<&'a ()>>,
}

impl<'a> Host<'a> {
    /// An instance of the exported plugin, [`Probe`].
    fn new() -> Self {
        Self::of(exported())
    }

    fn of(descriptor: &'static lv2::Descriptor) -> Self {
        let handle = instantiate(descriptor, 48_000.0);

        Self {
            descriptor,
            handle,
            buffers: PhantomData,
        }
    }

    fn connect(&self, port: u32, data: *mut c_void) {
        // SAFETY: the handle is live, and connecting only stores the pointer.
        unsafe { (self.descriptor.connect_port.unwrap())(self.handle, port, data) };
    }

    fn connect_audio(&self, port: u32, buffer: &'a [Cell<f32>; FRAMES]) {
        self.connect(port, buffer.as_ptr().cast_mut().cast());
    }

    fn connect_value(&self, port: u32, value: &'a Cell<f32>) {
        self.connect(port, value.as_ptr().cast());
    }

    /// Connects the four audio ports to `audio`, in port order, and the rest to `values`.
    fn wire(&self, audio: [&'a [Cell<f32>; FRAMES]; 4], values: &'a [Cell<f32>; 7]) {
        audio
            .into_iter()
            .zip(0..)
            .for_each(|(buffer, port)| self.connect_audio(port, buffer));
        values
            .iter()
            .zip(4..)
            .for_each(|(value, port)| self.connect_value(port, value));
    }

    fn activate(&self) {
        // SAFETY: the handle is live.
        unsafe { (self.descriptor.activate.unwrap())(self.handle) };
    }

    /// Deactivates the instance, if the plugin takes the call.
    fn deactivate(&self) {
        if let Some(deactivate) = self.descriptor.deactivate {
            // SAFETY: the handle is live.
            unsafe { deactivate(self.handle) };
        }
    }

    fn run(&self, frames: usize) {
        // SAFETY: the handle is live, `frames` is 0 or `FRAMES`, and the cells connected through
        // `connect_audio` and `connect_value` live for `'a`, as long as the instance.
        unsafe { (self.descriptor.run.unwrap())(self.handle, frames as u32) };
    }
}

impl Drop for Host<'_> {
    fn drop(&mut self) {
        // SAFETY: the handle is live and never used again.
        unsafe { (self.descriptor.cleanup.unwrap())(self.handle) };
    }
}

#[test]
fn separate_outputs_receive_the_processed_input() {
    let mut audio = [ramp(0), ramp(1), [0.0; FRAMES], [0.0; FRAMES]];
    let mut values = STRAIGHT_DOUBLED;
    let host = Host::new();

    host.wire(audio.each_mut().map(cells), cells(&mut values));
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(audio, [ramp(0), ramp(1), scaled(0, 2.0), scaled(1, 2.0)]);
}

#[test]
fn an_input_shared_with_the_other_output_is_read_first() {
    let mut audio = [ramp(0), ramp(1), [0.0; FRAMES]];
    let mut values = STRAIGHT_DOUBLED;
    let host = Host::new();
    let [a, b, c] = audio.each_mut().map(cells);

    host.wire([a, b, b, c], cells(&mut values));
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(audio, [ramp(0), scaled(0, 2.0), scaled(1, 2.0)]);
}

#[test]
fn inputs_crossed_onto_the_outputs_are_read_first() {
    let mut audio = [ramp(0), ramp(1)];
    let mut values = STRAIGHT_DOUBLED;
    let host = Host::new();
    let [a, b] = audio.each_mut().map(cells);

    host.wire([a, b, b, a], cells(&mut values));
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(audio, [scaled(1, 2.0), scaled(0, 2.0)]);
}

#[test]
fn two_outputs_on_one_buffer_skip_the_block() {
    let mut audio = [ramp(0), ramp(1), [7.0; FRAMES]];
    let mut values = STRAIGHT_DOUBLED;
    let host = Host::new();
    let [a, b, c] = audio.each_mut().map(cells);

    host.wire([a, b, c, c], cells(&mut values));
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(audio, [ramp(0), ramp(1), [7.0; FRAMES]]);
    assert_eq!(values, STRAIGHT_DOUBLED);
}

#[test]
fn an_empty_block_is_skipped_and_keeps_the_reset() {
    let mut audio = [[0.0; FRAMES]; 2];
    let values = [1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 40.0].map(Cell::new);
    let host = Host::new();
    let [left, right] = audio.each_mut().map(cells);

    host.wire([left, right, left, right], &values);
    host.activate();
    host.run(0);

    let blocks_after_empty = values[5].get();

    host.run(FRAMES);
    drop(host);

    assert_eq!(blocks_after_empty, 0.0);
    assert_eq!((values[4].get(), values[5].get()), (1.0, 1.0));
}

#[test]
fn a_block_with_an_unconnected_port_does_nothing() {
    let mut output = [7.0_f32; FRAMES];
    let host = Host::new();

    host.connect_audio(2, cells(&mut output));
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(output, [7.0; FRAMES]);
}

#[derive(Controls)]
struct Starts {
    #[control(output, min = 0, max = 1000)]
    activations: u32,
    #[control(output, min = 0, max = 1000)]
    resets: u32,
}

/// Counts the blocks that follow an activation and the blocks that reset.
struct Starter {
    activations: u32,
    resets: u32,
}

impl Plugin for Starter {
    type Controls = Starts;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:starter",
        name: "Starter",
        description: "Counts activations and resets.",
        abbreviation: "STR",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self {
            activations: 0,
            resets: 0,
        }
    }

    fn run(&mut self, _controls: &Starts, context: &mut Context<'_, Self>) {
        self.activations += u32::from(context.is_activated());
        self.resets += u32::from(context.is_reset());
        context.write(Starts::ACTIVATIONS, self.activations);
        context.write(Starts::RESETS, self.resets);
    }
}

static STARTER: lv2::Descriptor = lv2::descriptor::<Starter>();

#[test]
fn only_the_first_block_after_each_activation_is_activated() {
    let layout = port::Layout::of::<Starter>();
    let audio = [0.0; FRAMES].map(Cell::new);
    let values = [1.0, 0.0, 0.0, 40.0].map(Cell::new);
    let reset = Cell::new(0.0);
    let host = Host::of(&STARTER);
    let ports = [
        layout.enabled(),
        layout.control(0),
        layout.control(1),
        layout.tempo(),
    ];

    host.connect_audio(layout.audio_in(0), &audio);
    host.connect_audio(layout.audio_out(0), &audio);
    host.connect_value(layout.reset(), &reset);
    ports
        .into_iter()
        .zip(&values)
        .for_each(|(port, value)| host.connect_value(port, value));
    host.activate();
    host.run(0);
    host.run(FRAMES);
    host.run(FRAMES);

    let first = [values[1].get(), values[2].get()];

    reset.set(1.0);
    host.run(FRAMES);
    reset.set(0.0);
    host.deactivate();
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!(
        (first, [values[1].get(), values[2].get()]),
        ([1.0, 1.0], [2.0, 3.0])
    );
}

#[test]
fn a_reactivated_rig_starts_its_next_block_activated() {
    let mut rig = Rig::<Starter>::new();
    let counts = |block: Block<Starter>| (block.controls.activations, block.controls.resets);

    let first = counts(rig.process(Block::default()));
    let second = counts(rig.process(Block::default()));

    rig.reactivate();

    let reactivated = counts(rig.process(Block::default()));
    let after = counts(rig.process(Block::default()));

    assert_eq!(
        [first, second, reactivated, after],
        [(1, 1), (1, 1), (2, 2), (2, 2)]
    );
}

#[test]
fn ports_past_the_last_are_ignored() {
    let mut audio = [[0.0; FRAMES]; 2];
    let mut values = STRAIGHT_DOUBLED;
    let stray = Cell::new(0.5);
    let host = Host::new();
    let [left, right] = audio.each_mut().map(cells);

    host.wire([left, right, left, right], cells(&mut values));
    host.connect_value(99, &stray);
    host.activate();
    host.run(FRAMES);
    drop(host);

    assert_eq!((stray.get(), values[5]), (0.5, 1.0));
}

#[test]
fn an_output_past_its_range_reaches_the_port_at_its_maximum() {
    let mut audio = [[0.0; FRAMES]; 2];
    let mut values = STRAIGHT_DOUBLED;
    let host = Host::new();
    let [left, right] = audio.each_mut().map(cells);

    host.wire([left, right, left, right], cells(&mut values));
    host.activate();
    (0..20).for_each(|_| host.run(FRAMES));
    drop(host);

    assert_eq!(values[5], Params::BLOCKS.as_ref().max());
}

#[derive(Controls)]
struct Rising {
    #[control(output, default = 0.5)]
    level: f32,
}

/// Raises its own output by a tenth every block.
struct Riser;

impl Plugin for Riser {
    type Controls = Rising;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:riser",
        name: "Riser",
        description: "Raises its output.",
        abbreviation: "RSR",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, controls: &Rising, context: &mut Context<'_, Self>) {
        context.write(Rising::LEVEL, controls.level + 0.1);
    }
}

#[test]
fn an_output_starts_at_its_default_and_keeps_what_the_plugin_wrote() {
    let mut rig = Rig::<Riser>::new();
    let first = rig.process(Block::default());
    let second = rig.process(Block {
        controls: Rising { level: 0.0 },
        ..Block::default()
    });

    let levels = [first.controls.level, second.controls.level];

    assert!(
        (levels[0] - 0.6).abs() < 1e-6 && (levels[1] - 0.7).abs() < 1e-6,
        "{levels:?}"
    );
}

#[derive(Controls)]
struct Tempo {
    #[control(output, min = 0, max = 400)]
    bpm: u32,
}

/// Writes the tempo the host reports to its output.
struct Metronome;

impl Plugin for Metronome {
    type Controls = Tempo;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:metronome",
        name: "Metronome",
        description: "Reads the host tempo.",
        abbreviation: "MET",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _controls: &Tempo, context: &mut Context<'_, Self>) {
        context.write(Tempo::BPM, context.bpm().round() as u32);
    }
}

fn reported(rig: &mut Rig<Metronome>, bpm: f32) -> u32 {
    rig.process(Block {
        bpm,
        ..Block::default()
    })
    .controls
    .bpm
}

#[test]
fn the_tempo_port_comes_last() {
    let ports = port::ports::<Metronome>();

    assert_eq!(ports.last().map(port::Port::role), Some(port::Role::Tempo));
}

#[test]
fn a_plugin_reads_the_tempo_the_host_reports() {
    let mut rig = Rig::<Metronome>::new();

    assert_eq!(reported(&mut rig, 133.0), 133);
    assert_eq!(reported(&mut rig, 90.0), 90);
}

#[test]
fn a_nan_tempo_reads_as_the_slowest_tempo() {
    assert_eq!(reported(&mut Rig::<Metronome>::new(), f32::NAN), 40);
}

#[test]
fn a_tempo_out_of_range_reads_as_the_nearest_bound() {
    let mut rig = Rig::<Metronome>::new();

    assert_eq!(reported(&mut rig, 350.0), 300);
    assert_eq!(reported(&mut rig, 10.0), 40);
}

#[test]
fn another_sample_rate_is_refused() {
    assert!(instantiate(exported(), 44_100.0).is_null());
}
