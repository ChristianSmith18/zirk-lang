//! Runtime helpers for `Char` classification and normalization.
//!
//! A `Char` shares `String`'s representation bit for bit — one extended
//! grapheme stored as a `ZirkString` (ADR-014) — so every function here takes
//! the same opaque `*const c_void` handle and reads it through
//! [`crate::string::borrow`].

use std::ffi::c_void;

use crate::string::{alloc_owned, borrow};

/// The first code point of the grapheme, which carries the classification
/// for the whole `Char`.
///
/// # Safety
///
/// The handle must come from this runtime.
unsafe fn first_char(handle: *const c_void) -> Option<char> {
    let string = unsafe { borrow(handle) }?;
    unsafe { string.as_str() }.chars().next()
}

/// Whether the character is an uppercase letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_uppercase(handle: *const c_void) -> bool {
    unsafe { first_char(handle) }.is_some_and(char::is_uppercase)
}

/// Whether the character is a lowercase letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_lowercase(handle: *const c_void) -> bool {
    unsafe { first_char(handle) }.is_some_and(char::is_lowercase)
}

/// Whether the character is a decimal digit.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_digit(handle: *const c_void) -> bool {
    unsafe { first_char(handle) }.is_some_and(char::is_numeric)
}

/// Whether the character is whitespace.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_whitespace(handle: *const c_void) -> bool {
    unsafe { first_char(handle) }.is_some_and(char::is_whitespace)
}

/// Whether the character is an alphabetic letter.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_char_is_letter(handle: *const c_void) -> bool {
    unsafe { first_char(handle) }.is_some_and(char::is_alphabetic)
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
    let upper: String = text.chars().flat_map(char::to_uppercase).collect();
    alloc_owned(&upper)
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
    let lower: String = text.chars().flat_map(char::to_lowercase).collect();
    alloc_owned(&lower)
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
    match unsafe { first_char(handle) } {
        Some(c) if c.is_ascii() => c as i32,
        _ => -1,
    }
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
    unsafe { first_char(handle) }.is_some_and(char::is_alphanumeric)
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
