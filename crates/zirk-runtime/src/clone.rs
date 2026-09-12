//! Deep clone graph traversal (roadmap Phase 4e, `fase-4e-clone`, design D2).
//!
//! `zirk_rt_clone` is the sole generated-code-facing entry point codegen
//! calls for every `.clone()` this compiler derives automatically
//! (`crates/zirk-ir/src/ir.rs`'s `InstKind::Clone`, whose own doc comment
//! explains why the whole recursive walk lives here — in one generic,
//! descriptor-driven Rust function — rather than being unrolled across
//! several IR instructions the way this change's own design doc first
//! sketched it).
//!
//! The traversal reuses [`crate::collector::gc_field_offsets`] — the exact
//! same per-class byte-offset table `collector::mark_object` already reads
//! from each object's own header — as its own field-walk driver, so an
//! object's reference-typed fields are found identically whether the
//! collector is tracing it or `Clone` is copying it. The checker only ever
//! accepts `.clone()`/`T from Clone` for a `class` whose entire declared
//! field graph is itself `Clone` (`zirk-sema`'s own `Checker::class_is_clone`,
//! design D1) — every object [`zirk_rt_clone`] can actually reach through
//! [`crate::collector::gc_field_offsets`] starting from such a root is
//! therefore guaranteed, by that compile-time check, to itself be an
//! ordinary `Clone`-derived class object: never a `Resource`, a `Pointer<T>`
//! (a raw, non-managed value — never a [`crate::collector::gc_field_offsets`]
//! entry to begin with), a `Weak<T>` WeakCell, or a contract-typed
//! reference. This is what lets the traversal below treat every reference
//! field it finds uniformly, with no per-kind dispatch.
//!
//! # Memoization and cycle safety (design D2)
//!
//! [`zirk_rt_clone`] owns one `HashMap<usize, *mut c_void>` — source address
//! to its own already-cloned copy — scoped to that one top-level call.
//! [`clone_recursive`] looks a source address up in it before doing
//! anything else; a hit (a field that aliases an object already cloned
//! earlier in this same call, *or* a field reaching back to an ancestor
//! still being built) returns the existing mapping instead of allocating
//! again or recursing further. The mapping for a freshly allocated object
//! is recorded *before* its own fields are populated (`design`'s own
//! ordering requirement) — this is what makes a cycle terminate: a field
//! that points back at an ancestor already has a recorded mapping by the
//! time the recursive call reaches it.
//!
//! # Collector safety (design D3, `zirk-object-memory`'s own "Deep-clone
//! traversal state is collector-safe for its whole duration" requirement)
//!
//! Every destination object is pushed onto [`CLONE_ROOTS`] — a thread-local,
//! flat extra-root list [`crate::collector::mark`] walks in addition to the
//! ordinary shadow stack — the instant it is allocated and given a valid
//! descriptor, before any of its own fields (which may themselves trigger
//! further allocations, and therefore a collection) are populated. A
//! collection triggered by any of this call's own later allocations
//! therefore always finds every object this call has produced so far
//! already reachable, exactly like an ordinary root.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

