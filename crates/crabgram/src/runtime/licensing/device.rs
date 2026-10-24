use core::ffi::{CStr, c_void};

use crate::{anagram::FRAMES, lv2::Feature};

#[allow(
    missing_docs,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    reason = "generated bindings"
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/nickel.rs"));
}

/// Starts the license check of `uri`; whether it is granted.
///
/// # Safety
///
/// `features` must be the null-terminated feature array the host passed to `instantiate`.
pub unsafe fn init(sample_rate: f64, features: *const *const Feature, uri: &CStr) -> bool {
    // SAFETY: `features` is a null-terminated array valid for the call, and `uri` a C string.
    unsafe { ffi::nickel_init(sample_rate, features, uri.as_ptr()) }
}

/// Counts one block; the count for [`run_silence`].
pub fn run_begin(run_count: u32) -> u32 {
    // SAFETY: the call takes no pointer.
    unsafe { ffi::nickel_license_run_begin(run_count, FRAMES as u32) }
}

/// Silences `buffer`, the output of `channel`, without a granted license.
pub fn run_silence(run_count: u32, buffer: &mut [f32; FRAMES], channel: u32) {
    // SAFETY: `buffer` is `FRAMES` samples borrowed exclusively for the call.
    unsafe {
        ffi::nickel_license_run_silence(run_count, buffer.as_mut_ptr(), FRAMES as u32, channel);
    }
}

/// The license interface of `extension_data`.
pub fn interface() -> *const c_void {
    // SAFETY: the URI is a static C string.
    unsafe { ffi::nickel_license_interface(crate::runtime::lv2::LICENSE_INTERFACE_URI.as_ptr()) }
}
