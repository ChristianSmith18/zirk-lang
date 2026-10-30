//! Runtime helpers for `Map<K, V>`.

use std::ffi::c_void;

use crate::ordered::{CollectionTextField, CompositeKey, OrderedMap};

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

#[repr(C)]
struct MapBody {
    entries: MapEntries,
    key_reference_offsets: Box<[usize]>,
    value_reference_offsets: Box<[usize]>,
    key_union_roots: Box<[CollectionUnionRoot]>,
    value_union_roots: Box<[CollectionUnionRoot]>,
    key_text_fields: Box<[CollectionTextField]>,
    key_size: usize,
    value_size: usize,
    structural_version: u64,
}

struct CollectionUnionRoot {
    offset: usize,
    descriptor: *const usize,
    present_offset: Option<usize>,
    enum_guards: Box<[(usize, u32)]>,
}

enum MapEntries {
    Words(OrderedMap<Vec<u8>, Vec<u8>>),
    Text(OrderedMap<Option<Box<str>>, Vec<u8>>),
    Composite(OrderedMap<CompositeKey, Vec<u8>>),
}

impl MapEntries {
    fn len(&self) -> usize {
        match self {
            Self::Words(entries) => entries.len(),
            Self::Text(entries) => entries.len(),
            Self::Composite(entries) => entries.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Words(entries) => entries.is_empty(),
            Self::Text(entries) => entries.is_empty(),
            Self::Composite(entries) => entries.is_empty(),
        }
    }
}

fn text_key(bytes: &[u8]) -> Option<Box<str>> {
    let word = if bytes.len() == WORD {
        read_word(bytes)
    } else {
        if bytes.first().copied().unwrap_or(0) == 0 {
            return None;
        }
        let pointer_offset = WORD;
        read_word(bytes.get(pointer_offset..)?)
    };
    let handle = word as usize as *mut c_void;
    if handle.is_null() {
        return None;
    }
    let text = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
}

fn encode_text_key(size: usize, key: Option<&str>) -> Vec<u8> {
    let handle = key.map_or(std::ptr::null_mut(), |text| unsafe {
        crate::string::zirk_str_from_utf8(text.as_ptr(), text.len())
    });
    if size == WORD {
        return (handle as usize as i64).to_ne_bytes().to_vec();
    }
    let pointer_offset = WORD;
    let mut encoded = vec![0_u8; size];
    if key.is_some() {
        assert!(
            encoded.len() >= pointer_offset + WORD,
            "nullable text key layout is too small"
        );
        encoded[0] = 1;
        let pointer = (handle as usize).to_ne_bytes();
        encoded[pointer_offset..pointer_offset + WORD].copy_from_slice(&pointer);
    }
    encoded
}

fn write_text_key(
    array: *mut c_void,
    index: usize,
    stride: usize,
    offset: usize,
    size: usize,
    key: Option<&str>,
) {
    let encoded = encode_text_key(size, key);
    if size == WORD {
        let word = i64::from_ne_bytes(encoded.as_slice().try_into().expect("word-sized key"));
        unsafe { crate::array::write_word_at(array, index, stride, offset, size, word) };
        return;
    }
    unsafe { crate::array::write_bytes_at(array, index, stride, offset, &encoded) };
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

unsafe fn reference_offsets(pointer: *const usize, count: usize, size: usize) -> Box<[usize]> {
    if count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "map reference offsets are null");
    let offsets = unsafe { std::slice::from_raw_parts(pointer, count) };
    for &offset in offsets {
        assert!(
            offset.checked_add(WORD).is_some_and(|end| end <= size),
            "map reference offset is outside its entry"
        );
    }
    offsets.to_vec().into_boxed_slice()
}

/// `Map<K, V>()` — allocates a GC-managed handle with no declared managed
/// key/value edges and one-word compatibility layouts.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_new() -> *mut c_void {
    unsafe {
        zirk_rt_map_new_with_layout(
            std::ptr::null(),
            0,
            WORD,
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            false,
            WORD,
        )
    }
}

