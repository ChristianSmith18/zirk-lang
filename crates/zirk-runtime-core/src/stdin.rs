//! Platform-neutral decoding/bounding logic for `std.io::stdin`.
//!
//! This module never touches a real file descriptor: it operates over any
//! `std::io::Read`, so the native runtime (`zirk-runtime/src/io.rs`, wrapping
//! real process standard input) and the interpreter (wrapping a bounded,
//! injectable byte fixture through `InterpreterHost::read_stdin`) share the
//! exact same line/char/byte/exact-read semantics, per
//! `add-stdin-stdout-standard-library`'s design D2/D3 and the
//! `zirk-standard-streams` capability.
//!
//! Every outcome is carried as a small integer code rather than the `IoError`
//! enum type itself, because this crate never depends on `zirk-sema`/
//! `zirk-ir`. The codes match `std.io`'s `IoError` declaration order exactly
//! (`stdlib/std/io/package.zrk`): `Io = 0`, `Decoding = 1`, `Limit = 2`,
//! `EndOfStream = 3`. Both callers translate a code into the real `IoError`
//! variant using that same fixed order (the native runtime by selecting the
//! matching enum discriminant when building the tagged value, the
//! interpreter by indexing its own copy of the same enum layout).

use std::io::Read;

/// The IoError discriminant for a generic host I/O failure.
pub const ERROR_IO: i32 = 0;
/// The IoError discriminant for invalid UTF-8 input.
pub const ERROR_DECODING: i32 = 1;
/// The IoError discriminant for a `read_bytes` request over the configured
/// bound.
pub const ERROR_LIMIT: i32 = 2;
/// The IoError discriminant for a `read_exact` call that ran out of bytes.
pub const ERROR_END_OF_STREAM: i32 = 3;

/// An arbitrary but documented sane bound on a single `read_bytes` request.
/// The `zirk-standard-streams` specification does not fix a limit; this
/// value keeps a single call from attempting an unbounded allocation while
/// remaining generous for ordinary interactive and piped use.
pub const MAX_STDIN_READ: u64 = 16 * 1024 * 1024;

/// The outcome of a `read_line_result()`/`read_char()` call: either it read
/// something, hit end-of-stream before any byte, or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOutcome {
    Value(String),
    End,
    /// One of the `ERROR_*` discriminants above.
    Error(i32),
}

/// The outcome of a `read_bytes(max)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytesOutcome {
    Value(Vec<u8>),
    End,
    /// One of the `ERROR_*` discriminants above.
    Error(i32),
}

/// The outcome of a `read_exact(count)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactOutcome {
    Ok(Vec<u8>),
    /// One of the `ERROR_*` discriminants above.
    Err(i32),
}

/// Reads one line, stopping at `\n` (stripped along with a preceding `\r`)
/// or end-of-stream.
///
/// An empty terminated line reads as `Value("")`, matching the
/// `zirk-standard-streams` "empty line is not EOF" scenario. End-of-stream
/// reached before any byte reads as `End`. End-of-stream reached mid-line
/// (no trailing terminator) returns the pending text as one final `Value`,
/// the same convention as `read_line` in most host languages — the next
/// call then observes `End`.
pub fn read_line<R: Read>(reader: &mut R) -> TextOutcome {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => {
                if buf.is_empty() {
                    return TextOutcome::End;
                }
                break;
            }
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
            }
            Err(_) => return TextOutcome::Error(ERROR_IO),
        }
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    match String::from_utf8(buf) {
        Ok(text) => TextOutcome::Value(text),
        Err(_) => TextOutcome::Error(ERROR_DECODING),
    }
}

/// The number of bytes a UTF-8 sequence starting with `lead` should occupy,
/// or `None` when `lead` can never start a valid sequence.
fn utf8_sequence_len(lead: u8) -> Option<usize> {
    if lead & 0x80 == 0 {
        Some(1)
    } else if lead & 0xE0 == 0xC0 {
        Some(2)
    } else if lead & 0xF0 == 0xE0 {
        Some(3)
    } else if lead & 0xF8 == 0xF0 {
        Some(4)
    } else {
        None
    }
}

