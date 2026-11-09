//! A requested change takes and gives back a value of its field's own type.

#![feature(generic_const_exprs)]

use crabgram::{
    Choice, Context, Controls, Info, Plugin,
    testing::{Block, Rig},
};

#[derive(Debug, PartialEq, Eq, Choice)]
enum Wave {
    Sine,
    Square,
    Saw,
}

#[derive(Controls)]
struct Params {
    wave: Wave,
    sync: bool,
    #[control(min = 1, max = 8)]
    steps: u8,
    /// Whether to ask the other controls to change.
    ask: bool,
}

/// Asks for the saw wave, sync on and more steps than the range holds while `ask` is on.
struct Asker;

impl Plugin for Asker {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:asker",
        name: "Asker",
        description: "Asks for typed changes.",
        abbreviation: "ASK",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
        if controls.ask {
            let _ = context.request_change(Params::WAVE, Wave::Saw);
            let _ = context.request_change(Params::SYNC, true);
            let _ = context.request_change(Params::STEPS, 20);
        }
    }
}

crabgram::export!(Asker);

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

#[test]
fn a_typed_request_reads_back_as_its_field_type_within_the_range() {
    let mut rig = Rig::<Asker>::new();
    let controls = rig
        .process(Block {
            controls: Params {
                ask: true,
                ..Params::default()
            },
            ..Block::default()
        })
        .controls;

    assert_eq!(
        (controls.wave, controls.sync, controls.steps, controls.ask),
        (Wave::Saw, true, 8, true)
    );
}
