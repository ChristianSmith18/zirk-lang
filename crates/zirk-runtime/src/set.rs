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

/// `set.to_string()`'s element snapshot
/// (`universal-to-string-and-multiline-strings`): the elements, sorted —
/// numerically, or by contents when `string_elements` says the words are
/// `String`/`Char` handles — written into a word-sized-element `Array` the
/// generated loop then walks with `ArrayLength`/`ArrayListLoad`.
///
/// # Safety
///
/// `set` must be a handle produced by `zirk_rt_set_new`; with
/// `string_elements` set, every stored element must be a live string handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_set_sorted_elements(
    set: *mut c_void,
    string_elements: i64,
) -> *mut c_void {
    let mut elements: Vec<i64> = if set.is_null() {
        Vec::new()
    } else {
        let set = unsafe { &*(set as *const HashSet<i64>) };
        set.iter().copied().collect()
    };
    unsafe { crate::map::sort_words(&mut elements, string_elements != 0) };
    let array = unsafe {
        crate::array::zirk_rt_array_new(
            elements.len(),
            std::mem::size_of::<i64>(),
            std::mem::align_of::<i64>(),
            false,
        )
    };
    for (index, element) in elements.iter().enumerate() {
        unsafe { crate::array::write_word(array, index, *element) };
    }
    array
}

#[cfg(test)]
mod tests {
    use super::{zirk_rt_set_add, zirk_rt_set_new, zirk_rt_set_sorted_elements};

    #[test]
    fn sorted_element_snapshot_uses_string_contents() {
        let set = zirk_rt_set_new();
        for text in ["z", "a", "m"] {
            zirk_rt_set_add(set, crate::string::alloc_owned(text) as i64);
        }

        let snapshot = unsafe { zirk_rt_set_sorted_elements(set, 1) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|index| {
                    let pointer = crate::array::zirk_rt_array_element(snapshot, index as i64, 8);
                    *(pointer as *const i64)
                })
                .collect::<Vec<_>>()
        };
        let text = unsafe {
            words
                .iter()
                .map(|word| (*crate::string::payload_ptr(*word as *mut _)).as_str().to_string())
                .collect::<Vec<_>>()
        };

        assert_eq!(text, ["a", "m", "z"]);
    }
}
