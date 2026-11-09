//! The license check, linked in with the `licensing` feature and a no-op without it.

#[cfg(feature = "licensing")]
mod device;
#[cfg(not(feature = "licensing"))]
mod native;

#[cfg(feature = "licensing")]
pub use self::device::{init, interface, run_begin, run_silence};
#[cfg(not(feature = "licensing"))]
pub use self::native::{init, interface, run_begin, run_silence};
