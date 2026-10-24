//! The heap check of `Rig::process`, under the counting allocator.

#![feature(generic_const_exprs)]

use crabgram::{
    Context, Controls, Info, Plugin,
    anagram::FRAMES,
    testing::{Block, Rig},
};

#[derive(Controls)]
struct Params {
    /// Whether `run` copies the block onto the heap.
    hoard: bool,
}

/// Keeps a copy of the last block it hoarded, in a buffer it allocates in `new`.
struct Hoarder {
    hoard: Vec<f32>,
}

impl Plugin for Hoarder {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:hoarder",
        name: "Hoarder",
        description: "Copies blocks onto the heap.",
        abbreviation: "HRD",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self {
            hoard: vec![0.0; FRAMES],
        }
    }

    fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
        let [samples] = context.buffers();

        if controls.hoard {
            self.hoard = samples.to_vec();
        } else {
            self.hoard.copy_from_slice(samples);
        }
    }
}

crabgram::export!(Hoarder);

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

fn hoarding(hoard: bool) -> Block<Hoarder> {
    Block {
        controls: Params { hoard },
        ..Block::default()
    }
}

#[test]
#[should_panic(
    expected = "`heap_guard::Hoarder::run` allocated, reallocated or freed 2 times; it must not touch the heap"
)]
fn a_run_that_touches_the_heap_fails_the_test() {
    let mut rig = Rig::<Hoarder>::new();
    let quiet = rig.process(hoarding(false));

    rig.process(Block {
        controls: Params { hoard: true },
        ..quiet
    });
}

#[test]
fn a_run_that_leaves_the_heap_alone_passes() {
    let mut rig = Rig::<Hoarder>::new();

    rig.process(Block::default());
}
