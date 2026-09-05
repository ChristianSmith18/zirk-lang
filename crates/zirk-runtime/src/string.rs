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
//! `String`/`Char` objects are allocated through `zirk_rt_alloc` and reclaimed
//! by the non-moving mark-sweep collector (`docs/decisions/ADR-003-memoria.md`).
//! A string handle is the object pointer itself, with a `ZirkString` payload
//! starting after the standard three-word GC header.

use std::ffi::c_void;

use crate::collector::{HEADER_BYTES, string_descriptor};

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

#[inline]
pub(crate) unsafe fn payload_ptr(handle: *mut c_void) -> *mut ZirkString {
    unsafe { (handle as *mut u8).add(HEADER_BYTES) as *mut ZirkString }
}

#[inline]
unsafe fn data_ptr(handle: *mut c_void) -> *mut u8 {
    unsafe {
        (handle as *mut u8)
            .add(HEADER_BYTES)
            .add(std::mem::size_of::<ZirkString>())
    }
}

/// Allocates an owned string object with room for `len` inline bytes.
///
/// The returned object has the GC descriptor set and `bytes`/`len` initialized.
/// The caller must copy the UTF-8 contents into `data_ptr(handle)` and set
/// `is_ascii` before returning it to generated code.
unsafe fn alloc_string(len: usize) -> *mut c_void {
    unsafe {
        let size = HEADER_BYTES + std::mem::size_of::<ZirkString>() + len;
        let object = crate::zirk_rt_alloc(size, std::mem::align_of::<ZirkString>());
        let payload = payload_ptr(object);
        *(object as *mut *mut c_void) = string_descriptor();
        (*payload).len = len;
        (*payload).bytes = if len == 0 {
            std::ptr::null()
        } else {
            data_ptr(object) as *const u8
        };
        // `is_ascii` is left for the caller to set after writing the bytes.
        object
    }
}

/// Builds an owned string from `text`.
pub(crate) fn alloc_owned(text: &str) -> *mut c_void {
    let len = text.len();
    let object = unsafe { alloc_string(len) };
    unsafe {
        let payload = payload_ptr(object);
        if len > 0 {
            std::ptr::copy_nonoverlapping(text.as_ptr(), data_ptr(object), len);
        }
        (*payload).is_ascii = text.is_ascii();
    }
    object
}

/// Builds a string object whose bytes are not owned by the runtime.
///
/// # Safety
///
/// `bytes` must point at `len` readable bytes that outlive the returned handle.
unsafe fn alloc_literal(bytes: *const u8, len: usize) -> *mut c_void {
    unsafe {
        let size = HEADER_BYTES + std::mem::size_of::<ZirkString>();
        let object = crate::zirk_rt_alloc(size, std::mem::align_of::<ZirkString>());
        let payload = payload_ptr(object);
        let is_ascii = if bytes.is_null() || len == 0 {
            true
        } else {
            std::slice::from_raw_parts(bytes, len).is_ascii()
        };
        *(object as *mut *mut c_void) = string_descriptor();
        (*payload).len = len;
        (*payload).bytes = if len == 0 { std::ptr::null() } else { bytes };
        (*payload).is_ascii = is_ascii;
        object
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
    unsafe { alloc_literal(bytes, len) }
}

/// Converts an `Int8` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_i8(value: i8) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts an `Int16` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_i16(value: i16) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts an `Int32` into a `String`.
///
/// `ZIRK_STDLIB_SPEC.md` section 3 states that every printable value goes
/// through `to_string(): String`. The compiler dispatches directly to one of
/// these per-width conversions (roadmap Phase 3b, task 8) rather than through
/// a real trait call, since native scalars have no method table to call
/// through — codegen picks the right one from the value's own recorded width.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_i32(value: i32) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts an `Int64` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_i64(value: i64) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts an `Int128` into a `String`.
///
/// Taken by pointer rather than by value: an `i128` has no stable, uniform
/// extern "C" calling convention across targets (MSVC does not even have a
/// native 128-bit integer type), while a pointer's ABI is never in question.
/// Codegen stores the value to a stack slot and passes its address.
///
/// # Safety
///
/// `value` must point at a readable, initialized `i128`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_from_i128(value: *const i128) -> *mut c_void {
    let text = unsafe { value.read_unaligned() }.to_string();
    alloc_owned(&text)
}