/// Reads exactly one Unicode scalar value, strictly validating UTF-8.
pub fn read_char<R: Read>(reader: &mut R) -> TextOutcome {
    let mut lead = [0u8; 1];
    match reader.read(&mut lead) {
        Ok(0) => return TextOutcome::End,
        Ok(_) => {}
        Err(_) => return TextOutcome::Error(ERROR_IO),
    }
    let Some(expected) = utf8_sequence_len(lead[0]) else {
        return TextOutcome::Error(ERROR_DECODING);
    };
    let mut buf = vec![lead[0]];
    for _ in 1..expected {
        let mut next = [0u8; 1];
        match reader.read(&mut next) {
            Ok(0) => return TextOutcome::Error(ERROR_DECODING),
            Ok(_) => buf.push(next[0]),
            Err(_) => return TextOutcome::Error(ERROR_IO),
        }
    }
    match String::from_utf8(buf) {
        Ok(text) => TextOutcome::Value(text),
        Err(_) => TextOutcome::Error(ERROR_DECODING),
    }
}

/// Reads up to `max` bytes in one bounded read — not necessarily filling
/// `max` (`zirk-standard-streams`: "bounded... not necessarily filling
/// max").
pub fn read_bytes<R: Read>(reader: &mut R, max: u64) -> BytesOutcome {
    if max > MAX_STDIN_READ {
        return BytesOutcome::Error(ERROR_LIMIT);
    }
    let mut buf = vec![0u8; max as usize];
    match reader.read(&mut buf) {
        Ok(0) => BytesOutcome::End,
        Ok(n) => {
            buf.truncate(n);
            BytesOutcome::Value(buf)
        }
        Err(_) => BytesOutcome::Error(ERROR_IO),
    }
}

/// Reads exactly `count` bytes, or fails with `EndOfStream` when fewer
/// remain.
pub fn read_exact<R: Read>(reader: &mut R, count: u64) -> ExactOutcome {
    let mut buf = vec![0u8; count as usize];
    match reader.read_exact(&mut buf) {
        Ok(()) => ExactOutcome::Ok(buf),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            ExactOutcome::Err(ERROR_END_OF_STREAM)
        }
        Err(_) => ExactOutcome::Err(ERROR_IO),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn empty_line_is_a_value_not_end() {
        let mut input = Cursor::new(b"\nrest".to_vec());
        assert_eq!(read_line(&mut input), TextOutcome::Value(String::new()));
    }

    #[test]
    fn eof_before_any_byte_is_end() {
        let mut input = Cursor::new(Vec::new());
        assert_eq!(read_line(&mut input), TextOutcome::End);
    }

    #[test]
    fn crlf_terminator_is_stripped() {
        let mut input = Cursor::new(b"hi\r\n".to_vec());
        assert_eq!(read_line(&mut input), TextOutcome::Value("hi".into()));
    }

    #[test]
    fn invalid_utf8_line_is_a_decoding_error() {
        let mut input = Cursor::new(vec![0xFF, 0xFE, b'\n']);
        assert_eq!(read_line(&mut input), TextOutcome::Error(ERROR_DECODING));
    }

    #[test]
    fn multi_byte_char_reads_as_one_scalar_value() {
        let mut input = Cursor::new("é".as_bytes().to_vec());
        assert_eq!(read_char(&mut input), TextOutcome::Value("é".into()));
    }

    #[test]
    fn truncated_multi_byte_char_is_a_decoding_error() {
        let mut input = Cursor::new(vec!["é".as_bytes()[0]]);
        assert_eq!(read_char(&mut input), TextOutcome::Error(ERROR_DECODING));
    }

    #[test]
    fn char_at_eof_is_end() {
        let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        assert_eq!(read_char(&mut input), TextOutcome::End);
    }

    #[test]
    fn bytes_over_the_bound_is_a_limit_error() {
        let mut input = Cursor::new(Vec::new());
        assert_eq!(
            read_bytes(&mut input, MAX_STDIN_READ + 1),
            BytesOutcome::Error(ERROR_LIMIT)
        );
    }

    #[test]
    fn bytes_read_need_not_fill_max() {
        let mut input = Cursor::new(b"ab".to_vec());
        assert_eq!(
            read_bytes(&mut input, 10),
            BytesOutcome::Value(b"ab".to_vec())
        );
    }

    #[test]
    fn bytes_at_eof_is_end() {
        let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        assert_eq!(read_bytes(&mut input, 4), BytesOutcome::End);
    }

    #[test]
    fn exact_read_succeeds_when_enough_bytes_remain() {
        let mut input = Cursor::new(b"abcd".to_vec());
        assert_eq!(
            read_exact(&mut input, 4),
            ExactOutcome::Ok(b"abcd".to_vec())
        );
    }

    #[test]
    fn exact_read_fails_early_at_end_of_stream() {
        let mut input = Cursor::new(b"ab".to_vec());
        assert_eq!(
            read_exact(&mut input, 4),
            ExactOutcome::Err(ERROR_END_OF_STREAM)
        );
    }
}
