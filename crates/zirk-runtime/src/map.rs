//! Runtime helpers for `Map<K, V>`.
//!
//! This is an intentionally minimal first implementation: it stores
//! `i64` keys and values in a `HashMap`. It is enough for primitive
//! keys and values (`Int`, `Boolean`, `Char`, `Duration`) and for
//! object pointers stored as opaque `i64` values, but it does not
//! integrate with the garbage collector's tracing pass yet.

use std::collections::HashMap;
use std::ffi::c_void;

/// `Map<K, V>()` — allocates an empty map.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_new() -> *mut c_void {
    let map = Box::new(HashMap::<i64, i64>::new());
    Box::into_raw(map) as *mut c_void
}

/// `map.is_empty()` — returns whether the map is empty.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_is_empty(map: *mut c_void) -> i32 {
    if map.is_null() {
        return 1;
    }
    let map = unsafe { &*(map as *const HashMap<i64, i64>) };
    map.is_empty() as i32
}

/// `map.length()` — returns the number of entries.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_length(map: *mut c_void) -> usize {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const HashMap<i64, i64>) };
    map.len()
}

/// `map.set(key, value)` — inserts or updates the key/value pair.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_set(map: *mut c_void, key: i64, value: i64) {
    if map.is_null() {
        return;
    }
    let map = unsafe { &mut *(map as *mut HashMap<i64, i64>) };
    map.insert(key, value);
}

/// `map.contains_key(key)` — returns `1` if the key is present.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_contains_key(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const HashMap<i64, i64>) };
    map.contains_key(&key) as i32
}

/// `map.get_or_null(key)` — returns the value associated with the key, or `0`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_get(map: *mut c_void, key: i64) -> i64 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &*(map as *const HashMap<i64, i64>) };
    map.get(&key).copied().unwrap_or(0)
}

/// `map.remove(key)` — removes the key and returns `1` if it existed.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_remove(map: *mut c_void, key: i64) -> i32 {
    if map.is_null() {
        return 0;
    }
    let map = unsafe { &mut *(map as *mut HashMap<i64, i64>) };
    map.remove(&key).is_some() as i32
}