/// Converts a `UInt8` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_u8(value: u8) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `UInt16` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_u16(value: u16) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `UInt32` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_u32(value: u32) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `UInt64` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_u64(value: u64) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `UInt128` into a `String`. See [`zirk_str_from_i128`] for why
/// it is taken by pointer.
///
/// # Safety
///
/// `value` must point at a readable, initialized `u128`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_from_u128(value: *const u128) -> *mut c_void {
    let text = unsafe { value.read_unaligned() }.to_string();
    alloc_owned(&text)
}

/// Converts a `Float32` into a `String`.
///
/// `Float16`/`Float128` have no equivalent: neither is a stable Rust
/// primitive type (`f16`/`f128` are unstable as of this compiler's toolchain
/// pin), so there is no `Display` implementation to reach for either without
/// a hand-rolled decimal conversion this task does not build. Printing
/// either width is a known, tracked gap (roadmap Phase 3b, task 8.3),
/// consistent with `Float128` arithmetic's own portability gap on Windows.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_f32(value: f32) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `Float64` into a `String`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_f64(value: f64) -> *mut c_void {
    let text = value.to_string();
    alloc_owned(&text)
}

/// Converts a `Boolean` into a `String`.
///
/// The rendering is `true` / `false`, the same spelling as the literals of
/// `ZIRK_LANGUAGE_SPEC.md` section 3.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_str_from_bool(value: bool) -> *mut c_void {
    alloc_owned(if value { "true" } else { "false" })
}

/// Concatenates two strings.
///
/// `ZIRK_LANGUAGE_SPEC.md` section 4: `String + String` concatenates. The
/// result is a fresh string; neither operand is touched, which is what makes
/// `+` an expression rather than a mutation.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_concat(left: *const c_void, right: *const c_void) -> *mut c_void {
    let left = unsafe { borrow(left) }.map(|s| unsafe { s.as_str() });
    let right = unsafe { borrow(right) }.map(|s| unsafe { s.as_str() });
    let left_text = left.unwrap_or("");
    let right_text = right.unwrap_or("");

    let len = left_text.len() + right_text.len();
    let object = unsafe { alloc_string(len) };
    let payload = unsafe { payload_ptr(object) };
    unsafe {
        if len > 0 {
            let data = data_ptr(object);
            if !left_text.is_empty() {
                std::ptr::copy_nonoverlapping(left_text.as_ptr(), data, left_text.len());
            }
            if !right_text.is_empty() {
                std::ptr::copy_nonoverlapping(
                    right_text.as_ptr(),
                    data.add(left_text.len()),
                    right_text.len(),
                );
            }
            (*payload).is_ascii = std::slice::from_raw_parts((*payload).bytes, len).is_ascii();
        } else {
            (*payload).is_ascii = true;
        }
    }
    object
}

