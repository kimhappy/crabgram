//! Runs a plugin through its LV2 entry points in tests, failing a `run` that touches the heap.
//!
//! Pass a [`Block`] to [`Rig::process`], then the [`Block`] it returns to the next call. Make
//! [`CountingAllocator`] the global allocator.

mod allocator;
mod block;
mod recorder;
mod rig;

pub use self::{allocator::CountingAllocator, block::Block, rig::Rig};
