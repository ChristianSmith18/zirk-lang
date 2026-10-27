//! Runtime helpers for `Set<T>`.

use std::ffi::c_void;

use crate::ordered::{OrderedMap, TryKeyClone};

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

#[repr(C)]
struct SetBody {
    elements: SetElements,
    reference_offsets: Box<[usize]>,
    element_size: usize,
}

enum SetElements {
    Words(OrderedMap<Vec<u8>, ()>),
    Text(OrderedMap<Option<Box<str>>, ()>),
}

impl SetElements {
    fn len(&self) -> usize {
        match self {
            Self::Words(elements) => elements.len(),
            Self::Text(elements) => elements.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Words(elements) => elements.is_empty(),
            Self::Text(elements) => elements.is_empty(),
        }
    }
}

fn text_element(bytes: &[u8]) -> Option<Box<str>> {
    let word = if bytes.len() == WORD {
        read_word(bytes)
    } else {
        if bytes.first().copied().unwrap_or(0) == 0 {
            return None;
        }
        read_word(bytes.get(WORD..)?)
    };
    let handle = word as usize as *mut c_void;
    if handle.is_null() {
        return None;
    }
    let text = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
}

fn encode_text_element(size: usize, text: Option<&str>) -> Vec<u8> {
    let handle = text.map_or(std::ptr::null_mut(), |text| unsafe {
        crate::string::zirk_str_from_utf8(text.as_ptr(), text.len())
    });
    if size == WORD {
        return (handle as usize as i64).to_ne_bytes().to_vec();
    }
    let mut bytes = vec![0; size];
    if text.is_some() {
        assert!(
            size >= WORD * 2,
            "nullable text element layout is too small"
        );
        bytes[0] = 1;
        bytes[WORD..WORD * 2].copy_from_slice(&(handle as usize).to_ne_bytes());
    }
    bytes
}

fn read_word(bytes: &[u8]) -> i64 {
    let mut word = 0_i64;
    let count = bytes.len().min(WORD);
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), (&mut word as *mut i64).cast(), count) };
    word
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_set_descriptor: [usize; 4] = [0, 0, 0, 0];

pub(crate) fn set_descriptor() -> *mut c_void {
    (&raw const zirk_rt_set_descriptor) as *mut c_void
}

unsafe fn body(set: *mut c_void) -> *mut SetBody {
    unsafe { (set as *mut u8).add(BODY_OFFSET).cast() }
}

unsafe fn reference_offsets(pointer: *const usize, count: usize, size: usize) -> Box<[usize]> {
    if count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "set reference offsets are null");
    let offsets = unsafe { std::slice::from_raw_parts(pointer, count) };
    for &offset in offsets {
        assert!(
            offset.checked_add(WORD).is_some_and(|end| end <= size),
            "set reference offset is outside its element"
        );
    }
    offsets.to_vec().into_boxed_slice()
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_new() -> *mut c_void {
    unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, WORD, false) }
}

/// Creates a set and records managed-reference offsets in its typed
/// elements. The metadata is owned by the GC handle and released on sweep.
///
/// # Safety
/// A non-empty offset list must point to readable `usize` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_new_with_layout(
    offsets: *const usize,
    offset_count: usize,
    element_size: usize,
    string_content: bool,
) -> *mut c_void {
    assert!(
        !string_content || offset_count == 0,
        "snapshotted text elements cannot carry managed-reference offsets"
    );
    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<SetBody>(),
            std::mem::align_of::<SetBody>(),
        )
    };
    let reference_offsets = unsafe { reference_offsets(offsets, offset_count, element_size) };
    unsafe {
        *(object as *mut *mut c_void) = set_descriptor();
        body(object).write(SetBody {
            elements: if string_content {
                SetElements::Text(OrderedMap::default())
            } else {
                SetElements::Words(OrderedMap::default())
            },
            reference_offsets,
            element_size,
        });
    }
    object
}

pub(crate) unsafe fn mark_entries(
    set: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*body(set) };
    let mut visit = |element: &[u8]| {
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
    };
    match &body.elements {
        SetElements::Words(elements) => elements.iter().for_each(|(value, _)| visit(value)),
        SetElements::Text(_) => {}
    }
}

