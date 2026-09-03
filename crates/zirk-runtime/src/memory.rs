//! Object allocation.
//!
//! This is the **single point** where the memory strategy is decided, which is
//! what lets the IR express `alloc <type>` without naming one
//! (`docs/decisions/ADR-003-memoria.md`, and decision D1 of the objects phase).
//! Neither the IR nor codegen knows what happens on the other side of this
//! boundary.
//!
//! # It frees, now
//!
//! Until Phase 4e's memory ADR closed ("Cierre de la decisión",
//! `docs/decisions/ADR-003-memoria.md`), this deliberately never freed
//! anything — cycles need trace-based collection, not counting, and getting
//! that wrong quietly is worse than not trying, so the bound was written down
//! ("a program of this phase terminates and the operating system reclaims
//! everything") rather than papered over with a reference count that could
//! not honor it.
//!
//! `fase-4e-colector-mark-sweep` is that trace-based collector, real: this
//! function now links every allocation onto [`crate::collector`]'s own
//! intrusive all-allocations list and triggers a mark-sweep collection
//! cooperatively (design D3) when the configured live-byte threshold is
//! crossed, before serving a new allocation. `crate::collector` owns mark and
//! sweep; this module stays the single entry point that decides an
//! allocation happens at all and hands back zeroed memory, exactly as before.

use std::alloc::{Layout, alloc_zeroed};
use std::ffi::c_void;

/// Allocates an object of `size` bytes aligned to at least `align`.
///
/// The memory is **zeroed**, which is what makes a partially built object
/// readable rather than a window onto whatever the allocator last held there.
/// The checker already requires every field to be written by the constructor,
/// so nothing observable depends on this — it is the second lock on the same
/// door, and the one that still holds if the first one is ever wrong.
///
/// Before allocating, this may run a full garbage collection (design D3,
/// `fase-4e-colector-mark-sweep`) if doing so is needed to stay under the
/// configured live-byte threshold — transparent to the caller either way:
/// the returned pointer is always zeroed, fresh, and not equal to any other
/// live object's address.
///
/// # Safety
///
/// `size` and `align` must form a valid layout up to alignment: `align` a
/// power of two, and `size` rounded up to it not overflowing `isize`.
/// Codegen computes both from the type it is building. The actual alignment
/// used may be wider than requested
/// ([`crate::collector::allocation_align`]) — always safe, since
/// over-aligning a base allocation never invalidates a narrower alignment
/// requirement.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_alloc(size: usize, align: usize) -> *mut c_void {
    // A zero-sized object still has identity, so it still needs an address of
    // its own: `is` compares references, and two zero-sized objects that
    // shared an address would be indistinguishable. Every real allocation is
    // already at least header-sized (`object_struct` emits the header as its
    // first three fields), but this floor is enforced regardless: `register`
    // unconditionally writes into the header's `next`/`size` words, so
    // serving anything smaller would write past the allocation.
    let size = size.max(1).max(crate::collector::HEADER_BYTES);
    let align = crate::collector::allocation_align(align);

    crate::collector::maybe_collect(size);

    let Ok(layout) = Layout::from_size_align(size, align) else {
        crate::failure::zirk_rt_allocation_failed()
    };

    let pointer = unsafe { alloc_zeroed(layout) };
    if pointer.is_null() {
        crate::failure::zirk_rt_allocation_failed()
    }
    let pointer = pointer as *mut c_void;

    // Links this allocation onto the collector's own intrusive list and
    // initializes its `next`/`size` header words (design D1) — the
    // descriptor word (word 0) is left exactly as `alloc_zeroed` left it
    // (null) until the caller's own codegen stores it right after this call
    // returns; nothing can trigger a collection in that window (single-
    // threaded, and a collection only ever runs at the top of this
    // function), so a momentarily null descriptor is never observed by the
    // collector.
    unsafe { crate::collector::register(pointer, size) };

    pointer
}

