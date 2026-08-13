//! Standard output.
//!
//! `ZIRK_STDLIB_SPEC.md` section 3 defines `std.io` with three streams. Phase 1
//! only implements `println` on standard output, reached through the
//! `stdout.println` intrinsic while modules do not exist (design D4).

use crate::string;
use std::ffi::c_void;
use std::io::Write;

/// The platform line break.
///
/// `ZIRK_STDLIB_SPEC.md` section 3 states that `println` writes "the platform
/// line break", so this is not Rust's `writeln!`, which always emits `\n`.
///
/// The consequence is real and worth stating: output piped from a Windows
/// program carries CRLF. That is what the spec asks for, and changing it is a
/// change to the spec, not to this file.
const LINE_BREAK: &str = if cfg!(windows) { "\r\n" } else { "\n" };

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
    let _ = write!(out, "{text}{LINE_BREAK}");
}

/// Flushes the output streams.
///
/// Step 6 of the ordered shutdown in `ZIRK_RUNTIME_SPEC.md` section 11.
pub(crate) fn flush() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}

#[cfg(test)]
mod tests {
    use super::LINE_BREAK;

    #[test]
    fn the_line_break_is_the_platform_one() {
        // `ZIRK_STDLIB_SPEC.md` section 3 requires the platform line break, not
        // an unconditional `\n`.
        if cfg!(windows) {
            assert_eq!(LINE_BREAK, "\r\n");
        } else {
            assert_eq!(LINE_BREAK, "\n");
        }
    }
}
