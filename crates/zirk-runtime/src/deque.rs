//! Runtime storage for typed, collector-managed `Deque<T>` values.

use std::collections::VecDeque;
use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

#[repr(C)]
struct DequeBody {
    elements: VecDeque<Vec<u8>>,
    reference_offsets: Box<[usize]>,
    union_roots: Box<[CollectionUnionRoot]>,
    element_size: usize,
    structural_version: u64,
}

struct CollectionUnionRoot {
    offset: usize,
    descriptor: *const usize,
    present_offset: Option<usize>,
    enum_guards: Box<[(usize, u32)]>,
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn fail_next_reservation_for_tests() {
    FAIL_NEXT_RESERVATION.with(|fail| fail.set(true));
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_deque_descriptor: [usize; 4] = [0, 0, 0, 0];

pub(crate) fn deque_descriptor() -> *mut c_void {
    (&raw const zirk_rt_deque_descriptor) as *mut c_void
}

/// Returns an indexed element pointer for generic ordered traversal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_element(deque: *mut c_void, index: usize) -> *mut c_void {
    if deque.is_null() {
        return std::ptr::null_mut();
    }
    let body = unsafe { &*body(deque) };
    body.elements
        .get(index)
        .map_or(std::ptr::null_mut(), |element| {
            element.as_ptr() as *mut c_void
        })
}

unsafe fn body(deque: *mut c_void) -> *mut DequeBody {
    unsafe { (deque as *mut u8).add(BODY_OFFSET).cast() }
}

/// Creates a collector-managed deque with recursive element-reference offsets.
///
/// # Safety
/// `reference_offsets` must point to `reference_offset_count` offsets within
/// one element, or be null when the count is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_new_with_layout(
    element_size: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    unsafe {
        zirk_rt_deque_new_with_union_layout(
            element_size,
            reference_offsets,
            reference_offset_count,
            std::ptr::null(),
            0,
        )
    }
}

/// Creates an ordered collection with both ordinary reference offsets and
/// tag-aware union-root metadata for each stored element.
///
/// # Safety
/// Both metadata tables must remain valid for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_new_with_union_layout(
    element_size: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
    union_words: *const usize,
    union_word_count: usize,
) -> *mut c_void {
    let offsets: Box<[usize]> = if reference_offset_count == 0 {
        Box::new([])
    } else {
        assert!(
            !reference_offsets.is_null(),
            "deque reference offsets are null"
        );
        let offsets =
            unsafe { std::slice::from_raw_parts(reference_offsets, reference_offset_count) };
        for &offset in offsets {
            assert!(
                offset
                    .checked_add(WORD)
                    .is_some_and(|end| end <= element_size),
                "deque GC offset is outside its element"
            );
        }
        let mut copied = Vec::new();
        if copied.try_reserve_exact(reference_offset_count).is_err() {
            return std::ptr::null_mut();
        }
        copied.extend_from_slice(offsets);
        copied.into_boxed_slice()
    };
    let union_roots =
        unsafe { collection_union_roots(union_words, union_word_count, element_size) };
    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<DequeBody>(),
            std::mem::align_of::<DequeBody>(),
        )
    };
    unsafe {
        *(object as *mut *mut c_void) = deque_descriptor();
        body(object).write(DequeBody {
            elements: VecDeque::new(),
            reference_offsets: offsets,
            union_roots,
            element_size,
            structural_version: 0,
        });
    }
    object
}

