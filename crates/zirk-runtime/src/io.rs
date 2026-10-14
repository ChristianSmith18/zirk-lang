//! Standard output.
//!
//! `ZIRK_STDLIB_SPEC.md` section 3 defines `std.io` with three streams. Phase 1
//! only implements `print` and `println` on standard output, reached through
//! the `stdout` intrinsics while modules do not exist (design D4).

use crate::array;
use crate::string;
use std::ffi::c_void;
use std::io::Write;
use zirk_runtime_core::stdin as stdin_core;

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

/// Writes a `String` to standard output without a line break.
///
/// # Safety
///
/// `handle` must come from `zirk_str_from_utf8` and must not have been
/// released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_print(handle: *const c_void) {
    let text = match unsafe { string::borrow(handle) } {
        Some(string) => unsafe { string.as_str() },
        None => "",
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = write!(out, "{text}");
}

/// Writes a `String` to standard error followed by a line break.
///
/// # Safety
///
/// `handle` must come from `zirk_str_from_utf8` and must not have been
/// released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_eprintln(handle: *const c_void) {
    let text = match unsafe { string::borrow(handle) } {
        Some(string) => unsafe { string.as_str() },
        None => "",
    };
    let stderr = std::io::stderr();
    let mut out = stderr.lock();
    let _ = write!(out, "{text}{LINE_BREAK}");
}

/// Writes a `String` to standard error without a line break.
///
/// # Safety
///
/// `handle` must come from `zirk_str_from_utf8` and must not have been
/// released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_eprint(handle: *const c_void) {
    let text = match unsafe { string::borrow(handle) } {
        Some(string) => unsafe { string.as_str() },
        None => "",
    };
    let stderr = std::io::stderr();
    let mut out = stderr.lock();
    let _ = write!(out, "{text}");
}

/// Flushes the output streams.
///
/// Step 6 of the ordered shutdown in `ZIRK_RUNTIME_SPEC.md` section 11.
pub(crate) fn flush() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}

/// `stdout.flush()`/`stderr.flush()` (`add-stdin-stdout-standard-library`):
/// `0` on success, `1` on a real host failure — the caller (codegen) maps
/// `1` to `IoError.Io`, the only failure this operation can report.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_io_stdout_flush() -> i32 {
    match std::io::stdout().flush() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// See [`zirk_io_stdout_flush`].
