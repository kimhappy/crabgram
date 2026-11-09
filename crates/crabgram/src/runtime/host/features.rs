use core::{ffi::c_void, marker::PhantomPinned, pin::Pin, ptr};
use std::sync::Arc;

use super::ControlState;
use crate::runtime::lv2::{CHANGE_REQUEST_URI, Feature, STATE_UPDATE_URI};

/// The `update_state` callback of [`StateUpdate`].
pub type UpdateStateFn = unsafe extern "C" fn(handle: *mut c_void, index: u32, state: u32) -> u32;

/// The `request_change` callback of [`ChangeRequest`].
pub type RequestChangeFn = unsafe extern "C" fn(handle: *mut c_void, index: u32, value: f32) -> u32;

/// The state-update feature.
#[repr(C)]
pub struct StateUpdate {
    /// The host's own data.
    pub handle: *mut c_void,
    /// May be null.
    pub update_state: Option<UpdateStateFn>,
}

/// The change-request feature.
#[repr(C)]
pub struct ChangeRequest {
    /// The host's own data.
    pub handle: *mut c_void,
    /// May be null.
    pub request_change: Option<RequestChangeFn>,
}

/// The status of a call the host took.
pub const SUCCESS: u32 = 0;

/// Why a host refuses a request of a [`Context`](crate::Context).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum Refusal {
    /// The host cannot take the call now.
    #[error("the host cannot take the call")]
    Unknown,
    /// No such port takes the call.
    #[error("no port of that index takes the call")]
    InvalidIndex,
    /// The host lacks the feature that takes the call.
    #[error("the host lacks the feature that takes the call")]
    Unsupported,
}

impl Refusal {
    /// The result a host reports as `status`.
    pub(super) const fn of_status(status: u32) -> Result<(), Self> {
        match status {
            SUCCESS => Ok(()),
            2 => Err(Self::InvalidIndex),
            _ => Err(Self::Unknown),
        }
    }
}

/// Why a [`HostCalls`] host refuses a call: one of the statuses a plugin can read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HostRefusal {
    /// The host cannot take the call now.
    Unknown,
    /// No such port takes the call.
    InvalidIndex,
}

impl HostRefusal {
    pub(super) const fn status(self) -> u32 {
        match self {
            Self::Unknown => 1,
            Self::InvalidIndex => 2,
        }
    }
}

/// What a host does when the plugin calls one of its [`HostFeatures`].
pub trait HostCalls {
    /// Puts the control port `index` in `state`.
    ///
    /// # Errors
    ///
    /// When the host does not take the state.
    fn update_state(&self, index: u32, state: ControlState) -> Result<(), HostRefusal>;

    /// Sets the control input port `index` to `value`.
    ///
    /// # Errors
    ///
    /// When the host does not take the change.
    fn request_change(&self, index: u32, value: f32) -> Result<(), HostRefusal>;
}

fn status(result: Result<(), HostRefusal>) -> u32 {
    result.map_or_else(HostRefusal::status, |()| SUCCESS)
}

/// The `update_state` of [`StateUpdate`].
///
/// # Safety
///
/// `handle` must point to the live host `H` of the [`HostFeatures<H>`] that made it.
unsafe extern "C" fn update_state<H: HostCalls>(
    handle: *mut c_void,
    index: u32,
    state: u32,
) -> u32 {
    // SAFETY: the caller passes the handle of live features, which points at their host.
    let host = unsafe { &*handle.cast::<H>() };

    status(
        ControlState::from_number(state).map_or(Err(HostRefusal::Unknown), |state| {
            host.update_state(index, state)
        }),
    )
}

/// The `request_change` of [`ChangeRequest`].
///
/// # Safety
///
/// As for [`update_state`].
unsafe extern "C" fn request_change<H: HostCalls>(
    handle: *mut c_void,
    index: u32,
    value: f32,
) -> u32 {
    // SAFETY: as in `update_state`.
    let host = unsafe { &*handle.cast::<H>() };

    status(host.request_change(index, value))
}

/// The host features behind [`Context::set_state`](crate::Context::set_state) and
/// [`Context::request_change`](crate::Context::request_change), calling into one [`HostCalls`].
pub struct HostFeatures<H>(Pin<Box<Inner<H>>>);

#[expect(
    clippy::non_send_fields_in_send_ty,
    reason = "the raw pointers of `Inner` are what this impl vouches for"
)]
// SAFETY: the pointers lead into the pinned box, which moves along, and at the host, which an
// `Arc` keeps alive and `H: Send + Sync` lets other threads reach.
unsafe impl<H: Send + Sync> Send for HostFeatures<H> {}

struct Inner<H> {
    host: Arc<H>,
    state_update: StateUpdate,
    change_request: ChangeRequest,
    entries: [Feature; 2],
    list: [*const Feature; 3],
    /// The pointers above lead into this allocation.
    _pinned: PhantomPinned,
}

impl<H: HostCalls> HostFeatures<H> {
    /// Both features, calling into `host`.
    #[must_use]
    pub fn new(host: Arc<H>) -> Self {
        let handle = Arc::as_ptr(&host).cast_mut().cast();
        let mut inner = Box::new(Inner {
            state_update: StateUpdate {
                handle,
                update_state: Some(update_state::<H>),
            },
            change_request: ChangeRequest {
                handle,
                request_change: Some(request_change::<H>),
            },
            host,
            entries: [
                Feature {
                    uri: STATE_UPDATE_URI.as_ptr(),
                    data: ptr::null_mut(),
                },
                Feature {
                    uri: CHANGE_REQUEST_URI.as_ptr(),
                    data: ptr::null_mut(),
                },
            ],
            list: [ptr::null(); 3],
            _pinned: PhantomPinned,
        });

        inner.entries[0].data = ptr::from_mut(&mut inner.state_update).cast();
        inner.entries[1].data = ptr::from_mut(&mut inner.change_request).cast();
        inner.list = [
            ptr::from_ref(&inner.entries[0]),
            ptr::from_ref(&inner.entries[1]),
            ptr::null(),
        ];

        Self(Box::into_pin(inner))
    }
}

impl<H> HostFeatures<H> {
    /// The null-terminated feature list, valid while `self` lives.
    #[must_use]
    pub fn list(&self) -> *const *const Feature {
        self.0.list.as_ptr()
    }

    /// The host the features call into.
    #[must_use]
    pub fn host(&self) -> &H {
        &self.0.host
    }

    #[cfg(test)]
    pub(crate) fn state_update(&self) -> &StateUpdate {
        &self.0.state_update
    }

    #[cfg(test)]
    pub(crate) fn change_request(&self) -> &ChangeRequest {
        &self.0.change_request
    }
}
