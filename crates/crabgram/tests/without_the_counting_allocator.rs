//! `Rig` in a test crate without the counting allocator.

#![feature(generic_const_exprs)]

use core::hint;

use crabgram::{
    Context, Controls, Info, Plugin,
    testing::{Block, Rig},
};

#[derive(Controls)]
struct Params {
    level: f32,
}

/// Allocates and frees a copy of every block.
struct Wasteful;

impl Plugin for Wasteful {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:wasteful",
        name: "Wasteful",
        description: "Allocates in run.",
        abbreviation: "WST",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _controls: &Params, context: &mut Context<'_, Self>) {
        let [samples] = context.buffers();

        drop(hint::black_box(samples.to_vec()));
    }
}

crabgram::export!(Wasteful);

#[test]
fn a_rig_without_the_heap_check_runs_on() {
    let mut rig = Rig::<Wasteful>::new().without_heap_check();
    let block = rig.process(Block::default());

    assert_eq!(rig.process(block).audio, [[0.0; 16]]);
}

#[test]
#[should_panic(expected = "needs `CountingAllocator`")]
fn a_rig_needs_the_counting_allocator() {
    Rig::<Wasteful>::new().process(Block::default());
}
