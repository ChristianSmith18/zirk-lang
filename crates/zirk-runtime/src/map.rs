//! Runtime helpers for `Map<K, V>`.
//!
//! This is an intentionally minimal first implementation: it stores
//! `i64` keys and values in a `HashMap`. It is enough for primitive
//! keys and values (`Int`, `Boolean`, `Char`, `Duration`) and for
//! object pointers stored as opaque `i64` values, but it does not
//! integrate with the garbage collector's tracing pass yet.

use std::ffi::c_void;
use zirk_runtime_core::collections::{self, WordMap};

/// Sorts stored words for `to_string()`'s deterministic ordering
/// (`universal-to-string-and-multiline-strings`): numerically for scalar
/// words, by contents when `string_content` says the words name
/// `String`/`Char` handles. Shared with `set.rs`.
///
/// # Safety
///
/// With `string_content` set, every non-zero word must be a live string
/// handle produced by this runtime.
pub(crate) unsafe fn sort_words(words: &mut [i64], string_content: bool) {
    if !string_content {
        words.sort_unstable();
        return;
    }
    words.sort_by(|&a, &b| {
        let read = |word: i64| -> &str {
            if word == 0 {
                return "";
            }
            unsafe { (*crate::string::payload_ptr(word as *mut c_void)).as_str() }
        };
        read(a).cmp(read(b))
    });
}

/// `Map<K, V>()` — allocates an empty map.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_new() -> *mut c_void {
    let map = Box::new(WordMap::new());
    Box::into_raw(map) as *mut c_void
}

/// `map.is_empty()` — returns whether the map is empty.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_is_empty(map: *mut c_void) -> i32 {
    if map.is_null() {
        return 1;
    }
    let map = unsafe { &*(map as *const WordMap) };
    map.is_empty() as i32
}

/// `map.length()` — returns the number of entries.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_length(map: *mut c_void) -> usize {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const WordMap) };
    map.len()
}

/// `map.set(key, value)` — inserts or updates the key/value pair.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_set(map: *mut c_void, key: i64, value: i64) {
    if map.is_null() {
        return;
    }
    let map = unsafe { &mut *(map as *mut WordMap) };
    collections::map_set(map, key, value);
}

/// `map.contains_key(key)` — returns `1` if the key is present.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_contains_key(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const WordMap) };
    map.contains_key(&key) as i32
}

/// `map.get_or_null(key)` — returns the value associated with the key, or `0`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_get(map: *mut c_void, key: i64) -> i64 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const WordMap) };
    collections::map_get(map, key).unwrap_or(0)
}

/// `map.remove(key)` — removes the key and returns `1` if it existed.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_remove(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &mut *(map as *mut WordMap) };
    collections::map_remove(map, key) as i32
}

/// `map.to_string()`'s key snapshot
/// (`universal-to-string-and-multiline-strings`): the keys, sorted —
/// numerically, or by contents when `string_keys` says the words are
/// `String`/`Char` handles — written into a word-sized-element `Array` the
/// generated loop then walks with `ArrayLength`/`ArrayListLoad`.
///
/// # Safety
///
/// `map` must be a handle produced by `zirk_rt_map_new`; with
/// `string_keys` set, every stored key must be a live string handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_sorted_keys(
    map: *mut c_void,
    string_keys: i64,
) -> *mut c_void {
    let mut keys: Vec<i64> = if map.is_null() {
        Vec::new()
    } else {
        let map = unsafe { &*(map as *const WordMap) };
        map.keys().copied().collect()
    };
    unsafe { sort_words(&mut keys, string_keys != 0) };
    let array = unsafe {
        crate::array::zirk_rt_array_new(
            keys.len(),
            std::mem::size_of::<i64>(),
            std::mem::align_of::<i64>(),
            false,
        )
    };
    for (index, key) in keys.iter().enumerate() {
        unsafe { crate::array::write_word(array, index, *key) };
    }
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
                .map(|index| {
                    let pointer = crate::array::zirk_rt_array_element(snapshot, index as i64, 8);
                    *(pointer as *const i64)
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(words, [1, 2, 10]);
    }

    #[test]
    fn native_map_adapter_matches_runtime_core_value_operations() {
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
}
