//! Runtime helpers for `Regex`.
//!
//! A `Regex` is represented as an opaque pointer produced by
//! `zirk_regex_from_pattern`.  The handle is the address of a `regex::Regex`
//! stored in a process-wide cache so that identical patterns are compiled only
//! once, no matter how many times they appear in the source. The cache has
//! fixed entry and pattern-byte budgets. It rejects new unique patterns when
//! full instead of evicting entries, because generated code may retain opaque
//! handles with no release operation.
//!
//! # Memory
//!
//! The regex objects themselves live in the cache for the lifetime of the
//! process. They are intentionally leaked (`Box::leak`) because the current
//! ABI has no release operation for opaque regex handles. The fixed cache
//! budgets bound this retention, and each compiled regex has a size limit.
//!
//! Output buffers returned by `zirk_regex_replace` and `zirk_regex_to_string`
//! are also leaked for the same reason: there is no runtime hook to free them
//! once the front end wraps them into a Zirk `String`.  The front end is
//! expected to copy the bytes into a normal GC `String` and treat the returned
//! pointer as an unowned, transient buffer.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

use zirk_runtime_core::regex::{self as core_regex, Regex};

const MAX_REGEX_CACHE_ENTRIES: usize = 128;
const MAX_REGEX_PATTERN_BYTES: usize = 16 * 1024;
const MAX_REGEX_CACHE_PATTERN_BYTES: usize = 256 * 1024;

/// Compiled regexes keyed by source. Entries are never evicted: opaque handles
/// have no release operation, so eviction would make still-live handles dangle.
struct RegexCache {
    entries: HashMap<String, &'static Regex>,
    retained_pattern_bytes: usize,
    max_entries: usize,
    max_pattern_bytes: usize,
    max_total_pattern_bytes: usize,
    compilations: usize,
}

impl RegexCache {
    fn new(max_entries: usize, max_pattern_bytes: usize, max_total_pattern_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            retained_pattern_bytes: 0,
            max_entries,
            max_pattern_bytes,
            max_total_pattern_bytes,
            compilations: 0,
        }
    }

    fn get_or_compile(&mut self, pattern: &str) -> Option<*const Regex> {
        if let Some(regex) = self.entries.get(pattern) {
            return Some(*regex as *const Regex);
        }
        if pattern.is_empty()
            || pattern.len() > self.max_pattern_bytes
            || self.entries.len() >= self.max_entries
            || self.retained_pattern_bytes.checked_add(pattern.len())?
                > self.max_total_pattern_bytes
        {
            return None;
        }

        self.compilations += 1;
        let regex = core_regex::compile(pattern).ok()?;
        let leaked = Box::leak(Box::new(regex));
        self.entries.insert(pattern.to_owned(), leaked);
        self.retained_pattern_bytes += pattern.len();
        Some(leaked as *const Regex)
    }
}

static REGEX_CACHE: LazyLock<Mutex<RegexCache>> = LazyLock::new(|| {
    Mutex::new(RegexCache::new(
        MAX_REGEX_CACHE_ENTRIES,
        MAX_REGEX_PATTERN_BYTES,
        MAX_REGEX_CACHE_PATTERN_BYTES,
    ))
});

fn cached_regex(pattern: &str) -> Option<*const Regex> {
    REGEX_CACHE.lock().ok()?.get_or_compile(pattern)
}

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
    cached_regex(pattern)
        .map(|regex| regex as *mut c_void)
        .unwrap_or(std::ptr::null_mut())
}

