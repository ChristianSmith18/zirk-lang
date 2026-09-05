//! Runtime helpers for `Array<T>`.

use std::alloc::{Layout, alloc};
use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const HEADER_BYTES: usize = crate::collector::HEADER_BYTES;
const LENGTH_OFFSET: usize = HEADER_BYTES;
const DATA_OFFSET: usize = HEADER_BYTES + WORD;

/// Descriptor for non-reference arrays: no GC fields to trace.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_array_descriptor: [usize; 4] = [0, 0, 0, 0];

/// Builds a descriptor that lists every element slot of a reference array as a
/// GC field so the collector can mark the children.
unsafe fn make_descriptor(base: usize, stride: usize, count: usize) -> *mut c_void {
    let words = 4usize.saturating_add(count);
    let size = words.saturating_mul(WORD);
    let layout = Layout::from_size_align(size, WORD).expect("descriptor layout");
    let pointer = unsafe { alloc(layout) as *mut usize };
    if pointer.is_null() {
        crate::failure::zirk_rt_allocation_failed();
    }
    unsafe {
        pointer.add(0).write(0);
        pointer.add(1).write(0);
        pointer.add(2).write(0);
        pointer.add(3).write(count);
        for i in 0..count {
            pointer.add(4 + i).write(base + i * stride);
        }
    }
    pointer as *mut c_void
}

unsafe fn set_descriptor(object: *mut c_void, descriptor: *mut c_void) {
    unsafe { *(object as *mut *mut c_void) = descriptor };
}

unsafe fn write_usize(object: *mut c_void, offset: usize, value: usize) {
    unsafe { *((object as *mut u8).add(offset) as *mut usize) = value };
}

unsafe fn read_usize(object: *mut c_void, offset: usize) -> usize {
    unsafe { *((object as *mut u8).add(offset) as *mut usize) }
}

/// `Array<T>(capacity)` — allocates a collector-managed array of `capacity`
/// zero-initialized elements.
///
/// # Safety
///
/// `elem_size` and `elem_align` must form a valid layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_new(
    capacity: usize,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    let size = DATA_OFFSET.saturating_add(capacity.saturating_mul(elem_size));
    let align = crate::collector::allocation_align(elem_align);
    let object = unsafe { crate::memory::zirk_rt_alloc(size, align) };

    let descriptor = if is_ref {
        if capacity == 0 {
            &raw const zirk_rt_array_descriptor as *mut c_void
        } else {
            unsafe { make_descriptor(DATA_OFFSET, elem_size, capacity) }
        }
    } else {
        &raw const zirk_rt_array_descriptor as *mut c_void
    };
    unsafe { set_descriptor(object, descriptor) };
    unsafe { write_usize(object, LENGTH_OFFSET, capacity) };
    object
}

/// Returns the length of an `Array<T>`.
///
/// # Safety
///
/// `array` must be a live array produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_length(array: *mut c_void) -> usize {
    unsafe { read_usize(array, LENGTH_OFFSET) }
}

/// `array.clone()` — a new array holding the same elements
/// (`native-type-member-surface`): a shallow copy of the inline element
/// storage.
///
/// # Safety
///
/// `array` must be a live array produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_clone(
    array: *mut c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    let length = unsafe { zirk_rt_array_length(array) };
    let clone = unsafe { zirk_rt_array_new(length, elem_size, elem_align, is_ref) };
    if length > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                (array as *const u8).add(DATA_OFFSET),
                (clone as *mut u8).add(DATA_OFFSET),
                length.saturating_mul(elem_size),
            )
        };
    }
    clone
}

/// `array[start:end:step]` — a new array holding the selected elements
/// (roadmap Phase 7, `Array` slicing). The copy is shallow: for a reference
/// array the new header's descriptor already lists every slot as a GC field,
/// so copying the element pointers is all tracing needs.
///
/// Bounds follow the Python convention `zirk_str_slice` uses: `i64::MIN`
/// marks a part the source left out, negative indices count from the end,
/// out-of-range values clamp, a positive `step` walks `[start, end)` forward
/// and a negative one walks `(end, start]` backward (defaulting to the whole
/// array reversed). A `step` of `0` is invalid and terminates the program,
/// the same check `Range` performs before a range value exists.
///
/// # Safety
///
/// `array` must be a live array produced by this runtime and `elem_size` /
/// `elem_align` must describe its elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_slice(
    array: *mut c_void,
    start: i64,
    end: i64,
    step: i64,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    const MISSING: i64 = i64::MIN;

    let step = if step == MISSING { 1 } else { step };
    if step == 0 {
        // `crate::failure` has no dedicated invalid-step handler yet; the
        // shared `fatal` reports the same cause `Range` checks for.
        crate::failure::fatal("a range's step cannot be zero");
    }

    let n = unsafe { zirk_rt_array_length(array) } as i64;
    if n == 0 {
        return unsafe { zirk_rt_array_new(0, elem_size, elem_align, is_ref) };
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

    // Collects the source indices the slice selects — forward `[lo, hi)`,
    // backward `(hi, lo]` with `-1` meaning "to the beginning".
    let mut indices: Vec<usize> = Vec::new();
    if step > 0 {
        let lo = if start == MISSING { 0 } else { forward(start) };
        let hi = if end == MISSING { n } else { forward(end) };
        let mut i = lo;
        while i < hi {
            indices.push(i as usize);
            i += step;
        }
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
        let mut i = lo;
        while i > hi {
            indices.push(i as usize);
            i += step;
        }
    }

    let length = indices.len();
    let slice = unsafe { zirk_rt_array_new(length, elem_size, elem_align, is_ref) };
    let source = unsafe { (array as *const u8).add(DATA_OFFSET) };
    let target = unsafe { (slice as *mut u8).add(DATA_OFFSET) };
    for (slot, index) in indices.into_iter().enumerate() {
        unsafe {
            std::ptr::copy_nonoverlapping(
                source.add(index * elem_size),
                target.add(slot * elem_size),
                elem_size,
            )
        };
    }
    slice
}

/// Returns a pointer to the element at `index`, or terminates on an out-of-bounds index.
///
/// # Safety
///
/// `array` must be a live array produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_element(
    array: *mut c_void,
    index: i64,
    elem_size: usize,
) -> *mut c_void {
    let length = unsafe { zirk_rt_array_length(array) } as i64;
    let index = if index < 0 { length + index } else { index };
    if index < 0 || index >= length {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    unsafe { (array as *mut u8).add(DATA_OFFSET + index as usize * elem_size) as *mut c_void }
}
