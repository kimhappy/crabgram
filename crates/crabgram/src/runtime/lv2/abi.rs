#![allow(missing_docs, reason = "the fields mirror the LV2 C structs")]

use core::{
    ffi::{CStr, c_char, c_void},
    ptr,
};

/// The host feature behind [`Context::set_state`](crate::Context::set_state).
pub const STATE_UPDATE_URI: &CStr =
    c"http://www.darkglass.com/lv2/ns/lv2ext/control-port-state-update";

/// The host feature behind [`Context::request_change`](crate::Context::request_change).
pub const CHANGE_REQUEST_URI: &CStr =
    c"http://kx.studio/ns/lv2ext/control-input-port-change-request";

/// The license interface of `extension_data`.
pub const LICENSE_INTERFACE_URI: &CStr =
    c"http://www.darkglass.com/lv2/ns/lv2ext/license#interface";

/// The other license a [`Free`](crate::Licensing::Free) plugin takes.
pub const FREE_LICENSE_URI: &CStr = c"urn:darkglass:pablito";

/// An instance, opaque to the host.
pub type Handle = *mut c_void;

/// A host feature: its URI and data.
#[repr(C)]
pub struct Feature {
    pub uri: *const c_char,
    pub data: *mut c_void,
}

pub type InstantiateFn = unsafe extern "C" fn(
    descriptor: *const Descriptor,
    sample_rate: f64,
    bundle_path: *const c_char,
    features: *const *const Feature,
) -> Handle;

pub type ConnectPortFn = unsafe extern "C" fn(instance: Handle, port: u32, data: *mut c_void);

pub type HandleFn = unsafe extern "C" fn(instance: Handle);

pub type RunFn = unsafe extern "C" fn(instance: Handle, sample_count: u32);

/// A null-terminated feature list with no feature.
pub const NO_FEATURES: &[*const Feature] = &[ptr::null()];

/// The entry points of a plugin, returned by `lv2_descriptor`.
#[repr(C)]
pub struct Descriptor {
    pub uri: *const c_char,
    pub instantiate: Option<InstantiateFn>,
    pub connect_port: Option<ConnectPortFn>,
    pub activate: Option<HandleFn>,
    pub run: Option<RunFn>,
    pub deactivate: Option<HandleFn>,
    pub cleanup: Option<HandleFn>,
    pub extension_data: Option<unsafe extern "C" fn(uri: *const c_char) -> *const c_void>,
}

// SAFETY: it holds only a static string and function pointers.
unsafe impl Sync for Descriptor {}
