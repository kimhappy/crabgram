//! The host features a plugin reaches through its context, as the rig offers them.

#![feature(generic_const_exprs)]

use crabgram::{
    Activity, Context, ControlState, Controls, Info, Plugin,
    testing::{Block, Rig},
};

#[derive(Controls)]
struct Params {
    /// Whether `level` is blocked.
    lock: bool,
    #[control(default = 0.5, min = 0.0, max = 1.0, blockable)]
    level: f32,
    /// The value to ask `level` to take; none while negative.
    #[control(default = -1.0, min = -1.0, max = 1.0)]
    ask: f32,
}

/// Blocks `level` and deactivates `ask` while `lock` is on, and asks `level` to take `ask`.
struct Follower;

impl Plugin for Follower {
    type Controls = Params;

    const INFO: Info = crabgram::info! {
        uri: c"urn:crabgram:test:follower",
        name: "Follower",
        description: "Asks the host for states and changes.",
        abbreviation: "FLW",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
        let _ = context.set_state(
            Params::LEVEL,
            if controls.lock {
                ControlState::Blocked
            } else {
                ControlState::Normal
            },
        );

        let _ = context.set_state(
            Params::ASK,
            if controls.lock {
                Activity::Inactive
            } else {
                Activity::Active
            },
        );

        if controls.ask >= 0.0 {
            let _ = context.request_change(Params::LEVEL, controls.ask);
        }
    }
}

crabgram::export!(Follower);

#[global_allocator]
static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;

fn with(controls: Params) -> Block<Follower> {
    Block {
        controls,
        ..Block::default()
    }
}

fn locked(lock: bool) -> Block<Follower> {
    with(Params {
        lock,
        ..Params::default()
    })
}

#[test]
fn the_rig_shows_the_state_the_plugin_set_last() {
    let mut rig = Rig::<Follower>::new();

    assert_eq!(rig.state(Params::ASK), Activity::Active);

    rig.process(locked(true));
    assert_eq!(rig.state(Params::LEVEL), ControlState::Blocked);
    assert_eq!(rig.state(Params::ASK), Activity::Inactive);

    rig.process(locked(false));
    assert_eq!(rig.state(Params::LEVEL), ControlState::Normal);
}

#[test]
fn a_requested_change_moves_the_control_of_the_returned_block() {
    let mut rig = Rig::<Follower>::new();
    let block = rig.process(with(Params {
        ask: 0.25,
        ..Params::default()
    }));

    assert_eq!(block.controls.level, 0.25);

    let block = rig.process(Block {
        controls: Params {
            ask: -1.0,
            ..block.controls
        },
        ..block
    });

    assert_eq!(
        block.controls.level, 0.25,
        "the control stays where it moved"
    );
}

#[test]
fn a_change_is_asked_for_again_once_the_control_went_back() {
    let mut rig = Rig::<Follower>::new();
    let asking = Params {
        ask: 0.25,
        ..Params::default()
    };
    let moved = rig.process(with(asking));
    let applied = rig.process(with(Params {
        ask: -1.0,
        ..moved.controls
    }));

    assert_eq!(applied.controls.level, 0.25);

    let back = rig.process(with(Params {
        level: 0.5,
        ask: 0.25,
        ..applied.controls
    }));

    assert_eq!(
        back.controls.level, 0.25,
        "the request reached the host again"
    );
}

#[test]
fn host_features_move_between_threads() {
    fn sends<T: Send>(_: &T) {}

    sends(&crabgram::lv2::HostFeatures::new(std::sync::Arc::new(
        Recorder,
    )));
}

struct Recorder;

impl crabgram::lv2::HostCalls for Recorder {
    fn update_state(&self, _: u32, _: ControlState) -> Result<(), crabgram::lv2::HostRefusal> {
        Ok(())
    }

    fn request_change(&self, _: u32, _: f32) -> Result<(), crabgram::lv2::HostRefusal> {
        Ok(())
    }
}
