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
use std::ffi::c_void;

thread_local! {
    /// The exception in flight, if any — an object pointer this compiler
    /// allocated, or null. One slot is enough: a second `throw` can only
    /// happen after the first has been taken by a `catch` or has already
    /// unwound out of the thread entirely (an uncaught exception terminates
    /// the thread's own Zirk code before anything else on it runs again).
    static PENDING: Cell<*const c_void> = const { Cell::new(std::ptr::null()) };
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