/// Whether `pattern` compiles — the `ok` half of `Regex.parse`
/// (`native-type-member-surface`). The validation call seeds the bounded cache;
/// the following handle construction reuses that compiled instance.
///
/// # Safety
///
/// `pattern` must be a Zirk `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_is_valid_pattern(pattern: *mut c_void) -> bool {
    let pattern = unsafe { regex_string(pattern) };
    cached_regex(pattern).is_some()
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
    core_regex::is_match(re, text)
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

    let result = core_regex::replace(re, text, repl);
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

    for part in core_regex::split(re, text) {
        let value = crate::string::alloc_owned(&part);
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        MAX_REGEX_CACHE_ENTRIES, MAX_REGEX_CACHE_PATTERN_BYTES, MAX_REGEX_PATTERN_BYTES, RegexCache,
    };

    fn text(handle: *mut std::ffi::c_void) -> String {
        unsafe { (*crate::string::payload_ptr(handle)).as_str().to_owned() }
    }

    #[test]
    fn to_string_returns_the_original_pattern() {
        let pattern = crate::string::alloc_owned("a+b*");
        let regex = unsafe { super::zirk_regex_from_pattern(pattern) };
        let rendered = unsafe { super::zirk_regex_to_string(regex) };
        let text = unsafe { (*crate::string::payload_ptr(rendered)).as_str() };

        assert_eq!(text, "a+b*");
    }

    #[test]
    fn extracted_regex_operations_preserve_native_handle_results() {
        let regex = unsafe { super::zirk_regex_from_pattern(crate::string::alloc_owned("a+")) };
        let haystack = crate::string::alloc_owned("baacaa");
        assert!(unsafe { super::zirk_regex_is_match(regex, haystack) });
        let replacement =
            unsafe { super::zirk_regex_replace(regex, haystack, crate::string::alloc_owned("x")) };
        assert_eq!(text(replacement), "bxcx");
        let mut found = super::NullableObject {
            present: false,
            value: std::ptr::null_mut(),
        };
        unsafe { super::zirk_regex_find(&mut found, regex, haystack) };
        assert!(found.present);
        let matched = unsafe { &*super::regex_match_payload(found.value) };
        assert_eq!((matched.start, matched.end), (1, 3));
    }

    #[test]
    fn regex_cache_reuses_compilation_and_keeps_handles_stable_at_capacity() {
        let mut cache = RegexCache::new(2, 64, 64);
        let first = cache.get_or_compile("alpha+").unwrap();
        let same = cache.get_or_compile("alpha+").unwrap();
        let second = cache.get_or_compile("beta+").unwrap();
        assert_eq!(first, same);
        assert_eq!(cache.compilations, 2);
        assert!(cache.get_or_compile("gamma+").is_none());
        assert_eq!(cache.get_or_compile("alpha+"), Some(first));
        assert_eq!(cache.get_or_compile("beta+"), Some(second));
        // A pointer returned before saturation still points to its original
        // compiled program; there is no eviction/reuse of handle addresses.
        assert!(unsafe { &*first }.is_match("alpha"));
    }

    #[test]
    fn regex_cache_enforces_pattern_and_aggregate_byte_budgets() {
        let mut cache = RegexCache::new(8, 8, 10);
        assert!(cache.get_or_compile(&"a".repeat(9)).is_none());
        assert!(cache.get_or_compile("alpha").is_some());
        assert!(cache.get_or_compile("beta").is_some());
        assert_eq!(cache.retained_pattern_bytes, 9);
        assert!(cache.get_or_compile("c{2}").is_none());
        const {
            assert!(MAX_REGEX_CACHE_ENTRIES > 0);
            assert!(MAX_REGEX_PATTERN_BYTES <= MAX_REGEX_CACHE_PATTERN_BYTES);
        }
        assert!(MAX_REGEX_CACHE_PATTERN_BYTES > cache.retained_pattern_bytes);
    }

    #[test]
    fn validity_check_compiles_once_then_handle_creation_reuses_it() {
        let pattern = crate::string::alloc_owned("(?:zirk-cache-reuse-unique)+");
        assert!(unsafe { super::zirk_regex_is_valid_pattern(pattern) });
        let first = unsafe { super::zirk_regex_from_pattern(pattern) };
        let second = unsafe { super::zirk_regex_from_pattern(pattern) };
        assert!(!first.is_null());
        assert_eq!(first, second);
    }

    #[test]
    fn runtime_rejects_patterns_over_the_per_entry_byte_limit() {
        let pattern = crate::string::alloc_owned(&"a".repeat(MAX_REGEX_PATTERN_BYTES + 1));
        assert!(!unsafe { super::zirk_regex_is_valid_pattern(pattern) });
        assert!(unsafe { super::zirk_regex_from_pattern(pattern) }.is_null());
    }
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
/// Writes a `Regex.Match?` through `out`: present when a match exists,
/// absent otherwise. Both `handle` and `text` are Zirk opaque handles.
///
/// `out` is written through a pointer rather than returned by value:
/// `NullableObject` is a 16-byte `{ bool, ptr }` aggregate, and the x86-64
/// Windows/MSVC and SysV (Linux/macOS) C ABIs disagree on how such a
/// struct is returned by value (SysV packs it into two registers; MSVC's
/// aggregate-return rule only allows that for sizes that are themselves a
/// power of two — 1/2/4/8 bytes — and demands a hidden out-pointer
/// otherwise) — confirmed: this crashed with `STATUS_ACCESS_VIOLATION` on
/// Windows while working correctly on every other platform. Every other
/// `Nullable<T>` in this compiler is built and consumed entirely by our
/// own codegen (never crossing an `extern "C"` boundary), so this
/// function was the only place the ambiguity could bite.
///
/// # Safety
///
/// `out` must be a valid, writable `NullableObject` pointer. `handle` must
/// be a non-null regex handle returned by `zirk_regex_from_pattern`.
/// `text` must be a non-null Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_regex_find(
    out: *mut NullableObject,
    handle: *mut c_void,
    text: *mut c_void,
) {
    if handle.is_null() {
        unsafe {
            *out = NullableObject {
                present: false,
                value: std::ptr::null_mut(),
            }
        };
        return;
    }

    let re = unsafe { &*(handle as *const Regex) };
    let hay = unsafe { regex_string(text) };

    let result = match core_regex::find(re, hay) {
        Some((start, end)) => {
            let text_handle = crate::string::alloc_owned(&hay[start..end]);
            let haystack_handle = crate::string::alloc_owned(hay);
            let object = unsafe {
                alloc_regex_match(
                    text_handle,
                    haystack_handle,
                    start as i64,
                    end as i64,
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
    };
    unsafe { *out = result };
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

    for (start, end) in core_regex::find_all(re, hay) {
        let text_handle = crate::string::alloc_owned(&hay[start..end]);
        let object = unsafe {
            alloc_regex_match(
                text_handle,
                haystack_handle,
                start as i64,
                end as i64,
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
