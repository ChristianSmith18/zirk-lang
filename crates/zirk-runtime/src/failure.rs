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

/// Reports that an object could not be allocated and terminates.
///
/// Returning null instead would hand the generated code a pointer it has no
/// way to check: there is no nullable object type at the point an object is
/// built, so nothing downstream would look.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_allocation_failed() -> ! {
    fatal("could not allocate an object")
}
