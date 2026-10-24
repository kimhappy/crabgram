#![allow(
    clippy::redundant_pub_crate,
    reason = "the entry points reach `testing` through a re-export, which `pub` would widen"
)]

use core::{
    ffi::{CStr, c_char, c_void},
    ptr,
};

use super::{
    Descriptor, FREE_LICENSE_URI, Feature, Handle, LICENSE_INTERFACE_URI, NO_FEATURES,
    instance::Instance,
};
use crate::{
    Licensing, Plugin,
    anagram::SAMPLE_RATE,
    runtime::{host::Host, licensing},
};

/// # Safety
///
/// `handle` must come from [`instantiate`] for the same `P`, and no other reference to its
/// instance may be live.
unsafe fn instance<'a, P: Plugin>(handle: Handle) -> &'a mut Instance<P>
where
    [(); P::PORTS]:,
{
    // SAFETY: `instantiate` made `handle` from a `Box<Instance<P>>` that `cleanup` has not freed.
    unsafe { &mut *handle.cast::<Instance<P>>() }
}

/// The `instantiate` of the descriptor: null at another rate than
/// [`SAMPLE_RATE`], which [`Plugin::new`] assumes.
///
/// # Safety
///
/// `features` must be null or a null-terminated array of features valid for the call, each with
/// `data` valid until the instance is cleaned up.
pub(crate) unsafe fn instantiate<P: Plugin>(
    _descriptor: *const Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    features: *const *const Feature,
) -> Handle
where
    [(); P::PORTS]:,
{
    if sample_rate != SAMPLE_RATE {
        return ptr::null_mut();
    }

    if P::INFO.licensing().checks() {
        let uri = P::INFO.c_uri();
        let features = if features.is_null() {
            NO_FEATURES.as_ptr()
        } else {
            features
        };

        // SAFETY: the array is valid for the call, and a null one became the empty array.
        unsafe {
            let granted = licensing::init(sample_rate, features, uri);

            if !granted && P::INFO.licensing() == Licensing::Free {
                licensing::init(sample_rate, features, FREE_LICENSE_URI);
            }
        }
    }

    // SAFETY: as this function requires.
    let host = unsafe { Host::new(features) };

    Box::into_raw(Box::new(Instance::<P>::new(host))).cast()
}

pub(crate) unsafe fn connect_port<P: Plugin>(handle: Handle, port: u32, data: *mut c_void)
where
    [(); P::PORTS]:,
{
    // SAFETY: the host passes a handle from `instantiate`.
    unsafe { instance::<P>(handle) }.connect(port, data);
}

pub(crate) unsafe fn activate<P: Plugin>(handle: Handle)
where
    [(); P::PORTS]:,
{
    // SAFETY: the host passes a handle from `instantiate`.
    unsafe { instance::<P>(handle) }.activate();
}

pub(crate) unsafe fn run<P: Plugin>(handle: Handle, sample_count: u32)
where
    [(); P::PORTS]:,
{
    // SAFETY: the contract of `run` is a handle from `instantiate`, `sample_count` 0 or `FRAMES`,
    // and every connected buffer live and that long.
    unsafe { instance::<P>(handle).run(sample_count) };
}

pub(crate) unsafe fn cleanup<P: Plugin>(handle: Handle)
where
    [(); P::PORTS]:,
{
    // SAFETY: the host passes a handle from `instantiate` and never uses it again.
    drop(unsafe { Box::from_raw(handle.cast::<Instance<P>>()) });
}

/// The `extension_data` of the descriptor: the license interface for a plugin with
/// [licensing](crate::Info::licensing), else nothing.
///
/// # Safety
///
/// `uri` must be null or a null-terminated string.
unsafe extern "C" fn extension_data<P: Plugin>(uri: *const c_char) -> *const c_void {
    // SAFETY: the caller passes null or a null-terminated URI.
    let uri = (!uri.is_null()).then(|| unsafe { CStr::from_ptr(uri) });

    if P::INFO.licensing().checks() && uri == Some(LICENSE_INTERFACE_URI) {
        licensing::interface()
    } else {
        ptr::null()
    }
}

/// The C entry points of the descriptor, each the Rust entry point of its name. A panic aborts.
mod c {
    use core::ffi::{c_char, c_void};

    use super::super::{Descriptor, Feature, Handle};
    use crate::Plugin;

    macro_rules! entry_points {
        ($($name:ident($($argument:ident: $ty:ty),*) $(-> $output:ty)?;)*) => {$(
            /// # Safety
            ///
            #[doc = concat!("As for [`super::", stringify!($name), "`].")]
            pub(super) unsafe extern "C" fn $name<P: Plugin>($($argument: $ty),*) $(-> $output)?
            where
                [(); P::PORTS]:,
            {
                // SAFETY: the host upholds the contract of the entry point.
                unsafe { super::$name::<P>($($argument),*) }
            }
        )*};
    }

    entry_points! {
        instantiate(
            descriptor: *const Descriptor,
            sample_rate: f64,
            bundle_path: *const c_char,
            features: *const *const Feature
        ) -> Handle;
        connect_port(handle: Handle, port: u32, data: *mut c_void);
        activate(handle: Handle);
        run(handle: Handle, sample_count: u32);
        cleanup(handle: Handle);
    }
}

/// The descriptor of `P`, for the `static` [`export!`](crate::export) makes.
#[must_use]
pub const fn descriptor<P: Plugin>() -> Descriptor
where
    [(); P::PORTS]:,
{
    Descriptor {
        uri: P::INFO.c_uri().as_ptr(),
        instantiate: Some(c::instantiate::<P>),
        connect_port: Some(c::connect_port::<P>),
        activate: Some(c::activate::<P>),
        run: Some(c::run::<P>),
        deactivate: None,
        cleanup: Some(c::cleanup::<P>),
        extension_data: Some(extension_data::<P>),
    }
}
