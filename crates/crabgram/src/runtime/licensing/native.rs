use core::{
    ffi::{CStr, c_void},
    ptr,
};

use crate::{anagram::FRAMES, lv2::Feature};

/// Grants every license.
///
/// # Safety
///
/// Any arguments: none is read.
pub const unsafe fn init(_sample_rate: f64, _features: *const *const Feature, _uri: &CStr) -> bool {
    true
}

pub const fn run_begin(run_count: u32) -> u32 {
    run_count
}

pub const fn run_silence(_run_count: u32, _buffer: &mut [f32; FRAMES], _channel: u32) {}

pub const fn interface() -> *const c_void {
    ptr::null()
}
