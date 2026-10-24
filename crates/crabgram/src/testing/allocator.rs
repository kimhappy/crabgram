use core::{
    alloc::{self, GlobalAlloc},
    cell::Cell,
};
use std::alloc::System;

#[cfg(doc)]
use super::Rig;

thread_local! {
    /// The heap operations of this thread inside [`heap_operations`].
    static HEAP_OPERATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

/// The system allocator, counting heap operations while [`Rig::process`] runs the plugin.
///
/// Make it the global allocator of a test crate:
///
/// ```
/// #[global_allocator]
/// static ALLOCATOR: crabgram::testing::CountingAllocator = crabgram::testing::CountingAllocator;
/// ```
pub struct CountingAllocator;

impl CountingAllocator {
    fn count() {
        HEAP_OPERATIONS.set(HEAP_OPERATIONS.get().map(|count| count + 1));
    }
}

// SAFETY: every call goes on to the system allocator unchanged.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        Self::count();
        // SAFETY: the caller keeps the contract of `alloc`, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: alloc::Layout) -> *mut u8 {
        Self::count();
        // SAFETY: as in `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: alloc::Layout) {
        Self::count();
        // SAFETY: `ptr` came from `System` through this allocator, with `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: alloc::Layout, new_size: usize) -> *mut u8 {
        Self::count();
        // SAFETY: as in `dealloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// How often `operations` allocated, reallocated or freed; zero unless [`CountingAllocator`] is
/// the global allocator.
pub(super) fn heap_operations(operations: impl FnOnce()) -> usize {
    HEAP_OPERATIONS.set(Some(0));
    operations();
    HEAP_OPERATIONS.take().unwrap_or_default()
}

/// Whether [`CountingAllocator`] is the global allocator.
pub(super) fn counts_the_heap() -> bool {
    heap_operations(|| drop(core::hint::black_box(Box::new(0_u8)))) > 0
}