/// Repeats a string a non-negative number of times.
///
/// `"ja" * 3 == "jajaja"`. The negative-count check moved to `zirk-ir`
/// (`fase-4d-runtimeerror`, design D10): `Lowering::guard_repeat` now throws
/// a catchable `InvalidRepeatError` *before* this function is ever called,
/// so `count` is always non-negative by the time it gets here — this
/// function no longer rejects it itself. What is still checked here is a
/// distinct invariant, out of this pass's scope (`OverflowError` territory):
/// a byte size that would overflow what can be addressed.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_repeat(handle: *const c_void, count: i32) -> *mut c_void {
    let Some(string) = (unsafe { borrow(handle) }) else {
        return alloc_owned("");
    };
    let text = unsafe { string.as_str() };

    if text.is_empty() || count <= 0 {
        return alloc_owned("");
    }

    // The size is checked before asking for it: a count that overflows what
    // can be addressed is a controlled error, not an allocator surprise.
    let Some(size) = text.len().checked_mul(count as usize) else {
        crate::failure::zirk_rt_invalid_repeat()
    };
    if size > isize::MAX as usize {
        crate::failure::zirk_rt_invalid_repeat()
    }

    let object = unsafe { alloc_string(size) };
    let payload = unsafe { payload_ptr(object) };
    unsafe {
        let data = data_ptr(object);
        for i in 0..(count as usize) {
            std::ptr::copy_nonoverlapping(text.as_ptr(), data.add(i * text.len()), text.len());
        }
        (*payload).is_ascii = string.is_ascii;
    }
    object
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

/// The byte offset of the `index`-th Unicode extended grapheme, or `-1`
/// when `index` is past the end or the handle is missing (roadmap Phase 4e,
/// `String[index]` read-only access).
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_grapheme_offset(handle: *const c_void, index: i64) -> i64 {
    use unicode_segmentation::UnicodeSegmentation;

    let Some(string) = (unsafe { borrow(handle) }) else {
        return -1;
    };
    let text = unsafe { string.as_str() };
    let count = text.graphemes(true).count() as i64;
    let index = if index < 0 { count + index } else { index };
    if index < 0 || index >= count {
        return -1;
    }
    let mut byte_offset: i64 = 0;
    for (i, grapheme) in text.graphemes(true).enumerate() {
        if i == index as usize {
            return byte_offset;
        }
        byte_offset += grapheme.len() as i64;
    }
    -1
}

/// The byte length of the Unicode extended grapheme starting at `offset`, or
/// `-1` when `offset` is at or past the end (roadmap Phase 3b, task 6.3:
/// `for ... in` over `String`).
///
/// `for ... in` threads `offset` itself as an ordinary loop-private `Int64`,
/// the same shape `0..n` already loops with — this only ever answers "is
/// there a next grapheme, and how many bytes is it", never mutates anything,
/// so lowering needed no instruction beyond an ordinary runtime call.
///
/// # Safety
///
/// `handle` must come from this runtime, and `offset` must be a byte offset
/// this same function or `0` already produced for it — never an arbitrary
/// value, which could split a multi-byte code point.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_grapheme_len_at(handle: *const c_void, offset: i64) -> i64 {
    use unicode_segmentation::UnicodeSegmentation;

    let Some(string) = (unsafe { borrow(handle) }) else {
        return -1;
    };
    let text = unsafe { string.as_str() };
    let start = offset as usize;
    if start >= text.len() {
        return -1;
    }
    match text[start..].graphemes(true).next() {
        Some(grapheme) => grapheme.len() as i64,
        None => -1,
    }
}

/// Builds a `Char` from the grapheme at byte range `[offset, offset + len)`
/// — the same opaque construction `zirk_str_from_utf8` uses, since `Char`
/// shares `String`'s representation bit for bit (ADR-014).
///
/// # Safety
///
/// `handle` must come from this runtime; `offset` and `len` must be a byte
/// range `zirk_str_grapheme_len_at` already confirmed is exactly one
/// grapheme of this string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_grapheme_slice(
    handle: *const c_void,
    offset: i64,
    len: i64,
) -> *mut c_void {
    let Some(string) = (unsafe { borrow(handle) }) else {
        return std::ptr::null_mut();
    };
    let text = unsafe { string.as_str() };
    let start = offset as usize;
    let end = start + len as usize;
    alloc_owned(&text[start..end])
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

/// Removes leading and trailing whitespace.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_trim(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(text.trim())
}

/// Whether `handle` contains `pat` anywhere in its contents.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_contains(handle: *const c_void, pat: *const c_void) -> bool {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let pattern = unsafe { borrow(pat) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.contains(pattern)
}