thread_local! {
    /// Every destination address a still-in-progress `clone()` call (at any
    /// nesting depth currently active — see this module's own doc comment)
    /// has produced so far. Flat rather than one list per call: each call
    /// only ever appends while it runs, and truncates back to the length it
    /// observed on entry when it finishes ([`zirk_rt_clone`]), which
    /// correctly unwinds nesting (a manual `clone()` body calling
    /// `.clone()` again on an unrelated value, re-entering this same entry
    /// point) without needing to track which entries belong to which call.
    static CLONE_ROOTS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

/// [`crate::collector::mark`]'s own extra-root pass (design D3): every
/// address currently in [`CLONE_ROOTS`], marked exactly like an ordinary
/// shadow-stack root.
pub(crate) fn mark_clone_roots(mut mark_object: impl FnMut(*mut c_void)) {
    CLONE_ROOTS.with(|roots| {
        for &addr in roots.borrow().iter() {
            mark_object(addr);
        }
    });
}

/// The deep-clone-graph traversal (roadmap Phase 4e, `fase-4e-clone`,
/// design D2): allocates a fresh, independent copy of the whole reference
/// graph reachable from `source`, preserving internal sharing (two fields
/// that alias in the source alias the one new clone in the result, never
/// the source itself and never two unrelated copies) and cycles (a field
/// reaching back to an ancestor already being cloned in this same call
/// reuses that ancestor's in-progress mapping instead of recursing
/// forever).
///
/// # Safety
///
/// `source` must be a non-null pointer to a live object this compiler
/// allocated, whose descriptor carries a real class's
/// [`crate::collector::gc_field_offsets`] table — always true for anything
/// generated code actually calls this with, since the checker only emits
/// `InstKind::Clone` for a `Clone`-derived class's own receiver (this
/// module's own doc comment explains why that guarantee extends to every
/// object the traversal goes on to reach, not only `source` itself).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_clone(source: *mut c_void) -> *mut c_void {
    let mut memo: HashMap<usize, *mut c_void> = HashMap::new();
    let roots_start = CLONE_ROOTS.with(|roots| roots.borrow().len());

    let result = unsafe { clone_recursive(source, &mut memo) };

    // This call (and every nested one it made — see this module's own doc
    // comment) is finished: the result is either about to be handed back to
    // generated code, which spills it to an ordinary shadow-stack root
    // itself (`fase-4e-colector-mark-sweep` design D4's unconditional
    // synthetic-slot spill), or is being consumed by an *enclosing* still-
    // active clone call, which already re-pushed it as one of its own
    // sources' mappings before recursing further. Either way this call's
    // own entries stop being needed the instant it returns.
    CLONE_ROOTS.with(|roots| roots.borrow_mut().truncate(roots_start));

    result
}

/// The header's field region starts right after its three fixed words
/// (`crate::collector`'s own module doc comment: descriptor, next, size) —
/// only that region is ever copied byte-for-byte from `source` into a fresh
/// destination; `next`/`size` are left exactly as [`crate::memory::zirk_rt_alloc`]'s
/// own registration already set them for the destination, never overwritten
/// with `source`'s (which would corrupt the collector's own intrusive
/// allocation list).
const FIELD_REGION_START: usize = crate::collector::HEADER_BYTES;