unsafe fn collection_union_roots(
    pointer: *const usize,
    word_count: usize,
    value_size: usize,
) -> Box<[CollectionUnionRoot]> {
    if word_count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "deque union-root metadata is null");
    let words = unsafe { std::slice::from_raw_parts(pointer, word_count) };
    let mut cursor = 0;
    let mut roots = Vec::new();
    while cursor < words.len() {
        assert!(
            words.len() - cursor >= 4,
            "truncated deque union-root metadata"
        );
        let offset = words[cursor];
        let descriptor = words[cursor + 1] as *const usize;
        let present = words[cursor + 2];
        let guard_count = words[cursor + 3];
        cursor += 4;
        assert!(!descriptor.is_null(), "deque union-root descriptor is null");
        assert!(
            offset.checked_add(4).is_some_and(|end| end <= value_size),
            "deque union tag is outside its element"
        );
        let present_offset = (present != usize::MAX).then_some(present);
        if let Some(present_offset) = present_offset {
            assert!(
                present_offset < value_size,
                "deque union presence is outside its element"
            );
        }
        assert!(
            guard_count <= 64,
            "deque union-root has too many enum guards"
        );
        assert!(
            guard_count
                .checked_mul(2)
                .is_some_and(|count| count <= words.len() - cursor),
            "truncated deque union-root guards"
        );
        let mut enum_guards = Vec::with_capacity(guard_count);
        for _ in 0..guard_count {
            let tag_offset = words[cursor];
            let expected_tag =
                u32::try_from(words[cursor + 1]).expect("deque enum guard exceeds u32");
            cursor += 2;
            assert!(
                tag_offset
                    .checked_add(4)
                    .is_some_and(|end| end <= value_size),
                "deque enum guard is outside its element"
            );
            enum_guards.push((tag_offset, expected_tag));
        }
        roots.push(CollectionUnionRoot {
            offset,
            descriptor,
            present_offset,
            enum_guards: enum_guards.into_boxed_slice(),
        });
    }
    roots.into_boxed_slice()
}

/// Reports the deque's item references to the collector.
///
/// # Safety
/// `deque` must be a live object created by this module.
pub(crate) unsafe fn mark_entries(
    deque: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*body(deque) };
    for element in &body.elements {
        for &offset in &body.reference_offsets {
            let candidate = unsafe {
                element
                    .as_ptr()
                    .add(offset)
                    .cast::<*mut c_void>()
                    .read_unaligned()
            };
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
        mark_collection_union_roots(element, &body.union_roots, mark);
    }
}

fn mark_collection_union_roots(
    bytes: &[u8],
    roots: &[CollectionUnionRoot],
    mark: &mut dyn FnMut(*mut c_void),
) {
    for root in roots {
        if !root.enum_guards.iter().all(|(offset, expected)| {
            let actual = unsafe { bytes.as_ptr().add(*offset).cast::<u32>().read_unaligned() };
            actual == *expected
        }) {
            continue;
        }
        let union_root = crate::collector::UnionRoot {
            base: unsafe { bytes.as_ptr().add(root.offset) },
            descriptor: root.descriptor,
            present: root
                .present_offset
                .map_or(std::ptr::null(), |offset| unsafe {
                    bytes.as_ptr().add(offset)
                }),
        };
        crate::collector::mark_union_root(&union_root, mark);
    }
}

/// Drops host allocations owned by a deque before the collector frees it.
pub(crate) unsafe fn drop_body(deque: *mut c_void) {
    unsafe { std::ptr::drop_in_place(body(deque)) };
}

unsafe fn reserve_and_copy(body: &mut DequeBody, element: *const c_void) -> Result<Vec<u8>, ()> {
    if body.element_size != 0 && element.is_null() {
        return Err(());
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(body.element_size).map_err(|_| ())?;
    if body.element_size != 0 {
        bytes.extend_from_slice(unsafe {
            std::slice::from_raw_parts(element.cast::<u8>(), body.element_size)
        });
    }
    Ok(bytes)
}

fn try_reserve(elements: &mut VecDeque<Vec<u8>>) -> Result<(), ()> {
    #[cfg(test)]
    if FAIL_NEXT_RESERVATION.with(std::cell::Cell::get) {
        FAIL_NEXT_RESERVATION.with(|fail| fail.set(false));
        return Err(());
    }
    elements.try_reserve(1).map_err(|_| ())
}

unsafe fn push(deque: *mut c_void, element: *const c_void, front: bool) -> i32 {
    if deque.is_null() {
        return 3;
    }
    let body = unsafe { &mut *body(deque) };
    if try_reserve(&mut body.elements).is_err() {
        return 2;
    }
    let Ok(bytes) = (unsafe { reserve_and_copy(body, element) }) else {
        return 2;
    };
    if front {
        body.elements.push_front(bytes);
    } else {
        body.elements.push_back(bytes);
    }
    body.structural_version = body.structural_version.wrapping_add(1);
    0
}

/// Pushes an element at the front: 0 succeeds, 2 reports allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_push_front(
    deque: *mut c_void,
    element: *const c_void,
) -> i32 {
    unsafe { push(deque, element, true) }
}

/// Pushes an element at the back: 0 succeeds, 2 reports allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_push_back(
    deque: *mut c_void,
    element: *const c_void,
) -> i32 {
    unsafe { push(deque, element, false) }
}

