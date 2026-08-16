//! Object allocation.
//!
//! This is the **single point** where the memory strategy is decided, which is
//! what lets the IR express `alloc <type>` without naming one
//! (`docs/decisions/ADR-003-memoria.md`, and decision D1 of the objects phase).
//! Neither the IR nor codegen knows what happens on the other side of this
//! boundary.
//!
//! # It does not free
//!
//! That is not an oversight. Freeing requires having decided **when**, and that
//! is precisely the question ADR-003 leaves open until the memory phase:
//! generational GC, reference counting with cycle detection, regions, or a
//! hybrid. Each answers "when" differently.
//!
//! Writing a reference count here because it is the easiest thing to reach for
//! would be work to undo — `ZIRK_RUNTIME_SPEC.md` section 9 requires collecting
//! cycles, and plain counting cannot — with the added trap that a half-built
//! count appears to work until the first cycle.
//!
//! A program of this phase terminates and the operating system reclaims
//! everything. That is the bound, and it is written down rather than assumed.

use std::alloc::{Layout, alloc_zeroed};
use std::ffi::c_void;

/// Allocates an object of `size` bytes aligned to `align`.
///
/// The memory is **zeroed**, which is what makes a partially built object
/// readable rather than a window onto whatever the allocator last held there.
/// The checker already requires every field to be written by the constructor,
/// so nothing observable depends on this — it is the second lock on the same
/// door, and the one that still holds if the first one is ever wrong.
///
/// # Safety
///
/// `size` and `align` must form a valid layout: `align` a power of two, and
/// `size` rounded up to it not overflowing `isize`. Codegen computes both from
/// the type it is building.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_alloc(size: usize, align: usize) -> *mut c_void {
    // A zero-sized object still has identity, so it still needs an address of
    // its own: `is` compares references, and two zero-sized objects that
    // shared an address would be indistinguishable.
    let size = size.max(1);

    let Ok(layout) = Layout::from_size_align(size, align.max(1)) else {
        crate::failure::zirk_rt_allocation_failed()
    };

    let pointer = unsafe { alloc_zeroed(layout) };
    if pointer.is_null() {
        crate::failure::zirk_rt_allocation_failed()
    }

    pointer as *mut c_void
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_allocation_is_zeroed() {
        let size = 32;
        let pointer = unsafe { zirk_rt_alloc(size, 8) } as *const u8;
        let bytes = unsafe { std::slice::from_raw_parts(pointer, size) };

        assert!(
            bytes.iter().all(|b| *b == 0),
            "a fresh object must not show whatever was there before"
        );
    }

    #[test]
    fn two_allocations_have_distinct_addresses() {
        // Identity is the address, so two objects can never share one.
        let a = unsafe { zirk_rt_alloc(16, 8) };
        let b = unsafe { zirk_rt_alloc(16, 8) };
        assert_ne!(a, b);
    }

    #[test]
    fn a_zero_sized_object_still_gets_an_address() {
        let a = unsafe { zirk_rt_alloc(0, 1) };
        let b = unsafe { zirk_rt_alloc(0, 1) };

        assert!(!a.is_null());
        assert_ne!(a, b, "identity needs an address of its own");
    }
}
