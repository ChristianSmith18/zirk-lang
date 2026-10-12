//! Runtime helpers for `Set<T>`.

use std::collections::HashSet;
use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

#[repr(C)]
struct SetBody {
    elements: SetElements,
    reference_offsets: Box<[usize]>,
}

enum SetElements {
    Words(HashSet<i64>),
    Text(HashSet<Option<Box<str>>>),
}

impl SetElements {
    fn len(&self) -> usize {
        match self {
            Self::Words(elements) => elements.len(),
            Self::Text(elements) => elements.len(),
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn text_element(word: i64) -> Option<Box<str>> {
    let handle = word as usize as *mut c_void;
    if handle.is_null() {
        return None;
    }
    let text = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
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

unsafe fn reference_offsets(pointer: *const usize, count: usize) -> Box<[usize]> {
    if count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "set reference offsets are null");
    let offsets = unsafe { std::slice::from_raw_parts(pointer, count) };
    for &offset in offsets {
        assert!(
            offset.checked_add(WORD).is_some_and(|end| end <= WORD),
            "set reference offset is outside its word-sized element"
        );
    }
    offsets.to_vec().into_boxed_slice()
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_new() -> *mut c_void {
    unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, false) }
}

/// Creates a set and records managed-reference offsets in its word-sized
/// elements. The metadata is owned by the GC handle and released on sweep.
///
/// # Safety
/// A non-empty offset list must point to readable `usize` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_new_with_layout(
    offsets: *const usize,
    offset_count: usize,
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
    let reference_offsets = unsafe { reference_offsets(offsets, offset_count) };
    unsafe {
        *(object as *mut *mut c_void) = set_descriptor();
        body(object).write(SetBody {
            elements: if string_content {
                SetElements::Text(HashSet::new())
            } else {
                SetElements::Words(HashSet::new())
            },
            reference_offsets,
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
    let mut visit = |element_word: i64| {
        let candidate = element_word as usize as *mut c_void;
        if allocations.contains(&(candidate as usize)) {
            mark(candidate);
        }
        for &offset in &body.reference_offsets {
            let candidate = unsafe {
                ((&element_word as *const i64 as *const u8)
                    .add(offset)
                    .cast::<*mut c_void>())
                .read_unaligned()
            };
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
    };
    if let SetElements::Words(elements) = &body.elements {
        elements.iter().copied().for_each(&mut visit);
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
pub extern "C" fn zirk_rt_set_length(set: *mut c_void) -> usize {
    if set.is_null() {
        return 0;
    }
    unsafe { (&*body(set)).elements.len() }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_add(set: *mut c_void, value: i64) {
    if !set.is_null() {
        let body = unsafe { &mut *body(set) };
        match &mut body.elements {
            SetElements::Words(elements) => {
                elements.insert(value);
            }
            SetElements::Text(elements) => {
                elements.insert(text_element(value));
            }
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_contains(set: *mut c_void, value: i64) -> i32 {
    if set.is_null() {
        return 0;
    }
    match &unsafe { &*body(set) }.elements {
        SetElements::Words(elements) => elements.contains(&value) as i32,
        SetElements::Text(elements) => elements.contains(&text_element(value)) as i32,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_remove(set: *mut c_void, value: i64) -> i32 {
    if set.is_null() {
        return 0;
    }
    match &mut unsafe { &mut *body(set) }.elements {
        SetElements::Words(elements) => elements.remove(&value) as i32,
        SetElements::Text(elements) => elements.remove(&text_element(value)) as i32,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_sorted_elements(
    set: *mut c_void,
    string_content: i64,
) -> *mut c_void {
    if string_content != 0 {
        let mut elements: Vec<Option<Box<str>>> = if set.is_null() {
            Vec::new()
        } else {
            match &unsafe { &*body(set) }.elements {
                SetElements::Text(elements) => elements.iter().cloned().collect(),
                SetElements::Words(elements) => elements.iter().map(|_| None).collect(),
            }
        };
        elements.sort();
        return crate::map::snapshots_array(&elements);
    }
    let mut elements: Vec<i64> = if set.is_null() {
        Vec::new()
    } else {
        match &unsafe { &*body(set) }.elements {
            SetElements::Words(elements) => elements.iter().copied().collect(),
            SetElements::Text(elements) => elements.iter().map(|_| 0).collect(),
        }
    };
    elements.sort_unstable();
    let array = unsafe { crate::array::zirk_rt_array_new(elements.len(), WORD, WORD, false) };
    for (index, value) in elements.iter().enumerate() {
        unsafe { crate::array::write_word(array, index, *value) };
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_operations_preserve_word_value_semantics() {
        let set = zirk_rt_set_new();
        zirk_rt_set_add(set, 7);
        zirk_rt_set_add(set, 7);
        assert_eq!(zirk_rt_set_length(set), 1);
        assert_eq!(zirk_rt_set_contains(set, 7), 1);
        assert_eq!(zirk_rt_set_remove(set, 7), 1);
        assert_eq!(zirk_rt_set_remove(set, 7), 0);
        assert_eq!(zirk_rt_set_is_empty(set), 1);
    }

    #[test]
    fn string_content_elements_use_language_equality_and_hashing() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let set = unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, true) };
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
        let set = unsafe { zirk_rt_set_new_with_layout(std::ptr::null(), 0, true) };
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
        let set = unsafe { zirk_rt_set_new_with_layout(offsets.as_ptr(), 1, false) };
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
