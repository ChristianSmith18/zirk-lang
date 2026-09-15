//! Runtime helpers for `Char` classification and normalization.
//!
//! A `Char` shares `String`'s representation bit for bit — one extended
//! grapheme stored as a `ZirkString` (ADR-014) — so every function here takes
//! the same opaque `*const c_void` handle and reads it through
//! [`crate::string::borrow`].

use std::ffi::c_void;

use crate::string::{alloc_owned, borrow};
use zirk_runtime_core::char as core_char;

/// Whether the character is an uppercase letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_uppercase(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }
        .is_some_and(|string| core_char::is_uppercase(unsafe { string.as_str() }))
}

/// Whether the character is a lowercase letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_lowercase(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }
        .is_some_and(|string| core_char::is_lowercase(unsafe { string.as_str() }))
}

/// Whether the character is a decimal digit.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_digit(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }.is_some_and(|string| core_char::is_digit(unsafe { string.as_str() }))
}

/// Whether the character is whitespace.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_whitespace(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }
        .is_some_and(|string| core_char::is_whitespace(unsafe { string.as_str() }))
}

/// Whether the character is an alphabetic letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_letter(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }.is_some_and(|string| core_char::is_letter(unsafe { string.as_str() }))
}

/// The uppercase form of the character as a `Char` (one grapheme string).
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_to_uppercase(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&core_char::uppercase(text))
}

/// The lowercase form of the character as a `Char` (one grapheme string).
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_to_lowercase(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&core_char::lowercase(text))
}

/// `c.byte_length` — the grapheme's UTF-8 byte count.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_byte_length(handle: *const c_void) -> i32 {
    unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .len() as i32
}

/// `c.codepoint_count` — how many code points the grapheme holds.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_codepoint_count(handle: *const c_void) -> i32 {
    unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .chars()
        .count() as i32
}

/// `c.ascii_code()` — the code of an ASCII character, `-1` otherwise.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_ascii_code(handle: *const c_void) -> i32 {
    unsafe { borrow(handle) }
        .map(|string| core_char::ascii_code(unsafe { string.as_str() }))
        .unwrap_or(-1)
}

/// Whether the grapheme is a single ASCII code point.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_ascii(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .is_some_and(|text| text.is_ascii())
}

/// `c.is_alphabetic()` — the documented alias of `is_letter`.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_alphabetic(handle: *const c_void) -> bool {
    unsafe { zirk_char_is_letter(handle) }
}

/// `c.is_numeric()` — the documented alias of `is_digit`.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_numeric(handle: *const c_void) -> bool {
    unsafe { zirk_char_is_digit(handle) }
}

/// Whether the character is alphabetic or numeric.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_alphanumeric(handle: *const c_void) -> bool {
    unsafe { borrow(handle) }
        .is_some_and(|string| core_char::is_alphanumeric(unsafe { string.as_str() }))
}

/// `c.normalize(form)` — the grapheme normalized to the named form
/// (`"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"`); unknown forms return it
/// unchanged.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_normalize(
    handle: *const c_void,
    form: *const c_void,
) -> *mut c_void {
    unsafe { crate::string::zirk_str_normalize(handle, form) }
}

/// `c.bytes()` — a `List<UInt8>` of the grapheme's UTF-8 bytes.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_bytes(handle: *const c_void) -> *mut c_void {
    unsafe { crate::string::zirk_str_bytes(handle) }
}

/// `c.codepoints()` — a `List<UInt32>` of the grapheme's Unicode scalars.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_codepoints(handle: *const c_void) -> *mut c_void {
    unsafe { crate::string::zirk_str_codepoints(handle) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(handle: *mut c_void) -> String {
        unsafe { (*crate::string::payload_ptr(handle)).as_str().to_owned() }
    }

    #[test]
    fn extracted_char_semantics_match_the_native_handles() {
        let uppercase = crate::string::alloc_owned("Ñ");
        let sharp_s = crate::string::alloc_owned("ß");
        let non_ascii = crate::string::alloc_owned("ñ");
        assert!(unsafe { zirk_char_is_uppercase(uppercase) });
        assert!(unsafe { zirk_char_is_letter(non_ascii) });
        assert_eq!(unsafe { zirk_char_ascii_code(non_ascii) }, -1);
        assert_eq!(text(unsafe { zirk_char_to_uppercase(sharp_s) }), "SS");
    }
}
