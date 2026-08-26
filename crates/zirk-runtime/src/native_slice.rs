//! `NativeSlice<T>`/`NativeSliceMut<T>` construction validation.
//!
//! Roadmap Phase 4e, `fase-4e-native-slice`, design D3: `pointer.as_slice(length)`/
//! `.as_slice_mut(length)` validate once, at construction, before a view value
//! exists — nullability, alignment against the element type, extent
//! representability, and — when the underlying storage's own size is known at
//! the call site (`zirk-ir`'s own `Self::known_pointer_extent`, a compile-time,
//! syntactic scope decision, not general provenance tracking) — that `length`
//! does not exceed it.
//!
//! This is a pure computation, not a `fatal`-and-terminate handler like
//! `crate::failure`'s: a failed check becomes `Result::Error(NativeError)` in
//! the generated code, not a process exit, so it returns a plain `bool`
//! instead of diverging.

use std::ffi::c_void;

/// Validates a `NativeSlice<T>`/`NativeSliceMut<T>` construction, returning
/// whether every check passed.
///
/// - `pointer` must not be null.
/// - `pointer`'s address must be a multiple of `elem_align` (when
///   `elem_align > 1`).
/// - `length * elem_size` must not overflow a `u64` — an unrepresentable
///   extent.
/// - When `known_length >= 0`, `length` must not exceed it — the one case
///   this pass can cross-check against a real, statically-known extent
///   (`zirk-ir`'s own `Self::known_pointer_extent`). `known_length < 0`
///   means opaque provenance: nothing further is checked, and the caller's
///   `length` is trusted beyond what is checkable above (the accepted risk
///   `design.md`'s own "Risks/Trade-offs" documents).
///
/// # Safety
///
/// `pointer` is not dereferenced — only its address is inspected (null
/// check, alignment arithmetic) — so this is safe to call with any pointer
/// value, including one that does not point to a real allocation.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_native_slice_validate(
    pointer: *const c_void,
    length: u64,
    elem_size: u64,
    elem_align: u64,
    known_length: i64,
) -> bool {
    if pointer.is_null() {
        return false;
    }
    if elem_align > 1 && !(pointer as usize).is_multiple_of(elem_align as usize) {
        return false;
    }
    if length.checked_mul(elem_size).is_none() {
        return false;
    }
    if known_length >= 0 && length > known_length as u64 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn a_valid_construction_passes() {
        let mut value: i32 = 0;
        let pointer = ptr::addr_of_mut!(value) as *const c_void;
        assert!(zirk_rt_native_slice_validate(pointer, 1, 4, 4, 1));
    }

    #[test]
    fn a_null_pointer_is_rejected() {
        assert!(!zirk_rt_native_slice_validate(ptr::null(), 1, 4, 4, -1));
    }

    #[test]
    fn a_misaligned_pointer_is_rejected() {
        let bytes: [u8; 8] = [0; 8];
        // Deliberately offset by one byte so a 4-byte-aligned element type
        // never lands on a multiple of its own alignment.
        let pointer = unsafe { bytes.as_ptr().add(1) } as *const c_void;
        if (pointer as usize).is_multiple_of(4) {
            // The host allocator happened to align `bytes` such that +1 is
            // still 4-aligned (vanishingly unlikely, but not impossible) —
            // skip rather than assert a false failure.
            return;
        }
        assert!(!zirk_rt_native_slice_validate(pointer, 1, 4, 4, -1));
    }

    #[test]
    fn an_unrepresentable_extent_is_rejected() {
        let mut value: i32 = 0;
        let pointer = ptr::addr_of_mut!(value) as *const c_void;
        assert!(!zirk_rt_native_slice_validate(pointer, u64::MAX, 8, 4, -1));
    }

    #[test]
    fn a_length_exceeding_a_known_extent_is_rejected() {
        let mut value: i32 = 0;
        let pointer = ptr::addr_of_mut!(value) as *const c_void;
        assert!(!zirk_rt_native_slice_validate(pointer, 2, 4, 4, 1));
    }

    #[test]
    fn opaque_provenance_trusts_the_caller_length() {
        let mut value: i32 = 0;
        let pointer = ptr::addr_of_mut!(value) as *const c_void;
        assert!(zirk_rt_native_slice_validate(pointer, 5, 4, 4, -1));
    }
}