pub(crate) unsafe fn drop_body(set: *mut c_void) {
    unsafe { std::ptr::drop_in_place(body(set)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_is_empty(set: *mut c_void) -> i32 {
    if set.is_null() {
        return 1;
    }
    unsafe { (&*body(set)).elements.is_empty() as i32 }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_length(set: *mut c_void) -> u64 {
    if set.is_null() {
        return 0;
    }
    unsafe { (&*body(set)).elements.len() as u64 }
}

/// Returns the elements in insertion order as a typed Array snapshot.
/// Text elements are materialized as fresh immutable String handles.
///
/// # Safety
/// The set must be live. The element layout and reference offsets must match
/// its declared `Set<T>` element type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_values(
    set: *mut c_void,
    element_size: usize,
    element_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    if set.is_null() {
        return std::ptr::null_mut();
    }
    let set_body = unsafe { &*body(set) };
    assert_eq!(
        element_size, set_body.element_size,
        "Set.values layout differs from the stored element layout"
    );
    let elements = &set_body.elements;
    let result = unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            elements.len(),
            element_size,
            element_align,
            reference_offsets,
            reference_offset_count,
        )
    };
    match elements {
        SetElements::Words(elements) => {
            for (index, (value, _)) in elements.iter().enumerate() {
                unsafe { crate::array::write_bytes(result, index, element_size, value) };
            }
        }
        SetElements::Text(elements) => {
            for (index, (text, _)) in elements.iter().enumerate() {
                let bytes = encode_text_element(element_size, text.as_deref());
                if element_size == WORD {
                    let word = i64::from_ne_bytes(
                        bytes
                            .as_slice()
                            .try_into()
                            .expect("word-sized text element"),
                    );
                    unsafe { crate::array::write_word_sized(result, index, element_size, word) };
                } else {
                    unsafe { crate::array::write_bytes(result, index, element_size, &bytes) };
                }
            }
        }
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_add(set: *mut c_void, value: i64) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts((&value as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_set_add_typed(set, bytes.as_ptr(), bytes.len()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_add_typed(
    set: *mut c_void,
    value: *const u8,
    size: usize,
) -> i32 {
    assert!(!value.is_null() || size == 0, "set element pointer is null");
    let value = unsafe { std::slice::from_raw_parts(value, size) };
    if !set.is_null() {
        let body = unsafe { &mut *body(set) };
        assert_eq!(
            size, body.element_size,
            "set element size differs from its declared layout"
        );
        let mut value_copy = Vec::new();
        if value_copy.try_reserve_exact(value.len()).is_err() {
            return 2;
        }
        value_copy.extend_from_slice(value);
        let value = value_copy;
        return match &mut body.elements {
            SetElements::Words(elements) => match elements.insert(value, ()) {
                Ok(previous) => previous.is_none() as i32,
                Err(()) => 2,
            },
            SetElements::Text(elements) => match elements.insert(text_element(&value), ()) {
                Ok(previous) => previous.is_none() as i32,
                Err(()) => 2,
            },
        };
    }
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_contains(set: *mut c_void, value: i64) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts((&value as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_set_contains_typed(set, bytes.as_ptr(), bytes.len()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_contains_typed(
    set: *mut c_void,
    value: *const u8,
    size: usize,
) -> i32 {
    if set.is_null() {
        return 0;
    }
    assert!(!value.is_null() || size == 0, "set element pointer is null");
    let value = unsafe { std::slice::from_raw_parts(value, size) };
    assert_eq!(
        size,
        unsafe { (*body(set)).element_size },
        "set element size differs from its declared layout"
    );
    match &unsafe { &*body(set) }.elements {
        SetElements::Words(elements) => elements.get(&value.to_vec()).is_some() as i32,
        SetElements::Text(elements) => elements.get(&text_element(value)).is_some() as i32,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_remove(set: *mut c_void, value: i64) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts((&value as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_set_remove_typed(set, bytes.as_ptr(), bytes.len()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_remove_typed(
    set: *mut c_void,
    value: *const u8,
    size: usize,
) -> i32 {
    if set.is_null() {
        return 0;
    }
    assert!(!value.is_null() || size == 0, "set element pointer is null");
    let value = unsafe { std::slice::from_raw_parts(value, size) };
    assert_eq!(
        size,
        unsafe { (*body(set)).element_size },
        "set element size differs from its declared layout"
    );
    match &mut unsafe { &mut *body(set) }.elements {
        SetElements::Words(elements) => elements.remove(&value.to_vec()).is_some() as i32,
        SetElements::Text(elements) => elements.remove(&text_element(value)).is_some() as i32,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_clear(set: *mut c_void) {
    if !set.is_null() {
        match &mut unsafe { &mut *body(set) }.elements {
            SetElements::Words(elements) => elements.clear(),
            SetElements::Text(elements) => elements.clear(),
        }
    }
}

fn new_like(set: *mut c_void) -> *mut c_void {
    if set.is_null() {
        return zirk_rt_set_new();
    }
    let body = unsafe { &*body(set) };
    match &body.elements {
        SetElements::Words(_) => unsafe {
            zirk_rt_set_new_with_layout(
                body.reference_offsets.as_ptr(),
                body.reference_offsets.len(),
                body.element_size,
                false,
            )
        },
        SetElements::Text(_) => unsafe {
            zirk_rt_set_new_with_layout(std::ptr::null(), 0, WORD, true)
        },
    }
}

/// Applies one deterministic set operation: 0 union, 1 intersection,
/// 2 difference, 3 symmetric difference.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_algebra(
    left: *mut c_void,
    right: *mut c_void,
    operation: i32,
) -> *mut c_void {
    let result = new_like(left);
    if left.is_null() || right.is_null() {
        return result;
    }
    let left_elements = &unsafe { &*body(left) }.elements;
    let right_elements = &unsafe { &*body(right) }.elements;
    let result_elements = &mut unsafe { &mut *body(result) }.elements;
    match (left_elements, right_elements, result_elements) {
        (SetElements::Words(left), SetElements::Words(right), SetElements::Words(result)) => {
            for (value, _) in left.iter() {
                let in_right = right.get(value).is_some();
                if operation == 0
                    || (operation == 1 && in_right)
                    || (operation == 2 && !in_right)
                    || (operation == 3 && !in_right)
                {
                    result
                        .insert(
                            value
                                .try_clone_key()
                                .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed()),
                            (),
                        )
                        .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed());
                }
            }
            if matches!(operation, 0 | 3) {
                for (value, _) in right.iter() {
                    let in_left = left.get(value).is_some();
                    if (operation == 0 || operation == 3) && !in_left {
                        result
                            .insert(
                                value.try_clone_key().unwrap_or_else(|()| {
                                    crate::failure::zirk_rt_allocation_failed()
                                }),
                                (),
                            )
                            .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed());
                    }
                }
            }
        }
        (SetElements::Text(left), SetElements::Text(right), SetElements::Text(result)) => {
            for (value, _) in left.iter() {
                let in_right = right.get(value).is_some();
                if operation == 0
                    || (operation == 1 && in_right)
                    || (operation == 2 && !in_right)
                    || (operation == 3 && !in_right)
                {
                    result
                        .insert(
                            value
                                .try_clone_key()
                                .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed()),
                            (),
                        )
                        .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed());
                }
            }
            if matches!(operation, 0 | 3) {
                for (value, _) in right.iter() {
                    let in_left = left.get(value).is_some();
                    if (operation == 0 || operation == 3) && !in_left {
                        result
                            .insert(
                                value.try_clone_key().unwrap_or_else(|()| {
                                    crate::failure::zirk_rt_allocation_failed()
                                }),
                                (),
                            )
                            .unwrap_or_else(|()| crate::failure::zirk_rt_allocation_failed());
                    }
                }
            }
        }
        _ => crate::failure::fatal("set algebra received incompatible layouts"),
    }
    result
}

/// Compares two sets: 0 subset, 1 superset, 2 disjoint.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_relation(
    left: *mut c_void,
    right: *mut c_void,
    relation: i32,
) -> i32 {
    if left.is_null() || right.is_null() {
        return (relation == 2) as i32;
    }
    let left = &unsafe { &*body(left) }.elements;
    let right = &unsafe { &*body(right) }.elements;
    match (left, right) {
        (SetElements::Words(left), SetElements::Words(right)) => match relation {
            0 => left.iter().all(|(value, _)| right.get(value).is_some()) as i32,
            1 => right.iter().all(|(value, _)| left.get(value).is_some()) as i32,
            2 => left.iter().all(|(value, _)| right.get(value).is_none()) as i32,
            _ => 0,
        },
        (SetElements::Text(left), SetElements::Text(right)) => match relation {
            0 => left.iter().all(|(value, _)| right.get(value).is_some()) as i32,
            1 => right.iter().all(|(value, _)| left.get(value).is_some()) as i32,
            2 => left.iter().all(|(value, _)| right.get(value).is_none()) as i32,
            _ => 0,
        },
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_sorted_elements(
    set: *mut c_void,
    string_content: i64,
) -> *mut c_void {
    unsafe { set_elements_snapshot(set, string_content, true) }
}

/// Returns elements in insertion order for collection iteration. Rendering
/// uses `zirk_rt_set_sorted_elements` to retain its canonical text form.
#[cfg(test)]
pub(crate) unsafe fn set_elements_in_insertion_order(
    set: *mut c_void,
    string_content: i64,
) -> *mut c_void {
    unsafe { set_elements_snapshot(set, string_content, false) }
}

unsafe fn set_elements_snapshot(
    set: *mut c_void,
    string_content: i64,
    sorted: bool,
) -> *mut c_void {
    if string_content != 0 {
        let mut elements: Vec<Option<Box<str>>> = if set.is_null() {
            Vec::new()
        } else {
            match &unsafe { &*body(set) }.elements {
                SetElements::Text(elements) => elements
                    .iter()
                    .map(|(element, _)| element.clone())
                    .collect(),
                SetElements::Words(elements) => elements.iter().map(|_| None).collect(),
            }
        };
        if sorted {
            elements.sort();
        }
        return crate::map::snapshots_array(&elements);
    }
    let (element_size, reference_offsets, mut elements): (usize, Vec<usize>, Vec<Vec<u8>>) =
        if set.is_null() {
            (WORD, Vec::new(), Vec::new())
        } else {
            let set_body = unsafe { &*body(set) };
            let elements = match &set_body.elements {
                SetElements::Words(elements) => {
                    elements.iter().map(|(value, _)| value.clone()).collect()
                }
                SetElements::Text(elements) => elements
                    .iter()
                    .map(|_| vec![0; set_body.element_size])
                    .collect(),
            };
            (
                set_body.element_size,
                set_body.reference_offsets.to_vec(),
                elements,
            )
        };
    if sorted {
        elements.sort_unstable();
    }
    let array = unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            elements.len(),
            element_size,
            element_size.max(1).next_power_of_two(),
            reference_offsets.as_ptr(),
            reference_offsets.len(),
        )
    };
    for (index, value) in elements.iter().enumerate() {
        unsafe { crate::array::write_bytes(array, index, element_size, value) };
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_operations_preserve_word_value_semantics() {
        let set = zirk_rt_set_new();
        assert_eq!(zirk_rt_set_add(set, 7), 1);
        assert_eq!(zirk_rt_set_add(set, 7), 0);
        assert_eq!(zirk_rt_set_length(set), 1);
        assert_eq!(zirk_rt_set_contains(set, 7), 1);
        assert_eq!(zirk_rt_set_remove(set, 7), 1);
        assert_eq!(zirk_rt_set_remove(set, 7), 0);
        assert_eq!(zirk_rt_set_is_empty(set), 1);
    }

    #[test]
    fn insertion_reservation_failure_leaves_set_unchanged() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let set = zirk_rt_set_new();
        assert_eq!(zirk_rt_set_add(set, 7), 1);

        crate::ordered::fail_next_insert_reservation_for_tests();
        assert_eq!(zirk_rt_set_add(set, 9), 2);
        assert_eq!(zirk_rt_set_length(set), 1);
        assert_eq!(zirk_rt_set_contains(set, 7), 1);
        assert_eq!(zirk_rt_set_contains(set, 9), 0);
        assert_eq!(zirk_rt_set_add(set, 7), 0);
    }

    #[test]
    fn element_snapshot_preserves_insertion_order() {
        let set = zirk_rt_set_new();
        zirk_rt_set_add(set, 9);
        zirk_rt_set_add(set, 3);
        zirk_rt_set_add(set, 7);
        let snapshot = unsafe { set_elements_in_insertion_order(set, 0) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|i| {
                    *(crate::array::zirk_rt_array_element(snapshot, i as i64, WORD) as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(words, [9, 3, 7]);
    }

    #[test]
    fn sorted_element_snapshot_uses_natural_order_for_rendering() {
        let set = zirk_rt_set_new();
        zirk_rt_set_add(set, 9);
        zirk_rt_set_add(set, 3);
        zirk_rt_set_add(set, 7);
        let snapshot = unsafe { zirk_rt_set_sorted_elements(set, 0) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|i| {
                    *(crate::array::zirk_rt_array_element(snapshot, i as i64, WORD) as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(words, [3, 7, 9]);
    }

    #[test]
    fn set_algebra_preserves_contract_order_and_does_not_mutate_inputs() {
        let left = zirk_rt_set_new();
        let right = zirk_rt_set_new();
        for value in [3, 1, 4] {
            zirk_rt_set_add(left, value);
        }
        for value in [1, 2, 4] {
            zirk_rt_set_add(right, value);
        }
        let values = |set| unsafe {
            let snapshot = set_elements_in_insertion_order(set, 0);
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|index| {
                    *(crate::array::zirk_rt_array_element(snapshot, index as i64, WORD)
                        as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(values(zirk_rt_set_algebra(left, right, 0)), [3, 1, 4, 2]);
        assert_eq!(values(zirk_rt_set_algebra(left, right, 1)), [1, 4]);
        assert_eq!(values(zirk_rt_set_algebra(left, right, 2)), [3]);
        assert_eq!(values(zirk_rt_set_algebra(left, right, 3)), [3, 2]);
        assert_eq!(zirk_rt_set_relation(left, right, 0), 0);
        assert_eq!(zirk_rt_set_relation(left, right, 1), 0);
        assert_eq!(zirk_rt_set_relation(left, right, 2), 0);
        assert_eq!(values(left), [3, 1, 4]);
        assert_eq!(values(right), [1, 2, 4]);
    }

    #[test]
    fn string_content_elements_use_language_equality_and_hashing() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let set = unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, WORD, true) };
        let first = crate::string::alloc_owned("same-element");
        let second = crate::string::alloc_owned("same-element");
        assert_ne!(first, second);
        zirk_rt_set_add(set, first as i64);
        zirk_rt_set_add(set, second as i64);
        assert_eq!(zirk_rt_set_length(set), 1);
        assert_eq!(zirk_rt_set_contains(set, second as i64), 1);
        assert_eq!(zirk_rt_set_remove(set, first as i64), 1);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn mutable_string_element_keeps_its_inserted_value_and_survives_source_mutation() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let set = unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, WORD, true) };
        let original = crate::string::alloc_owned("before");
        let replacement = crate::string::alloc_owned("after");
        let equivalent = crate::string::alloc_owned("before");
        zirk_rt_set_add(set, original as i64);

        let mut set_root = set;
        let mut original_root = original;
        let mut replacement_root = replacement;
        let mut equivalent_root = equivalent;
        let mut roots = [
            (&mut set_root as *mut *mut c_void).cast(),
            (&mut original_root as *mut *mut c_void).cast(),
            (&mut replacement_root as *mut *mut c_void).cast(),
            (&mut equivalent_root as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        unsafe { crate::string::zirk_str_set(original_root, 0, 6, replacement_root) };

        assert_eq!(zirk_rt_set_contains(set_root, equivalent_root as i64), 1);
        assert_eq!(zirk_rt_set_contains(set_root, original_root as i64), 0);
        let elements = unsafe { zirk_rt_set_sorted_elements(set_root, 1) };
        let element = unsafe {
            *(crate::array::zirk_rt_array_element(elements, 0, WORD) as *const *mut c_void)
        };
        assert_eq!(
            unsafe { (*crate::string::payload_ptr(element)).as_str() },
            "before"
        );
        crate::zirk_rt_pop_frame();

        let mut set_root = set;
        let mut equivalent_root = equivalent;
        let mut roots = [
            (&mut set_root as *mut *mut c_void).cast(),
            (&mut equivalent_root as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(original));
        assert_eq!(zirk_rt_set_contains(set_root, equivalent_root as i64), 1);
        crate::zirk_rt_pop_frame();

        crate::collector::reset_state_for_tests();
        drop(guard);
    }

    #[test]
    fn set_traces_declared_reference_elements() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let offsets = [0usize];
        let set = unsafe { zirk_rt_set_new_with_layout(offsets.as_ptr(), 1, WORD, false) };
        let value = crate::string::alloc_owned("set-root");
        zirk_rt_set_add(set, value as i64);
        let mut handle = set;
        let mut roots = [&mut handle as *mut *mut c_void as *mut c_void];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(value));
        crate::zirk_rt_pop_frame();
        crate::collector::reset_state_for_tests();
        drop(guard);
    }
}
