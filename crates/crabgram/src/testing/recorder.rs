use core::{cell::Cell, marker::PhantomData};

#[cfg(doc)]
use super::Rig;
use crate::{
    ControlState, Controls, Plugin,
    lv2::{HostCalls, HostRefusal},
    port::Layout,
};

/// What the host of a [`Rig`] took from `P`.
pub(super) struct Recorder<P> {
    /// The state the plugin last put each control in.
    pub(super) states: Box<[Cell<ControlState>]>,
    /// The last value the plugin asked for in the running block.
    pub(super) requests: Box<[Cell<Option<f32>>]>,
    plugin: PhantomData<fn() -> P>,
}

impl<P: Plugin> Recorder<P> {
    pub(super) fn new() -> Self {
        let controls = P::Controls::CONTROLS.len();

        Self {
            states: vec![Cell::new(ControlState::Normal); controls].into_boxed_slice(),
            requests: vec![Cell::new(None); controls].into_boxed_slice(),
            plugin: PhantomData,
        }
    }

    fn position(index: u32) -> Result<usize, HostRefusal> {
        Layout::of::<P>()
            .position(index)
            .map(|position| position as usize)
            .ok_or(HostRefusal::InvalidIndex)
    }
}

/// Takes any state of a control, and any value of an input control.
impl<P: Plugin> HostCalls for Recorder<P> {
    fn update_state(&self, index: u32, state: ControlState) -> Result<(), HostRefusal> {
        let position = Self::position(index)?;

        self.states[position].set(state);
        Ok(())
    }

    fn request_change(&self, index: u32, value: f32) -> Result<(), HostRefusal> {
        let position = Self::position(index)?;

        if P::Controls::CONTROLS[position].is_output() {
            return Err(HostRefusal::InvalidIndex);
        }

        self.requests[position].set(Some(value));
        Ok(())
    }
}
