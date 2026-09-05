//! Runtime helpers for `Regex`.
//!
//! A `Regex` is represented as an opaque pointer produced by
//! `zirk_regex_from_pattern`.  The handle is the address of a `regex::Regex`
//! stored in a process-wide cache so that identical patterns are compiled only
//! once, no matter how many times they appear in the source.
//!
//! # Memory
//!
//! The regex objects themselves live in the cache for the lifetime of the
//! process.  They are intentionally leaked (`Box::leak`) because the current
//! garbage collector has no finalizers and therefore cannot reclaim objects
//! with native resources behind them.  This is a controlled, bounded leak: the
//! number of distinct patterns in a program is finite and small, and the
//! actual native resources are owned by the `regex` crate.
//!
//! Output buffers returned by `zirk_regex_replace` and `zirk_regex_to_string`
//! are also leaked for the same reason: there is no runtime hook to free them
//! once the front end wraps them into a Zirk `String`.  The front end is
//! expected to copy the bytes into a normal GC `String` and treat the returned
//! pointer as an unowned, transient buffer.

use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

use regex::Regex;

/// Process-wide cache of compiled regexes, keyed by their source pattern.
///
/// Each value is a `&'static Regex` pointing to a `Box::leak` allocation.
/// Keeping the compiled form pinned makes the opaque handles returned to
/// generated code safe to use as raw pointers.
static REGEX_CACHE: LazyLock<Mutex<std::collections::HashMap<String, &'static Regex>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

unsafe fn regex_string(handle: *mut c_void) -> &'static str {
    if handle.is_null() {
        return "";
    }
    let payload = unsafe { &*crate::string::payload_ptr(handle) };
    unsafe { std::mem::transmute::<&str, &'static str>(payload.as_str()) }
}

/// Compiles a regex from a Zirk `String` pattern and returns an opaque handle.
///
/// On an invalid regex, `null` is returned.  The returned handle is the
/// address of a `regex::Regex` stored in a process-wide cache.
///
/// # Safety
///
/// `pattern` must be a non-null Zirk `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_from_pattern(pattern: *mut c_void) -> *mut c_void {
    let pattern = unsafe { regex_string(pattern) };
    if pattern.is_empty() {
        return std::ptr::null_mut();
    }

    let mut cache = REGEX_CACHE.lock().unwrap();
    if let Some(re) = cache.get(pattern) {
        return (*re) as *const Regex as *mut c_void;
    }

    let re = match Regex::new(pattern) {
        Ok(re) => re,
        Err(_) => return std::ptr::null_mut(),
    };

    let leaked = Box::leak(Box::new(re));
    cache.insert(pattern.to_string(), leaked);
    leaked as *const Regex as *mut c_void
}

/// Whether `pattern` compiles — the `ok` half of `Regex.parse`
/// (`native-type-member-surface`). The handle half is the existing
/// `zirk_regex_from_pattern`, whose cache makes the double compile cheap.
///
/// # Safety
///
/// `pattern` must be a Zirk `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_is_valid_pattern(pattern: *mut c_void) -> bool {
    let pattern = unsafe { regex_string(pattern) };
    !pattern.is_empty() && Regex::new(pattern).is_ok()
}

/// Returns whether the regex in `handle` matches `text`.
///
/// Both `handle` and `text` are Zirk opaque handles.  A null or invalid
/// handle returns `false`.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_is_match(handle: *mut c_void, text: *mut c_void) -> bool {
    if handle.is_null() {
        return false;
    }
    let re = unsafe { &*(handle as *const Regex) };
    let text = unsafe { regex_string(text) };
    re.is_match(text)
}

/// Replaces all non-overlapping matches of the regex in `handle` in `text`
/// with `repl`, returning a new Zirk `String`.
///
/// All three arguments are Zirk opaque handles.  A null or invalid handle
/// returns a null pointer.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` and `repl` must be non-null Zirk `String` handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_replace(
    handle: *mut c_void,
    text: *mut c_void,
    repl: *mut c_void,
) -> *mut c_void {
    if handle.is_null() || text.is_null() || repl.is_null() {
        return std::ptr::null_mut();
    }

    let re = unsafe { &*(handle as *const Regex) };
    let text = unsafe { regex_string(text) };
    let repl = unsafe { regex_string(repl) };

    let result = re.replace_all(text, repl).into_owned();
    crate::string::alloc_owned(&result)
}

