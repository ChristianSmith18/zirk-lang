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

/// How many elements the sequence produces, in `i128` so a huge range
/// (`0..i64::MAX` stepping `1`) never overflows mid-computation.
fn element_count(range: &ZirkRange) -> i128 {
    let (start, end, step) = (range.start as i128, range.end as i128, range.step as i128);
    if step == 0 {
        return 0;
    }
    // The distance still to cover — for a forward range `end - start`, for
    // a backward one `start - end` — plus one more when the bound itself
    // is included.
    let distance = if step > 0 { end - start } else { start - end };
    let distance = distance + i128::from(range.inclusive != 0);
    if distance <= 0 {
        return 0;
    }
    // ceil(distance / |step|)
    let magnitude = step.abs();
    distance.div_euclid(magnitude) + i128::from(distance.rem_euclid(magnitude) != 0)
}

/// The `index`-th element of the sequence (`index` is a position, not a
/// bound): `start + index * step`, in `i128` then saturated back to `i64` —
/// a sliced bound can legitimately sit just past `i64`'s range when the
/// original range's own end did.
fn element_at(range: &ZirkRange, index: i128) -> i64 {
    (range.start as i128 + index * range.step as i128).clamp(i64::MIN as i128, i64::MAX as i128)
        as i64
}

/// `r.reverse()` (roadmap Phase 7): the same elements, produced last to
/// first — a new `Range`, the receiver untouched. A reversed empty range
/// is empty.
///
/// # Safety
///
/// `handle` must come from `zirk_range_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_range_reverse(handle: *const c_void) -> *mut c_void {
    let Some(range) = (unsafe { borrow(handle) }) else {
        return std::ptr::null_mut();
    };
    let count = element_count(range);
    if count == 0 {
        return unsafe { zirk_range_new(0, 0, 1, 0) };
    }
    let last = element_at(range, count - 1);
    // The reversed sequence runs `last, last - step, …, start`: a backward
    // range whose exclusive bound sits one step below the original `start`,
    // so the `current > end` walk still lands exactly on `start`.
    unsafe { zirk_range_new(last, range.start.saturating_sub(range.step), -range.step, 0) }
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
    const MISSING: i64 = i64::MIN;

    let Some(range) = (unsafe { borrow(handle) }) else {
        return std::ptr::null_mut();
    };
    let step = if step == MISSING { 1 } else { step };
    if step == 0 {
        return unsafe { zirk_range_new(0, 0, 1, 0) };
    }
    let count = element_count(range);
    if count == 0 {
        return unsafe { zirk_range_new(0, 0, 1, 0) };
    }
    let n = count.clamp(0, i64::MAX as i128) as i64;

    // Resolve `lo`/`hi` in index space — the same convention
    // `zirk_str_slice` uses: forward `[lo, hi)`, backward `(hi, lo]`.
    let forward = |bound: i64| -> i64 {
        if bound < 0 {
            (n + bound).max(0)
        } else {
            bound.min(n)
        }
    };
    let (first, last) = if step > 0 {
        let lo = if start == MISSING { 0 } else { forward(start) };
        let hi = if end == MISSING { n } else { forward(end) };
        if lo >= hi {
            return unsafe { zirk_range_new(0, 0, 1, 0) };
        }
        (lo, lo + (hi - 1 - lo) / step * step)
    } else {
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
        if lo <= hi {
            return unsafe { zirk_range_new(0, 0, 1, 0) };
        }
        (lo, lo + (hi + 1 - lo) / step * step)
    };

    // The result's step is the composition of both strides: `st` positions
    // of `st_range` elements each.
    let new_step =
        (range.step as i128 * step as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    // Exclusive end, one result-step past the last included element — the
    // walk's own convention (`<`/`>` by step sign) then stops exactly after
    // `last` without an extra compare at build time.
    let new_end = element_at(range, last as i128).saturating_add(new_step);
    unsafe { zirk_range_new(element_at(range, first as i128), new_end, new_step, 0) }
}
