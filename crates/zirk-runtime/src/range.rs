//! Runtime support for `Range<T>` (roadmap Phase 7).
//!
//! A `Range<T>` is a finite arithmetic sequence: `start`, `end`, `step` and
//! the `..=` flag. It is represented as an opaque handle — a collector-
//! tracked object whose payload is four `i64` words (`ZirkRange`), the same
//! category `ZirkString` already is. The element type is a compile-time
//! concern only: an `Int32` range stores its bounds sign-extended, a
//! `Range<Duration>` stores nanoseconds — the runtime never needs to know
//! which it is holding.
//!
//! Iteration convention: for `step > 0` the sequence runs while
//! `current < end` (`<=` when `inclusive`); for `step < 0` while
//! `current > end` (`>=` when `inclusive`). `zirk_range_slice` always
//! produces the exclusive form — it computes exact bounds instead.

use std::ffi::c_void;

use crate::collector::HEADER_BYTES;
use zirk_runtime_core::range::Range;

/// The `Range` payload: four `i64` words right after the object header.
#[repr(C)]
pub struct ZirkRange {
    pub start: i64,
    pub end: i64,
    pub step: i64,
    /// `0` is exclusive (`..`), anything else inclusive (`..=`).
    pub inclusive: i64,
}

/// The sentinel descriptor every `Range` object carries (design D6): a
/// range holds no managed references, so — exactly like the string
/// descriptor — an all-zero table is the whole answer the collector's
/// `gc_field_offsets` walk needs.
#[unsafe(no_mangle)]
pub static zirk_rt_range_descriptor: [usize; 4] = [0, 0, 0, 0];

fn range_descriptor() -> *mut c_void {
    zirk_rt_range_descriptor.as_ptr() as *mut c_void
}

/// Reads a handle produced by `zirk_range_new`.
///
/// # Safety
///
/// `handle` must come from this runtime and must not have been released.
unsafe fn borrow<'a>(handle: *const c_void) -> Option<&'a ZirkRange> {
    if handle.is_null() {
        return None;
    }
    let payload = unsafe { (handle as *const u8).add(HEADER_BYTES) } as *const ZirkRange;
    Some(unsafe { &*payload })
}

/// Builds the collector-tracked object a `start..end(..step)` expression
/// produces. The caller (generated code) already throws `InvalidStepError`
/// for `step == 0`; a zero step reaching here directly produces an empty
/// range rather than a fault, keeping this helper total.
///
/// # Safety
///
/// The arguments are plain scalars; the returned handle is managed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_new(
    start: i64,
    end: i64,
    step: i64,
    inclusive: i64,
) -> *mut c_void {
    let size = HEADER_BYTES + std::mem::size_of::<ZirkRange>();
    let object = unsafe { crate::zirk_rt_alloc(size, std::mem::align_of::<ZirkRange>()) };
    unsafe {
        *(object as *mut *mut c_void) = range_descriptor();
        let payload = (object as *mut u8).add(HEADER_BYTES) as *mut ZirkRange;
        *payload = ZirkRange {
            start,
            end,
            step,
            inclusive,
        };
    }
    object
}

/// The `start` field of a `Range` — `0` for a null handle.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_start(handle: *const c_void) -> i64 {
    unsafe { borrow(handle) }.map(|r| r.start).unwrap_or(0)
}

/// The `end` field of a `Range` — `0` for a null handle.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_end(handle: *const c_void) -> i64 {
    unsafe { borrow(handle) }.map(|r| r.end).unwrap_or(0)
}

/// The `step` field of a `Range` — `1` for a null handle.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_step(handle: *const c_void) -> i64 {
    unsafe { borrow(handle) }.map(|r| r.step).unwrap_or(1)
}

/// Whether `handle` is an inclusive (`..=`) range — `0`/`1`.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_inclusive(handle: *const c_void) -> i64 {
    unsafe { borrow(handle) }.map(|r| r.inclusive).unwrap_or(0)
}

