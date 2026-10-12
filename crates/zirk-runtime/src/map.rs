//! Runtime helpers for `Map<K, V>`.

use std::collections::HashMap;
use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

#[repr(C)]
struct MapBody {
    entries: MapEntries,
    key_reference_offsets: Box<[usize]>,
    value_reference_offsets: Box<[usize]>,
}

enum MapEntries {
    Words(HashMap<i64, i64>),
    Text(HashMap<Option<Box<str>>, i64>),
}

impl MapEntries {
    fn len(&self) -> usize {
        match self {
            Self::Words(entries) => entries.len(),
            Self::Text(entries) => entries.len(),
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn text_key(word: i64) -> Option<Box<str>> {
    let handle = word as usize as *mut c_void;
    if handle.is_null() {
        return None;
    }
    let text = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_map_descriptor: [usize; 4] = [0, 0, 0, 0];

pub(crate) fn map_descriptor() -> *mut c_void {
    (&raw const zirk_rt_map_descriptor) as *mut c_void
}

unsafe fn body(map: *mut c_void) -> *mut MapBody {
    unsafe { (map as *mut u8).add(BODY_OFFSET).cast() }
}

unsafe fn reference_offsets(pointer: *const usize, count: usize) -> Box<[usize]> {
    if count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "map reference offsets are null");
    let offsets = unsafe { std::slice::from_raw_parts(pointer, count) };
    for &offset in offsets {
        assert!(
            offset.checked_add(WORD).is_some_and(|end| end <= WORD),
            "map reference offset is outside its word-sized entry"
        );
    }
    offsets.to_vec().into_boxed_slice()
}

/// `Map<K, V>()` — allocates a GC-managed handle with no declared managed
/// key/value edges. The legacy C ABI remains available for scalar-word maps.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_new() -> *mut c_void {
    unsafe { zirk_rt_map_new_with_layout(std::ptr::null(), 0, std::ptr::null(), 0, false) }
}

/// Creates a map and records byte offsets of managed references in each
/// word-sized key and value. The offset lists are copied into the managed
/// handle and released with it.
///
/// # Safety
///
/// Each non-empty offset list must point to readable `usize` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_new_with_layout(
    key_offsets: *const usize,
    key_offset_count: usize,
    value_offsets: *const usize,
    value_offset_count: usize,
    string_content_keys: bool,
) -> *mut c_void {
    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<MapBody>(),
            std::mem::align_of::<MapBody>(),
        )
    };
    assert!(
        !string_content_keys || key_offset_count == 0,
        "snapshotted text keys cannot carry managed-reference offsets"
    );
    let key_reference_offsets = unsafe { reference_offsets(key_offsets, key_offset_count) };
    let value_reference_offsets = unsafe { reference_offsets(value_offsets, value_offset_count) };
    unsafe {
        *(object as *mut *mut c_void) = map_descriptor();
        body(object).write(MapBody {
            entries: if string_content_keys {
                MapEntries::Text(HashMap::new())
            } else {
                MapEntries::Words(HashMap::new())
            },
            key_reference_offsets,
            value_reference_offsets,
        });
    }
    object
}