/// One node of the traversal (design D2) — see this module's own doc
/// comment for the algorithm as a whole.
///
/// # Safety
///
/// Same obligation as [`zirk_rt_clone`]: `source` must be null, or a live
/// object this compiler allocated whose descriptor is a real class's own.
unsafe fn clone_recursive(
    source: *mut c_void,
    memo: &mut HashMap<usize, *mut c_void>,
) -> *mut c_void {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    if let Some(&existing) = memo.get(&(source as usize)) {
        return existing;
    }

    let size = unsafe { crate::collector::object_size(source) };
    let descriptor = unsafe { crate::collector::object_descriptor(source) };

    // `String`/`Char` objects are immutable, have no reference fields, and
    // their `bytes` pointer points to either the inline region after the
    // payload or a global constant. Copying the whole object byte-for-byte
    // would leave the clone's `bytes` pointer pointing at the source's inline
    // bytes, so sharing the handle is the correct and sound behaviour.
    if descriptor == crate::collector::string_descriptor() {
        return source;
    }

    // `align` is never read back from a live object (design D1 of
    // `fase-4e-colector-mark-sweep`: only `size` is in the header) and
    // `crate::collector::allocation_align` substitutes its own fixed
    // constant for anything at or under it regardless — any value not
    // wider than that constant is equivalent here.
    let dest = unsafe { crate::memory::zirk_rt_alloc(size, std::mem::size_of::<usize>()) };

    // The descriptor must be valid before `dest` is pushed onto
    // `CLONE_ROOTS` below (design D3): `crate::collector::mark_object` reads
    // it unconditionally the moment it visits a root.
    unsafe { *(dest as *mut *mut c_void) = descriptor };

    if size > FIELD_REGION_START {
        unsafe {
            std::ptr::copy_nonoverlapping(
                (source as *const u8).add(FIELD_REGION_START),
                (dest as *mut u8).add(FIELD_REGION_START),
                size - FIELD_REGION_START,
            );
        }
    }

    // Recorded before recursing into any field (design D2's own ordering
    // requirement — see this module's own doc comment on cycle safety), and
    // pushed as a collector root in the same breath (design D3): from this
    // point on, a collection triggered by any allocation this call still
    // has left to make finds `dest` reachable, fields byte-identical to
    // `source`'s own (its reference fields still point at `source`'s own
    // children until the loop below rewrites them — safe either way, since
    // those are themselves live, already-reachable objects).
    memo.insert(source as usize, dest);
    CLONE_ROOTS.with(|roots| roots.borrow_mut().push(dest));

    let offsets = unsafe { crate::collector::gc_field_offsets(descriptor as *const c_void) };
    for &offset in offsets {
        let field_addr = (dest as usize + offset) as *mut *mut c_void;
        let child_source = unsafe { *field_addr };
        let child_dest = unsafe { clone_recursive(child_source, memo) };
        unsafe { *field_addr = child_dest };
    }

    dest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector;
    use crate::collector::{zirk_rt_pop_frame, zirk_rt_push_frame};

    /// Collector test state is process-global (the heap list and totals moved
    /// process-wide with `ADR-019`), so this module's synthetic-object tests
    /// serialize against the *same* mutex `collector`'s own tests use.
    fn reset_state() -> std::sync::MutexGuard<'static, ()> {
        let guard = collector::test_support::test_guard();
        collector::reset_state_for_tests();
        guard
    }

    /// Builds a synthetic object with `field_count` pointer-sized fields
    /// (all initially null), registered with the collector exactly like a
    /// real allocation, whose descriptor's `gc_field_offsets` names every
    /// one of those fields as a managed reference — mirrors
    /// `collector.rs`'s own `descriptor_with_fields`/`synthetic_object` test
    /// helpers.
    unsafe fn synthetic_clonable_object(
        descriptor: *const c_void,
        field_count: usize,
    ) -> *mut c_void {
        unsafe { collector::synthetic_object_for_tests(descriptor, field_count * 8) }
    }

    #[test]
    fn cloning_a_leaf_object_produces_a_new_independent_address() {
        let _guard = reset_state();
        let descriptor = collector::descriptor_with_fields_for_tests(&[]);
        unsafe {
            let source = synthetic_clonable_object(descriptor.as_ptr() as *const c_void, 0);
            let cloned = zirk_rt_clone(source);
            assert_ne!(source, cloned, "a clone must have new identity");
            assert_eq!(
                collector::object_descriptor(cloned),
                collector::object_descriptor(source),
                "a clone keeps the source's own class descriptor"
            );
        }
    }

    #[test]
    fn cloning_preserves_a_shared_child_as_one_new_shared_clone() {
        let _guard = reset_state();
        // `Parent { left: Child; right: Child; }`, `left is right` before
        // cloning — the spec's own named invariant
        // (`zirk-memory-safety`'s "Deep clone contract").
        let child_descriptor = collector::descriptor_with_fields_for_tests(&[]);
        let parent_descriptor = collector::descriptor_with_fields_for_tests(&[24, 32]);
        unsafe {
            let child = synthetic_clonable_object(child_descriptor.as_ptr() as *const c_void, 0);
            let parent = synthetic_clonable_object(parent_descriptor.as_ptr() as *const c_void, 2);
            let left_field = (parent as usize + 24) as *mut *mut c_void;
            let right_field = (parent as usize + 32) as *mut *mut c_void;
            *left_field = child;
            *right_field = child;

            let cloned_parent = zirk_rt_clone(parent);
            let cloned_left = *((cloned_parent as usize + 24) as *mut *mut c_void);
            let cloned_right = *((cloned_parent as usize + 32) as *mut *mut c_void);

            assert_eq!(
                cloned_left, cloned_right,
                "b.left is b.right must hold after cloning"
            );
            assert_ne!(
                cloned_left, child,
                "b.left is a.left must not hold after cloning"
            );
        }
    }

    #[test]
    fn cloning_a_self_referential_cycle_terminates_and_stays_within_the_new_graph() {
        let _guard = reset_state();
        // `Node { next: Node; }`, `node.next is node` (a one-node cycle).
        let node_descriptor = collector::descriptor_with_fields_for_tests(&[24]);
        unsafe {
            let node = synthetic_clonable_object(node_descriptor.as_ptr() as *const c_void, 1);
            let next_field = (node as usize + 24) as *mut *mut c_void;
            *next_field = node;

            let cloned = zirk_rt_clone(node);
            let cloned_next = *((cloned as usize + 24) as *mut *mut c_void);

            assert_eq!(
                cloned, cloned_next,
                "the cloned cycle must point within the new graph"
            );
            assert_ne!(
                cloned_next, node,
                "the cloned cycle must never point back into the source graph"
            );
        }
    }

    #[test]
    fn a_collection_triggered_mid_clone_does_not_reclaim_a_partially_built_clone() {
        let _guard = reset_state();
        collector::set_threshold_for_tests(1);

        // A short chain deep enough that cloning it crosses a
        // one-byte threshold partway through, forcing
        // `crate::memory::zirk_rt_alloc`'s own cooperative collection to run
        // while `zirk_rt_clone` is still recursing.
        let leaf_descriptor = collector::descriptor_with_fields_for_tests(&[]);
        let chain_descriptor = collector::descriptor_with_fields_for_tests(&[24]);
        unsafe {
            let leaf = synthetic_clonable_object(leaf_descriptor.as_ptr() as *const c_void, 0);
            let middle = synthetic_clonable_object(chain_descriptor.as_ptr() as *const c_void, 1);
            *((middle as usize + 24) as *mut *mut c_void) = leaf;
            let mut head_slot =
                synthetic_clonable_object(chain_descriptor.as_ptr() as *const c_void, 1);
            *((head_slot as usize + 24) as *mut *mut c_void) = middle;

            // A real compiled program keeps a `.clone()` call's own receiver
            // rooted through an ordinary shadow-stack slot for the whole
            // call (`fase-4e-colector-mark-sweep` design D4's unconditional
            // synthetic-slot spill) — a genuine, load-bearing precondition
            // this synthetic test must reproduce by hand, not an
            // implementation detail to skip: without it, `head`/`middle`/
            // `leaf` (the *source* chain `zirk_rt_clone` still needs to
            // read from as it walks) would themselves be reclaimed by the
            // very collection this test forces, which would test nothing
            // about the destination chain at all.
            let mut roots: [*mut c_void; 1] = [(&mut head_slot as *mut *mut c_void).cast()];
            zirk_rt_push_frame(roots.as_mut_ptr(), 1);

            let cloned_head = zirk_rt_clone(head_slot);

            zirk_rt_pop_frame();

            let cloned_middle = *((cloned_head as usize + 24) as *mut *mut c_void);
            assert!(!cloned_middle.is_null(), "the cloned chain must be intact");
            let cloned_leaf = *((cloned_middle as usize + 24) as *mut *mut c_void);
            assert!(
                !cloned_leaf.is_null(),
                "the cloned chain's leaf must survive a collection triggered mid-clone"
            );
        }
    }

    #[test]
    fn cloning_a_class_with_a_string_field_shares_the_string_handle() {
        let _guard = reset_state();
        let string = crate::string::alloc_owned("adjunto");
        let class_descriptor = collector::descriptor_with_fields_for_tests(&[24]);
        unsafe {
            let object = synthetic_clonable_object(class_descriptor.as_ptr() as *const c_void, 1);
            let field = (object as usize + 24) as *mut *mut c_void;
            *field = string;

            let cloned = zirk_rt_clone(object);
            let cloned_field = *((cloned as usize + 24) as *mut *mut c_void);

            assert_eq!(
                cloned_field, string,
                "the clone must share the original string handle"
            );
        }
    }
}
