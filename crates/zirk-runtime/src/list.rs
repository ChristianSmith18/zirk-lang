//! Runtime helpers for `List<T>`.

use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const HEADER_BYTES: usize = crate::collector::HEADER_BYTES;
const LENGTH_OFFSET: usize = HEADER_BYTES;
const CAPACITY_OFFSET: usize = HEADER_BYTES + WORD;
const DATA_PTR_OFFSET: usize = HEADER_BYTES + 2 * WORD;
const LAYOUT_PTR_OFFSET: usize = HEADER_BYTES + 3 * WORD;
const LAYOUT_COUNT_OFFSET: usize = HEADER_BYTES + 4 * WORD;
const ELEMENT_SIZE_OFFSET: usize = HEADER_BYTES + 5 * WORD;
const UNION_LAYOUT_PTR_OFFSET: usize = HEADER_BYTES + 6 * WORD;
const UNION_LAYOUT_COUNT_OFFSET: usize = HEADER_BYTES + 7 * WORD;

struct CollectionUnionRoot {
    offset: usize,
    descriptor: *const usize,
    present_offset: Option<usize>,
    enum_guards: Box<[(usize, u32)]>,
}

/// Descriptor for a `List<T>` header object: it has a single GC field, the
/// pointer to the data buffer.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_list_descriptor: [usize; 5] = [0, 0, 0, 1, DATA_PTR_OFFSET];

#[inline]
pub(crate) fn list_descriptor() -> *mut c_void {
    (&raw const zirk_rt_list_descriptor) as *mut c_void
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
    let offsets = if is_ref { &[0usize][..] } else { &[] };
    unsafe { zirk_rt_list_new_with_layout(elem_size, elem_align, offsets.as_ptr(), offsets.len()) }
}

/// Creates a list retaining the complete set of GC reference offsets for its
/// elements. Subsequent resizes reuse this metadata.
///
/// # Safety
///
/// `reference_offsets` must be null when `reference_offset_count` is zero, or
/// point to that many valid offsets within an element of `elem_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_new_with_layout(
    elem_size: usize,
    elem_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    unsafe {
        zirk_rt_list_new_with_union_layout(
            elem_size,
            elem_align,
            reference_offsets,
            reference_offset_count,
            std::ptr::null(),
            0,
        )
    }
}

/// Creates a list with per-element tag-aware union tracing metadata.
///
/// # Safety
/// Both metadata tables must remain valid for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_new_with_union_layout(
    elem_size: usize,
    elem_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
    union_words: *const usize,
    union_word_count: usize,
) -> *mut c_void {
    let size = HEADER_BYTES + 8 * WORD;
    let object = unsafe { crate::memory::zirk_rt_alloc(size, 1) };
    let offsets = if reference_offset_count == 0 {
        Vec::new()
    } else {
        assert!(
            !reference_offsets.is_null(),
            "list reference offsets are null"
        );
        unsafe { std::slice::from_raw_parts(reference_offsets, reference_offset_count) }.to_vec()
    };
    for &offset in &offsets {
        assert!(
            offset.checked_add(WORD).is_some_and(|end| end <= elem_size),
            "list GC offset is outside its element"
        );
    }
    let (offset_ptr, offset_count) = if offsets.is_empty() {
        (std::ptr::null_mut(), 0)
    } else {
        let boxed = offsets.into_boxed_slice();
        let count = boxed.len();
        (Box::into_raw(boxed) as *mut usize, count)
    };
    let union_table = if union_word_count == 0 {
        std::ptr::null_mut()
    } else {
        assert!(!union_words.is_null(), "list union-root metadata is null");
        let words = unsafe { std::slice::from_raw_parts(union_words, union_word_count) };
        let boxed = words.to_vec().into_boxed_slice();
        Box::into_raw(boxed) as *mut usize
    };
    unsafe {
        set_descriptor(object, &raw const zirk_rt_list_descriptor as *mut c_void);
        write_usize(object, LENGTH_OFFSET, 0);
        write_usize(object, CAPACITY_OFFSET, 0);
        set_ptr(object, DATA_PTR_OFFSET, std::ptr::null_mut());
        set_ptr(object, LAYOUT_PTR_OFFSET, offset_ptr.cast());
        write_usize(object, LAYOUT_COUNT_OFFSET, offset_count);
        write_usize(object, ELEMENT_SIZE_OFFSET, elem_size);
        set_ptr(object, UNION_LAYOUT_PTR_OFFSET, union_table.cast());
        write_usize(object, UNION_LAYOUT_COUNT_OFFSET, union_word_count);
    }
    let _ = elem_align;
    object
}

