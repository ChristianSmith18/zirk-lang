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
    crate::collector::register_dynamic_descriptor(pointer.cast(), layout);
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

/// Writes `value` into slot `index` of a word-sized-element array — the
/// `i64` snapshots `zirk_rt_map_sorted_keys`/`zirk_rt_set_sorted_elements`
/// build for `to_string()` (`universal-to-string-and-multiline-strings`).
///
/// # Safety
///
/// `array` must be a live array produced by `zirk_rt_array_new` with an
/// element size of at least one word, and `index` must be in bounds.
pub(crate) unsafe fn write_word(array: *mut c_void, index: usize, value: i64) {
    unsafe {
        *((array as *mut u8).add(DATA_OFFSET + index * WORD) as *mut i64) = value;
    }
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
    let indices = match zirk_runtime_core::collections::slice_indices(
        unsafe { zirk_rt_array_length(array) },
        start,
        end,
        step,
    ) {
        Ok(indices) => indices,
        Err(message) => crate::failure::fatal(message),
    };

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

/// Builds an owned `Array<Byte>` from `bytes`, copying its contents.
///
/// Used by `stdin.read_bytes`/`stdin.read_exact` (`add-stdin-stdout-standard-library`)
/// to turn a host-read buffer into a Zirk value: `Byte` is a one-byte,
/// non-reference element, so this is `zirk_rt_array_new(bytes.len(), 1, 1,
/// false)` followed by a raw copy into its data region.
pub(crate) fn alloc_byte_array(bytes: &[u8]) -> *mut c_void {
    let object = unsafe { zirk_rt_array_new(bytes.len(), 1, 1, false) };
    if !bytes.is_empty() {
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (object as *mut u8).add(DATA_OFFSET),
                bytes.len(),
            );
        }
    }
    object
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
