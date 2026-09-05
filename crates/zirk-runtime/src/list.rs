//! Runtime helpers for `List<T>`.

use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const HEADER_BYTES: usize = crate::collector::HEADER_BYTES;
const LENGTH_OFFSET: usize = HEADER_BYTES;
const CAPACITY_OFFSET: usize = HEADER_BYTES + WORD;
const DATA_PTR_OFFSET: usize = HEADER_BYTES + 2 * WORD;

/// Descriptor for a `List<T>` header object: it has a single GC field, the
/// pointer to the data buffer.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_list_descriptor: [usize; 5] = [0, 0, 0, 1, DATA_PTR_OFFSET];

unsafe fn set_descriptor(object: *mut c_void, descriptor: *mut c_void) {
    unsafe { *(object as *mut *mut c_void) = descriptor };
}

unsafe fn write_usize(object: *mut c_void, offset: usize, value: usize) {
    unsafe { *((object as *mut u8).add(offset) as *mut usize) = value };
}

unsafe fn read_usize(object: *mut c_void, offset: usize) -> usize {
    unsafe { *((object as *mut u8).add(offset) as *mut usize) }
}

unsafe fn set_ptr(object: *mut c_void, offset: usize, value: *mut c_void) {
    unsafe { *((object as *mut u8).add(offset) as *mut *mut c_void) = value };
}

unsafe fn read_ptr(object: *mut c_void, offset: usize) -> *mut c_void {
    unsafe { *((object as *mut u8).add(offset) as *mut *mut c_void) }
}

/// `List<T>()` — allocates an empty list.
///
/// # Safety
///
/// `elem_size` and `elem_align` must form a valid layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_new(
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    let _ = (elem_size, elem_align, is_ref);
    let size = HEADER_BYTES + 3 * WORD;
    let object = unsafe { crate::memory::zirk_rt_alloc(size, 1) };
    unsafe {
        set_descriptor(object, &raw const zirk_rt_list_descriptor as *mut c_void);
        write_usize(object, LENGTH_OFFSET, 0);
        write_usize(object, CAPACITY_OFFSET, 0);
        set_ptr(object, DATA_PTR_OFFSET, std::ptr::null_mut());
    }
    object
}

/// Returns the length of a `List<T>`.
///
/// # Safety
///
/// `list` must be a live list produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_length(list: *mut c_void) -> usize {
    unsafe { read_usize(list, LENGTH_OFFSET) }
}

unsafe fn list_capacity(list: *mut c_void) -> usize {
    unsafe { read_usize(list, CAPACITY_OFFSET) }
}

unsafe fn data_pointer(list: *mut c_void) -> *mut c_void {
    unsafe { read_ptr(list, DATA_PTR_OFFSET) }
}

unsafe fn element_pointer(data: *mut c_void, index: usize, elem_size: usize) -> *mut c_void {
    unsafe { (data as *mut u8).add(HEADER_BYTES + index * elem_size) as *mut c_void }
}

unsafe fn make_descriptor(base: usize, stride: usize, count: usize) -> *mut c_void {
    use std::alloc::{Layout, alloc};
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

unsafe fn grow_buffer(
    list: *mut c_void,
    needed: usize,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) {
    let current = unsafe { list_capacity(list) };
    if current >= needed {
        return;
    }
    let new_capacity = needed.max(current.saturating_mul(2)).max(1);
    let size = HEADER_BYTES.saturating_add(new_capacity.saturating_mul(elem_size));
    let align = crate::collector::allocation_align(elem_align);
    let new_data = unsafe { crate::memory::zirk_rt_alloc(size, align) };

    let descriptor = if is_ref && new_capacity > 0 {
        unsafe { make_descriptor(HEADER_BYTES, elem_size, new_capacity) }
    } else {
        &raw const crate::array::zirk_rt_array_descriptor as *mut c_void
    };
    unsafe { set_descriptor(new_data, descriptor) };

    let old_data = unsafe { data_pointer(list) };
    if !old_data.is_null() {
        let length = unsafe { zirk_rt_list_length(list) };
        let bytes = length.saturating_mul(elem_size);
        unsafe {
            std::ptr::copy_nonoverlapping(
                (old_data as *mut u8).add(HEADER_BYTES),
                (new_data as *mut u8).add(HEADER_BYTES),
                bytes,
            )
        };
    }

    unsafe {
        write_usize(list, CAPACITY_OFFSET, new_capacity);
        set_ptr(list, DATA_PTR_OFFSET, new_data);
    }
}

/// Returns a pointer to the element at `index`, or terminates on an out-of-bounds index.
///
/// # Safety
///
/// `list` must be a live list produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_element(
    list: *mut c_void,
    index: i64,
    elem_size: usize,
) -> *mut c_void {
    let length = unsafe { zirk_rt_list_length(list) } as i64;
    let index = if index < 0 { length + index } else { index };
    if index < 0 || index >= length {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    let data = unsafe { data_pointer(list) };
    if data.is_null() {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    unsafe { element_pointer(data, index as usize, elem_size) }
}

/// `list.add(value)` — appends a value.
///
/// # Safety
///
/// `list` must be a live list and `value` a pointer to an element of `elem_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_add(
    list: *mut c_void,
    value: *const c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) {
    let length = unsafe { zirk_rt_list_length(list) };
    unsafe { grow_buffer(list, length + 1, elem_size, elem_align, is_ref) };
    let data = unsafe { data_pointer(list) };
    let slot = unsafe { element_pointer(data, length, elem_size) };
    unsafe { std::ptr::copy_nonoverlapping(value as *const u8, slot as *mut u8, elem_size) };
    unsafe { write_usize(list, LENGTH_OFFSET, length + 1) };
}

/// `list.insert(index, value)` — inserts a value at `index`.
///
/// # Safety
///
/// `list` must be a live list and `index` must be at most the current length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_insert(
    list: *mut c_void,
    index: i64,
    value: *const c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) {
    let length = unsafe { zirk_rt_list_length(list) } as i64;
    let index = if index < 0 { length + index } else { index };
    if index < 0 || index > length {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    let index_u = index as usize;
    let length_u = length as usize;
    unsafe { grow_buffer(list, length_u + 1, elem_size, elem_align, is_ref) };
    let data = unsafe { data_pointer(list) };
    if index_u < length_u {
        let src = unsafe { element_pointer(data, index_u, elem_size) };
        let dst = unsafe { element_pointer(data, index_u + 1, elem_size) };
        let bytes = (length_u - index_u).saturating_mul(elem_size);
        unsafe { std::ptr::copy(src as *const u8, dst as *mut u8, bytes) };
    }
    let slot = unsafe { element_pointer(data, index_u, elem_size) };
    unsafe { std::ptr::copy_nonoverlapping(value as *const u8, slot as *mut u8, elem_size) };
    unsafe { write_usize(list, LENGTH_OFFSET, length_u + 1) };
}

/// `list.clone()` — a new list holding the same elements
/// (`native-type-member-surface`): a shallow copy of the element buffer,
/// so mutating the clone never touches the source.
///
/// # Safety
///
/// `list` must be a live list produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_clone(
    list: *mut c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    let length = unsafe { zirk_rt_list_length(list) };
    let clone = unsafe { zirk_rt_list_new(elem_size, elem_align, is_ref) };
    let old_data = unsafe { data_pointer(list) };
    if length > 0 && !old_data.is_null() {
        unsafe { grow_buffer(clone, length, elem_size, elem_align, is_ref) };
        let new_data = unsafe { data_pointer(clone) };
        unsafe {
            std::ptr::copy_nonoverlapping(
                (old_data as *mut u8).add(HEADER_BYTES),
                (new_data as *mut u8).add(HEADER_BYTES),
                length.saturating_mul(elem_size),
            );
            write_usize(clone, LENGTH_OFFSET, length);
        }
    }
    clone
}

