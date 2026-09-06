//! Runtime helpers for `Set<T>`.
//!
//! Minimal first implementation storing `i64` values in a `HashSet`.
//! See `map.rs` for the same GC caveats.

use std::collections::HashSet;
use std::ffi::c_void;

/// `Set<T>()` — allocates an empty set.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_new() -> *mut c_void {
    let set = Box::new(HashSet::<i64>::new());
    Box::into_raw(set) as *mut c_void
}

/// `set.is_empty()` — returns whether the set is empty.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_is_empty(set: *mut c_void) -> i32 {
    if set.is_null() {
        return 1;
    }
    let set = unsafe { &*(set as *const HashSet<i64>) };
    set.is_empty() as i32
}

/// `set.length()` — returns the number of elements.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_length(set: *mut c_void) -> usize {
    if set.is_null() {
        return 0;
    }
    let set = unsafe { &*(set as *const HashSet<i64>) };
    set.len()
}

/// `set.add(value)` — inserts `value`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_add(set: *mut c_void, value: i64) {
    if set.is_null() {
        return;
    }
    let set = unsafe { &mut *(set as *mut HashSet<i64>) };
    set.insert(value);
}

/// `set.contains(value)` — returns `1` if `value` is present.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_contains(set: *mut c_void, value: i64) -> i32 {
    if set.is_null() {
        return 0;
    }
    let set = unsafe { &*(set as *const HashSet<i64>) };
    set.contains(&value) as i32
}

/// `set.remove(value)` — removes the value and returns `1` if it existed.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_set_remove(set: *mut c_void, value: i64) -> i32 {
    if set.is_null() {
        return 0;
    }
    let set = unsafe { &mut *(set as *mut HashSet<i64>) };
    set.remove(&value) as i32
}
