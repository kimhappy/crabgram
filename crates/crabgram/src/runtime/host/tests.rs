#![allow(
    clippy::arc_with_non_send_sync,
    reason = "a desk counts with cells and stays on the test thread"
)]

use core::cell::Cell;
use std::sync::Arc;

use super::{
    ControlState, Host, HostCalls, HostFeatures, HostRefusal, Refusal, Requests, StateUpdate,
};
use crate::runtime::lv2::{Feature, STATE_UPDATE_URI};

/// Refuses the first `refusals` change requests and counts every call.
#[derive(Default)]
struct Desk {
    refusals: Cell<u32>,
    updates: Cell<u32>,
    requests: Cell<u32>,
}

impl HostCalls for Desk {
    fn update_state(&self, _: u32, _: ControlState) -> Result<(), HostRefusal> {
        self.updates.set(self.updates.get() + 1);
        Ok(())
    }

    fn request_change(&self, _: u32, _: f32) -> Result<(), HostRefusal> {
        let refusals = self.refusals.get();

        self.requests.set(self.requests.get() + 1);
        self.refusals.set(refusals.saturating_sub(1));

        if refusals == 0 {
            Ok(())
        } else {
            Err(HostRefusal::InvalidIndex)
        }
    }
}

fn at_desk(refusals: u32, test: impl FnOnce(&Requests, &Desk)) {
    let features = HostFeatures::new(Arc::new(Desk {
        refusals: Cell::new(refusals),
        ..Desk::default()
    }));
    // SAFETY: the list is null-terminated, and the features outlive the requests.
    let host = unsafe { Host::new(features.list()) };

    test(&Requests::new(host, 1), features.host());
}

#[test]
fn a_state_the_feature_cannot_name_is_refused_as_unknown() {
    let features = HostFeatures::new(Arc::new(Desk::default()));
    let update = features.state_update();

    let call = update.update_state.expect("the features offer the call");
    let statuses = [0, 2, 3].map(|state| {
        // SAFETY: the handle is the live features' own, and `call` is the callback made for it.
        unsafe { call(update.handle, 0, state) }
    });

    assert_eq!(statuses, [0, 0, HostRefusal::Unknown.status()]);
    assert_eq!(features.host().updates.get(), 2);
}

/// Refuses every call.
struct Lacking;

impl HostCalls for Lacking {
    fn update_state(&self, _: u32, _: ControlState) -> Result<(), HostRefusal> {
        Err(HostRefusal::Unknown)
    }

    fn request_change(&self, _: u32, _: f32) -> Result<(), HostRefusal> {
        Err(HostRefusal::InvalidIndex)
    }
}

#[test]
fn a_refusal_reaches_the_plugin_as_an_lv2_status() {
    let features = HostFeatures::new(Arc::new(Lacking));
    let (update, change) = (features.state_update(), features.change_request());
    let (update_call, change_call) = (
        update.update_state.expect("the features offer the call"),
        change.request_change.expect("the features offer the call"),
    );

    // SAFETY: the handles are the live features' own, and the calls are the callbacks made for
    // them.
    let statuses = unsafe {
        [
            update_call(update.handle, 0, 0),
            change_call(change.handle, 0, 1.0),
        ]
    };

    assert_eq!(
        statuses,
        [
            HostRefusal::Unknown.status(),
            HostRefusal::InvalidIndex.status()
        ]
    );
}

#[test]
fn a_state_reaches_the_host_once_per_change() {
    at_desk(0, |requests, desk| {
        assert!(requests.update_state(0, 3, ControlState::Blocked).is_ok());
        assert!(requests.update_state(0, 3, ControlState::Blocked).is_ok());
        assert_eq!(desk.updates.get(), 1);
        assert!(requests.update_state(0, 3, ControlState::Normal).is_ok());
        assert_eq!(desk.updates.get(), 2);
    });
}

#[test]
fn forgetting_sends_the_state_again() {
    at_desk(0, |requests, desk| {
        assert!(requests.update_state(0, 3, ControlState::Blocked).is_ok());
        requests.forget();
        assert!(requests.update_state(0, 3, ControlState::Blocked).is_ok());
        assert_eq!(desk.updates.get(), 2);
    });
}

#[test]
fn a_refused_request_is_sent_again_until_the_host_takes_it() {
    at_desk(2, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_err());
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_err());
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert_eq!(desk.requests.get(), 3);
    });
}

