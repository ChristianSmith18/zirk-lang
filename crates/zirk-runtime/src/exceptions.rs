//! `throw`/`try`/`catch`/`finally` propagation (roadmap Phase 4b).
//!
//! There is no real stack unwinding here — no landing pads, no personality
//! function, no DWARF tables. The checker's own "catch or declare" analysis
//! (`zirk-sema/src/checker.rs`'s `pending_throws`/`current_throws`) already
//! proves, at compile time, that every exception a function's body can
//! produce is either caught locally or named in that function's own
//! `throws`, transitively, all the way up to wherever it is finally caught.
//! That proof is what makes a much simpler mechanism sound: one pending
//! exception slot per thread, set by `throw` and checked by codegen right
//! after every call to a function that can throw — the same shape `errno`
//! or Swift's own `swifterror` convention uses, chosen here because it needs
//! no new calling convention or multi-value return, only an ordinary
//! extern "C" call at each of the small number of points that need one
//! (`zirk-ir/src/lower.rs`'s own doc comment on the mechanism has the full
//! picture, design decision D1 of `fase-4b-excepciones`).

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

static STACK_TRACES: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static SUPPRESSED: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn stack_traces() -> &'static Mutex<HashMap<usize, usize>> {
    STACK_TRACES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn suppressed_entries() -> &'static Mutex<HashMap<usize, usize>> {
    SUPPRESSED.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    /// The exception in flight, if any — an object pointer this compiler
    /// allocated, or null. One slot is enough: a second `throw` can only
    /// happen after the first has been taken by a `catch` or has already
    /// unwound out of the thread entirely (an uncaught exception terminates
    /// the thread's own Zirk code before anything else on it runs again).
    static PENDING: Cell<*const c_void> = const { Cell::new(std::ptr::null()) };

}

/// Visits cached references with their owning exception. The collector only
/// traces an edge when the owner is a marked allocation, so these caches do
/// not become roots on their own.
pub(crate) fn visit_owned_edges(mut visit: impl FnMut(*mut c_void, *mut c_void)) {
    let traces: Vec<_> = stack_traces()
        .lock()
        .unwrap()
        .iter()
        .map(|(&owner, &edge)| (owner, edge))
        .collect();
    let suppressed: Vec<_> = suppressed_entries()
        .lock()
        .unwrap()
        .iter()
        .map(|(&owner, &edge)| (owner, edge))
        .collect();
    for (owner, edge) in traces.into_iter().chain(suppressed) {
        visit(owner as *mut c_void, edge as *mut c_void);
    }
}

/// Removes side-table metadata when its owner is reclaimed.
pub(crate) fn owner_swept(owner: *mut c_void) {
    let owner = owner as usize;
    stack_traces().lock().unwrap().remove(&owner);
    suppressed_entries().lock().unwrap().remove(&owner);
}

/// Records `exception` as the pending one (roadmap Phase 4b) — `throw`'s own
/// lowering calls this and then returns from the current function
/// immediately, the same way a diverging `fatalError` does, except this
/// returns normally instead of aborting the process.
///
/// # Safety
///
/// `exception` must be an object pointer this compiler allocated, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_throw(exception: *const c_void) {
    // Capture the Throwable before publishing it as pending. The executor
    // records it only for an opted-in task throwing inside cancellation
    // cleanup (cancel requested + cancellation shield active).
    crate::executor::capture_cancel_cleanup_throw(exception);
    PENDING.with(|cell| cell.set(exception));
}

/// Whether an exception is pending (roadmap Phase 4b) — every call site to a
/// function that can throw checks this immediately after the call.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_has_pending_exception() -> bool {
    PENDING.with(|cell| !cell.get().is_null())
}

/// Takes the pending exception, clearing the slot (roadmap Phase 4b) — a
/// `catch` that matches calls this to bind the value it caught; clearing it
/// is what keeps the same exception from being seen as still-propagating
/// once a handler has it.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_take_pending_exception() -> *const c_void {
    PENDING.with(|cell| cell.replace(std::ptr::null()))
}

/// Returns a cached `String` describing the stack trace of `exception`,
/// building it on the first call. This keeps `throw` itself allocation-free
/// and makes `stack_trace()` idempotent for the same exception object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_stack_trace(exception: *const c_void) -> *mut c_void {
    let key = exception as usize;
    if let Some(handle) = stack_traces().lock().unwrap().get(&key).copied() {
        return handle as *mut c_void;
    }

    let message = format!("Throwable stack trace at {exception:p}");
    let handle = crate::string::alloc_owned(&message);
    stack_traces().lock().unwrap().insert(key, handle as usize);
    handle
}

/// Returns the exception suppressed by `exception`, or `null` if none was set.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_suppressed(exception: *const c_void) -> *mut c_void {
    let key = exception as usize;
    suppressed_entries()
        .lock()
        .unwrap()
        .get(&key)
        .copied()
        .unwrap_or(0) as *mut c_void
}

