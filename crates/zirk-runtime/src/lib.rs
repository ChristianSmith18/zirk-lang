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

mod clone;
mod collector;
mod exceptions;
mod failure;
mod io;
mod memory;
mod string;

pub use collector::{zirk_rt_pop_frame, zirk_rt_push_frame};
pub use failure::{
    zirk_rt_allocation_failed, zirk_rt_division_by_zero, zirk_rt_fatal_error, zirk_rt_overflow,
};
pub use io::zirk_io_println;
pub use memory::zirk_rt_alloc;
pub use string::{
    zirk_str_concat, zirk_str_eq, zirk_str_from_bool, zirk_str_from_f32, zirk_str_from_f64,
    zirk_str_from_i8, zirk_str_from_i16, zirk_str_from_i32, zirk_str_from_i64, zirk_str_from_i128,
    zirk_str_from_u8, zirk_str_from_u16, zirk_str_from_u32, zirk_str_from_u64, zirk_str_from_u128,
    zirk_str_from_utf8, zirk_str_grapheme_len_at, zirk_str_grapheme_slice, zirk_str_hash,
    zirk_str_is_ascii, zirk_str_repeat,
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