#[test]
fn a_taken_request_is_sent_again_only_once_the_input_moves() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert_eq!(desk.requests.get(), 1);
        assert!(requests.request_change(0, 3, 1.0, 0.5).is_ok());
        assert_eq!(desk.requests.get(), 2);
    });
}

#[test]
fn a_taken_request_is_sent_again_once_its_input_moved_and_came_back() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        requests.forget_moved(&[1.0]);
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert_eq!(desk.requests.get(), 2);
    });
}

#[test]
fn a_taken_request_is_kept_while_its_input_stays() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        requests.forget_moved(&[0.0]);
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert_eq!(desk.requests.get(), 1);
    });
}

#[test]
fn the_moved_inputs_are_scanned_only_while_a_taken_change_remains() {
    at_desk(0, |requests, _| {
        assert!(!requests.scans_moved_inputs());
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        requests.forget_moved(&[0.0]);
        assert!(requests.scans_moved_inputs());
        requests.forget_moved(&[1.0]);
        assert!(!requests.scans_moved_inputs());
        assert!(requests.request_change(0, 3, 1.0, 1.0).is_ok());
        assert!(!requests.scans_moved_inputs());
    });
}

#[test]
fn an_input_that_holds_the_value_already_is_left_alone() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 1.0).is_ok());
        assert_eq!(desk.requests.get(), 0);
    });
}

#[test]
fn forgetting_lets_a_taken_request_through_again() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        requests.forget();
        assert!(requests.request_change(0, 3, 1.0, 0.0).is_ok());
        assert_eq!(desk.requests.get(), 2);
    });
}

#[test]
fn a_nan_input_reaches_the_host_once() {
    at_desk(0, |requests, desk| {
        assert!(requests.request_change(0, 3, 1.0, f32::NAN).is_ok());
        assert!(requests.request_change(0, 3, 1.0, f32::NAN).is_ok());
        assert_eq!(desk.requests.get(), 1);
    });
}

#[test]
fn the_features_keep_their_addresses_when_the_owner_moves() {
    let features = HostFeatures::new(Arc::new(Desk::default()));
    let list = features.list();
    let moved = Box::new(features);
    // SAFETY: the list is null-terminated, and the features outlive the requests.
    let host = unsafe { Host::new(list) };

    assert!(host.update_state(0, ControlState::Blocked).is_ok());
    assert!(host.request_change(0, 1.0).is_ok());
    assert_eq!(
        (moved.host().updates.get(), moved.host().requests.get()),
        (1, 1)
    );
}

#[test]
fn a_feature_without_a_uri_does_not_stop_the_walk() {
    let features = HostFeatures::new(Arc::new(Desk::default()));
    let nameless = Feature {
        uri: core::ptr::null(),
        data: core::ptr::null_mut(),
    };
    let named = Feature {
        uri: STATE_UPDATE_URI.as_ptr(),
        data: core::ptr::from_ref(features.state_update())
            .cast_mut()
            .cast(),
    };
    let list = [
        core::ptr::from_ref(&nameless),
        core::ptr::from_ref(&named),
        core::ptr::null(),
    ];
    // SAFETY: the list is null-terminated, and the features outlive the host.
    let host = unsafe { Host::new(list.as_ptr()) };

    assert!(host.update_state(0, ControlState::Blocked).is_ok());
    assert_eq!(features.host().updates.get(), 1);
}

#[test]
fn a_feature_without_a_callback_refuses_the_call() {
    let update = StateUpdate {
        handle: core::ptr::null_mut(),
        update_state: None,
    };
    let feature = Feature {
        uri: STATE_UPDATE_URI.as_ptr(),
        data: core::ptr::from_ref(&update).cast_mut().cast(),
    };
    let list = [core::ptr::from_ref(&feature), core::ptr::null()];
    // SAFETY: the list is null-terminated, and the feature outlives the host.
    let host = unsafe { Host::new(list.as_ptr()) };

    assert_eq!(
        host.update_state(0, ControlState::Blocked),
        Err(Refusal::Unsupported)
    );
}

#[test]
fn a_host_without_the_features_refuses_as_unsupported() {
    assert_eq!(
        Host::NONE.update_state(0, ControlState::Blocked),
        Err(Refusal::Unsupported)
    );
    assert_eq!(Host::NONE.request_change(0, 1.0), Err(Refusal::Unsupported));
}

#[test]
fn a_refusal_reaches_the_plugin_as_the_host_named_it() {
    at_desk(1, |requests, _| {
        assert_eq!(
            requests.request_change(0, 3, 1.0, 0.0),
            Err(Refusal::InvalidIndex)
        );
    });
}
