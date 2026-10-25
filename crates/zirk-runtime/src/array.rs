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
unsafe fn make_descriptor(
    base: usize,
    stride: usize,
    count: usize,
    element_offsets: &[usize],
) -> *mut c_void {
    let field_count = count
        .checked_mul(element_offsets.len())
        .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
    let words = 4usize
        .checked_add(field_count)
        .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
    let size = words
        .checked_mul(WORD)
        .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
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
        pointer.add(3).write(field_count);
        let mut index = 0;
        for element in 0..count {
            for &offset in element_offsets {
                let field = base
                    .checked_add(element.saturating_mul(stride))
                    .and_then(|x| x.checked_add(offset))
                    .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
                pointer.add(4 + index).write(field);
                index += 1;
            }
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
    let offsets = if is_ref { &[0usize][..] } else { &[] };
    unsafe { array_new_with_offsets(capacity, elem_size, elem_align, offsets) }
}

/// Allocates an array using recursive managed-reference offsets relative to
/// one element. Offset metadata is expanded into the object's descriptor.
///
/// # Safety
/// A non-empty offset list must point to `usize` values within an element.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_new_with_layout(
    capacity: usize,
    elem_size: usize,
    elem_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    if reference_offset_count > 0 && reference_offsets.is_null() {
        crate::failure::fatal("array reference offsets are null");
    }
    let offsets = if reference_offset_count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(reference_offsets, reference_offset_count) }
    };
    for &offset in offsets {
        if offset.checked_add(WORD).is_none_or(|end| end > elem_size) {
            crate::failure::fatal("array GC offset is outside its element");
        }
    }
    unsafe { array_new_with_offsets(capacity, elem_size, elem_align, offsets) }
}

unsafe fn array_new_with_offsets(
    capacity: usize,
    elem_size: usize,
    elem_align: usize,
    offsets: &[usize],
) -> *mut c_void {
    let size = DATA_OFFSET.saturating_add(capacity.saturating_mul(elem_size));
    let align = crate::collector::allocation_align(elem_align);
    let object = unsafe { crate::memory::zirk_rt_alloc(size, align) };

    let descriptor = if !offsets.is_empty() {
        if capacity == 0 {
            &raw const zirk_rt_array_descriptor as *mut c_void
        } else {
            unsafe { make_descriptor(DATA_OFFSET, elem_size, capacity, offsets) }
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
    let offsets = if is_ref { &[0usize][..] } else { &[] };
    let clone = unsafe { array_new_with_offsets(length, elem_size, elem_align, offsets) };
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

#[unsafe(no_mangle)]
/// # Safety
/// `array` must be a live runtime array; `offsets` must be readable and valid
/// for the supplied element size when `offset_count` is nonzero.
pub unsafe extern "C" fn zirk_rt_array_clone_with_layout(
    array: *mut c_void,
    elem_size: usize,
    elem_align: usize,
    offsets: *const usize,
    offset_count: usize,
) -> *mut c_void {
    let length = unsafe { zirk_rt_array_length(array) };
    let clone = unsafe {
        zirk_rt_array_new_with_layout(length, elem_size, elem_align, offsets, offset_count)
    };
    if length > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                (array as *const u8).add(DATA_OFFSET),
                (clone as *mut u8).add(DATA_OFFSET),
                length.saturating_mul(elem_size),
            );
        }
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
    let offsets = if is_ref { &[0usize][..] } else { &[] };
    unsafe { array_slice_with_offsets(array, start, end, step, elem_size, elem_align, offsets) }
}

#[unsafe(no_mangle)]
/// # Safety
/// `array` must be a live runtime array; `offsets` must be readable and valid
/// for the supplied element size when `offset_count` is nonzero.
pub unsafe extern "C" fn zirk_rt_array_slice_with_layout(
    array: *mut c_void,
    start: i64,
    end: i64,
    step: i64,
    elem_size: usize,
    elem_align: usize,
    offsets: *const usize,
    offset_count: usize,
) -> *mut c_void {
    if offset_count > 0 && offsets.is_null() {
        crate::failure::fatal("array reference offsets are null");
    }
    let offsets = if offset_count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(offsets, offset_count) }
    };
    for &offset in offsets {
        if offset.checked_add(WORD).is_none_or(|end| end > elem_size) {
            crate::failure::fatal("array GC offset is outside its element");
        }
    }
    unsafe { array_slice_with_offsets(array, start, end, step, elem_size, elem_align, offsets) }
}