/// Appends `suppressed` to the end of the exception's suppressed chain.
/// Called before `zirk_rt_throw` when a `throw` statement occurs inside a
/// `catch`, and by concurrent failure aggregation. Existing suppressed
/// exceptions are preserved; duplicate edges and cycles are ignored.
///
/// # Safety
///
/// Both pointers must be non-null Throwable objects allocated by this
/// runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_suppressed(
    exception: *const c_void,
    suppressed: *const c_void,
) {
    if exception.is_null() || suppressed.is_null() {
        crate::failure::fatal("cannot link a null suppressed Throwable");
    }

    let owner = exception as usize;
    let child = suppressed as usize;
    if owner == child {
        return;
    }

    let mut entries = suppressed_entries().lock().unwrap();
    let mut owner_chain = Vec::new();
    let mut owner_nodes = HashSet::new();
    let mut cursor = owner;
    loop {
        if cursor == child || !owner_nodes.insert(cursor) {
            // The child is already present, or the existing chain is cyclic.
            return;
        }
        owner_chain.push(cursor);
        match entries.get(&cursor).copied() {
            Some(next) if next != 0 => cursor = next,
            _ => break,
        }
    }

    // Appending an existing chain is safe only if it does not point back into
    // the owner's chain. Bound the traversal by detecting malformed cycles.
    let mut child_nodes = HashSet::new();
    cursor = child;
    while cursor != 0 {
        if owner_nodes.contains(&cursor) || !child_nodes.insert(cursor) {
            return;
        }
        match entries.get(&cursor).copied() {
            Some(next) => cursor = next,
            None => break,
        }
    }

    let tail = *owner_chain.last().expect("the owner chain starts at owner");
    entries.insert(tail, child);
}

/// Reports an exception that escaped `main` uncaught and terminates
/// (roadmap Phase 4b) — `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
/// section 3: "an uncaught `main` exception emits a diagnostic/trace and
/// exits nonzero". A fixed message rather than the exception's own: reading
/// its `message()` dynamically would need a virtual call built by hand at
/// the one codegen site (`emit_c_entrypoint`) that runs outside the
/// ordinary IR-driven emission pipeline this pass's other work goes
/// through — the same narrowing `stack_trace()`'s own empty stub already
/// makes, for the same reason (rich diagnostics are a separate piece).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_uncaught_exception() -> ! {
    crate::failure::fatal("an exception escaped `main` uncaught")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_exception_edges_are_traced_only_while_the_owner_is_live() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let owner = unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, 8) };
        unsafe {
            *(owner as *mut *mut c_void) =
                crate::array::zirk_rt_array_descriptor.as_ptr() as *mut c_void
        };
        let trace = unsafe { zirk_rt_stack_trace(owner) };
        let mut root = owner;
        let mut roots = [&mut root as *mut *mut c_void as *mut c_void];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(trace));
        crate::zirk_rt_pop_frame();
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(owner));
        assert!(!crate::collector::is_live_for_tests(trace));
        assert!(
            !stack_traces()
                .lock()
                .unwrap()
                .contains_key(&(owner as usize))
        );
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn suppressed_exception_cache_edge_is_traced_and_pruned_with_its_owner() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let owner = unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, 8) };
        let suppressed = unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, 8) };
        unsafe {
            *(owner as *mut *mut c_void) =
                crate::array::zirk_rt_array_descriptor.as_ptr() as *mut c_void;
            *(suppressed as *mut *mut c_void) =
                crate::array::zirk_rt_array_descriptor.as_ptr() as *mut c_void;
            zirk_rt_set_suppressed(owner, suppressed);
        }
        let mut root = owner;
        let mut roots = [&mut root as *mut *mut c_void as *mut c_void];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(suppressed));
        crate::zirk_rt_pop_frame();
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(owner));
        assert!(!crate::collector::is_live_for_tests(suppressed));
        assert!(
            !suppressed_entries()
                .lock()
                .unwrap()
                .contains_key(&(owner as usize))
        );
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn suppressed_exception_links_append_without_overwriting_or_creating_cycles() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let make_throwable = || unsafe {
            let object = crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, 8);
            *(object as *mut *mut c_void) =
                crate::array::zirk_rt_array_descriptor.as_ptr() as *mut c_void;
            object
        };
        let primary = make_throwable();
        let existing = make_throwable();
        let later = make_throwable();

        unsafe {
            zirk_rt_set_suppressed(primary, existing);
            zirk_rt_set_suppressed(primary, later);
        }
        assert_eq!(zirk_rt_suppressed(primary), existing);
        assert_eq!(zirk_rt_suppressed(existing), later);
        assert!(zirk_rt_suppressed(later).is_null());

        // Repeating a cause or linking back into the chain must not replace
        // the original edge or introduce a cycle.
        unsafe {
            zirk_rt_set_suppressed(primary, existing);
            zirk_rt_set_suppressed(later, primary);
        }
        assert_eq!(zirk_rt_suppressed(primary), existing);
        assert_eq!(zirk_rt_suppressed(existing), later);
        assert!(zirk_rt_suppressed(later).is_null());

        let mut entries = suppressed_entries().lock().unwrap();
        for object in [primary, existing, later] {
            entries.remove(&(object as usize));
        }
        drop(entries);
        crate::collector::reset_state_for_tests();
    }
}