#[unsafe(no_mangle)]
pub extern "C" fn zirk_io_stderr_flush() -> i32 {
    match std::io::stderr().flush() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// The uniform out-parameter shape every `stdin.read_*` FFI operation writes,
/// consumed directly by LLVM codegen (`add-stdin-stdout-standard-library`).
///
/// `kind` is the outer outcome: for `ReadResult<T>`-shaped operations
/// (`read_line_result`, `read_char`, `read_bytes`) `0 = Value`, `1 = End`,
/// `2 = Error`; for the `Result<T, IoError>`-shaped `read_exact`, `0 = Ok`,
/// `1 = Err`. `error` is one of `zirk_runtime_core::stdin::ERROR_*` and is
/// only meaningful when `kind` denotes an error. `payload` is an
/// already-built Zirk `String`/`Char`/`Array<Byte>` handle when `kind`
/// denotes a value, and a null pointer otherwise — codegen inserts it into
/// the tagged result unconditionally, but a null/never-read field is sound
/// the same way an unused `BuildEnum` field already is
/// (`crates/zirk-codegen-llvm/src/emit.rs`'s own `BuildEnum` arm).
#[repr(C)]
pub struct StdinReadOut {
    pub kind: i32,
    pub error: i32,
    pub payload: *mut c_void,
}

fn write_text_outcome(out: *mut StdinReadOut, outcome: stdin_core::TextOutcome) {
    let result = match outcome {
        stdin_core::TextOutcome::Value(text) => StdinReadOut {
            kind: 0,
            error: 0,
            payload: string::alloc_owned(&text),
        },
        stdin_core::TextOutcome::End => StdinReadOut {
            kind: 1,
            error: 0,
            payload: std::ptr::null_mut(),
        },
        stdin_core::TextOutcome::Error(code) => StdinReadOut {
            kind: 2,
            error: code,
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = result };
}

fn write_bytes_outcome(out: *mut StdinReadOut, outcome: stdin_core::BytesOutcome) {
    let result = match outcome {
        stdin_core::BytesOutcome::Value(bytes) => StdinReadOut {
            kind: 0,
            error: 0,
            payload: array::alloc_byte_array(&bytes),
        },
        stdin_core::BytesOutcome::End => StdinReadOut {
            kind: 1,
            error: 0,
            payload: std::ptr::null_mut(),
        },
        stdin_core::BytesOutcome::Error(code) => StdinReadOut {
            kind: 2,
            error: code,
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = result };
}

fn write_exact_outcome(out: *mut StdinReadOut, outcome: stdin_core::ExactOutcome) {
    let result = match outcome {
        stdin_core::ExactOutcome::Ok(bytes) => StdinReadOut {
            kind: 0,
            error: 0,
            payload: array::alloc_byte_array(&bytes),
        },
        stdin_core::ExactOutcome::Err(code) => StdinReadOut {
            kind: 1,
            error: code,
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = result };
}

/// `stdin.read_line_result()`. Reads one line from real process standard
/// input.
///
/// # Safety
///
/// `out` must point at valid, writable `StdinReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_stdin_read_line(out: *mut StdinReadOut) {
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    write_text_outcome(out, stdin_core::read_line(&mut lock));
}

/// `stdin.read_char()`. Reads one Unicode scalar value from real process
/// standard input.
///
/// # Safety
///
/// `out` must point at valid, writable `StdinReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_stdin_read_char(out: *mut StdinReadOut) {
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    write_text_outcome(out, stdin_core::read_char(&mut lock));
}

/// `stdin.read_bytes(max)`. Reads up to `max` bytes from real process
/// standard input in one bounded read.
///
/// # Safety
///
/// `out` must point at valid, writable `StdinReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_stdin_read_bytes(max: u64, out: *mut StdinReadOut) {
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    write_bytes_outcome(out, stdin_core::read_bytes(&mut lock, max));
}

/// `stdin.read_exact(count)`. Reads exactly `count` bytes from real process
/// standard input, or fails with `IoError.EndOfStream`.
///
/// # Safety
///
/// `out` must point at valid, writable `StdinReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_io_stdin_read_exact(count: u64, out: *mut StdinReadOut) {
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    write_exact_outcome(out, stdin_core::read_exact(&mut lock, count));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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

    fn out_slot() -> StdinReadOut {
        StdinReadOut {
            kind: -1,
            error: -1,
            payload: std::ptr::null_mut(),
        }
    }

    fn read_payload_text(payload: *mut c_void) -> String {
        let handle = unsafe { string::borrow(payload) }.expect("a text payload handle");
        unsafe { handle.as_str() }.to_string()
    }

    fn read_payload_bytes(payload: *mut c_void, len: usize) -> Vec<u8> {
        assert_eq!(unsafe { array::zirk_rt_array_length(payload) }, len);
        (0..len)
            .map(|i| unsafe { *(array::zirk_rt_array_element(payload, i as i64, 1) as *const u8) })
            .collect()
    }

    #[test]
    fn read_line_result_reports_value_end_and_decoding_error() {
        let mut out = out_slot();
        write_text_outcome(
            &mut out,
            stdin_core::read_line(&mut Cursor::new(b"hi\n".to_vec())),
        );
        assert_eq!(out.kind, 0);
        assert_eq!(read_payload_text(out.payload), "hi");

        let mut out = out_slot();
        write_text_outcome(
            &mut out,
            stdin_core::read_line(&mut Cursor::new(Vec::new())),
        );
        assert_eq!(out.kind, 1);
        assert!(out.payload.is_null());

        let mut out = out_slot();
        write_text_outcome(
            &mut out,
            stdin_core::read_line(&mut Cursor::new(vec![0xFF, b'\n'])),
        );
        assert_eq!(out.kind, 2);
        assert_eq!(out.error, stdin_core::ERROR_DECODING);
    }

    #[test]
    fn read_bytes_reports_value_end_and_limit_error() {
        let mut out = out_slot();
        write_bytes_outcome(
            &mut out,
            stdin_core::read_bytes(&mut Cursor::new(b"abc".to_vec()), 8),
        );
        assert_eq!(out.kind, 0);
        assert_eq!(read_payload_bytes(out.payload, 3), b"abc");

        let mut out = out_slot();
        write_bytes_outcome(
            &mut out,
            stdin_core::read_bytes(&mut Cursor::new(Vec::new()), 4),
        );
        assert_eq!(out.kind, 1);

        let mut out = out_slot();
        write_bytes_outcome(
            &mut out,
            stdin_core::read_bytes(&mut Cursor::new(Vec::new()), stdin_core::MAX_STDIN_READ + 1),
        );
        assert_eq!(out.kind, 2);
        assert_eq!(out.error, stdin_core::ERROR_LIMIT);
    }

    #[test]
    fn read_exact_reports_ok_and_premature_end_of_stream() {
        let mut out = out_slot();
        write_exact_outcome(
            &mut out,
            stdin_core::read_exact(&mut Cursor::new(b"abcd".to_vec()), 4),
        );
        assert_eq!(out.kind, 0);
        assert_eq!(read_payload_bytes(out.payload, 4), b"abcd");

        let mut out = out_slot();
        write_exact_outcome(
            &mut out,
            stdin_core::read_exact(&mut Cursor::new(b"ab".to_vec()), 4),
        );
        assert_eq!(out.kind, 1);
        assert_eq!(out.error, stdin_core::ERROR_END_OF_STREAM);
    }
}
