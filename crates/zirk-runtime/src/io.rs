//! Standard output.
//!
//! `ZIRK_STDLIB_SPEC.md` section 3 defines `std.io` with three streams. Phase 1
//! only implements `println` on standard output, reached through the
//! `stdout.println` intrinsic while modules do not exist (design D4).

use crate::string;
use std::ffi::c_void;
use std::io::Write;

/// Writes a `String` to standard output followed by a line break.
///
/// # Safety
///
/// `handle` must come from `zirk_str_from_utf8` and must not have been
/// released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_println(handle: *const c_void) {
    let text = match unsafe { string::borrow(handle) } {
        Some(string) => unsafe { string.as_str() },
        None => "",
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    // A failed write is not turned into a panic: standard output can legitimately
    // be closed —a pipe that ended— and that is not a program error. Typed I/O
    // errors arrive with `std.io` proper in Phase 7.
    let _ = writeln!(out, "{text}");
}

/// Flushes the output streams.
///
/// Step 6 of the ordered shutdown in `ZIRK_RUNTIME_SPEC.md` section 11.
pub(crate) fn flush() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}