/// Creates a map and records byte offsets of managed references in each
/// typed key and value. The offset lists are copied into the managed
/// handle and released with it.
///
/// # Safety
///
/// Each non-empty offset list must point to readable `usize` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_new_with_layout(
    key_offsets: *const usize,
    key_offset_count: usize,
    key_size: usize,
    key_union_roots: *const usize,
    key_union_root_word_count: usize,
    value_offsets: *const usize,
    value_offset_count: usize,
    value_union_roots: *const usize,
    value_union_root_word_count: usize,
    string_content_keys: bool,
    value_size: usize,
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
    let key_reference_offsets =
        unsafe { reference_offsets(key_offsets, key_offset_count, key_size) };
    let value_reference_offsets =
        unsafe { reference_offsets(value_offsets, value_offset_count, value_size) };
    let key_union_roots =
        unsafe { collection_union_roots(key_union_roots, key_union_root_word_count, key_size) };
    let value_union_roots = unsafe {
        collection_union_roots(value_union_roots, value_union_root_word_count, value_size)
    };
    unsafe {
        *(object as *mut *mut c_void) = map_descriptor();
        body(object).write(MapBody {
            entries: if string_content_keys {
                MapEntries::Text(OrderedMap::default())
            } else {
                MapEntries::Words(OrderedMap::default())
            },
            key_reference_offsets,
            value_reference_offsets,
            key_union_roots,
            value_union_roots,
            key_text_fields: Box::new([]),
            key_size,
            value_size,
            structural_version: 0,
        });
    }
    object
}

/// Creates a typed map whose key layout contains nested String/Char fields.
/// The metadata records each text pointer offset and nullable-presence guards.
///
/// # Safety
/// Each non-empty metadata list must point to readable `usize` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_new_with_text_fields(
    key_offsets: *const usize,
    key_offset_count: usize,
    key_size: usize,
    key_union_roots: *const usize,
    key_union_root_word_count: usize,
    value_offsets: *const usize,
    value_offset_count: usize,
    value_union_roots: *const usize,
    value_union_root_word_count: usize,
    text_fields: *const usize,
    text_field_word_count: usize,
    value_size: usize,
) -> *mut c_void {
    let map = unsafe {
        zirk_rt_map_new_with_layout(
            key_offsets,
            key_offset_count,
            key_size,
            key_union_roots,
            key_union_root_word_count,
            value_offsets,
            value_offset_count,
            value_union_roots,
            value_union_root_word_count,
            false,
            value_size,
        )
    };
    let map_body = unsafe { &mut *body(map) };
    map_body.entries = MapEntries::Composite(OrderedMap::default());
    map_body.key_text_fields = unsafe {
        crate::ordered::collection_text_fields(text_fields, text_field_word_count, key_size)
    };
    map
}

