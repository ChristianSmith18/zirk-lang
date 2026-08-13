//! Representation of `String` in the runtime.
//!
//! The layout is **private to this crate**: the compiler treats a `String` as an
//! opaque handle and never inspects it
//! (`docs/decisions/ADR-005-representacion-string.md`). That is what allows
//! adding the grapheme indexing of `ZIRK_LANGUAGE_SPEC.md` section 3 in Phase 7
//! without touching the lexer, the parser, the IR or codegen.
//!
//! # Memory
//!
//! Phase 1 **does not release** strings. The memory strategy is decided in
//! Phase 4 (`docs/decisions/ADR-003-memoria.md`), and inventing one here would
//! be exactly the kind of tacit assumption that ADR forbids. Programs of this
//! phase are short-lived and the leak is bounded by the literals in the source.

use std::ffi::c_void;

/// A Zirk string.
///
/// Today it is a pointer plus a length over UTF-8 bytes. Tomorrow it may carry
/// the adaptive grapheme index, and nothing outside this crate will notice.
#[repr(C)]
pub struct ZirkString {
    bytes: *const u8,
    len: usize,
}

impl ZirkString {
    /// The contents as a string slice.
    ///
    /// # Safety
    ///
    /// The caller guarantees the handle came from [`zirk_str_from_utf8`] and
    /// that its bytes are still alive.
    pub(crate) unsafe fn as_str(&self) -> &str {
        if self.bytes.is_null() || self.len == 0 {
            return "";
        }
        let slice = unsafe { std::slice::from_raw_parts(self.bytes, self.len) };
        // The compiler only produces literals that were valid UTF-8 in the
        // source, so the lossy path is unreachable in practice; it is there so
        // a corrupt handle cannot cause undefined behaviour.
        std::str::from_utf8(slice).unwrap_or("")
    }
}

/// Builds a `String` from UTF-8 bytes and a length.
///
/// This is how the generated code materializes a literal: the compiler never
/// builds a `String` itself.
///
/// # Safety
///
/// `bytes` must point at `len` readable bytes that outlive the returned handle.
/// Codegen satisfies this by passing global constants of the module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_from_utf8(bytes: *const u8, len: usize) -> *mut c_void {
    let string = Box::new(ZirkString { bytes, len });
    // Deliberately leaked: see the memory note in this module.
    Box::into_raw(string) as *mut c_void
}

/// Reads a handle produced by [`zirk_str_from_utf8`].
///
/// # Safety
///
/// `handle` must come from this runtime and must not have been released.
pub(crate) unsafe fn borrow<'a>(handle: *const c_void) -> Option<&'a ZirkString> {
    if handle.is_null() {
        return None;
    }
    Some(unsafe { &*(handle as *const ZirkString) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(text: &str) -> *mut c_void {
        unsafe { zirk_str_from_utf8(text.as_ptr(), text.len()) }
    }

    #[test]
    fn a_handle_round_trips_its_contents() {
        let handle = build("hola");
        let string = unsafe { borrow(handle) }.expect("valid handle");
        assert_eq!(unsafe { string.as_str() }, "hola");
    }

    #[test]
    fn non_ascii_contents_survive() {
        let handle = build("café ñandú 日本");
        let string = unsafe { borrow(handle) }.expect("valid handle");
        assert_eq!(unsafe { string.as_str() }, "café ñandú 日本");
    }

    #[test]
    fn an_empty_string_is_valid() {
        let handle = build("");
        let string = unsafe { borrow(handle) }.expect("valid handle");
        assert_eq!(unsafe { string.as_str() }, "");
    }

    #[test]
    fn a_null_handle_does_not_abort() {
        assert!(unsafe { borrow(std::ptr::null()) }.is_none());
    }
}
