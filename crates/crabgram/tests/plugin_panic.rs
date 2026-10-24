//! A plugin that panics under a rig fails its test instead of aborting the test binary.

#![feature(generic_const_exprs)]

use crabgram::{
    Context, Controls, Info, Plugin,
    testing::{Block, Rig},
};

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

#[derive(Controls)]
struct Params {
    level: f32,
}

struct Panicking;

impl Plugin for Panicking {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:panicking",
        name: "Panicking",
        description: "Panics on every block.",
        abbreviation: "PAN",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {
        panic!("the plugin gave up");
    }
}

#[test]
#[should_panic(expected = "the plugin gave up")]
fn a_panic_in_run_fails_the_test() {
    Rig::<Panicking>::new().process(Block::default());
}