/// Decodes records of `[value offset, descriptor, present offset, guard count,
/// (enum tag offset, expected tag)*]` words generated for a collection layout.
unsafe fn collection_union_roots(
    pointer: *const usize,
    word_count: usize,
    value_size: usize,
) -> Box<[CollectionUnionRoot]> {
    if word_count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "map union-root metadata is null");
    let words = unsafe { std::slice::from_raw_parts(pointer, word_count) };
    let mut cursor = 0;
    let mut roots = Vec::new();
    while cursor < words.len() {
        assert!(
            words.len() - cursor >= 4,
            "truncated map union-root metadata"
        );
        let offset = words[cursor];
        let descriptor = words[cursor + 1] as *const usize;
        let present = words[cursor + 2];
        let guard_count = words[cursor + 3];
        cursor += 4;
        assert!(!descriptor.is_null(), "map union-root descriptor is null");
        assert!(
            offset.checked_add(4).is_some_and(|end| end <= value_size),
            "map union tag is outside its entry"
        );
        let present_offset = (present != usize::MAX).then_some(present);
        if let Some(present_offset) = present_offset {
            assert!(
                present_offset < value_size,
                "map union presence is outside its entry"
            );
        }
        assert!(guard_count <= 64, "map union-root has too many enum guards");
        assert!(
            guard_count
                .checked_mul(2)
                .is_some_and(|count| count <= words.len() - cursor),
            "truncated map union-root enum guards"
        );
        let mut enum_guards = Vec::with_capacity(guard_count);
        for _ in 0..guard_count {
            let tag_offset = words[cursor];
            let expected_tag =
                u32::try_from(words[cursor + 1]).expect("map enum guard tag exceeds u32");
            cursor += 2;
            assert!(
                tag_offset
                    .checked_add(4)
                    .is_some_and(|end| end <= value_size),
                "map enum guard is outside its entry"
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

/// Called by the collector after validating the owner allocation.
pub(crate) unsafe fn mark_entries(
    map: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*body(map) };
    let mut visit = |key: Option<&[u8]>, value: &[u8]| {
        if let Some(key) = key {
            for &offset in &body.key_reference_offsets {
                let candidate = unsafe {
                    key.as_ptr()
                        .add(offset)
                        .cast::<*mut c_void>()
                        .read_unaligned()
                };
                if allocations.contains(&(candidate as usize)) {
                    mark(candidate);
                }
            }
            mark_collection_union_roots(key, &body.key_union_roots, mark);
        }
        for &offset in &body.value_reference_offsets {
            let candidate =
                unsafe { (value.as_ptr().add(offset).cast::<*mut c_void>()).read_unaligned() };
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
        mark_collection_union_roots(value, &body.value_union_roots, mark);
    };
    match &body.entries {
        MapEntries::Words(entries) => entries
            .iter()
            .for_each(|(key, value)| visit(Some(key), value)),
        MapEntries::Text(entries) => entries.iter().for_each(|(_, value)| visit(None, value)),
        MapEntries::Composite(entries) => entries
            .iter()
            .for_each(|(key, value)| visit(Some(&key.raw), value)),
    }
}

fn mark_collection_union_roots(
    bytes: &[u8],
    roots: &[CollectionUnionRoot],
    mark: &mut dyn FnMut(*mut c_void),
) {
    for root in roots {
        let active = root.enum_guards.iter().all(|(offset, expected)| {
            let actual = unsafe { bytes.as_ptr().add(*offset).cast::<u32>().read_unaligned() };
            actual == *expected
        });
        if !active {
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
pub extern "C" fn zirk_rt_map_length(map: *mut c_void) -> u64 {
    if map.is_null() {
        return 0;
    }
    unsafe { (&*body(map)).entries.len() as u64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_version(map: *mut c_void) -> u64 {
    if map.is_null() {
        return 0;
    }
    unsafe { (*body(map)).structural_version }
}

unsafe fn snapshot_array(
    length: usize,
    element_size: usize,
    element_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            length,
            element_size,
            element_align,
            reference_offsets,
            reference_offset_count,
        )
    }
}

/// Returns the current keys in insertion order as a typed Array snapshot.
/// Text-key collections materialize fresh immutable String handles; other keys
/// are copied using their complete declared layout.
///
/// # Safety
/// The collection must be live. The element layout and reference offsets
/// must describe the statically declared key type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_keys(
    map: *mut c_void,
    element_size: usize,
    element_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    if map.is_null() {
        return std::ptr::null_mut();
    }
    let map_body = unsafe { &*body(map) };
    assert_eq!(
        element_size, map_body.key_size,
        "Map.keys element layout differs from the stored key layout"
    );
    let entries = &map_body.entries;
    let result = unsafe {
        snapshot_array(
            entries.len(),
            element_size,
            element_align,
            reference_offsets,
            reference_offset_count,
        )
    };
    match entries {
        MapEntries::Words(entries) => {
            for (index, (key, _)) in entries.iter().enumerate() {
                unsafe { crate::array::write_bytes(result, index, element_size, key) };
            }
        }
        MapEntries::Text(entries) => {
            for (index, (key, _)) in entries.iter().enumerate() {
                write_text_key(result, index, element_size, 0, element_size, key.as_deref());
            }
        }
        MapEntries::Composite(entries) => {
            for (index, (key, _)) in entries.iter().enumerate() {
                let (bytes, _roots) = key
                    .materialize(&map_body.key_text_fields)
                    .expect("materialize composite map key");
                unsafe { crate::array::write_bytes(result, index, element_size, &bytes) };
            }
        }
    }
    result
}

/// Returns the values in insertion order as a typed Array snapshot.
///
/// # Safety
/// The collection must be live. The element layout and reference offsets
/// must describe the statically declared value type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_values(
    map: *mut c_void,
    element_size: usize,
    element_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    if map.is_null() {
        return std::ptr::null_mut();
    }
    let map_body = unsafe { &*body(map) };
    assert_eq!(
        element_size, map_body.value_size,
        "Map.values element layout differs from the stored value layout"
    );
    let entries = &map_body.entries;
    let result = unsafe {
        snapshot_array(
            entries.len(),
            element_size,
            element_align,
            reference_offsets,
            reference_offset_count,
        )
    };
    match entries {
        MapEntries::Words(entries) => {
            for (index, (_, value)) in entries.iter().enumerate() {
                unsafe { crate::array::write_bytes(result, index, element_size, value) };
            }
        }
        MapEntries::Text(entries) => {
            for (index, (_, value)) in entries.iter().enumerate() {
                unsafe { crate::array::write_bytes(result, index, element_size, value) };
            }
        }
        MapEntries::Composite(entries) => {
            for (index, (_, value)) in entries.iter().enumerate() {
                unsafe { crate::array::write_bytes(result, index, element_size, value) };
            }
        }
    }
    result
}

/// Returns insertion-ordered `Tuple<K, V>` rows as a typed Array snapshot.
/// Keys and values are copied from their complete typed byte layouts.
///
/// # Safety
/// The map and offsets must be live and describe the requested tuple layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_entries(
    map: *mut c_void,
    tuple_size: usize,
    tuple_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
    key_offset: usize,
    key_size: usize,
    value_offset: usize,
    value_size: usize,
) -> *mut c_void {
    if map.is_null() {
        return std::ptr::null_mut();
    }
    let map_body = unsafe { &*body(map) };
    assert_eq!(
        key_size, map_body.key_size,
        "Map.entries key layout differs from the stored key layout"
    );
    assert_eq!(
        value_size, map_body.value_size,
        "Map.entries value layout differs from the stored value layout"
    );
    let entries = &map_body.entries;
    let result = unsafe {
        snapshot_array(
            entries.len(),
            tuple_size,
            tuple_align,
            reference_offsets,
            reference_offset_count,
        )
    };
    match entries {
        MapEntries::Words(entries) => {
            for (index, (key, value)) in entries.iter().enumerate() {
                unsafe {
                    crate::array::write_bytes_at(result, index, tuple_size, key_offset, key);
                    crate::array::write_bytes_at(
                        result,
                        index,
                        tuple_size,
                        value_offset,
                        &value[..value_size],
                    );
                }
            }
        }
        MapEntries::Text(entries) => {
            for (index, (key, value)) in entries.iter().enumerate() {
                unsafe {
                    write_text_key(
                        result,
                        index,
                        tuple_size,
                        key_offset,
                        key_size,
                        key.as_deref(),
                    );
                    crate::array::write_bytes_at(
                        result,
                        index,
                        tuple_size,
                        value_offset,
                        &value[..value_size],
                    );
                }
            }
        }
        MapEntries::Composite(entries) => {
            for (index, (key, value)) in entries.iter().enumerate() {
                let (key_bytes, _roots) = key
                    .materialize(&map_body.key_text_fields)
                    .expect("materialize composite map entry key");
                unsafe {
                    crate::array::write_bytes_at(result, index, tuple_size, key_offset, &key_bytes);
                    crate::array::write_bytes_at(
                        result,
                        index,
                        tuple_size,
                        value_offset,
                        &value[..value_size],
                    );
                }
            }
        }
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_set(map: *mut c_void, key: i64, value: i64) -> i32 {
    let key_bytes = unsafe { std::slice::from_raw_parts((&key as *const i64).cast::<u8>(), WORD) };
    let value_bytes =
        unsafe { std::slice::from_raw_parts((&value as *const i64).cast::<u8>(), WORD) };
    unsafe { map_set_bytes(map, key_bytes, value_bytes) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_set_typed(
    map: *mut c_void,
    key: *const u8,
    key_size: usize,
    value: *const u8,
    value_size: usize,
) -> i32 {
    assert!(!key.is_null() || key_size == 0, "map key pointer is null");
    let key = unsafe { std::slice::from_raw_parts(key, key_size) };
    let value = unsafe { std::slice::from_raw_parts(value, value_size) };
    unsafe { map_set_bytes(map, key, value) }
}

unsafe fn map_set_bytes(map: *mut c_void, key: &[u8], value: &[u8]) -> i32 {
    if !map.is_null() {
        assert_eq!(
            key.len(),
            unsafe { (*body(map)).key_size },
            "map key size differs from its declared layout"
        );
        assert_eq!(
            value.len(),
            unsafe { (*body(map)).value_size },
            "map value size differs from its declared layout"
        );
        let Ok(key) = try_copy_bytes(key) else {
            return 2;
        };
        let Ok(value) = try_copy_bytes(value) else {
            return 2;
        };
        let body = unsafe { &mut *body(map) };
        let inserted = match &mut body.entries {
            MapEntries::Words(entries) => match entries.insert(key, value) {
                Ok(previous) => previous.is_none(),
                Err(()) => return 2,
            },
            MapEntries::Text(entries) => match entries.insert(text_key(&key), value) {
                Ok(previous) => previous.is_none(),
                Err(()) => return 2,
            },
            MapEntries::Composite(entries) => {
                let Ok(mut composite) = CompositeKey::from_bytes(&key, &body.key_text_fields)
                else {
                    return 2;
                };
                let Ok(_roots) = composite.snapshot_handles(&body.key_text_fields) else {
                    return 2;
                };
                match entries.insert(composite, value) {
                    Ok(previous) => previous.is_none(),
                    Err(()) => return 2,
                }
            }
        };
        if inserted {
            body.structural_version = body.structural_version.wrapping_add(1);
        }
        return 0;
    }
    0
}

fn try_copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, ()> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len()).map_err(|_| ())?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_contains_key(map: *mut c_void, key: i64) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts((&key as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_map_contains_key_typed(map, bytes.as_ptr(), bytes.len()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_contains_key_typed(
    map: *mut c_void,
    key: *const u8,
    size: usize,
) -> i32 {
    if map.is_null() {
        return 0;
    }
    assert!(!key.is_null() || size == 0, "map key pointer is null");
    let key = unsafe { std::slice::from_raw_parts(key, size) };
    assert_eq!(
        size,
        unsafe { (*body(map)).key_size },
        "map key size differs from its declared layout"
    );
    match &unsafe { &*body(map) }.entries {
        MapEntries::Words(entries) => entries.get(&key.to_vec()).is_some() as i32,
        MapEntries::Text(entries) => entries.get(&text_key(key)).is_some() as i32,
        MapEntries::Composite(entries) => {
            CompositeKey::from_bytes(key, &unsafe { &*body(map) }.key_text_fields)
                .ok()
                .is_some_and(|key| entries.get(&key).is_some()) as i32
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_get(map: *mut c_void, key: i64) -> i64 {
    let bytes = unsafe { std::slice::from_raw_parts((&key as *const i64).cast::<u8>(), WORD) };
    let mut output = [0_u8; WORD];
    if unsafe { zirk_rt_map_get_keyed(map, bytes.as_ptr(), bytes.len(), output.as_mut_ptr(), WORD) }
        == 0
    {
        return 0;
    }
    i64::from_ne_bytes(output)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_get_keyed(
    map: *mut c_void,
    key: *const u8,
    key_size: usize,
    output: *mut u8,
    value_size: usize,
) -> i32 {
    if map.is_null() {
        return 0;
    }
    assert!(!key.is_null() || key_size == 0, "map key pointer is null");
    let key = unsafe { std::slice::from_raw_parts(key, key_size) };
    let map_body = unsafe { &*body(map) };
    assert_eq!(
        key_size, map_body.key_size,
        "map key size differs from its declared layout"
    );
    assert!(
        value_size <= map_body.value_size,
        "map output exceeds its declared layout"
    );
    match &map_body.entries {
        MapEntries::Words(entries) => {
            copy_map_value(entries.get(&key.to_vec()), output, value_size)
        }
        MapEntries::Text(entries) => {
            copy_map_value(entries.get(&text_key(key)), output, value_size)
        }
        MapEntries::Composite(entries) => {
            let Some(key) = CompositeKey::from_bytes(key, &map_body.key_text_fields).ok() else {
                return 0;
            };
            copy_map_value(entries.get(&key), output, value_size)
        }
    }
}

fn copy_map_value(value: Option<&Vec<u8>>, output: *mut u8, size: usize) -> i32 {
    let Some(value) = value else {
        return 0;
    };
    assert_eq!(
        value.len(),
        size,
        "map output layout does not match stored value"
    );
    unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), output, size) };
    1
}

fn read_word(bytes: &[u8]) -> i64 {
    let mut word = 0i64;
    let count = bytes.len().min(WORD);
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), (&mut word as *mut i64).cast(), count) };
    word
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_get_typed(
    map: *mut c_void,
    key: i64,
    output: *mut u8,
    size: usize,
) -> i32 {
    if map.is_null() {
        return 0;
    }
    let key_bytes = unsafe { std::slice::from_raw_parts((&key as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_map_get_keyed(map, key_bytes.as_ptr(), WORD, output, size) }
}
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_remove(map: *mut c_void, key: i64) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts((&key as *const i64).cast::<u8>(), WORD) };
    unsafe { zirk_rt_map_remove_typed(map, bytes.as_ptr(), bytes.len()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_remove_typed(
    map: *mut c_void,
    key: *const u8,
    size: usize,
) -> i32 {
    if map.is_null() {
        return 0;
    }
    assert!(!key.is_null() || size == 0, "map key pointer is null");
    let key = unsafe { std::slice::from_raw_parts(key, size) };
    assert_eq!(
        size,
        unsafe { (*body(map)).key_size },
        "map key size differs from its declared layout"
    );
    let map_body = unsafe { &mut *body(map) };
    let removed = match &mut map_body.entries {
        MapEntries::Words(entries) => entries.remove(&key.to_vec()).is_some() as i32,
        MapEntries::Text(entries) => entries.remove(&text_key(key)).is_some() as i32,
        MapEntries::Composite(entries) => CompositeKey::from_bytes(key, &map_body.key_text_fields)
            .ok()
            .is_some_and(|key| entries.remove(&key).is_some())
            as i32,
    };
    if removed != 0 {
        map_body.structural_version = map_body.structural_version.wrapping_add(1);
    }
    removed
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_map_clear(map: *mut c_void) {
    if !map.is_null() {
        let map_body = unsafe { &mut *body(map) };
        let was_nonempty = !map_body.entries.is_empty();
        match &mut map_body.entries {
            MapEntries::Words(entries) => entries.clear(),
            MapEntries::Text(entries) => entries.clear(),
            MapEntries::Composite(entries) => entries.clear(),
        }
        if was_nonempty {
            map_body.structural_version = map_body.structural_version.wrapping_add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_map_sorted_keys(
    map: *mut c_void,
    string_keys: i64,
) -> *mut c_void {
    unsafe { map_keys_snapshot(map, string_keys, true) }
}

/// Returns keys in insertion order for collection iteration. Rendering uses
/// `zirk_rt_map_sorted_keys` to retain its canonical sorted text form.
#[cfg(test)]
pub(crate) unsafe fn map_keys_in_insertion_order(
    map: *mut c_void,
    string_keys: i64,
) -> *mut c_void {
    unsafe { map_keys_snapshot(map, string_keys, false) }
}

unsafe fn map_keys_snapshot(map: *mut c_void, string_keys: i64, sorted: bool) -> *mut c_void {
    if string_keys != 0 {
        let mut keys: Vec<Option<Box<str>>> = if map.is_null() {
            Vec::new()
        } else {
            match &unsafe { &*body(map) }.entries {
                MapEntries::Text(entries) => entries.iter().map(|(key, _)| key.clone()).collect(),
                MapEntries::Words(entries) => entries.iter().map(|_| None).collect(),
                MapEntries::Composite(entries) => entries.iter().map(|_| None).collect(),
            }
        };
        if sorted {
            keys.sort();
        }
        return snapshots_array(&keys);
    }
    let mut snapshot_roots = Vec::new();
    let (key_size, key_offsets, mut keys): (usize, Vec<usize>, Vec<Vec<u8>>) = if map.is_null() {
        (WORD, Vec::new(), Vec::new())
    } else {
        let map_body = unsafe { &*body(map) };
        let keys = match &map_body.entries {
            MapEntries::Words(entries) => entries.iter().map(|(key, _)| key.clone()).collect(),
            MapEntries::Text(entries) => {
                let mut text_keys: Vec<_> = entries.iter().map(|(key, _)| key.clone()).collect();
                if sorted {
                    text_keys.sort();
                }
                text_keys
                    .iter()
                    .map(|key| encode_text_key(map_body.key_size, key.as_deref()))
                    .collect()
            }
            MapEntries::Composite(entries) => entries
                .iter()
                .map(|(key, _)| {
                    let (bytes, roots) = key
                        .materialize(&map_body.key_text_fields)
                        .expect("materialize composite map snapshot key");
                    snapshot_roots.push(roots);
                    bytes
                })
                .collect(),
        };
        let key_offsets =
            if matches!(&map_body.entries, MapEntries::Text(_)) && map_body.key_size > WORD {
                vec![WORD]
            } else {
                map_body.key_reference_offsets.to_vec()
            };
        (map_body.key_size, key_offsets, keys)
    };
    if sorted {
        keys.sort_unstable();
    }
    let array = unsafe {
        crate::array::zirk_rt_array_new_with_layout(
            keys.len(),
            key_size,
            key_size.max(1).next_power_of_two(),
            key_offsets.as_ptr(),
            key_offsets.len(),
        )
    };
    for (index, key) in keys.iter().enumerate() {
        unsafe { crate::array::write_bytes(array, index, key_size, key) };
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
    fn sorted_key_snapshot_uses_natural_order_for_rendering() {
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
    fn insertion_key_snapshot_preserves_source_order() {
        let map = zirk_rt_map_new();
        zirk_rt_map_set(map, 10, 0);
        zirk_rt_map_set(map, 2, 0);
        zirk_rt_map_set(map, 1, 0);
        let snapshot = unsafe { map_keys_in_insertion_order(map, 0) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|i| {
                    *(crate::array::zirk_rt_array_element(snapshot, i as i64, WORD) as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(words, [10, 2, 1]);
    }

    #[test]
    fn updating_retains_order_and_remove_then_insert_appends() {
        let map = zirk_rt_map_new();
        assert_eq!(zirk_rt_map_set(map, 4, 40), 0);
        assert_eq!(zirk_rt_map_set(map, 2, 20), 0);
        assert_eq!(zirk_rt_map_set(map, 4, 41), 0);
        assert_eq!(zirk_rt_map_remove(map, 4), 1);
        assert_eq!(zirk_rt_map_set(map, 4, 42), 0);
        let snapshot = unsafe { map_keys_in_insertion_order(map, 0) };
        let words = unsafe {
            (0..crate::array::zirk_rt_array_length(snapshot))
                .map(|i| {
                    *(crate::array::zirk_rt_array_element(snapshot, i as i64, WORD) as *const i64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(words, [2, 4]);
    }

    #[test]
    fn map_operations_preserve_word_value_semantics() {
        let map = zirk_rt_map_new();
        assert_eq!(zirk_rt_map_set(map, 4, 10), 0);
        assert_eq!(zirk_rt_map_set(map, 4, 20), 0);
        assert_eq!(zirk_rt_map_length(map), 1);
        assert_eq!(zirk_rt_map_get(map, 4), 20);
        assert_eq!(zirk_rt_map_contains_key(map, 4), 1);
        assert_eq!(zirk_rt_map_remove(map, 4), 1);
        assert_eq!(zirk_rt_map_remove(map, 4), 0);
        assert_eq!(zirk_rt_map_is_empty(map), 1);
    }

    #[test]
    fn structural_version_changes_only_when_key_membership_changes() {
        let map = zirk_rt_map_new();
        assert_eq!(zirk_rt_map_version(map), 0);
        zirk_rt_map_set(map, 4, 10);
        assert_eq!(zirk_rt_map_version(map), 1);
        zirk_rt_map_set(map, 4, 20);
        assert_eq!(zirk_rt_map_version(map), 1);
        assert_eq!(zirk_rt_map_remove(map, 99), 0);
        assert_eq!(zirk_rt_map_version(map), 1);
        assert_eq!(zirk_rt_map_remove(map, 4), 1);
        assert_eq!(zirk_rt_map_version(map), 2);
        assert_eq!(zirk_rt_map_set(map, 5, 50), 0);
        zirk_rt_map_clear(map);
        assert_eq!(zirk_rt_map_version(map), 4);
        zirk_rt_map_clear(map);
        assert_eq!(zirk_rt_map_version(map), 4);
    }

    #[test]
    fn insertion_reservation_failure_does_not_mutate_or_invalidate() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map = zirk_rt_map_new();
        assert_eq!(zirk_rt_map_set(map, 1, 10), 0);
        let version = zirk_rt_map_version(map);

        crate::ordered::fail_next_insert_reservation_for_tests();
        assert_eq!(zirk_rt_map_set(map, 2, 20), 2);
        assert_eq!(zirk_rt_map_length(map), 1);
        assert_eq!(zirk_rt_map_contains_key(map, 1), 1);
        assert_eq!(zirk_rt_map_contains_key(map, 2), 0);
        assert_eq!(zirk_rt_map_get(map, 1), 10);
        assert_eq!(zirk_rt_map_version(map), version);

        assert_eq!(zirk_rt_map_set(map, 1, 11), 0);
        assert_eq!(zirk_rt_map_get(map, 1), 11);
        assert_eq!(zirk_rt_map_version(map), version);
    }

    #[test]
    fn string_content_keys_use_language_equality_and_hashing() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map = unsafe {
            zirk_rt_map_new_with_layout(
                std::ptr::null(),
                0,
                WORD,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                true,
                WORD,
            )
        };
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
    fn composite_text_keys_compare_by_content_and_materialize_snapshots() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let reference_offsets = [0usize];
        let text_fields = [0usize, 0usize, 0usize];
        let map = unsafe {
            zirk_rt_map_new_with_text_fields(
                reference_offsets.as_ptr(),
                reference_offsets.len(),
                WORD * 2,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                text_fields.as_ptr(),
                text_fields.len(),
                WORD,
            )
        };
        let first = crate::string::alloc_owned("e\u{301}");
        let equivalent = crate::string::alloc_owned("é");
        let first_key = [first as usize, 7usize];
        let equal_key = [equivalent as usize, 7usize];
        let first_bytes =
            unsafe { std::slice::from_raw_parts(first_key.as_ptr().cast::<u8>(), WORD * 2) };
        let equal_bytes =
            unsafe { std::slice::from_raw_parts(equal_key.as_ptr().cast::<u8>(), WORD * 2) };
        let value = 19_i64.to_ne_bytes();
        assert_eq!(unsafe { map_set_bytes(map, first_bytes, &value) }, 0);
        assert_eq!(
            unsafe { zirk_rt_map_contains_key_typed(map, equal_bytes.as_ptr(), equal_bytes.len()) },
            1
        );
        let mut output = [0_u8; WORD];
        assert_eq!(
            unsafe {
                zirk_rt_map_get_keyed(
                    map,
                    equal_bytes.as_ptr(),
                    equal_bytes.len(),
                    output.as_mut_ptr(),
                    WORD,
                )
            },
            1
        );
        assert_eq!(i64::from_ne_bytes(output), 19);
        assert_eq!(
            unsafe { zirk_rt_map_keys(map, WORD * 2, WORD, reference_offsets.as_ptr(), 1) }
                .is_null(),
            false
        );
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn canonically_equivalent_text_keys_share_one_hash_entry() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map = unsafe {
            zirk_rt_map_new_with_layout(
                std::ptr::null(),
                0,
                WORD,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                true,
                WORD,
            )
        };
        let composed = crate::string::alloc_owned("é");
        let decomposed = crate::string::alloc_owned("e\u{301}");
        assert_ne!(composed, decomposed);
        zirk_rt_map_set(map, composed as i64, 17);
        assert_eq!(zirk_rt_map_get(map, decomposed as i64), 17);
        assert_eq!(zirk_rt_map_length(map), 1);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn mutable_string_key_keeps_its_inserted_value_and_survives_source_mutation() {
        let guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let map = unsafe {
            zirk_rt_map_new_with_layout(
                std::ptr::null(),
                0,
                WORD,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                true,
                WORD,
            )
        };
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
        let map = unsafe {
            zirk_rt_map_new_with_layout(
                offsets.as_ptr(),
                1,
                WORD,
                std::ptr::null(),
                0,
                offsets.as_ptr(),
                1,
                std::ptr::null(),
                0,
                false,
                WORD,
            )
        };
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