/// `list.remove(index)` — removes the element at `index`.
///
/// # Safety
///
/// `list` must be a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_remove(
    list: *mut c_void,
    index: i64,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) {
    let _ = (elem_align,);
    let length = unsafe { zirk_rt_list_length(list) } as i64;
    let index = if index < 0 { length + index } else { index };
    if index < 0 || index >= length {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    let data = unsafe { data_pointer(list) };
    let index_u = index as usize;
    let length_u = length as usize;
    if index + 1 < length {
        let src = unsafe { element_pointer(data, index_u + 1, elem_size) };
        let dst = unsafe { element_pointer(data, index_u, elem_size) };
        let bytes = (length_u - index_u - 1).saturating_mul(elem_size);
        unsafe { std::ptr::copy(src as *const u8, dst as *mut u8, bytes) };
    }
    if is_ref && !data.is_null() {
        let last = unsafe { element_pointer(data, length_u - 1, elem_size) };
        unsafe { *(last as *mut *mut c_void) = std::ptr::null_mut() };
    }
    unsafe { write_usize(list, LENGTH_OFFSET, length_u - 1) };
}

/// Whether `pointer` refers to a runtime `String`/`Char` object: those are
/// the only objects whose header descriptor is `zirk_rt_string_descriptor`.
///
/// # Safety
///
/// `pointer` must be null or a live object produced by this runtime.
unsafe fn is_string_object(pointer: *const c_void) -> bool {
    if pointer.is_null() {
        return false;
    }
    unsafe { *(pointer as *const *const c_void) == crate::collector::string_descriptor() }
}

/// `list.remove(value)` — removes the first element equal to `value` and
/// returns whether one was found.
///
/// Value-type elements compare by their `elem_size` bytes; reference-type
/// elements compare by pointer, except strings, which compare by contents
/// through [`crate::string::zirk_str_eq`].
///
/// # Safety
///
/// `list` must be a live list and `value` a pointer to an element of
/// `elem_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_remove_value(
    list: *mut c_void,
    value: *const c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> bool {
    let _ = (elem_align,);
    let length = unsafe { zirk_rt_list_length(list) };
    let data = unsafe { data_pointer(list) };
    if length == 0 || data.is_null() {
        return false;
    }
    for index in 0..length {
        let slot = unsafe { element_pointer(data, index, elem_size) };
        let equal = if is_ref {
            let element = unsafe { *(slot as *const *const c_void) };
            let target = unsafe { *(value as *const *const c_void) };
            if unsafe { is_string_object(element) } || unsafe { is_string_object(target) } {
                unsafe { crate::string::zirk_str_eq(element, target) }
            } else {
                element == target
            }
        } else {
            let element = unsafe { std::slice::from_raw_parts(slot as *const u8, elem_size) };
            let target = unsafe { std::slice::from_raw_parts(value as *const u8, elem_size) };
            element == target
        };
        if equal {
            if index + 1 < length {
                let src = unsafe { element_pointer(data, index + 1, elem_size) };
                let bytes = (length - index - 1).saturating_mul(elem_size);
                unsafe { std::ptr::copy(src as *const u8, slot as *mut u8, bytes) };
            }
            if is_ref {
                let last = unsafe { element_pointer(data, length - 1, elem_size) };
                unsafe { *(last as *mut *mut c_void) = std::ptr::null_mut() };
            }
            unsafe { write_usize(list, LENGTH_OFFSET, length - 1) };
            return true;
        }
    }
    false
}