/// Splits `text` on every match of `re`, returning a `List<String>` with the
/// pieces.
///
/// `handle` and `text` are Zirk opaque handles.  A null or invalid handle
/// returns an empty list.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_split(handle: *mut c_void, text: *mut c_void) -> *mut c_void {
    if handle.is_null() || text.is_null() {
        return std::ptr::null_mut();
    }

    let re = unsafe { &*(handle as *const Regex) };
    let text = unsafe { regex_string(text) };

    // `List<String>` stores object references: the element size is the size of
    // a pointer, and `is_ref` marks the buffer as GC-traceable.
    let list = unsafe {
        crate::zirk_rt_list_new(
            std::mem::size_of::<*mut c_void>(),
            std::mem::align_of::<*mut c_void>(),
            true,
        )
    };

    for part in re.split(text) {
        let value = crate::string::alloc_owned(part);
        unsafe {
            crate::zirk_rt_list_add(
                list,
                &value as *const *mut c_void as *const c_void,
                std::mem::size_of::<*mut c_void>(),
                std::mem::align_of::<*mut c_void>(),
                true,
            )
        };
    }

    list
}

/// Returns the pattern string of the regex in `handle` as a Zirk `String`.
///
/// `handle` is a Zirk opaque handle.  A null or invalid handle returns a null
/// pointer.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_to_string(handle: *mut c_void) -> *mut c_void {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let re = unsafe { &*(handle as *const Regex) };
    crate::string::alloc_owned(re.as_str())
}

/// A `T?` value returned across the C ABI for object-typed optionals.
#[repr(C)]
pub struct NullableObject {
    present: bool,
    value: *mut c_void,
}

/// In-memory payload of a `Regex.Match` object, after the three-word GC
/// header.
#[repr(C)]
pub struct RegexMatch {
    pub text: *mut c_void,
    pub haystack: *mut c_void,
    pub start: i64,
    pub end: i64,
    pub regex: *mut c_void,
}

/// GC descriptor for a `RegexMatch` object.  The header layout expected by
/// `gc_field_offsets` is: method table, ancestor count, contract count,
/// gc-field count, then offsets.  Only `text` and `haystack` are traced.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_regex_match_descriptor: [usize; 6] = [
    0,                                                                   // method table
    0,                                                                   // ancestor count
    0,                                                                   // contract count
    2,                                                                   // gc-field count
    crate::collector::HEADER_BYTES,                                      // text offset
    crate::collector::HEADER_BYTES + std::mem::size_of::<*mut c_void>(), // haystack offset
];

/// Allocates a fresh `RegexMatch` object with the given fields.
unsafe fn alloc_regex_match(
    text: *mut c_void,
    haystack: *mut c_void,
    start: i64,
    end: i64,
    regex: *mut c_void,
) -> *mut c_void {
    let size = crate::collector::HEADER_BYTES + std::mem::size_of::<RegexMatch>();
    let object = unsafe { crate::zirk_rt_alloc(size, std::mem::align_of::<RegexMatch>()) };
    if object.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *(object as *mut *mut c_void) = zirk_rt_regex_match_descriptor.as_ptr() as *mut c_void;
        let payload = (object as *mut u8).add(crate::collector::HEADER_BYTES) as *mut RegexMatch;
        (*payload).text = text;
        (*payload).haystack = haystack;
        (*payload).start = start;
        (*payload).end = end;
        (*payload).regex = regex;
    }
    object
}

/// Finds the first match of the regex in `handle` within `text`.
///
/// Returns a `Regex.Match?`: present when a match exists, absent otherwise.
/// Both `handle` and `text` are Zirk opaque handles.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_find(handle: *mut c_void, text: *mut c_void) -> NullableObject {
    if handle.is_null() {
        return NullableObject {
            present: false,
            value: std::ptr::null_mut(),
        };
    }

    let re = unsafe { &*(handle as *const Regex) };
    let hay = unsafe { regex_string(text) };

    match re.find(hay) {
        Some(m) => {
            let text_handle = crate::string::alloc_owned(m.as_str());
            let haystack_handle = crate::string::alloc_owned(hay);
            let object = unsafe {
                alloc_regex_match(
                    text_handle,
                    haystack_handle,
                    m.start() as i64,
                    m.end() as i64,
                    handle,
                )
            };
            NullableObject {
                present: !object.is_null(),
                value: object,
            }
        }
        None => NullableObject {
            present: false,
            value: std::ptr::null_mut(),
        },
    }
}

