use core::{cell::Cell, cmp::Ordering, ffi::CStr, ptr::NonNull};

use super::{ChangeRequest, ControlState, Refusal, StateUpdate};
use crate::runtime::lv2::{CHANGE_REQUEST_URI, Feature, STATE_UPDATE_URI};

/// The features the host passed to `instantiate`.
///
/// Each pointer is none or, from [`Host::new`], valid until the instance is cleaned up.
#[derive(Clone, Copy)]
pub struct Host {
    state_update: Option<NonNull<StateUpdate>>,
    change_request: Option<NonNull<ChangeRequest>>,
}

impl Host {
    pub const NONE: Self = Self {
        state_update: None,
        change_request: None,
    };

    /// The features in `features`; the array is not kept.
    ///
    /// # Safety
    ///
    /// `features` must be null or a null-terminated array of features with live URIs, valid for
    /// the call. The `data` of a state-update or change-request feature must point to a
    /// [`StateUpdate`] or [`ChangeRequest`] valid until the instance is cleaned up.
    pub unsafe fn new(features: *const *const Feature) -> Self {
        if features.is_null() {
            return Self::NONE;
        }

        let mut host = Self::NONE;

        for index in 0.. {
            // SAFETY: the array is null-terminated, and the walk stops at the terminator.
            let feature = unsafe { *features.add(index) };

            if feature.is_null() {
                break;
            }

            // SAFETY: every listed feature is valid for the call.
            let Feature { uri, data } = unsafe { feature.read() };

            if uri.is_null() {
                continue;
            }

            // SAFETY: a non-null URI is live for the call.
            match unsafe { CStr::from_ptr(uri) } {
                uri if uri == STATE_UPDATE_URI => host.state_update = NonNull::new(data.cast()),
                uri if uri == CHANGE_REQUEST_URI => host.change_request = NonNull::new(data.cast()),
                _ => {}
            }
        }

        host
    }

    /// Asks the host to put the control port `index` in `state`.
    pub fn update_state(&self, index: u32, state: ControlState) -> Result<(), Refusal> {
        let (call, handle) = self
            .state_update
            // SAFETY: by `Host::new`, a live feature.
            .map(|feature| unsafe { feature.as_ref() })
            .and_then(|feature| feature.update_state.map(|call| (call, feature.handle)))
            .ok_or(Refusal::Unsupported)?;

        // SAFETY: `call` takes `handle`, and both stay valid until the instance is cleaned up.
        Refusal::of_status(unsafe { call(handle, index, state.into()) })
    }

    /// Asks the host to set the control input port `index` to `value`.
    pub fn request_change(&self, index: u32, value: f32) -> Result<(), Refusal> {
        let (call, handle) = self
            .change_request
            // SAFETY: by `Host::new`, a live feature.
            .map(|feature| unsafe { feature.as_ref() })
            .and_then(|feature| feature.request_change.map(|call| (call, feature.handle)))
            .ok_or(Refusal::Unsupported)?;

        // SAFETY: as in `update_state`.
        Refusal::of_status(unsafe { call(handle, index, value) })
    }
}

/// A change the host took, as bits so that a NaN input equals itself.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Change {
    value: u32,
    input: u32,
}

impl Change {
    const fn new(value: f32, input: f32) -> Self {
        Self {
            value: value.to_bits(),
            input: input.to_bits(),
        }
    }
}

/// The host features of an instance, and per control the last request the host took.
pub struct Requests {
    host: Host,
    states: Box<[Cell<Option<ControlState>>]>,
    changes: Box<[Cell<Option<Change>>]>,
    /// Whether `changes` may hold one.
    changed: Cell<bool>,
}

impl Requests {
    pub fn new(host: Host, controls: usize) -> Self {
        Self {
            host,
            states: vec![Cell::new(None); controls].into_boxed_slice(),
            changes: vec![Cell::new(None); controls].into_boxed_slice(),
            changed: Cell::new(false),
        }
    }

    /// Forgets what the host took, so that the next requests reach it again.
    pub fn forget(&self) {
        self.states.iter().for_each(|it| it.set(None));
        self.changes.iter().for_each(|it| it.set(None));
        self.changed.set(false);
    }

    /// Forgets every taken change whose input has moved from its value in `inputs`.
    pub fn forget_moved(&self, inputs: &[f32]) {
        if !self.changed.get() {
            return;
        }

        let kept = self
            .changes
            .iter()
            .zip(inputs)
            .fold(false, |kept, (taken, input)| {
                let change = taken.get().filter(|it| it.input == input.to_bits());

                taken.set(change);
                kept || change.is_some()
            });

        self.changed.set(kept);
    }

    #[cfg(test)]
    pub(super) const fn scans_moved_inputs(&self) -> bool {
        self.changed.get()
    }

    /// Asks the host to put the control at `position`, port `port`, in `state`, unless it took
    /// that state last.
    pub fn update_state(
        &self,
        position: usize,
        port: u32,
        state: ControlState,
    ) -> Result<(), Refusal> {
        let held = &self.states[position];

        if held.get() != Some(state) {
            self.host.update_state(port, state)?;
            held.set(Some(state));
        }

        Ok(())
    }

    /// Asks the host to set the input at `position`, port `port`, holding `input`, to `value`,
    /// unless it holds `value` or the host took this while it held `input`.
    pub fn request_change(
        &self,
        position: usize,
        port: u32,
        value: f32,
        input: f32,
    ) -> Result<(), Refusal> {
        let taken = &self.changes[position];
        let change = Change::new(value, input);
        let holds = input.partial_cmp(&value).is_some_and(Ordering::is_eq);

        if holds {
            taken.set(None);
        } else {
            if taken.get() != Some(change) {
                self.host.request_change(port, value)?;
            }

            taken.set(Some(change));
            self.changed.set(true);
        }

        Ok(())
    }
}