pub(crate) unsafe fn drop_body(list: *mut c_void) {
    let pointer = unsafe { read_ptr(list, LAYOUT_PTR_OFFSET) } as *mut usize;
    let count = unsafe { read_usize(list, LAYOUT_COUNT_OFFSET) };
    if !pointer.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                pointer, count,
            )))
        };
    }
    let union_pointer = unsafe { read_ptr(list, UNION_LAYOUT_PTR_OFFSET) } as *mut usize;
    let union_count = unsafe { read_usize(list, UNION_LAYOUT_COUNT_OFFSET) };
    if !union_pointer.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                union_pointer,
                union_count,
            )))
        };
    }
}

/// Marks the list's backing allocation and tag-aware references held in its
/// active elements. Ordinary references in the backing buffer are still
/// traced by that buffer's own descriptor.
pub(crate) unsafe fn mark_union_entries(
    list: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let data = unsafe { data_pointer(list) };
    if data.is_null() {
        return;
    }
    mark(data);
    if !allocations.contains(&(data as usize)) {
        return;
    }
    let element_size = unsafe { read_usize(list, ELEMENT_SIZE_OFFSET) };
    let length = unsafe { zirk_rt_list_length(list) };
    let count = unsafe { read_usize(list, UNION_LAYOUT_COUNT_OFFSET) };
    if count == 0 {
        return;
    }
    let words = unsafe {
        std::slice::from_raw_parts(
            read_ptr(list, UNION_LAYOUT_PTR_OFFSET).cast::<usize>(),
            count,
        )
    };
    let mut roots = Vec::new();
    let mut cursor = 0;
    while cursor < words.len() {
        assert!(
            words.len() - cursor >= 4,
            "truncated list union-root metadata"
        );
        let offset = words[cursor];
        let descriptor = words[cursor + 1] as *const usize;
        let present = words[cursor + 2];
        let guard_count = words[cursor + 3];
        cursor += 4;
        assert!(!descriptor.is_null(), "list union-root descriptor is null");
        assert!(
            guard_count <= 64,
            "list union-root has too many enum guards"
        );
        assert!(
            guard_count
                .checked_mul(2)
                .is_some_and(|n| n <= words.len() - cursor),
            "truncated list enum guards"
        );
        let mut guards = Vec::with_capacity(guard_count);
        for _ in 0..guard_count {
            let tag_offset = words[cursor];
            let tag = u32::try_from(words[cursor + 1]).expect("list enum guard exceeds u32");
            cursor += 2;
            guards.push((tag_offset, tag));
        }
        roots.push(CollectionUnionRoot {
            offset,
            descriptor,
            present_offset: (present != usize::MAX).then_some(present),
            enum_guards: guards.into_boxed_slice(),
        });
    }
    let data_bytes = unsafe { (data as *const u8).add(HEADER_BYTES) };
    for index in 0..length {
        let element = unsafe {
            std::slice::from_raw_parts(data_bytes.add(index * element_size), element_size)
        };
        for root in &roots {
            if !root.enum_guards.iter().all(|(offset, expected)| {
                let actual =
                    unsafe { element.as_ptr().add(*offset).cast::<u32>().read_unaligned() };
                actual == *expected
            }) {
                continue;
            }
            let union_root = crate::collector::UnionRoot {
                base: unsafe { element.as_ptr().add(root.offset) },
                descriptor: root.descriptor,
                present: root
                    .present_offset
                    .map_or(std::ptr::null(), |offset| unsafe {
                        element.as_ptr().add(offset)
                    }),
            };
            crate::collector::mark_union_root(&union_root, mark);
        }
    }
}

/// Returns the length of a `List<T>`.
///
/// # Safety
///
/// `list` must be a live list produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_list_length(list: *mut c_void) -> usize {
    if !list.is_null() && unsafe { read_ptr(list, 0) } == crate::deque::deque_descriptor() {
        return crate::deque::zirk_rt_deque_length(list) as usize;
    }
    if !list.is_null()
        && unsafe { read_ptr(list, 0) } == crate::priority_queue::priority_queue_descriptor()
    {
        return crate::priority_queue::zirk_rt_priority_queue_length(list) as usize;
    }
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