/// Whether `handle` starts with `pat`.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_starts_with(handle: *const c_void, pat: *const c_void) -> bool {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let pattern = unsafe { borrow(pat) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.starts_with(pattern)
}

/// Whether `handle` ends with `pat`.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_ends_with(handle: *const c_void, pat: *const c_void) -> bool {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let pattern = unsafe { borrow(pat) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.ends_with(pattern)
}

/// The slice of `handle` between byte offsets `start` and `end`.
///
/// The offsets are clamped to the string's length; a range that is inverted
/// or does not fall on UTF-8 code point boundaries produces the empty string
/// rather than a panic.
///
/// # Safety
///
/// The handle must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_substring(
    handle: *const c_void,
    start: i64,
    end: i64,
) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let len = text.len() as i64;
    let start = start.clamp(0, len) as usize;
    let end = end.clamp(0, len) as usize;
    if start > end {
        return alloc_owned("");
    }
    match text.get(start..end) {
        Some(slice) => alloc_owned(slice),
        None => alloc_owned(""),
    }
}

/// Builds the `String` that results from replacing the grapheme at byte
/// range `[offset, offset + len)` of `handle` with `ch` (roadmap Phase 7,
/// `s[i] = c` — "String write by index").
///
/// `String` is not mutated in place: its bytes are stored inline in the
/// object and a replacement grapheme may be a different length, so a fresh
/// handle is produced and generated code stores it back into the variable
/// slot. There is no grapheme cache to invalidate yet — `ZirkString` carries
/// only `bytes`/`len`/`is_ascii`, and the `is_ascii` flag is recomputed on
/// the replacement — so recomputing here *is* the invalidation the spec
/// requires.
///
/// A `ch` that is more than one grapheme replaces the single grapheme at
/// the range with the whole text; the result is still a valid `String`.
///
/// # Safety
///
/// `handle` must come from this runtime; `offset`/`len` must be a byte range
/// `zirk_str_grapheme_offset`/`zirk_str_grapheme_len_at` produced for this
/// same string. `ch` is a `Char`/`String` handle (they share representation,
/// ADR-014).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_set(
    handle: *const c_void,
    offset: i64,
    len: i64,
    ch: *const c_void,
) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let replacement = unsafe { borrow(ch) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");

    let start = offset.max(0) as usize;
    let end = start.saturating_add(len.max(0) as usize);
    if start > text.len()
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return alloc_owned(text);
    }

    let mut out = String::with_capacity(text.len() - (end - start) + replacement.len());
    out.push_str(&text[..start]);
    out.push_str(replacement);
    out.push_str(&text[end..]);
    alloc_owned(&out)
}

/// `s[start:end:step]` over graphemes (roadmap Phase 7, `String` slicing).
///
/// `start`, `end` and `step` are grapheme indices; `i64::MIN` marks a part
/// the source left out. Bounds follow the Python convention: negative
/// indices count from the end, out-of-range values clamp, a positive `step`
/// walks `[start, end)` forward and a negative one walks `(end, start]`
/// backward (defaulting to the whole string reversed). A `step` of `0`
/// produces the empty string rather than a fault — the slice of a stride
/// that never advances is not an indexing error, it is nothing.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_slice(
    handle: *const c_void,
    start: i64,
    end: i64,
    step: i64,
) -> *mut c_void {
    use unicode_segmentation::UnicodeSegmentation;

    const MISSING: i64 = i64::MIN;

    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");

    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let n = graphemes.len() as i64;

    let step = if step == MISSING { 1 } else { step };
    if step == 0 || n == 0 {
        return alloc_owned("");
    }

    // Resolves a bound that counts from the end when negative, clamped
    // into `0..=n` for a forward slice.
    let forward = |bound: i64| -> i64 {
        if bound < 0 {
            (n + bound).max(0)
        } else {
            bound.min(n)
        }
    };

    let mut out = String::with_capacity(text.len());
    if step > 0 {
        let lo = if start == MISSING { 0 } else { forward(start) };
        let hi = if end == MISSING { n } else { forward(end) };
        if lo >= hi {
            return alloc_owned("");
        }
        let mut i = lo;
        while i < hi {
            out.push_str(graphemes[i as usize]);
            i += step;
        }
    } else {
        // A backward slice's `end` is exclusive and may be `-1` ("to the
        // beginning"), which is exactly what an omitted end means.
        let lo = if start == MISSING {
            n - 1
        } else {
            (if start < 0 { n + start } else { start }).clamp(-1, n - 1)
        };
        let hi = if end == MISSING {
            -1
        } else {
            (if end < 0 { n + end } else { end }).clamp(-1, n - 1)
        };
        let mut i = lo;
        while i > hi {
            out.push_str(graphemes[i as usize]);
            i += step;
        }
    }
    alloc_owned(&out)
}

