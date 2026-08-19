//! Unrecoverable failure handlers.
//!
//! These are the ones the generated code calls when a check fails. They
//! correspond to `fatalError` in `ZIRK_LANGUAGE_SPEC.md` section 9: an
//! unrecoverable state that terminates the process after emitting a diagnostic.
//!
//! `ZIRK_LANGUAGE_SPEC.md` section 13 requires that an index error, a division
//! by zero or an invalid state never become undefined behaviour. These handlers
//! are what keeps that promise.

use crate::io;

/// Exit code for an unrecoverable failure.
///
/// 70 is `EX_SOFTWARE` from `sysexits.h`: an internal software error, which is
/// what these cases are from the point of view of whoever runs the program.
const FATAL_EXIT_CODE: i32 = 70;

fn fatal(reason: &str) -> ! {
    eprintln!("fatal error: {reason}");
    // The streams are flushed before terminating so the diagnostic is not lost
    // when output is buffered into a pipe.
    io::flush();
    std::process::exit(FATAL_EXIT_CODE);
}

/// Reports an arithmetic overflow and terminates.
///
/// `ZIRK_LANGUAGE_SPEC.md` section 3: ordinary overflow produces a controlled
/// error. The wrapping, saturating and checked variants are explicit operations
/// that do not exist in the Phase 1 subset.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_overflow() -> ! {
    fatal("arithmetic overflow")
}

/// Reports a division by zero and terminates.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_division_by_zero() -> ! {
    fatal("division by zero")
}

/// Reports an invalid string repetition count and terminates.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_invalid_repeat() -> ! {
    fatal("a string can only be repeated a non-negative number of times")
}

/// Reports a shift by a negative amount, or by at least the operand's own
/// bit width, and terminates (roadmap task 3b: bitwise and shift operators).
///
/// Neither has a defined result: LLVM's shift instructions are themselves
/// undefined behaviour past the operand's width, so this is checked before
/// the shift ever reaches the native instruction, not after.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_invalid_shift() -> ! {
    fatal("a shift amount must be non-negative and less than the operand's bit width")
}

/// Reports that an object does not carry a contract it was said to satisfy.
///
/// Unreachable in a well-formed program: the checker proved conformance and
/// codegen emitted the table. It exists so a compiler bug fails loudly instead
/// of jumping through whatever the memory happened to hold.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_missing_contract() -> ! {
    fatal("internal error: an object does not carry a contract it satisfies")
}

/// Reports that an object could not be allocated and terminates.
///
/// Returning null instead would hand the generated code a pointer it has no
/// way to check: there is no nullable object type at the point an object is
/// built, so nothing downstream would look.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_allocation_failed() -> ! {
    fatal("could not allocate an object")
}

/// Reports a checked cast (`as`) whose runtime type does not match and
/// terminates (roadmap task 11.6).
///
/// There is no exception machinery yet — `try`/`catch` is Phase 4 — so a
/// failed cast is a controlled process termination, the same as any other
/// checked invariant this module guards, per `ZIRK_LANGUAGE_SPEC.md` section
/// 13's promise that an invalid state never becomes undefined behaviour.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_invalid_cast() -> ! {
    fatal("invalid cast: the value's runtime type is not the target type")
}
