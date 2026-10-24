//! The counts a generic helper bounds on, read off the plugin.

#![feature(generic_const_exprs)]

use crabgram::{
    Context, Controls, Counts, Info, Plugin,
    anagram::FRAMES,
    port,
    testing::{Block, Rig},
};

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

#[derive(Controls)]
struct Params {
    level: f32,
}

struct Stereo;

impl Plugin for Stereo {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:stereo",
        name: "Stereo",
        description: "Fills each channel with its share of the channels.",
        abbreviation: "STR",
        category: Utility,
        channels: 2,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _controls: &Params, context: &mut Context<'_, Self>) {
        number(context);
    }
}

/// Fills each channel with its number, from 1, over the channel count.
fn number<P: Plugin>(context: &mut Context<'_, P>)
where
    [(); P::CHANNELS]:,
{
    context
        .buffers()
        .into_iter()
        .zip(1_u8..)
        .for_each(|(channel, number)| channel.fill(f32::from(number) / P::CHANNELS as f32));
}

const fn silence<P: Plugin>() -> [[f32; FRAMES]; P::CHANNELS]
where
    [(); P::CHANNELS]:,
{
    [[0.0; FRAMES]; P::CHANNELS]
}

const fn ports<P: Plugin>() -> [u32; P::PORTS]
where
    [(); P::PORTS]:,
{
    [0; P::PORTS]
}

#[test]
fn a_plugin_names_its_channel_and_port_counts() {
    assert_eq!(Stereo::CHANNELS, 2);
    assert_eq!(Stereo::PORTS, port::Layout::of::<Stereo>().count() as usize);
    assert_eq!(silence::<Stereo>().len(), 2);
    assert_eq!(ports::<Stereo>().len(), Stereo::PORTS);
}

#[test]
fn channels_are_the_buffers_in_order() {
    let block = Rig::<Stereo>::new().process(Block::default());

    assert_eq!(block.audio, [[0.5; FRAMES], [1.0; FRAMES]]);
}

#[test]
fn an_empty_block_keeps_its_audio() {
    let block = Rig::<Stereo>::new().process_empty(Block::default());

    assert_eq!(block.audio, [[0.0; FRAMES]; 2]);
}