unsafe fn array_slice_with_offsets(
    array: *mut c_void,
    start: i64,
    end: i64,
    step: i64,
    elem_size: usize,
    elem_align: usize,
    offsets: &[usize],
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
    let slice = unsafe { array_new_with_offsets(length, elem_size, elem_align, offsets) };
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

/// Builds a managed `Array<UInt64>` for immutable filesystem metadata
/// snapshots. `UInt64` values occupy one machine word and carry no GC edges.
pub(crate) fn alloc_u64_array(values: &[u64]) -> *mut c_void {
    let object = unsafe {
        zirk_rt_array_new(
            values.len(),
            std::mem::size_of::<u64>(),
            std::mem::align_of::<u64>(),
            false,
        )
    };
    if !values.is_empty() {
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr().cast::<u8>(),
                (object as *mut u8).add(DATA_OFFSET),
                values.len().saturating_mul(std::mem::size_of::<u64>()),
            );
        }
    }
    object
}

/// Copies an `Array<Byte>` into runtime-owned storage for a host protocol
/// boundary. The caller must keep the managed array rooted for the duration
/// of this call.
pub(crate) unsafe fn copy_byte_array(array: *mut c_void, max_bytes: usize) -> Option<Vec<u8>> {
    if array.is_null() {
        return None;
    }
    let length = unsafe { zirk_rt_array_length(array) };
    if length > max_bytes {
        return None;
    }
    let bytes =
        unsafe { std::slice::from_raw_parts((array as *const u8).add(DATA_OFFSET), length) };
    Some(bytes.to_vec())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_reference_offsets_survive_array_clone_slice_and_collection() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let offsets = [WORD];
        let element_size = 2 * WORD;
        let original = unsafe {
            zirk_rt_array_new_with_layout(2, element_size, WORD, offsets.as_ptr(), offsets.len())
        };
        let child = crate::string::alloc_owned("array-nested-edge");
        for index in 0..2 {
            let slot = unsafe { zirk_rt_array_element(original, index, element_size) };
            unsafe {
                (slot as *mut usize).write(0x55);
                (slot as *mut usize).add(1).write(child as usize);
            }
        }
        let clone = unsafe {
            zirk_rt_array_clone_with_layout(
                original,
                element_size,
                WORD,
                offsets.as_ptr(),
                offsets.len(),
            )
        };
        let slice = unsafe {
            zirk_rt_array_slice_with_layout(
                original,
                1,
                2,
                1,
                element_size,
                WORD,
                offsets.as_ptr(),
                offsets.len(),
            )
        };
        let mut root = original;
        let mut clone_root = clone;
        let mut slice_root = slice;
        let mut roots = [
            &mut root as *mut *mut c_void as *mut c_void,
            &mut clone_root as *mut *mut c_void as *mut c_void,
            &mut slice_root as *mut *mut c_void as *mut c_void,
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(child));
        for array in [original, clone, slice] {
            assert!(unsafe { zirk_rt_array_length(array) } > 0);
            let slot = unsafe { zirk_rt_array_element(array, 0, element_size) };
            assert_eq!(
                unsafe { (slot as *mut usize).add(1).read() },
                child as usize
            );
        }
        crate::zirk_rt_pop_frame();
        crate::collector::reset_state_for_tests();
    }
}
