//! Resource management stubs for roadmap Phase 4c.
//!
//! These functions are the C-ABI boundary the backend will call for grouped
//! resource cleanup, ownership transfer, and cancellation-aware close.  They
//! are intentionally minimal for this phase: they do not block, they return
//! predictable error/variant values, and they keep the build green while the
//! compiler side of the feature is still landing.

use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

/// Placeholder cancellation token for roadmap Phase 4c.
///
/// Real implementation will consult the active concurrency scope once Phase 5
/// tasks introduce cancellation tokens. Until then the token is always clear,
/// so `zirk_rt_is_cancelled` returns `false`.
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Reports whether the active cancellation token is set.
///
/// # Safety
///
/// This stub has no preconditions; it is called by generated code exactly as a
/// side-effect-free predicate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_is_cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

/// Stub: closes a group of already-acquired resources by calling each
/// `close()` thunk in reverse order.  For now this does nothing and returns
/// an opaque `Result<Void, Error>` placeholder: it is a runtime guardrail
/// against blocking and a place to hang the cancellation check later.
///
/// `resources` is an opaque, null-terminated array of resource pointers
/// supplied by the compiler when grouped `match with` lowering is ready.
/// `close` is an opaque function-pointer array in the same order.  The stub
/// simply walks it right-to-left without invoking any user code.
///
/// # Safety
///
/// This stub ignores its arguments and does not dereference them; the caller
/// must pass valid, aligned pointers once the function is no longer a stub.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_resource_close_group(
    _resources: *mut *mut c_void,
    _close: *mut Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
) -> *mut c_void {
    // Phase 4c stub: do not block, do not call blocking user code, and
    // return a `Ok(())`-shaped sentinel (`null` is fine at this stage
    // because no consumer interprets the payload yet).
    std::ptr::null_mut()
}

/// Stub: transfers ownership of a resource by returning the same pointer.
///
/// The real implementation will flip a `moved` bit in the resource header
/// and clone the descriptor into a fresh, owned slot so that debug builds
/// can report `use after transfer`.  The stub only fixes the C-ABI symbol.
///
/// # Safety
///
/// `resource` must be a live, owned resource pointer provided by generated
/// code.  The stub returns the same pointer; the caller must not use the
/// source slot once the transfer has been recorded.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_resource_transfer(resource: *mut c_void) -> *mut c_void {
    resource
}
