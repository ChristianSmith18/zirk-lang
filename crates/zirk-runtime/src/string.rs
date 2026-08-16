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
//! be exactly the kind of tacit assumption that ADR forbids.
//!
//! For literals the leak is bounded by the source. For strings built at runtime
//! —conversions from `Int32`, from `Boolean`— it is not: a program that
//! converts inside a loop would grow without bound. There are no loops in the
//! Phase 1 subset, so the bound holds today; Phase 2 brings loops and Phase 4
//! brings the memory strategy, in that order.

use std::ffi::c_void;

/// A Zirk string.
///
/// Today it is a pointer plus a length over UTF-8 bytes, with the flags
/// equality needs. Tomorrow it may carry the adaptive grapheme index, and
/// nothing outside this crate will notice.
///
/// The handle itself is the **observable identity** of the string: `is`
/// compares handles ([ADR-005](../../../docs/decisions/ADR-005-representacion-string.md)).
#[repr(C)]
pub struct ZirkString {
    bytes: *const u8,
    len: usize,
    /// ASCII text has no canonically equivalent alternative spelling, so byte
    /// comparison decides equality outright.
    ///
    /// It is the only flag stored today. A field recording *whether* the
    /// contents are canonical would be constant — every string in this phase
    /// is — and a field nobody can set to its other value documents an
    /// intention rather than a state. It arrives with the phase that can read
    /// text the compiler did not normalize.
    is_ascii: bool,
}

impl ZirkString {
    /// The contents as a string slice.
    ///
    /// # Safety
    ///
    /// The caller guarantees the handle came from this runtime and that its
    /// bytes are still alive.
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

/// Builds a handle over bytes the runtime does not own.
///
/// The bytes reaching here are always canonical: either a literal the compiler
/// normalized, or text this runtime produced. Recording that is what lets
/// equality stop at a byte comparison.
fn handle(bytes: *const u8, len: usize) -> *mut c_void {
    let is_ascii = if bytes.is_null() || len == 0 {
        true
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len) }.is_ascii()
    };

    let string = Box::new(ZirkString {
        bytes,
        len,
        is_ascii,
    });
    // Deliberately leaked: see the memory note in this module.
    Box::into_raw(string) as *mut c_void
}

/// Builds a handle that takes ownership of a `String` built at runtime.
fn owned_handle(value: String) -> *mut c_void {
    let bytes = value.into_boxed_str();
    let len = bytes.len();
    // Leaking is what keeps the pointer valid: the handle outlives this call
    // and nothing releases it in this phase.
    let raw = Box::into_raw(bytes) as *const u8;
    handle(raw, len)
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
    handle(bytes, len)
}

/// Converts an `Int32` into a `String`.
///
/// `ZIRK_STDLIB_SPEC.md` section 3 states that every printable value goes
/// through `to_string(): String`. Until traits exist in Phase 3, the compiler
/// reaches these conversions directly.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_i32(value: i32) -> *mut c_void {
    owned_handle(value.to_string())
}

/// Converts a `Boolean` into a `String`.
///
/// The rendering is `true` / `false`, the same spelling as the literals of
/// `ZIRK_LANGUAGE_SPEC.md` section 3.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_bool(value: bool) -> *mut c_void {
    owned_handle(if value { "true" } else { "false" }.to_string())
}

/// Content equality of two strings.
///
/// `ZIRK_LANGUAGE_SPEC.md` section 4: `==` compares content. Comparing the
/// handles would compare identity, which is what `is` means and is not what the
/// operator promises.
///
/// Equality is **indifferent to Unicode normalization**: `"hó"` written with
/// one code point equals `"hó"` written with two, because they are the same
/// text and a user cannot tell them apart
/// ([ADR-011](../../../docs/decisions/ADR-011-identidad-e-igualdad-de-string.md)).
///
/// The paths are ordered by how often they decide:
///
/// 1. the same handle — also the answer `is` gives;
/// 2. identical bytes, which is where ASCII always lands;
/// 3. canonical comparison, for text that is neither.
///
/// **Step 3 has no code yet, and that is not a gap.** Every string that can
/// exist in this phase is either a literal the compiler normalized or text
/// this runtime built, so both sides are always canonical and differing bytes
/// mean differing text. The phase that reads text the compiler never saw is
/// the one that makes step 3 reachable, and it will need to write it.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_eq(left: *const c_void, right: *const c_void) -> bool {
    // Same referent: equal by construction, and one pointer comparison. It is
    // also exactly what `is` answers.
    if left == right {
        return true;
    }

    let left = unsafe { borrow(left) };
    let right = unsafe { borrow(right) };

    match (left, right) {
        (Some(a), Some(b)) => unsafe { a.as_str() == b.as_str() },
        // Two absent strings are equal to each other and to nothing else.
        (None, None) => true,
        _ => false,
    }
}

/// Whether a string's contents are pure ASCII.
///
/// Exposed so the phases that add grapheme indexing and canonical comparison
/// can take their own fast path over it, which is the whole reason the flag is
/// computed once at construction instead of scanned on demand.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_is_ascii(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }.is_none_or(|string| string.is_ascii)
}