/// `range.to_string()` (`universal-to-string-and-multiline-strings`):
/// reproduces the literal spelling — `start..end`, `..=` when inclusive,
/// and `:step` only when the stored step differs from the one the
/// constructor infers for an unwritten step (`1` ascending, `-1`
/// descending), so `1..10` and `1..10:1` — the same sequence — render the
/// same. `is_duration` marks a `Range<Duration>`: its parts are
/// nanoseconds rendered as duration literals.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_to_string(
    handle: *mut c_void,
    is_duration: i64,
) -> *mut c_void {
    let Some(range) = (unsafe { borrow(handle) }) else {
        return std::ptr::null_mut();
    };
    crate::string::alloc_owned(
        &Range::new(range.start, range.end, range.step, range.inclusive != 0)
            .format(is_duration != 0),
    )
}

/// How many elements the sequence produces, in `i128` so a huge range
/// (`0..i64::MAX` stepping `1`) never overflows mid-computation.
fn element_count(range: &ZirkRange) -> i128 {
    Range::new(range.start, range.end, range.step, range.inclusive != 0)
        .len()
        .map(i128::from)
        .unwrap_or(i128::from(i64::MAX) + 1)
}

/// The `index`-th element of the sequence (`index` is a position, not a
/// bound): `start + index * step`, in `i128` then saturated back to `i64` —
/// a sliced bound can legitimately sit just past `i64`'s range when the
/// original range's own end did.
fn element_at(range: &ZirkRange, index: i128) -> i64 {
    Range::new(range.start, range.end, range.step, range.inclusive != 0)
        .element_at(index.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}

/// The number of elements in `handle`, or `-1` when it cannot fit in an
/// `i64`.  Lowering turns that sentinel into the language's ordinary
/// `ArithmeticOverflowError` before using the result as an allocation size.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_length(handle: *const c_void) -> i64 {
    let Some(range) = (unsafe { borrow(handle) }) else {
        return 0;
    };
    i64::try_from(element_count(range)).unwrap_or(-1)
}

/// The element at the zero-based sequence position `index`.  Callers obtain
/// `index` from `zirk_range_length`, so an out-of-range value is never used
/// by generated code; direct callers receive `0` for a null handle.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_element_at(handle: *const c_void, index: i64) -> i64 {
    let Some(range) = (unsafe { borrow(handle) }) else {
        return 0;
    };
    element_at(range, index as i128)
}

/// `r[lo:hi:st]` (roadmap Phase 7): slices the *element sequence* — `r[1:3]`
/// is the range of `r`'s own second and third elements, not the numbers
/// `1..3`. `i64::MIN` marks a part the source left out; indices follow the
/// Python convention (`zirk_str_slice`'s own): negative counts from the
/// end, out-of-range clamps, a `step` of `0` is empty. The result is always
/// an exclusive-bound range — the bounds here are exact.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_slice(
    handle: *const c_void,
    start: i64,
    end: i64,
    step: i64,
) -> *mut c_void {
    let Some(range) = (unsafe { borrow(handle) }) else {
        return std::ptr::null_mut();
    };
    let slice = Range::new(range.start, range.end, range.step, range.inclusive != 0)
        .slice(start, end, step);
    unsafe {
        zirk_range_new(
            slice.start,
            slice.end,
            slice.step,
            i64::from(slice.inclusive),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ZirkRange, element_at, element_count};

    #[test]
    fn counts_ascending_descending_and_inclusive_ranges() {
        assert_eq!(
            element_count(&ZirkRange {
                start: 1,
                end: 6,
                step: 2,
                inclusive: 0,
            }),
            3
        );
        assert_eq!(
            element_count(&ZirkRange {
                start: 6,
                end: 0,
                step: -2,
                inclusive: 1,
            }),
            4
        );
        assert_eq!(
            element_count(&ZirkRange {
                start: 4,
                end: 4,
                step: -1,
                inclusive: 1,
            }),
            1
        );
        assert_eq!(
            element_count(&ZirkRange {
                start: 4,
                end: 4,
                step: 1,
                inclusive: 0,
            }),
            0
        );
        assert_eq!(
            element_count(&ZirkRange {
                start: 0,
                end: 4,
                step: 0,
                inclusive: 0,
            }),
            0
        );
    }

    #[test]
    fn indexes_a_descending_range_in_sequence_order() {
        let range = ZirkRange {
            start: 5,
            end: 0,
            step: -2,
            inclusive: 0,
        };
        assert_eq!([0, 1, 2].map(|index| element_at(&range, index)), [5, 3, 1]);
    }
}