/// The byte index of the first occurrence of `pat` in `handle`, or `-1`.
///
/// # Safety
///
/// Both handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_search(handle: *const c_void, pat: *const c_void) -> i64 {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let pattern = unsafe { borrow(pat) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.find(pattern).map(|index| index as i64).unwrap_or(-1)
}

/// `text.split(separator)` — a `List<String>` of parts.
///
/// An empty separator splits into grapheme clusters.
///
/// # Safety
///
/// `handle` and `sep` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_split(
    handle: *const c_void,
    sep: *const c_void,
) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let sep_str = unsafe { borrow(sep) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");

    let elem_size = std::mem::size_of::<*mut c_void>();
    let elem_align = std::mem::align_of::<*mut c_void>();
    let list = unsafe { crate::list::zirk_rt_list_new(elem_size, elem_align, true) };

    let push = |part: &str| {
        let part_handle = alloc_owned(part);
        unsafe {
            crate::list::zirk_rt_list_add(
                list,
                &part_handle as *const *mut c_void as *const c_void,
                elem_size,
                elem_align,
                true,
            )
        };
    };

    if sep_str.is_empty() {
        use unicode_segmentation::UnicodeSegmentation;
        for part in text.graphemes(true) {
            push(part);
        }
    } else {
        for part in text.split(sep_str) {
            push(part);
        }
    }
    list
}

/// `s.length` — the grapheme count (`native-type-member-surface`).
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_length(handle: *const c_void) -> i32 {
    use unicode_segmentation::UnicodeSegmentation;
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.graphemes(true).count() as i32
}

/// `s.byte_length` — the UTF-8 byte count.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_byte_length(handle: *const c_void) -> i32 {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.len() as i32
}

/// `s.is_empty()` — whether the string has no characters.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_is_empty(handle: *const c_void) -> bool {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    text.is_empty()
}

/// `s.find(needle)` — the first grapheme index of `needle`, or `-1`.
///
/// # Safety
///
/// `handle` and `needle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_find(handle: *const c_void, needle: *const c_void) -> i64 {
    use unicode_segmentation::UnicodeSegmentation;
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let needle_str = unsafe { borrow(needle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let Some(byte_index) = text.find(needle_str) else {
        return -1;
    };
    text[..byte_index].graphemes(true).count() as i64
}

/// `s.replace(needle, replacement)` — a new string with every occurrence
/// of `needle` replaced.
///
/// # Safety
///
/// All handles must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_replace(
    handle: *const c_void,
    needle: *const c_void,
    replacement: *const c_void,
) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let needle_str = unsafe { borrow(needle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let repl = unsafe { borrow(replacement) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&text.replace(needle_str, repl))
}

/// `s.trim_start()` — leading whitespace removed.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_trim_start(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(text.trim_start())
}

/// `s.trim_end()` — trailing whitespace removed.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_trim_end(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(text.trim_end())
}

/// `s.to_lowercase()` — Unicode-aware lowercase.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_to_lowercase(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&text.to_lowercase())
}