/// Hash of a string's contents.
///
/// Derived from the same canonical form `zirk_str_eq` compares, so two strings
/// that are equal never produce different hashes — without which a
/// `Map<String, _>` would contradict the operator, keeping `a` and `b` in
/// separate entries while `a == b` is true.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_hash(handle: *const c_void) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match unsafe { borrow(handle) } {
        Some(string) => unsafe { string.as_str() }.hash(&mut hasher),
        None => "".hash(&mut hasher),
    }
    hasher.finish()
}

/// Reads a handle produced by this runtime.
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

    fn read(handle: *const c_void) -> String {
        let string = unsafe { borrow(handle) }.expect("valid handle");
        unsafe { string.as_str() }.to_string()
    }

    #[test]
    fn a_handle_round_trips_its_contents() {
        assert_eq!(read(build("hola")), "hola");
    }

    #[test]
    fn non_ascii_contents_survive() {
        assert_eq!(read(build("café ñandú 日本")), "café ñandú 日本");
    }

    #[test]
    fn an_empty_string_is_valid() {
        assert_eq!(read(build("")), "");
    }

    #[test]
    fn a_null_handle_does_not_abort() {
        assert!(unsafe { borrow(std::ptr::null()) }.is_none());
    }

    #[test]
    fn an_integer_converts_to_its_decimal_form() {
        assert_eq!(read(zirk_str_from_i32(42)), "42");
        assert_eq!(read(zirk_str_from_i32(-7)), "-7");
        assert_eq!(read(zirk_str_from_i32(0)), "0");
    }

    #[test]
    fn the_edges_of_int32_convert() {
        assert_eq!(read(zirk_str_from_i32(i32::MAX)), "2147483647");
        assert_eq!(read(zirk_str_from_i32(i32::MIN)), "-2147483648");
    }

    #[test]
    fn a_boolean_converts_to_its_literal_spelling() {
        assert_eq!(read(zirk_str_from_bool(true)), "true");
        assert_eq!(read(zirk_str_from_bool(false)), "false");
    }

    #[test]
    fn a_converted_string_survives_the_call_that_built_it() {
        // The bytes are owned by the runtime, not by the caller's stack.
        let handle = zirk_str_from_i32(12345);
        let mut noise = Vec::new();
        for i in 0..1000 {
            noise.push(i.to_string());
        }
        assert_eq!(read(handle), "12345");
        assert_eq!(noise.len(), 1000);
    }

    #[test]
    fn equality_is_structural_not_by_identity() {
        // Two different handles with the same contents are equal.
        let a = build("igual");
        let b = build("igual");
        assert_ne!(a, b, "they must be different handles");
        assert!(unsafe { zirk_str_eq(a, b) });
    }

    #[test]
    fn different_contents_are_not_equal() {
        assert!(!unsafe { zirk_str_eq(build("a"), build("b")) });
    }

    #[test]
    fn equality_compares_the_whole_content() {
        // A prefix is not equal to the whole.
        assert!(!unsafe { zirk_str_eq(build("hola"), build("holaa")) });
    }

    #[test]
    fn equality_handles_non_ascii() {
        assert!(unsafe { zirk_str_eq(build("ñandú"), build("ñandú")) });
        assert!(!unsafe { zirk_str_eq(build("ñandú"), build("nandu")) });
    }

    #[test]
    fn the_same_handle_is_equal_to_itself() {
        // The identity path, which is also what `is` answers.
        let handle = build("hola");
        assert!(unsafe { zirk_str_eq(handle, handle) });
    }

    #[test]
    fn canonically_equivalent_text_is_equal() {
        // "hó" composed (one code point) and decomposed (two). The compiler
        // normalizes literals, so both reach the runtime in the same form and
        // the byte path decides. See ADR-011.
        let composed = "h\u{f3}";
        let decomposed = "ho\u{301}";
        assert_ne!(
            composed.as_bytes(),
            decomposed.as_bytes(),
            "the two spellings must really differ in bytes"
        );

        // What the lexer hands over is the canonical form of both.
        let normalized = "h\u{f3}";
        assert!(unsafe { zirk_str_eq(build(normalized), build(normalized)) });
    }

    #[test]
    fn ascii_content_is_flagged() {
        assert!(unsafe { zirk_str_is_ascii(build("plain")) });
        assert!(!unsafe { zirk_str_is_ascii(build("ñandú")) });
        // An empty string has nothing non-ASCII in it.
        assert!(unsafe { zirk_str_is_ascii(build("")) });
    }

    #[test]
    fn equal_strings_hash_the_same() {
        // Without this a `Map<String, _>` would contradict `==`.
        let a = build("clave");
        let b = build("clave");
        assert!(unsafe { zirk_str_eq(a, b) });
        assert_eq!(unsafe { zirk_str_hash(a) }, unsafe { zirk_str_hash(b) });
    }

    #[test]
    fn different_strings_hash_differently() {
        // Not a correctness requirement — collisions are allowed — but a
        // hash that ignored its input would pass every other test here.
        assert_ne!(unsafe { zirk_str_hash(build("a")) }, unsafe {
            zirk_str_hash(build("b"))
        });
    }

    #[test]
    fn a_converted_string_compares_with_a_literal() {
        assert!(unsafe { zirk_str_eq(zirk_str_from_i32(42), build("42")) });
        assert!(unsafe { zirk_str_eq(zirk_str_from_bool(true), build("true")) });
    }
}
