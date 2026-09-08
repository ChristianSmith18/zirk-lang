//! # zirk-runtime
//!
//! **Responsibility:** the runtime linked into every binary Zirk produces. It
//! provides the application lifecycle and, later on, memory, scheduler,
//! channels and resources.
//!
//! **Boundary:** this crate is **not** part of the compiler and no compiler
//! crate depends on it. It is compiled to a `staticlib` for the target of the
//! compiled program, not for the host `zirkc` runs on.
//!
//! # C ABI boundary
//!
//! Every symbol intended for generated code is declared `extern "C"` without
//! mangling. It is the same boundary `ZIRK_LANGUAGE_SPEC.md` section 13
//! requires for native interoperability: not temporary scaffolding, but the
//! final boundary put to use early (see
//! `docs/decisions/ADR-002-runtime-staticlib.md`).
//!
//! These symbols are a compatibility surface: changing them breaks already
//! compiled binaries.
//!
//! # State
//!
//! The lifecycle of `ZIRK_RUNTIME_SPEC.md` section 2 is:
//!
//! ```text
//! validate init.zrk and permissions -> load minimal runtime -> init globals
//!   -> main() -> concurrency scopes -> close resources -> flush -> exit
//! ```
//!
//! In Phase 0 only the ends of that sequence exist, with empty bodies. They
//! are defined now on purpose: they fix the shape onto which Phase 4 (memory)
//! and Phase 5 (concurrency) hook without refactoring codegen.

mod array;
mod char;
mod clone;
mod collector;
mod context;
mod decimal;
mod duration;
mod exceptions;
mod failure;
mod io;
mod journal;
mod list;
mod map;
mod memory;
mod native_slice;
mod range;
mod regex;
mod resource;
mod scalar;
mod set;
mod string;
mod temporal;

pub use array::*;
pub use char::*;
pub use collector::{zirk_rt_pop_frame, zirk_rt_push_frame};
// Stackful task contexts (ADR-017). Not yet a C-ABI surface — the executor
// (roadmap Phase 5, task group 3) is what will drive these; exported now so the
// module and its ping-pong tests are part of the build.
pub use context::{DEFAULT_TASK_STACK_BYTES, Run, Suspender, TaskContext, spawn_default};
pub use failure::{
    zirk_rt_allocation_failed, zirk_rt_division_by_zero, zirk_rt_fatal_error,
    zirk_rt_index_out_of_bounds, zirk_rt_overflow,
};
pub use io::zirk_io_println;
pub use journal::{
    zirk_rt_journal_begin, zirk_rt_journal_commit, zirk_rt_journal_record, zirk_rt_journal_rollback,
};
pub use list::*;
pub use memory::{zirk_rt_alloc, zirk_rt_dependent_base, zirk_rt_pin_object, zirk_rt_unpin_object};
pub use range::{
    zirk_range_end, zirk_range_inclusive, zirk_range_new, zirk_range_reverse, zirk_range_slice,
    zirk_range_start, zirk_range_step,
};
pub use regex::{
    zirk_regex_find, zirk_regex_find_all, zirk_regex_from_pattern, zirk_regex_is_match,
    zirk_regex_match_group_name, zirk_regex_match_group_pos, zirk_regex_replace, zirk_regex_split,
    zirk_regex_to_string,
};
pub use resource::{zirk_rt_is_cancelled, zirk_rt_resource_close_group, zirk_rt_resource_transfer};
pub use string::{
    zirk_str_concat, zirk_str_eq, zirk_str_from_bool, zirk_str_from_f32, zirk_str_from_f64,
    zirk_str_from_i8, zirk_str_from_i16, zirk_str_from_i32, zirk_str_from_i64, zirk_str_from_i128,
    zirk_str_from_u8, zirk_str_from_u16, zirk_str_from_u32, zirk_str_from_u64, zirk_str_from_u128,
    zirk_str_from_utf8, zirk_str_grapheme_len_at, zirk_str_grapheme_slice, zirk_str_hash,
    zirk_str_is_ascii, zirk_str_repeat, zirk_str_set, zirk_str_slice,
};

/// Initializes the runtime before running `main`.
///
/// Corresponds to the "load minimal runtime" and "init globals" steps of
/// `ZIRK_RUNTIME_SPEC.md` section 2.
///
/// # Safety
///
/// Invoked by Zirk-generated code exactly once, before any other runtime
/// function. Calling it more than once, or after [`zirk_rt_shutdown`], is not
/// supported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_init() {
    // Phase 0: no subsystems to initialize.
    //
    // `ZIRK_RUNTIME_SPEC.md` section 1 asks for lazy subsystem initialization,
    // so this point will likely never do heavy work: it marks the start of the
    // lifecycle, it does not build the whole runtime.
}

/// Shuts the runtime down after `main` returns.
///
/// Corresponds to the "ordered shutdown of resources and managed threads" and
/// the "stream flush" of `ZIRK_RUNTIME_SPEC.md` section 2.
///
/// # Safety
///
/// Invoked by Zirk-generated code exactly once, after `main` and before the
/// process ends. Using any runtime function after this call is not supported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_shutdown() {
    // Step 6 of the ordered shutdown in `ZIRK_RUNTIME_SPEC.md` section 11.
    // There are no resources or managed threads yet, but the streams do have to
    // be flushed: without this, output redirected into a pipe can be lost.
    io::flush();
}

/// Allocates a boxed callable's capture block (roadmap Phase 4d,
/// `phase-4d-callables`, task 5).
///
/// The returned block is a normal GC object: its header carries the supplied
/// descriptor so the collector can trace any captured managed references, and
/// `zirk_rt_clone` can deep-copy them for `.clone()`.
///
/// # Safety
///
/// `descriptor` must be a compiler-emitted closure descriptor and `size` must
/// be the payload size in bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_alloc_callable(
    descriptor: *const std::ffi::c_void,
    size: u64,
) -> *mut std::ffi::c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let total = (size as usize).saturating_add(crate::collector::HEADER_BYTES);
    let pointer = unsafe { crate::memory::zirk_rt_alloc(total, std::mem::size_of::<usize>()) };
    // The descriptor word is left zero by `zirk_rt_alloc`.  No collection can
    // run between the return and this write (single-threaded, collections only
    // start at the top of `zirk_rt_alloc`), so the momentary null is safe.
    unsafe { *(pointer as *mut *const std::ffi::c_void) = descriptor };
    pointer
}

/// Clones a boxed callable's capture block (roadmap Phase 4d,
/// `phase-4d-callables`, task 6).
///
/// # Safety
///
/// `callable` must be a capture block pointer returned by `zirk_rt_alloc_callable`
/// or a previous `zirk_rt_clone_callable`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_clone_callable(
    callable: *const std::ffi::c_void,
) -> *mut std::ffi::c_void {
    if callable.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { crate::clone::zirk_rt_clone(callable as *mut std::ffi::c_void) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimal_lifecycle_is_callable() {
        unsafe {
            zirk_rt_init();
            zirk_rt_shutdown();
        }
    }
}