unsafe fn make_descriptor(
    base: usize,
    stride: usize,
    count: usize,
    reference_offsets: &[usize],
) -> *mut c_void {
    use std::alloc::{Layout, alloc};
    let entry_count = count
        .checked_mul(reference_offsets.len())
        .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
    let words = 4usize
        .checked_add(entry_count)
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
        pointer.add(3).write(entry_count);
        let mut entry = 0;
        for i in 0..count {
            for &offset in reference_offsets {
                let field = base
                    .checked_add(i.saturating_mul(stride))
                    .and_then(|value| value.checked_add(offset))
                    .unwrap_or_else(|| crate::failure::zirk_rt_allocation_failed());
                pointer.add(4 + entry).write(field);
                entry += 1;
            }
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

    let stored_count = unsafe { read_usize(list, LAYOUT_COUNT_OFFSET) };
    let stored_offsets = if stored_count == 0 {
        None
    } else {
        Some(unsafe {
            std::slice::from_raw_parts(
                read_ptr(list, LAYOUT_PTR_OFFSET) as *const usize,
                stored_count,
            )
        })
    };
    let direct_offsets = [0usize];
    let offsets = stored_offsets.unwrap_or(if is_ref { &direct_offsets } else { &[] });
    let descriptor = if !offsets.is_empty() && new_capacity > 0 {
        unsafe { make_descriptor(HEADER_BYTES, elem_size, new_capacity, offsets) }
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

/// Releases excess backing storage after a list has become substantially
/// smaller. The threshold avoids resize thrashing around a single boundary:
/// a list grows by powers of two and only contracts once it is at most one
/// quarter full.
unsafe fn shrink_buffer(list: *mut c_void, elem_size: usize, elem_align: usize, is_ref: bool) {
    let length = unsafe { zirk_rt_list_length(list) };
    let current = unsafe { list_capacity(list) };
    if current <= 1 || length.saturating_mul(4) > current {
        return;
    }
    let new_capacity = length.max(1).max(current / 2);
    if new_capacity >= current {
        return;
    }
    let size = HEADER_BYTES.saturating_add(new_capacity.saturating_mul(elem_size));
    let align = crate::collector::allocation_align(elem_align);
    let new_data = unsafe { crate::memory::zirk_rt_alloc(size, align) };
    let stored_count = unsafe { read_usize(list, LAYOUT_COUNT_OFFSET) };
    let stored_offsets = if stored_count == 0 {
        None
    } else {
        Some(unsafe {
            std::slice::from_raw_parts(
                read_ptr(list, LAYOUT_PTR_OFFSET) as *const usize,
                stored_count,
            )
        })
    };
    let direct_offsets = [0usize];
    let offsets = stored_offsets.unwrap_or(if is_ref { &direct_offsets } else { &[] });
    let descriptor = if !offsets.is_empty() {
        unsafe { make_descriptor(HEADER_BYTES, elem_size, new_capacity, offsets) }
    } else {
        &raw const crate::array::zirk_rt_array_descriptor as *mut c_void
    };
    unsafe { set_descriptor(new_data, descriptor) };
    let old_data = unsafe { data_pointer(list) };
    if length > 0 && !old_data.is_null() {
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
    let length = unsafe { zirk_rt_list_length(list) };
    let Some(index) = zirk_runtime_core::collections::element_index(length, index) else {
        crate::failure::zirk_rt_index_out_of_bounds();
    };
    if !list.is_null() && unsafe { read_ptr(list, 0) } == crate::deque::deque_descriptor() {
        let element = unsafe { crate::deque::zirk_rt_deque_element(list, index) };
        if element.is_null() {
            crate::failure::zirk_rt_index_out_of_bounds();
        }
        return element;
    }
    if !list.is_null()
        && unsafe { read_ptr(list, 0) } == crate::priority_queue::priority_queue_descriptor()
    {
        let element = unsafe { crate::priority_queue::zirk_rt_priority_queue_element(list, index) };
        if element.is_null() {
            crate::failure::zirk_rt_index_out_of_bounds();
        }
        return element;
    }
    let data = unsafe { data_pointer(list) };
    if data.is_null() {
        crate::failure::zirk_rt_index_out_of_bounds();
    }
    unsafe { element_pointer(data, index, elem_size) }
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
    let length_u = unsafe { zirk_rt_list_length(list) };
    let Some(index_u) = zirk_runtime_core::collections::insertion_index(length_u, index) else {
        crate::failure::zirk_rt_index_out_of_bounds();
    };
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
    let stored_count = unsafe { read_usize(list, LAYOUT_COUNT_OFFSET) };
    let stored_ptr = unsafe { read_ptr(list, LAYOUT_PTR_OFFSET) };
    let clone = unsafe {
        zirk_rt_list_new_with_layout(elem_size, elem_align, stored_ptr.cast(), stored_count)
    };
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

/// Converts a temporary list into a fixed-size array. This is used by eager
/// sequence transforms whose public result is `Array<T>`: the transform can
/// grow while it evaluates, and only the final length is fixed here.
/// Copies a list's elements into a newly allocated fixed array.
///
/// # Safety
/// `list` must be a valid runtime list whose elements use the supplied ABI
/// size and alignment. The caller must provide the matching managed-reference
/// flag for the element type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_array_from_list(
    list: *mut c_void,
    elem_size: usize,
    elem_align: usize,
    is_ref: bool,
) -> *mut c_void {
    let stored_count = unsafe { read_usize(list, LAYOUT_COUNT_OFFSET) };
    let stored_pointer = unsafe { read_ptr(list, LAYOUT_PTR_OFFSET) } as *const usize;
    let legacy_offsets = if is_ref { &[0usize][..] } else { &[] };
    let offsets = if stored_count > 0 {
        unsafe { std::slice::from_raw_parts(stored_pointer, stored_count) }
    } else {
        legacy_offsets
    };
    unsafe {
        zirk_rt_array_from_list_with_layout(
            list,
            elem_size,
            elem_align,
            offsets.as_ptr(),
            offsets.len(),
        )
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `list` must be a live runtime list; `offsets` must be readable and valid
/// for the supplied element size when `offset_count` is nonzero.
pub unsafe extern "C" fn zirk_rt_array_from_list_with_layout(
    list: *mut c_void,
    elem_size: usize,
    elem_align: usize,
    offsets: *const usize,
    offset_count: usize,
) -> *mut c_void {
    let length = unsafe { zirk_rt_list_length(list) };
    let array = unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            length,
            elem_size,
            elem_align,
            offsets,
            offset_count,
        )
    };
    for index in 0..length {
        let source = unsafe { zirk_rt_list_element(list, index as i64, elem_size) };
        let target = unsafe { crate::array::zirk_rt_array_element(array, index as i64, elem_size) };
        unsafe { std::ptr::copy_nonoverlapping(source as *const u8, target as *mut u8, elem_size) };
    }
    array
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
    let length_u = unsafe { zirk_rt_list_length(list) };
    let Some(index_u) = zirk_runtime_core::collections::element_index(length_u, index) else {
        crate::failure::zirk_rt_index_out_of_bounds();
    };
    let data = unsafe { data_pointer(list) };
    if index_u + 1 < length_u {
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
    unsafe { shrink_buffer(list, elem_size, elem_align, is_ref) };
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
            unsafe { shrink_buffer(list, elem_size, elem_align, is_ref) };
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_reference_offsets_survive_list_growth_clone_and_materialization() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let offsets = [WORD];
        let elem_size = 2 * WORD;
        let list = unsafe {
            zirk_rt_list_new_with_layout(elem_size, WORD, offsets.as_ptr(), offsets.len())
        };
        let child = crate::string::alloc_owned("list-nested-edge");
        for index in 0..5usize {
            let value = [index, child as usize];
            unsafe { zirk_rt_list_add(list, value.as_ptr().cast(), elem_size, WORD, false) };
        }
        let clone = unsafe { zirk_rt_list_clone(list, elem_size, WORD, false) };
        let array = unsafe { zirk_rt_array_from_list(list, elem_size, WORD, false) };
        let mut list_root = list;
        let mut clone_root = clone;
        let mut array_root = array;
        let mut roots = [
            &mut list_root as *mut *mut c_void as *mut c_void,
            &mut clone_root as *mut *mut c_void as *mut c_void,
            &mut array_root as *mut *mut c_void as *mut c_void,
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(child));
        for index in 0..5 {
            for container in [list, clone] {
                let slot = unsafe { zirk_rt_list_element(container, index, elem_size) };
                assert_eq!(
                    unsafe { (slot as *mut usize).add(1).read() },
                    child as usize
                );
            }
            let slot = unsafe { crate::array::zirk_rt_array_element(array, index, elem_size) };
            assert_eq!(
                unsafe { (slot as *mut usize).add(1).read() },
                child as usize
            );
        }
        crate::zirk_rt_pop_frame();
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn shrinking_after_removals_keeps_capacity_dynamic() {
        unsafe {
            let list = zirk_rt_list_new(
                std::mem::size_of::<i64>(),
                std::mem::align_of::<i64>(),
                false,
            );
            for value in 0_i64..8 {
                zirk_rt_list_add(
                    list,
                    &raw const value as *const c_void,
                    std::mem::size_of::<i64>(),
                    std::mem::align_of::<i64>(),
                    false,
                );
            }
            assert_eq!(list_capacity(list), 8);
            for _ in 0..6 {
                zirk_rt_list_remove(
                    list,
                    -1,
                    std::mem::size_of::<i64>(),
                    std::mem::align_of::<i64>(),
                    false,
                );
            }
            assert_eq!(zirk_rt_list_length(list), 2);
            assert_eq!(list_capacity(list), 4);
        }
    }
}
