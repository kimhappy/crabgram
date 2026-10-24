//! The host features a plugin asks for at instantiation, and the calls they allow.

mod features;
mod requests;
mod state;
#[cfg(test)]
mod tests;

pub use self::{
    features::{ChangeRequest, HostCalls, HostFeatures, HostRefusal, Refusal, StateUpdate},
    requests::{Host, Requests},
    state::{Activity, ControlState},
};