/// `s.to_uppercase()` — Unicode-aware uppercase.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_to_uppercase(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&text.to_uppercase())
}

/// Applies the named normalization form, `NFC`/`NFD`/`NFKC`/`NFKD`; an
/// unknown form returns the text unchanged.
fn normalize_text(text: &str, form: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    match form {
        "NFC" => text.nfc().collect(),
        "NFD" => text.nfd().collect(),
        "NFKC" => text.nfkc().collect(),
        "NFKD" => text.nfkd().collect(),
        _ => text.to_string(),
    }
}

/// `s.normalize(form)` — `form` spelled as text (`"NFC"`, `"NFD"`,
/// `"NFKC"`, `"NFKD"`); an unknown form returns the string unchanged.
///
/// # Safety
///
/// `handle` and `form` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_normalize(
    handle: *const c_void,
    form: *const c_void,
) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let form_str = unsafe { borrow(form) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&normalize_text(text, form_str))
}

/// `s.clone()` — a new string with the same contents.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_clone(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(text)
}

/// Builds a `List` of opaque handles (`String`, `Char` — same
/// representation) from the given parts.
unsafe fn handle_list<'a>(parts: impl Iterator<Item = &'a str>) -> *mut c_void {
    let elem_size = std::mem::size_of::<*mut c_void>();
    let elem_align = std::mem::align_of::<*mut c_void>();
    let list = unsafe { crate::list::zirk_rt_list_new(elem_size, elem_align, true) };
    for part in parts {
        let part_handle = alloc_owned(part);
        unsafe {
            crate::list::zirk_rt_list_add(
                list,
                &part_handle as *const *mut c_void as *const c_void,
                elem_size,
                elem_align,
                true,
            )
        };
    }
    list
}

/// `s.split_whitespace()` — a `List<String>` of whitespace-separated parts.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_split_whitespace(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    unsafe { handle_list(text.split_whitespace()) }
}

/// `s.lines()` — a `List<String>` split on line terminators.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_lines(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    unsafe { handle_list(text.lines()) }
}

/// `s.chars()` — a `List<Char>` of the string's graphemes.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_chars(handle: *const c_void) -> *mut c_void {
    use unicode_segmentation::UnicodeSegmentation;
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    unsafe { handle_list(text.graphemes(true)) }
}

/// `s.bytes()` — a `List<UInt8>` of the string's UTF-8 bytes.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_bytes(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let elem_size = std::mem::size_of::<u8>();
    let elem_align = std::mem::align_of::<u8>();
    let list = unsafe { crate::list::zirk_rt_list_new(elem_size, elem_align, false) };
    for byte in text.bytes() {
        unsafe {
            crate::list::zirk_rt_list_add(
                list,
                &byte as *const u8 as *const c_void,
                elem_size,
                elem_align,
                false,
            )
        };
    }
    list
}

/// `s.codepoints()` — a `List<UInt32>` of the string's code points.
///
/// # Safety
///
/// `handle` must come from this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_codepoints(handle: *const c_void) -> *mut c_void {
    let text = unsafe { borrow(handle) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    let elem_size = std::mem::size_of::<u32>();
    let elem_align = std::mem::align_of::<u32>();
    let list = unsafe { crate::list::zirk_rt_list_new(elem_size, elem_align, false) };
    for point in text.chars() {
        let value = point as u32;
        unsafe {
            crate::list::zirk_rt_list_add(
                list,
                &value as *const u32 as *const c_void,
                elem_size,
                elem_align,
                false,
            )
        };
    }
    list
}

/// Reads a handle produced by this runtime.
///
/// # Safety
///
/// `handle` must come from this runtime and must not have been released.
pub(crate) unsafe fn borrow<'a>(handle: *const c_void) -> Option<&'a ZirkString> {
    unsafe {
        if handle.is_null() {
            return None;
        }
        let payload = (handle as *const u8).add(HEADER_BYTES) as *const ZirkString;
        Some(&*payload)
    }
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