/// Returns a `List<Regex.Match>` holding every non-overlapping match of the
/// regex in `handle` within `text`.
///
/// `handle` and `text` are Zirk opaque handles.  A null or invalid handle
/// returns an empty list.
///
/// # Safety
///
/// `handle` must be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_find_all(
    handle: *mut c_void,
    text: *mut c_void,
) -> *mut c_void {
    // `List<Regex.Match>` stores object references: the element size is the
    // size of a pointer, and `is_ref` marks the buffer as GC-traceable.
    let list = unsafe {
        crate::zirk_rt_list_new(
            std::mem::size_of::<*mut c_void>(),
            std::mem::align_of::<*mut c_void>(),
            true,
        )
    };
    if handle.is_null() || text.is_null() {
        return list;
    }

    let re = unsafe { &*(handle as *const Regex) };
    let hay = unsafe { regex_string(text) };
    let haystack_handle = crate::string::alloc_owned(hay);

    for m in re.find_iter(hay) {
        let text_handle = crate::string::alloc_owned(m.as_str());
        let object = unsafe {
            alloc_regex_match(
                text_handle,
                haystack_handle,
                m.start() as i64,
                m.end() as i64,
                handle,
            )
        };
        unsafe {
            crate::zirk_rt_list_add(
                list,
                &object as *const *mut c_void as *const c_void,
                std::mem::size_of::<*mut c_void>(),
                std::mem::align_of::<*mut c_void>(),
                true,
            )
        };
    }

    list
}

unsafe fn regex_match_payload(m: *mut c_void) -> *mut RegexMatch {
    unsafe { (m as *mut u8).add(crate::collector::HEADER_BYTES) as *mut RegexMatch }
}

/// Returns the nth positional capture group of `m`, or an empty string if the
/// group is missing.
///
/// `m` is a `RegexMatch` object pointer; `n` is the group index (0 is the
/// whole match).
///
/// # Safety
///
/// `m` must be a non-null `RegexMatch` object pointer returned by `zirk_regex_find`.
/// The `Regex` it references must still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_match_group_pos(m: *mut c_void, n: i64) -> *mut c_void {
    if m.is_null() {
        return crate::string::alloc_owned("");
    }
    let m = unsafe { &*regex_match_payload(m) };

    let re = m.regex as *const Regex;
    if re.is_null() {
        return crate::string::alloc_owned("");
    }
    let re = unsafe { &*re };

    let hay = unsafe { regex_string(m.haystack) };
    let start = m.start as usize;
    let n = n as usize;

    for caps in re.captures_iter(hay) {
        let whole = caps.get(0);
        let Some(whole) = whole else { continue };
        if whole.start() == start {
            return match caps.get(n) {
                Some(group) => crate::string::alloc_owned(group.as_str()),
                None => crate::string::alloc_owned(""),
            };
        }
    }

    crate::string::alloc_owned("")
}

/// Returns the named capture group of `m`, or an empty string if the group is
/// missing.
///
/// `m` is a `RegexMatch` object pointer; `name` is a Zirk `String`.
///
/// # Safety
///
/// `m` must be a non-null `RegexMatch` object pointer returned by `zirk_regex_find`.
/// `name` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_match_group_name(
    m: *mut c_void,
    name: *mut c_void,
) -> *mut c_void {
    if m.is_null() {
        return crate::string::alloc_owned("");
    }
    let m = unsafe { &*regex_match_payload(m) };

    let re = m.regex as *const Regex;
    if re.is_null() {
        return crate::string::alloc_owned("");
    }
    let re = unsafe { &*re };

    let hay = unsafe { regex_string(m.haystack) };
    let start = m.start as usize;
    let name = unsafe { regex_string(name) };

    for caps in re.captures_iter(hay) {
        let whole = caps.get(0);
        let Some(whole) = whole else { continue };
        if whole.start() == start {
            return match caps.name(name) {
                Some(group) => crate::string::alloc_owned(group.as_str()),
                None => crate::string::alloc_owned(""),
            };
        }
    }

    crate::string::alloc_owned("")
}