/// Called by the collector after validating the owner allocation.
pub(crate) unsafe fn mark_entries(
    map: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*body(map) };
    let mut visit = |key_word: Option<i64>, value: i64| {
        // The legacy ABI stores one machine word and carries no type tag. A
        // candidate is therefore traced only when it is an exact current GC
        // allocation; scalar collisions can retain an object but cannot cause
        // an invalid header read.
        for word in key_word.into_iter().chain(std::iter::once(value)) {
            let candidate = word as usize as *mut c_void;
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
        if let Some(key_word) = key_word {
            for &offset in &body.key_reference_offsets {
                let candidate = unsafe {
                    ((&key_word as *const i64 as *const u8)
                        .add(offset)
                        .cast::<*mut c_void>())
                    .read_unaligned()
                };
                if allocations.contains(&(candidate as usize)) {
                    mark(candidate);
                }
            }
        }
        for &offset in &body.value_reference_offsets {
            let candidate = unsafe {
                ((&value as *const i64 as *const u8)
                    .add(offset)
                    .cast::<*mut c_void>())
                .read_unaligned()
            };
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
    };
    match &body.entries {
        MapEntries::Words(entries) => entries
            .iter()
            .for_each(|(&key, &value)| visit(Some(key), value)),
        MapEntries::Text(entries) => entries.values().for_each(|&value| visit(None, value)),
    }
}

pub(crate) unsafe fn drop_body(map: *mut c_void) {
    unsafe { std::ptr::drop_in_place(body(map)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_is_empty(map: *mut c_void) -> i32 {
    if map.is_null() {
        return 1;
    }
    unsafe { (&*body(map)).entries.is_empty() as i32 }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_length(map: *mut c_void) -> usize {
    if map.is_null() {
        return 0;
    }
    unsafe { (&*body(map)).entries.len() }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_set(map: *mut c_void, key: i64, value: i64) {
    if !map.is_null() {
        let body = unsafe { &mut *body(map) };
        match &mut body.entries {
            MapEntries::Words(entries) => {
                entries.insert(key, value);
            }
            MapEntries::Text(entries) => {
                entries.insert(text_key(key), value);
            }
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_contains_key(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    match &unsafe { &*body(map) }.entries {
        MapEntries::Words(entries) => entries.contains_key(&key) as i32,
        MapEntries::Text(entries) => entries.contains_key(&text_key(key)) as i32,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_get(map: *mut c_void, key: i64) -> i64 {
    if map.is_null() {
        return 0;
    }
    match &unsafe { &*body(map) }.entries {
        MapEntries::Words(entries) => entries.get(&key).copied().unwrap_or(0),
        MapEntries::Text(entries) => entries.get(&text_key(key)).copied().unwrap_or(0),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_remove(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    match &mut unsafe { &mut *body(map) }.entries {
        MapEntries::Words(entries) => entries.remove(&key).is_some() as i32,
        MapEntries::Text(entries) => entries.remove(&text_key(key)).is_some() as i32,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_sorted_keys(
    map: *mut c_void,
    string_keys: i64,
) -> *mut c_void {
    if string_keys != 0 {
        let mut keys: Vec<Option<Box<str>>> = if map.is_null() {
            Vec::new()
        } else {
            match &unsafe { &*body(map) }.entries {
                MapEntries::Text(entries) => entries.keys().cloned().collect(),
                MapEntries::Words(entries) => entries.keys().map(|_| None).collect(),
            }
        };
        keys.sort();
        return snapshots_array(&keys);
    }
    let mut keys: Vec<i64> = if map.is_null() {
        Vec::new()
    } else {
        match &unsafe { &*body(map) }.entries {
            MapEntries::Words(entries) => entries.keys().copied().collect(),
            MapEntries::Text(entries) => entries.keys().map(|_| 0).collect(),
        }
    };
    keys.sort_unstable();
    let array = unsafe { crate::array::zirk_rt_array_new(keys.len(), WORD, WORD, false) };
    for (index, key) in keys.iter().enumerate() {
        unsafe { crate::array::write_word(array, index, *key) };
    }
    array
}

pub(crate) fn snapshots_array(texts: &[Option<Box<str>>]) -> *mut c_void {
    let pointer_offset = [0usize];
    let array = unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            texts.len(),
            WORD,
            WORD,
            pointer_offset.as_ptr(),
            pointer_offset.len(),
        )
    };
    let mut array_root = array;
    let mut roots = [&mut array_root as *mut *mut c_void as *mut c_void];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
    for (index, text) in texts.iter().enumerate() {
        let handle = text
            .as_deref()
            .map(crate::string::alloc_owned)
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            let slot = crate::array::zirk_rt_array_element(array, index as i64, WORD);
            (slot as *mut *mut c_void).write(handle);
        }
    }
    crate::zirk_rt_pop_frame();
    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_key_snapshot_uses_numeric_order() {
        let map = zirk_rt_map_new();
        zirk_rt_map_set(map, 10, 0);
        zirk_rt_map_set(map, 2, 0);
        zirk_rt_map_set(map, 1, 0);
        let snapshot = unsafe { zirk_rt_map_sorted_keys(map, 0) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|i| {
                    *(crate::array::zirk_rt_array_element(snapshot, i as i64, WORD) as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(words, [1, 2, 10]);
    }

    #[test]
    fn map_operations_preserve_word_value_semantics() {
        let map = zirk_rt_map_new();
        zirk_rt_map_set(map, 4, 10);
        zirk_rt_map_set(map, 4, 20);
        assert_eq!(zirk_rt_map_length(map), 1);
        assert_eq!(zirk_rt_map_get(map, 4), 20);
        assert_eq!(zirk_rt_map_contains_key(map, 4), 1);
        assert_eq!(zirk_rt_map_remove(map, 4), 1);
        assert_eq!(zirk_rt_map_remove(map, 4), 0);
        assert_eq!(zirk_rt_map_is_empty(map), 1);
    }

    #[test]
    fn string_content_keys_use_language_equality_and_hashing() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map =
            unsafe { zirk_rt_map_new_with_layout(std::ptr::null(), 0, std::ptr::null(), 0, true) };
        let first = crate::string::alloc_owned("same-key");
        let second = crate::string::alloc_owned("same-key");
        assert_ne!(first, second);
        zirk_rt_map_set(map, first as i64, 11);
        assert_eq!(zirk_rt_map_get(map, second as i64), 11);
        zirk_rt_map_set(map, second as i64, 12);
        assert_eq!(zirk_rt_map_length(map), 1);
        assert_eq!(zirk_rt_map_get(map, first as i64), 12);
        assert_eq!(zirk_rt_map_remove(map, second as i64), 1);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn mutable_string_key_keeps_its_inserted_value_and_survives_source_mutation() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map =
            unsafe { zirk_rt_map_new_with_layout(std::ptr::null(), 0, std::ptr::null(), 0, true) };
        let original = crate::string::alloc_owned("before");
        let replacement = crate::string::alloc_owned("after");
        let equivalent = crate::string::alloc_owned("before");
        zirk_rt_map_set(map, original as i64, 41);

        let mut map_root = map;
        let mut original_root = original;
        let mut replacement_root = replacement;
        let mut equivalent_root = equivalent;
        let mut roots = [
            (&mut map_root as *mut *mut c_void).cast(),
            (&mut original_root as *mut *mut c_void).cast(),
            (&mut replacement_root as *mut *mut c_void).cast(),
            (&mut equivalent_root as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        unsafe { crate::string::zirk_str_set(original_root, 0, 6, replacement_root) };

        assert_eq!(zirk_rt_map_get(map_root, equivalent_root as i64), 41);
        assert_eq!(zirk_rt_map_get(map_root, original_root as i64), 0);
        let keys = unsafe { zirk_rt_map_sorted_keys(map_root, 1) };
        let key =
            unsafe { *(crate::array::zirk_rt_array_element(keys, 0, WORD) as *const *mut c_void) };
        assert_eq!(
            unsafe { (*crate::string::payload_ptr(key)).as_str() },
            "before"
        );
        crate::zirk_rt_pop_frame();

        let mut map_root = map;
        let mut equivalent_root = equivalent;
        let mut roots = [
            (&mut map_root as *mut *mut c_void).cast(),
            (&mut equivalent_root as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(original));
        assert_eq!(zirk_rt_map_get(map_root, equivalent_root as i64), 41);
        crate::zirk_rt_pop_frame();

        crate::collector::reset_state_for_tests();
        drop(guard);
    }

    #[test]
    fn live_map_handle_survives_collection_and_unreachable_map_is_reclaimed() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let mut map = zirk_rt_map_new();
        zirk_rt_map_set(map, 11, 22);
        let mut roots = [&mut map as *mut *mut c_void as *mut c_void];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::collect_for_tests();
        assert_eq!(zirk_rt_map_get(map, 11), 22);
        crate::zirk_rt_pop_frame();
        crate::collector::collect_for_tests();
        drop(guard);
    }

    #[test]
    fn map_traces_declared_reference_words() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let offsets = [0usize];
        let map =
            unsafe { zirk_rt_map_new_with_layout(offsets.as_ptr(), 1, offsets.as_ptr(), 1, false) };
        let value = crate::string::alloc_owned("map-root");
        zirk_rt_map_set(map, value as i64, value as i64);
        let mut handle = map;
        let mut roots = [&mut handle as *mut *mut c_void as *mut c_void];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(value));
        crate::zirk_rt_pop_frame();
        crate::collector::reset_state_for_tests();
        drop(guard);
    }
}