unsafe fn read_end(deque: *mut c_void, output: *mut c_void, front: bool, remove: bool) -> i32 {
    if deque.is_null() {
        return 0;
    }
    let body = unsafe { &mut *body(deque) };
    let Some(element) = (if front {
        body.elements.front()
    } else {
        body.elements.back()
    }) else {
        return 0;
    };
    if body.element_size != 0 && output.is_null() {
        return 2;
    }
    if body.element_size != 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(element.as_ptr(), output.cast::<u8>(), body.element_size)
        };
    }
    if remove {
        if front {
            body.elements.pop_front();
        } else {
            body.elements.pop_back();
        }
        body.structural_version = body.structural_version.wrapping_add(1);
    }
    1
}

/// Copies and removes the front element; returns 1 if present, 0 if empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_pop_front(deque: *mut c_void, output: *mut c_void) -> i32 {
    unsafe { read_end(deque, output, true, true) }
}

/// Copies and removes the back element; returns 1 if present, 0 if empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_pop_back(deque: *mut c_void, output: *mut c_void) -> i32 {
    unsafe { read_end(deque, output, false, true) }
}

/// Copies the front element without mutation; returns 1 if present, 0 if empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_front(deque: *mut c_void, output: *mut c_void) -> i32 {
    unsafe { read_end(deque, output, true, false) }
}

/// Copies the back element without mutation; returns 1 if present, 0 if empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_deque_back(deque: *mut c_void, output: *mut c_void) -> i32 {
    unsafe { read_end(deque, output, false, false) }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_deque_length(deque: *mut c_void) -> u64 {
    if deque.is_null() {
        return 0;
    }
    unsafe { (&*body(deque)).elements.len() as u64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_deque_is_empty(deque: *mut c_void) -> i32 {
    (zirk_rt_deque_length(deque) == 0) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_deque_version(deque: *mut c_void) -> u64 {
    if deque.is_null() {
        return 0;
    }
    unsafe { (&*body(deque)).structural_version }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_deque_clear(deque: *mut c_void) {
    if deque.is_null() {
        return;
    }
    let body = unsafe { &mut *body(deque) };
    if !body.elements.is_empty() {
        body.elements.clear();
        body.structural_version = body.structural_version.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deque_preserves_both_end_order_and_empty_outcomes() {
        unsafe {
            let deque = zirk_rt_deque_new_with_layout(4, std::ptr::null(), 0);
            let a = 10_i32;
            let b = 20_i32;
            let c = 30_i32;
            assert_eq!(zirk_rt_deque_push_back(deque, (&a as *const i32).cast()), 0);
            assert_eq!(zirk_rt_deque_push_back(deque, (&b as *const i32).cast()), 0);
            assert_eq!(
                zirk_rt_deque_push_front(deque, (&c as *const i32).cast()),
                0
            );
            let mut output = 0_i32;
            assert_eq!(
                zirk_rt_deque_front(deque, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 30);
            assert_eq!(
                zirk_rt_deque_pop_back(deque, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 20);
            assert_eq!(
                zirk_rt_deque_pop_front(deque, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 30);
            assert_eq!(
                zirk_rt_deque_pop_front(deque, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 10);
            assert_eq!(
                zirk_rt_deque_pop_back(deque, (&mut output as *mut i32).cast()),
                0
            );
            assert_eq!(zirk_rt_deque_length(deque), 0);
        }
    }

    #[test]
    fn failed_deque_growth_is_atomic_and_does_not_invalidate() {
        unsafe {
            let deque = zirk_rt_deque_new_with_layout(4, std::ptr::null(), 0);
            let first = 10_i32;
            let second = 20_i32;
            assert_eq!(
                zirk_rt_deque_push_back(deque, (&first as *const i32).cast()),
                0
            );
            let version = zirk_rt_deque_version(deque);
            fail_next_reservation_for_tests();
            assert_eq!(
                zirk_rt_deque_push_front(deque, (&second as *const i32).cast()),
                2
            );
            assert_eq!(zirk_rt_deque_length(deque), 1);
            assert_eq!(zirk_rt_deque_version(deque), version);
            let mut output = 0_i32;
            assert_eq!(
                zirk_rt_deque_front(deque, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 10);
        }
    }
}