/// Finds the dispatch table a descriptor holds for a contract.
///
/// A class satisfies several contracts and each needs its own indices, so the
/// descriptor keeps one table per contract and the call finds it here. The
/// search is linear over what one class implements — a handful of entries, not
/// a data structure.
///
/// The descriptor layout is fixed by codegen:
///
/// ```text
///    [ method_table | ancestor_count | ancestor_id* | contract_count | (contract_id, table)* ]
/// ```
///
/// # Safety
///
/// `descriptor` must be one this compiler emitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_contract_table(
    descriptor: *const c_void,
    contract: u64,
) -> *const c_void {
    if descriptor.is_null() {
        crate::failure::zirk_rt_missing_contract()
    }

    let words = descriptor as *const usize;
    // Slot 0 is the method table; slot 1 is how many ancestor ids follow —
    // see `zirk_rt_check_cast`, which reads those same slots. The contract
    // count sits right after them.
    let ancestor_count = unsafe { *words.add(1) };
    let contract_count_slot = 2 + ancestor_count;
    let count = unsafe { *words.add(contract_count_slot) };

    for entry in 0..count {
        let base = contract_count_slot + 1 + entry * 2;
        let id = unsafe { *words.add(base) } as u64;
        if id == contract {
            return unsafe { *words.add(base + 1) } as *const c_void;
        }
    }

    // The checker proved the value satisfies the contract, so reaching here is
    // a compiler bug rather than a program error.
    crate::failure::zirk_rt_missing_contract()
}

/// Confirms a checked cast (`as`) against a descriptor's ancestor list,
/// terminating if the runtime type is not one of them (roadmap task 11.6).
///
/// "Ancestor" includes the class itself: an identity cast (`x as SameClass`)
/// is checked the same way a real up- or downcast is, rather than being a
/// special case codegen has to recognize. The search is linear over one
/// class's own chain — short, not a data structure — the same shape
/// [`zirk_rt_contract_table`] searches its own list with.
///
/// # Safety
///
/// `descriptor` must be one this compiler emitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_check_cast(descriptor: *const c_void, target: u64) {
    if descriptor.is_null() {
        crate::failure::zirk_rt_invalid_cast()
    }

    let words = descriptor as *const usize;
    let count = unsafe { *words.add(1) };
    for entry in 0..count {
        let id = unsafe { *words.add(2 + entry) } as u64;
        if id == target {
            return;
        }
    }

    crate::failure::zirk_rt_invalid_cast()
}

/// Adds an object to the per-thread pin list (roadmap Phase 4e,
/// `phase-4e-memory`, design D1). The surface implementation is a stub:
/// the real per-thread list and non-moving compaction will be wired later.
///
/// # Safety
///
/// `object` must be a live, reference-typed value produced by this compiler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_pin_object(_object: *const c_void) {}

/// Removes an object from the per-thread pin list (roadmap Phase 4e,
/// `phase-4e-memory`, design D1). The surface implementation is a stub.
///
/// # Safety
///
/// `object` must be the same pointer previously passed to
/// [`zirk_rt_pin_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_unpin_object(_object: *const c_void) {}

/// Reads the base object pointer from a `Dependent<T>` value (roadmap
/// Phase 4e, `phase-4e-memory`, design D1). Surface stub: returns null
/// until the two-word dependent form is wired through codegen and the GC.
///
/// # Safety
///
/// `dependent` must be a `Dependent<T>` value produced by this compiler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_dependent_base(_dependent: *const c_void) -> *const c_void {
    std::ptr::null()
}

/// Tests a descriptor's ancestor list against `target`, the same list
/// [`zirk_rt_check_cast`] searches — but returns whether it matched instead
/// of terminating when it does not (roadmap Phase 4b: a `catch Type(name)`
/// tests whether the pending exception's runtime type is `Type` or one of
/// its ancestors, and must keep running either way).
///
/// # Safety
///
/// `descriptor` must be one this compiler emitted, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_is_instance(descriptor: *const c_void, target: u64) -> bool {
    if descriptor.is_null() {
        return false;
    }

    let words = descriptor as *const usize;
    let count = unsafe { *words.add(1) };
    for entry in 0..count {
        let id = unsafe { *words.add(2 + entry) } as u64;
        if id == target {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_allocation_is_zeroed() {
        // Bytes past the collector's own 3-word header (`next`/`size`,
        // `fase-4e-colector-mark-sweep` design D1) are the part a
        // constructor actually writes into — those are what must read as
        // freshly zeroed, not the header words the collector itself just
        // wrote non-zero bookkeeping (a real `size`, and possibly a `next`
        // link) into.
        let size = crate::collector::HEADER_BYTES + 16;
        let pointer = unsafe { zirk_rt_alloc(size, 8) } as *const u8;
        let fields =
            unsafe { std::slice::from_raw_parts(pointer.add(crate::collector::HEADER_BYTES), 16) };

        assert!(
            fields.iter().all(|b| *b == 0),
            "a fresh object's own fields must not show whatever was there before"
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
