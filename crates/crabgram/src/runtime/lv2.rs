//! The LV2 C interface and the entry points built from a [`Plugin`](crate::Plugin), used through
//! [`export!`](crate::export).

mod abi;
mod entry;
mod instance;
mod live;

pub use self::abi::{
    CHANGE_REQUEST_URI, ConnectPortFn, Descriptor, Feature, Handle, HandleFn, InstantiateFn,
    LICENSE_INTERFACE_URI, RunFn, STATE_UPDATE_URI,
};
pub(crate) use self::abi::{FREE_LICENSE_URI, NO_FEATURES};
#[doc(hidden)]
pub use self::entry::descriptor;
#[cfg(any(test, feature = "testing"))]
pub(crate) use self::entry::{activate, cleanup, connect_port, instantiate, run};
pub use super::host::{HostCalls, HostFeatures, HostRefusal};
