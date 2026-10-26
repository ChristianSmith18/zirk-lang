//! Managed storage for open, string-keyed Zirk objects and tagged values.
//!
//! Values are copied using their compiler-provided layout. Their tag keeps
//! their Zirk type identity and their reference offsets let the collector
//! trace nested objects, closures, and aggregates without scanning scalar
//! bytes that merely resemble addresses. Properties and keys retain insertion
//! order; replacing a property leaves it in place.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;
const WORD: usize = std::mem::size_of::<usize>();
const JSON_NUMBER_TAG: u32 = 0x10_0000;

#[derive(Clone)]
struct Property {
    key: Box<str>,
    value: usize,
}

struct ObjectBody {
    properties: Vec<Property>,
    sealed: bool,
}

// Class instances keep their declared fields in the compiler's fixed layout.
// Open properties therefore live in a sidecar keyed by the instance address;
// this preserves class layout and reference identity while allowing an
// `Object` view to carry additional properties safely.
static CLASS_PROPERTIES: OnceLock<Mutex<HashMap<usize, ObjectBody>>> = OnceLock::new();

fn class_properties() -> &'static Mutex<HashMap<usize, ObjectBody>> {
    CLASS_PROPERTIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_class_properties() -> std::sync::MutexGuard<'static, HashMap<usize, ObjectBody>> {
    class_properties()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn managed_descriptor(object: *mut c_void) -> Option<*mut c_void> {
    if !crate::collector::is_managed_allocation(object) {
        return None;
    }
    Some(unsafe { crate::collector::object_descriptor(object) })
}

fn with_body<R>(
    object: *mut c_void,
    create: bool,
    f: impl FnOnce(&mut ObjectBody) -> R,
) -> Option<R> {
    let descriptor = managed_descriptor(object)?;
    if descriptor == object_descriptor() {
        return Some(f(unsafe { &mut *object_body(object) }));
    }
    let mut sidecars = lock_class_properties();
    if create {
        let body = sidecars
            .entry(object as usize)
            .or_insert_with(|| ObjectBody {
                properties: Vec::new(),
                sealed: false,
            });
        Some(f(body))
    } else {
        sidecars.get_mut(&(object as usize)).map(f)
    }
}

struct DynamicValueBody {
    tag: u32,
    bytes: Box<[u8]>,
    reference_offsets: Box<[usize]>,
}

/// One statically known property requirement passed by generated contract
/// validation. Operation signatures use the same tag slot as callable values.
#[repr(C)]
pub struct ObjectRequirement {
    pub key: *const u8,
    pub key_len: usize,
    pub tag: u32,
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_object_descriptor: [usize; 4] = [0, 0, 0, 0];

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_dynamic_value_descriptor: [usize; 4] = [0, 0, 0, 0];

pub(crate) fn object_descriptor() -> *mut c_void {
    (&raw const zirk_rt_object_descriptor) as *mut c_void
}

pub(crate) fn dynamic_value_descriptor() -> *mut c_void {
    (&raw const zirk_rt_dynamic_value_descriptor) as *mut c_void
}

unsafe fn object_body(object: *mut c_void) -> *mut ObjectBody {
    unsafe { (object as *mut u8).add(BODY_OFFSET).cast() }
}

unsafe fn dynamic_body(value: *mut c_void) -> *mut DynamicValueBody {
    unsafe { (value as *mut u8).add(BODY_OFFSET).cast() }
}

fn key_text(key: *mut c_void) -> Option<Box<str>> {
    if key.is_null() {
        return None;
    }
    let text = unsafe { (*crate::string::payload_ptr(key)).as_str() };
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
}

fn key_text_bytes(key: *const u8, key_len: usize) -> Option<Box<str>> {
    if key_len != 0 && key.is_null() {
        return None;
    }
    let bytes = if key_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(key, key_len) }
    };
    let text = std::str::from_utf8(bytes).ok()?;
    Some(zirk_runtime_core::text::normalize(text, "NFC").into_boxed_str())
}

fn descriptor_implements_contract(descriptor: *mut c_void, contract: u32) -> bool {
    if descriptor.is_null() || descriptor == object_descriptor() {
        return false;
    }
    let words = descriptor.cast::<usize>();
    unsafe {
        let ancestor_count = *words.add(1);
        let contract_count_slot = 2 + ancestor_count;
        let contract_count = *words.add(contract_count_slot);
        (0..contract_count).any(|index| {
            let entry = contract_count_slot + 1 + index * 2;
            *words.add(entry) == contract as usize
        })
    }
}

#[derive(Clone, Copy)]
struct DescriptorField {
    offset: usize,
    size: usize,
    tag: u32,
    reference_offsets: *const usize,
    reference_count: usize,
}

/// Find a declared class field by its normalized UTF-8 source name. Class
/// descriptors include field layout metadata emitted by LLVM codegen.
fn descriptor_field(object: *mut c_void, key: &str) -> Option<DescriptorField> {
    let descriptor = managed_descriptor(object)?;
    if descriptor == object_descriptor() {
        return None;
    }
    let words = descriptor.cast::<usize>();
    unsafe {
        let ancestor_count = *words.add(1);
        let contract_count_slot = 2 + ancestor_count;
        let contract_count = *words.add(contract_count_slot);
        let gc_count_slot = contract_count_slot + 1 + contract_count * 2;
        let gc_count = *words.add(gc_count_slot);
        let field_count_slot = gc_count_slot + 1 + gc_count;
        let field_count = *words.add(field_count_slot);
        let mut entry = field_count_slot + 1;
        for _ in 0..field_count {
            let name_ptr = *words.add(entry) as *const u8;
            let name_len = *words.add(entry + 1);
            let offset = *words.add(entry + 2);
            let size = *words.add(entry + 3);
            let tag = *words.add(entry + 4) as u32;
            let reference_count = *words.add(entry + 5);
            let references = words.add(entry + 6);
            if name_len == key.len()
                && (name_len == 0
                    || std::slice::from_raw_parts(name_ptr, name_len) == key.as_bytes())
            {
                return Some(DescriptorField {
                    offset,
                    size,
                    tag,
                    reference_offsets: references,
                    reference_count,
                });
            }
            entry += 6 + reference_count;
        }
    }
    None
}

fn descriptor_fields(object: *mut c_void) -> Vec<(Box<str>, DescriptorField)> {
    let Some(descriptor) = managed_descriptor(object) else {
        return Vec::new();
    };
    if descriptor == object_descriptor() {
        return Vec::new();
    }
    let words = descriptor.cast::<usize>();
    let mut fields = Vec::new();
    unsafe {
        let ancestor_count = *words.add(1);
        let contract_count_slot = 2 + ancestor_count;
        let contract_count = *words.add(contract_count_slot);
        let gc_count_slot = contract_count_slot + 1 + contract_count * 2;
        let gc_count = *words.add(gc_count_slot);
        let field_count_slot = gc_count_slot + 1 + gc_count;
        let field_count = *words.add(field_count_slot);
        let mut entry = field_count_slot + 1;
        for _ in 0..field_count {
            let name_ptr = *words.add(entry) as *const u8;
            let name_len = *words.add(entry + 1);
            let name = std::str::from_utf8(std::slice::from_raw_parts(name_ptr, name_len))
                .unwrap_or_default()
                .into();
            let offset = *words.add(entry + 2);
            let size = *words.add(entry + 3);
            let tag = *words.add(entry + 4) as u32;
            let reference_count = *words.add(entry + 5);
            let references = words.add(entry + 6);
            fields.push((
                name,
                DescriptorField {
                    offset,
                    size,
                    tag,
                    reference_offsets: references,
                    reference_count,
                },
            ));
            entry += 6 + reference_count;
        }
    }
    fields
}

unsafe fn checked_offsets(
    offsets: *const usize,
    count: usize,
    size: usize,
) -> Option<Box<[usize]>> {
    if count == 0 {
        return Some(Box::new([]));
    }
    if offsets.is_null() {
        return None;
    }
    let offsets = unsafe { std::slice::from_raw_parts(offsets, count) };
    if offsets
        .iter()
        .any(|offset| offset.checked_add(WORD).is_none_or(|end| end > size))
    {
        return None;
    }
    Some(offsets.to_vec().into_boxed_slice())
}

/// Allocate an empty open object in the managed heap.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_new() -> *mut c_void {
    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<ObjectBody>(),
            std::mem::align_of::<ObjectBody>(),
        )
    };
    unsafe {
        *(object as *mut *mut c_void) = object_descriptor();
        object_body(object).write(ObjectBody {
            properties: Vec::new(),
            sealed: false,
        });
    }
    object
}

/// Copy a value with its runtime type tag into a managed box.
///
/// `reference_offsets` contains byte offsets of managed pointers within the
/// copied `bytes`. The caller must keep those source references rooted for
/// the duration of this call.
///
/// # Safety
/// `bytes` must point to `size` readable bytes, and the offset slice must be
/// readable and describe only complete pointer-sized words inside that data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_dynamic_value_new(
    tag: u32,
    bytes: *const u8,
    size: usize,
    reference_offsets: *const usize,
    reference_count: usize,
) -> *mut c_void {
    if size != 0 && bytes.is_null() {
        return std::ptr::null_mut();
    }
    let Some(reference_offsets) =
        (unsafe { checked_offsets(reference_offsets, reference_count, size) })
    else {
        return std::ptr::null_mut();
    };
    let payload = if size == 0 {
        Box::new([])
    } else {
        unsafe { std::slice::from_raw_parts(bytes, size) }
            .to_vec()
            .into_boxed_slice()
    };
    let value = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<DynamicValueBody>(),
            std::mem::align_of::<DynamicValueBody>(),
        )
    };
    unsafe {
        *(value as *mut *mut c_void) = dynamic_value_descriptor();
        dynamic_body(value).write(DynamicValueBody {
            tag,
            bytes: payload,
            reference_offsets,
        });
    }
    value
}

/// Return the static runtime type tag for a dynamic value, or zero for an
/// invalid handle.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_dynamic_value_tag(value: *mut c_void) -> u32 {
    if value.is_null() {
        return 0;
    }
    unsafe { (*dynamic_body(value)).tag }
}

/// Copy a dynamic payload to a typed destination after checking its tag.
/// Returns 0 on success, 1 on a tag mismatch, and -1 for an invalid handle,
/// output pointer, or size.
///
/// # Safety
/// `output` must be writable for `size` bytes when `size > 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_dynamic_value_copy_to(
    value: *mut c_void,
    expected_tag: u32,
    output: *mut u8,
    size: usize,
) -> i32 {
    if value.is_null() || (size != 0 && output.is_null()) {
        return -1;
    }
    let body = unsafe { &*dynamic_body(value) };
    if body.tag != expected_tag {
        return 1;
    }
    if body.bytes.len() != size {
        return -1;
    }
    if size != 0 {
        unsafe { std::ptr::copy_nonoverlapping(body.bytes.as_ptr(), output, size) };
    }
    0
}

/// Converts a tagged Zirk value to JSON text. Object properties and arrays of
/// Dynamic values are traversed recursively; unsupported values and cycles
/// return status 1 without writing a partial result.
///
/// # Safety
/// `output` must be a valid writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_json_stringify_dynamic(
    value: *mut c_void,
    output: *mut *mut c_void,
) -> i32 {
    if value.is_null() || output.is_null() {
        return -1;
    }
    let mut active = HashSet::new();
    let mut budget = 100_000usize;
    let Ok(value) = (unsafe { dynamic_json_value(value, &mut active, 0, &mut budget) }) else {
        return 1;
    };
    let Ok(document) = serde_json::to_string(&value) else {
        return 1;
    };
    let string = crate::string::alloc_owned(&document);
    if string.is_null() {
        return -1;
    }
    unsafe { output.write(string) };
    0
}

/// Parses a JSON document into managed `Object` values and `List<Dynamic>`
/// arrays. Numbers outside the ordinary integer range retain their exact JSON
/// token in a private Dynamic tag and stringify back without rounding.
///
/// # Safety
/// `document` must be a live Zirk String and `output` a writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_json_parse_dynamic(
    document: *mut c_void,
    list_dynamic_id: u32,
    output: *mut *mut c_void,
) -> i32 {
    if document.is_null() || output.is_null() {
        return -1;
    }
    let text = unsafe { (*crate::string::payload_ptr(document)).as_str() };
    if zirk_runtime_core::json::validate_document(text).is_err() {
        return 1;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return 1;
    };
    let mut budget = 100_000usize;
    let Ok(value) = (unsafe { json_value_to_dynamic(value, list_dynamic_id, 0, &mut budget) })
    else {
        return 1;
    };
    if value.is_null() {
        return -1;
    }
    unsafe { output.write(value) };
    0
}

unsafe fn json_value_to_dynamic(
    value: serde_json::Value,
    list_dynamic_id: u32,
    depth: usize,
    budget: &mut usize,
) -> Result<*mut c_void, ()> {
    if depth > zirk_runtime_core::json::MAX_NESTING_DEPTH || *budget == 0 {
        return Err(());
    }
    *budget -= 1;
    match value {
        serde_json::Value::Null => Ok(unsafe {
            zirk_rt_dynamic_value_new(0x0f_0000, [0u8].as_ptr(), 1, std::ptr::null(), 0)
        }),
        serde_json::Value::Bool(value) => {
            let payload = [u8::from(value)];
            Ok(unsafe {
                zirk_rt_dynamic_value_new(0x301, payload.as_ptr(), 1, std::ptr::null(), 0)
            })
        }
        serde_json::Value::String(value) => {
            let handle = crate::string::alloc_owned(&value);
            let offsets = [0usize];
            Ok(unsafe {
                zirk_rt_dynamic_value_new(
                    0x302,
                    (&handle as *const *mut c_void).cast(),
                    WORD,
                    offsets.as_ptr(),
                    1,
                )
            })
        }
        serde_json::Value::Number(number) => {
            let token = number.to_string();
            let bytes = token.as_bytes();
            Ok(unsafe {
                zirk_rt_dynamic_value_new(
                    JSON_NUMBER_TAG,
                    bytes.as_ptr(),
                    bytes.len(),
                    std::ptr::null(),
                    0,
                )
            })
        }
        serde_json::Value::Array(values) => {
            let offsets = [0usize];
            let list = unsafe {
                crate::list::zirk_rt_list_new_with_layout(WORD, WORD, offsets.as_ptr(), 1)
            };
            for value in values {
                let item =
                    unsafe { json_value_to_dynamic(value, list_dynamic_id, depth + 1, budget) }?;
                unsafe {
                    crate::list::zirk_rt_list_add(
                        list,
                        (&item as *const *mut c_void).cast(),
                        WORD,
                        WORD,
                        true,
                    )
                };
            }
            let tag = 0x9_0000u32.checked_add(list_dynamic_id).ok_or(())?;
            let offsets = [0usize];
            Ok(unsafe {
                zirk_rt_dynamic_value_new(
                    tag,
                    (&list as *const *mut c_void).cast(),
                    WORD,
                    offsets.as_ptr(),
                    1,
                )
            })
        }
        serde_json::Value::Object(values) => {
            let object = zirk_rt_object_new();
            for (key_text, value) in values {
                let key = crate::string::alloc_owned(&key_text);
                let item =
                    unsafe { json_value_to_dynamic(value, list_dynamic_id, depth + 1, budget) }?;
                if zirk_rt_object_set(object, key, item) != 0 {
                    return Err(());
                }
            }
            let offsets = [0usize];
            Ok(unsafe {
                zirk_rt_dynamic_value_new(
                    0x3_0000,
                    (&object as *const *mut c_void).cast(),
                    WORD,
                    offsets.as_ptr(),
                    1,
                )
            })
        }
    }
}

unsafe fn dynamic_json_value(
    value: *mut c_void,
    active: &mut HashSet<usize>,
    depth: usize,
    budget: &mut usize,
) -> Result<serde_json::Value, ()> {
    if value.is_null()
        || !crate::collector::is_managed_allocation(value)
        || depth > 256
        || *budget == 0
    {
        return Err(());
    }
    *budget -= 1;
    let body = unsafe { &*dynamic_body(value) };
    let bytes = body.bytes.as_ref();
    let tag = body.tag;
    match tag {
        // `IntWidth`'s stable IR discriminants: I8..I128, U8..U128.
        0x100..=0x109 => {
            let kind = (tag - 0x100) as usize;
            let bits = match kind % 5 {
                0 => 8,
                1 => 16,
                2 => 32,
                3 => 64,
                _ => 128,
            };
            let width = bits / 8;
            if bytes.len() != width {
                return Err(());
            }
            let mut raw = [0u8; 16];
            raw[..width].copy_from_slice(bytes);
            if kind < 5 && bytes[width - 1] & 0x80 != 0 {
                raw[width..].fill(0xff);
            }
            let number = i128::from_ne_bytes(raw);
            let text = if kind < 5 {
                number.to_string()
            } else {
                u128::from_ne_bytes(raw).to_string()
            };
            serde_json::from_str(&text).map_err(|_| ())
        }
        0x200..=0x203 => {
            let number = match tag - 0x200 {
                0 if bytes.len() == 2 => {
                    f16_to_f32(u16::from_ne_bytes(bytes.try_into().map_err(|_| ())?)) as f64
                }
                1 if bytes.len() == 4 => {
                    f32::from_ne_bytes(bytes.try_into().map_err(|_| ())?) as f64
                }
                2 if bytes.len() == 8 => f64::from_ne_bytes(bytes.try_into().map_err(|_| ())?),
                // Float128 cannot be represented by serde_json's native number
                // model without changing its token, so reject it explicitly.
                _ => return Err(()),
            };
            serde_json::Number::from_f64(number)
                .map(serde_json::Value::Number)
                .ok_or(())
        }
        0x301 => {
            let Some(&value) = bytes.first() else {
                return Err(());
            };
            Ok(serde_json::Value::Bool(value != 0))
        }
        JSON_NUMBER_TAG => {
            let token = std::str::from_utf8(bytes).map_err(|_| ())?;
            serde_json::from_str(token).map_err(|_| ())
        }
        0x302 | 0x303 => {
            let handle = read_payload_pointer(bytes)?;
            if handle.is_null() {
                return Err(());
            }
            let text = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
            Ok(serde_json::Value::String(text.to_owned()))
        }
        // Nullable<…> uses the leading presence flag. The all-zero inner tag
        // is the canonical nullable-Object representation for `null`.
        0x0f_0000 if bytes.first().copied() == Some(0) => Ok(serde_json::Value::Null),
        0x1_0000..=0x3_ffff => {
            let object = read_payload_pointer(bytes)?;
            unsafe { object_json_value(object, active, depth + 1, budget) }
        }
        // JSON parsing materializes arrays as List<Dynamic>; each element is
        // therefore itself a managed Dynamic box.
        0x9_0000..=0x9_ffff => {
            let list = read_payload_pointer(bytes)?;
            if list.is_null() || !active.insert(list as usize) {
                return Err(());
            }
            let length = unsafe { crate::list::zirk_rt_list_length(list) };
            if length > *budget {
                active.remove(&(list as usize));
                return Err(());
            }
            let mut items = Vec::with_capacity(length);
            for index in 0..length {
                let slot = unsafe { crate::list::zirk_rt_list_element(list, index as i64, WORD) };
                let child = unsafe { (slot as *const *mut c_void).read_unaligned() };
                items.push(unsafe { dynamic_json_value(child, active, depth + 1, budget) }?);
            }
            active.remove(&(list as usize));
            Ok(serde_json::Value::Array(items))
        }
        _ => Err(()),
    }
}

unsafe fn object_json_value(
    object: *mut c_void,
    active: &mut HashSet<usize>,
    depth: usize,
    budget: &mut usize,
) -> Result<serde_json::Value, ()> {
    if object.is_null() || !active.insert(object as usize) {
        return Err(());
    }
    let length = zirk_rt_object_length(object);
    if length > *budget {
        active.remove(&(object as usize));
        return Err(());
    }
    let mut map = serde_json::Map::new();
    for index in 0..length {
        let key = zirk_rt_object_key_at(object, index);
        if key.is_null() {
            active.remove(&(object as usize));
            return Err(());
        }
        let key_text = unsafe { (*crate::string::payload_ptr(key)).as_str() }.to_owned();
        let mut child = std::ptr::null_mut();
        if unsafe { zirk_rt_object_get(object, key, &mut child) } != 1 {
            active.remove(&(object as usize));
            return Err(());
        }
        let value = unsafe { dynamic_json_value(child, active, depth + 1, budget) }?;
        map.insert(key_text, value);
    }
    active.remove(&(object as usize));
    Ok(serde_json::Value::Object(map))
}

fn read_payload_pointer(bytes: &[u8]) -> Result<*mut c_void, ()> {
    if bytes.len() < WORD {
        return Err(());
    }
    Ok(unsafe { bytes.as_ptr().cast::<*mut c_void>().read_unaligned() })
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = u32::from(bits & 0x03ff);
    let result = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let shift = fraction.leading_zeros() - 22;
            let normalized = (fraction << (shift + 1)) & 0x03ff;
            sign | ((127 - 15 - shift) << 23) | (normalized << 13)
        }
        0x1f => sign | 0x7f80_0000 | (fraction << 13),
        _ => sign | ((u32::from(exponent) + 112) << 23) | (fraction << 13),
    };
    f32::from_bits(result)
}

/// Return the number of own properties, or zero for a null handle.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_length(object: *mut c_void) -> usize {
    let declared = descriptor_fields(object).len();
    declared + with_body(object, false, |body| body.properties.len()).unwrap_or(0)
}

/// Return the key at an insertion-order index as a managed Zirk `String`, or
/// null when the index is out of range. The caller must keep `object` rooted
/// while this function allocates the returned string.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_key_at(object: *mut c_void, index: usize) -> *mut c_void {
    let declared = descriptor_fields(object);
    if let Some((key, _)) = declared.get(index) {
        return crate::string::alloc_owned(key);
    }
    let dynamic_index = index.saturating_sub(declared.len());
    let Some(Some(key)) = with_body(object, false, |body| {
        body.properties
            .get(dynamic_index)
            .map(|property| property.key.to_string())
    }) else {
        return std::ptr::null_mut();
    };
    crate::string::alloc_owned(&key)
}

/// Set a property to a tagged value handle. Returns 0 on success, 1 when
/// sealing prevents insertion, and -1 for a null/invalid handle. Existing
/// values can be replaced after sealing; language-level mutability is
/// enforced by the checker.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_set(
    object: *mut c_void,
    key: *mut c_void,
    value: *mut c_void,
) -> i32 {
    let Some(key) = key_text(key) else {
        return -1;
    };
    if value.is_null() {
        return -1;
    }
    if let Some(field) = descriptor_field(object, &key) {
        let dynamic = unsafe { &*dynamic_body(value) };
        if dynamic.tag != field.tag || dynamic.bytes.len() != field.size {
            return 2;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                dynamic.bytes.as_ptr(),
                (object as *mut u8).add(field.offset),
                field.size,
            );
        }
        return 0;
    }
    with_body(object, true, |body| {
        if let Some(property) = body
            .properties
            .iter_mut()
            .find(|entry| entry.key.as_ref() == key.as_ref())
        {
            property.value = value as usize;
            return 0;
        }
        if body.sealed {
            return 1;
        }
        body.properties.push(Property {
            key,
            value: value as usize,
        });
        0
    })
    .unwrap_or(-1)
}

/// Read a property into an output handle. Returns 1 when present, 0 when
/// absent, and -1 for invalid pointers. Presence is separate from the value.
///
/// # Safety
/// `value_out` must be a valid writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_object_get(
    object: *mut c_void,
    key: *mut c_void,
    value_out: *mut *mut c_void,
) -> i32 {
    if value_out.is_null() {
        return -1;
    }
    let Some(key) = key_text(key) else {
        return -1;
    };
    if let Some(field) = descriptor_field(object, &key) {
        let dynamic = unsafe {
            zirk_rt_dynamic_value_new(
                field.tag,
                (object as *const u8).add(field.offset),
                field.size,
                field.reference_offsets,
                field.reference_count,
            )
        };
        if dynamic.is_null() {
            return -1;
        }
        unsafe { value_out.write(dynamic) };
        return 1;
    }
    with_body(object, false, |body| {
        let Some(property) = body.properties.iter().find(|entry| entry.key == key) else {
            return 0;
        };
        unsafe { value_out.write(property.value as *mut c_void) };
        1
    })
    .unwrap_or(0)
}

/// Reads an object property using a UTF-8 key known to generated code.
/// Returns 1 when present, 0 when absent, and -1 for invalid pointers.
///
/// # Safety
/// `key` must reference `key_len` readable bytes and `value_out` a writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_object_get_bytes(
    object: *mut c_void,
    key: *const u8,
    key_len: usize,
    value_out: *mut *mut c_void,
) -> i32 {
    if value_out.is_null() {
        return -1;
    }
    let Some(key) = key_text_bytes(key, key_len) else {
        return -1;
    };
    if let Some(field) = descriptor_field(object, &key) {
        let dynamic = unsafe {
            zirk_rt_dynamic_value_new(
                field.tag,
                (object as *const u8).add(field.offset),
                field.size,
                field.reference_offsets,
                field.reference_count,
            )
        };
        if dynamic.is_null() {
            return -1;
        }
        unsafe { value_out.write(dynamic) };
        return 1;
    }
    with_body(object, false, |body| {
        let Some(property) = body.properties.iter().find(|entry| entry.key == key) else {
            return 0;
        };
        unsafe { value_out.write(property.value as *mut c_void) };
        1
    })
    .unwrap_or(0)
}

/// Returns whether a managed value is an open dynamic Object allocation.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_is_open_object(object: *mut c_void) -> i32 {
    (managed_descriptor(object) == Some(object_descriptor())) as i32
}

/// Remove a property. Returns 1 if removed, 0 if absent, 2 if sealed, and -1
/// for a null object or key.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_remove(object: *mut c_void, key: *mut c_void) -> i32 {
    let Some(key) = key_text(key) else {
        return -1;
    };
    if descriptor_field(object, &key).is_some() {
        return 2;
    }
    with_body(object, false, |body| {
        if body.sealed {
            return 2;
        }
        let Some(index) = body.properties.iter().position(|entry| entry.key == key) else {
            return 0;
        };
        body.properties.remove(index);
        1
    })
    .unwrap_or(-1)
}

/// Seal the object's current key set. Repeated calls are idempotent.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_seal(object: *mut c_void) -> i32 {
    with_body(object, true, |body| body.sealed = true)
        .map(|()| 0)
        .unwrap_or(-1)
}

/// Validate required property tags and seal the current key set atomically.
/// Returns 0 on success, 1 if a property is absent, 2 if its tag differs, and
/// -1 for invalid pointers. `failed_index` receives the first failing
/// requirement index when non-null. Extra properties are retained.
///
/// # Safety
/// `requirements` must point to `count` readable `ObjectRequirement`s and
/// `failed_index`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_object_conform(
    object: *mut c_void,
    contract: u32,
    requirements: *const ObjectRequirement,
    count: usize,
    failed_index: *mut usize,
) -> i32 {
    if count != 0 && requirements.is_null() {
        return -1;
    }
    let descriptor = managed_descriptor(object);
    if descriptor.is_some_and(|descriptor| descriptor_implements_contract(descriptor, contract)) {
        return with_body(object, true, |body| {
            body.sealed = true;
            0
        })
        .unwrap_or(-1);
    }
    with_body(object, true, |body| {
        for index in 0..count {
            let requirement = unsafe { &*requirements.add(index) };
            let Some(key) = key_text_bytes(requirement.key, requirement.key_len) else {
                return -1;
            };
            let dynamic = body.properties.iter().find(|entry| entry.key == key);
            let declared = descriptor_field(object, &key);
            if dynamic.is_none() && declared.is_none() {
                if !failed_index.is_null() {
                    unsafe { failed_index.write(index) };
                }
                return 1;
            }
            let actual_tag = dynamic
                .map(|property| zirk_rt_dynamic_value_tag(property.value as *mut c_void))
                .or_else(|| declared.map(|field| field.tag));
            let tag_matches = match (requirement.tag, actual_tag) {
                (0, _) => true,
                (0x6_0000, Some(tag)) => tag & 0xffff_0000 == 0x6_0000,
                (expected, actual) => actual == Some(expected),
            };
            if !tag_matches {
                if !failed_index.is_null() {
                    unsafe { failed_index.write(index) };
                }
                return 2;
            }
        }
        body.sealed = true;
        0
    })
    .unwrap_or(-1)
}

/// Extract an object reference from a tagged Dynamic box. Returns zero and
/// writes the pointer on success, one for a non-object value, and minus one
/// for an invalid box or output pointer.
///
/// # Safety
/// `object_out` must be a valid writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_dynamic_object_unbox(
    value: *mut c_void,
    object_out: *mut *mut c_void,
) -> i32 {
    if value.is_null() || object_out.is_null() {
        return -1;
    }
    let body = unsafe { &*dynamic_body(value) };
    let object_tag = (0x1_0000..0x2_0000).contains(&body.tag) || body.tag == 0x3_0000;
    if !object_tag || body.bytes.len() != WORD {
        return 1;
    }
    let object = unsafe { body.bytes.as_ptr().cast::<*mut c_void>().read_unaligned() };
    if object.is_null() {
        return 1;
    }
    unsafe { object_out.write(object) };
    0
}

/// Called by the collector after validating the owner allocation.
/// Create a shallow copy preserving property order, tags, and nested identity.
/// The new object's key set is open even when the source had been sealed.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_spread(object: *mut c_void) -> *mut c_void {
    let Some(properties) = with_body(object, true, |body| body.properties.clone()) else {
        return std::ptr::null_mut();
    };
    let copied = zirk_rt_object_new();
    let destination = unsafe { &mut *object_body(copied) };
    destination.properties = properties;
    copied
}

/// Create a shallow object-rest copy, omitting the given own string keys.
///
/// # Safety
/// `excluded_keys` must point to `count` readable managed String handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_object_rest(
    object: *mut c_void,
    excluded_keys: *const *mut c_void,
    count: usize,
) -> *mut c_void {
    if count != 0 && excluded_keys.is_null() {
        return std::ptr::null_mut();
    }
    let Some(properties) = with_body(object, true, |body| body.properties.clone()) else {
        return std::ptr::null_mut();
    };
    let mut excluded = Vec::with_capacity(count);
    for index in 0..count {
        let key = unsafe { excluded_keys.add(index).read() };
        let Some(key) = key_text(key) else {
            return std::ptr::null_mut();
        };
        excluded.push(key);
    }
    let copied = zirk_rt_object_new();
    let destination = unsafe { &mut *object_body(copied) };
    destination.properties.extend(
        properties
            .iter()
            .filter(|property| !excluded.contains(&property.key))
            .map(|property| Property {
                key: property.key.clone(),
                value: property.value,
            }),
    );
    copied
}

/// Merge an object's own properties into another object using object-literal
/// source order. Existing destination keys retain their position while their
/// value is replaced; new source keys append in source order.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_object_merge(destination: *mut c_void, source: *mut c_void) -> i32 {
    let Some(source_properties) = with_body(source, false, |body| body.properties.clone()) else {
        return -1;
    };
    with_body(destination, true, |destination| {
        for property in source_properties {
            if let Some(existing) = destination
                .properties
                .iter_mut()
                .find(|existing| existing.key.as_ref() == property.key.as_ref())
            {
                existing.value = property.value;
            } else if destination.sealed {
                return 1;
            } else {
                destination.properties.push(property);
            }
        }
        0
    })
    .unwrap_or(-1)
}

pub(crate) unsafe fn mark_properties(
    object: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let descriptor = unsafe { crate::collector::object_descriptor(object) };
    let properties = if descriptor == object_descriptor() {
        unsafe { &*object_body(object) }.properties.clone()
    } else {
        lock_class_properties()
            .get(&(object as usize))
            .map(|body| body.properties.clone())
            .unwrap_or_default()
    };
    for property in properties {
        let candidate = property.value as *mut c_void;
        if allocations.contains(&(candidate as usize)) {
            mark(candidate);
        }
    }
}

/// Release the sidecar associated with any swept class instance.
pub(crate) fn owner_swept(object: *mut c_void) {
    lock_class_properties().remove(&(object as usize));
}

#[cfg(test)]
pub(crate) fn reset_class_properties_for_tests() {
    lock_class_properties().clear();
}

/// Called by the collector after validating a dynamic-value allocation.
pub(crate) unsafe fn mark_dynamic_value(
    value: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*dynamic_body(value) };
    for &offset in &body.reference_offsets {
        let candidate = unsafe {
            body.bytes
                .as_ptr()
                .add(offset)
                .cast::<*mut c_void>()
                .read_unaligned()
        };
        if allocations.contains(&(candidate as usize)) {
            mark(candidate);
        }
    }
}

pub(crate) unsafe fn drop_object_body(object: *mut c_void) {
    unsafe { std::ptr::drop_in_place(object_body(object)) };
}

pub(crate) unsafe fn drop_dynamic_value_body(value: *mut c_void) {
    unsafe { std::ptr::drop_in_place(dynamic_body(value)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> *mut c_void {
        crate::string::alloc_owned(text)
    }

    fn boxed_i64(tag: u32, value: i64) -> *mut c_void {
        unsafe {
            zirk_rt_dynamic_value_new(
                tag,
                (&value as *const i64).cast(),
                std::mem::size_of::<i64>(),
                std::ptr::null(),
                0,
            )
        }
    }

    fn boxed_pointer(tag: u32, value: *mut c_void) -> *mut c_void {
        unsafe {
            zirk_rt_dynamic_value_new(
                tag,
                (&value as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        }
    }

    #[test]
    fn dynamic_json_rejects_unmanaged_misaligned_handles_before_dereferencing() {
        let _guard = crate::collector::test_support::test_guard();
        let invalid = 0x1004usize as *mut c_void;
        let mut output = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_stringify_dynamic(invalid, &mut output) },
            1
        );
        assert!(output.is_null());
    }

    #[test]
    fn json_stringification_serializes_dynamic_object_properties() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let key = key("status");
        let string = crate::string::alloc_owned("ok");
        let boxed = boxed_pointer(0x302, string);
        assert_eq!(zirk_rt_object_set(object, key, boxed), 0);
        let dynamic_object = boxed_pointer(0x3_0000, object);
        let mut output = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_stringify_dynamic(dynamic_object, &mut output) },
            0
        );
        assert_eq!(
            unsafe { (*crate::string::payload_ptr(output)).as_str() },
            r#"{"status":"ok"}"#
        );
    }

    #[test]
    fn json_stringification_rejects_cycles_without_returning_partial_text() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let key = key("self");
        let boxed = boxed_pointer(0x3_0000, object);
        assert_eq!(zirk_rt_object_set(object, key, boxed), 0);
        let dynamic_object = boxed_pointer(0x3_0000, object);
        let mut output = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_stringify_dynamic(dynamic_object, &mut output) },
            1
        );
        assert!(output.is_null());
    }

    #[test]
    fn json_parse_dynamic_round_trips_nested_values_and_exact_numbers() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let document = crate::string::alloc_owned(
            r#"{"z":null,"name":"雪🙂","items":[true,-0.0,123456789012345678901234567890123456789]}"#,
        );
        let mut parsed = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_parse_dynamic(document, 0, &mut parsed) },
            0
        );
        assert!(!parsed.is_null());

        let mut output = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_stringify_dynamic(parsed, &mut output) },
            0
        );
        assert_eq!(
            unsafe { (*crate::string::payload_ptr(output)).as_str() },
            r#"{"z":null,"name":"雪🙂","items":[true,-0.0,123456789012345678901234567890123456789]}"#
        );
    }

    #[test]
    fn json_parse_dynamic_rejects_invalid_documents_without_partial_output() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let document = crate::string::alloc_owned(r#"{"status": }"#);
        let mut parsed = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_json_parse_dynamic(document, 0, &mut parsed) },
            1
        );
        assert!(parsed.is_null());
    }

    #[test]
    fn json_parse_dynamic_rejects_duplicate_keys_and_excessive_depth() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let documents = [
            r#"{"key":1,"key":2}"#.to_owned(),
            r#"{"outer":{"key":1,"\u006bey":2}}"#.to_owned(),
            format!(
                "{}0{}",
                "[".repeat(zirk_runtime_core::json::MAX_NESTING_DEPTH + 1),
                "]".repeat(zirk_runtime_core::json::MAX_NESTING_DEPTH + 1)
            ),
        ];
        for text in documents {
            let document = crate::string::alloc_owned(&text);
            let mut parsed = std::ptr::null_mut();
            assert_eq!(
                unsafe { zirk_rt_json_parse_dynamic(document, 0, &mut parsed) },
                1
            );
            assert!(parsed.is_null());
        }
    }

    fn requirement(key: &'static str, tag: u32) -> ObjectRequirement {
        ObjectRequirement {
            key: key.as_ptr(),
            key_len: key.len(),
            tag,
        }
    }

    #[test]
    fn class_instances_keep_dynamic_properties_in_a_traced_sidecar() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let instance =
            unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, WORD) };
        unsafe { (instance as *mut *mut c_void).write(descriptor.as_ptr() as *mut c_void) };

        let property_key = key("dynamic");
        let property = boxed_i64(7, 42);
        assert_eq!(zirk_rt_object_set(instance, property_key, property), 0);
        assert_eq!(zirk_rt_object_length(instance), 1);
        let mut found = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_object_get(instance, property_key, &mut found) },
            1
        );
        assert_eq!(found, property);

        let mut marked = Vec::new();
        let mut allocations = crate::collector::AllocationSet::default();
        allocations.insert(property as usize);
        unsafe {
            mark_properties(instance, &allocations, &mut |value| marked.push(value));
        }
        assert_eq!(marked, [property]);
        owner_swept(instance);
        assert_eq!(zirk_rt_object_length(instance), 0);
    }

    #[test]
    fn class_sidecar_keeps_nested_managed_values_alive_only_with_its_owner() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let owner = unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, WORD) };
        unsafe { (owner as *mut *mut c_void).write(descriptor.as_ptr() as *mut c_void) };
        let child = crate::string::alloc_owned("nested sidecar value");
        let boxed = unsafe {
            zirk_rt_dynamic_value_new(
                7,
                (&child as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        };
        assert_eq!(zirk_rt_object_set(owner, key("child"), boxed), 0);

        let mut rooted_owner = owner;
        let mut roots = [(&mut rooted_owner as *mut *mut c_void).cast()];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::test_support::collect_for_tests();
        assert!(crate::collector::test_support::is_live_for_tests(boxed));
        assert!(crate::collector::test_support::is_live_for_tests(child));
        crate::collector::zirk_rt_pop_frame();

        crate::collector::test_support::collect_for_tests();
        assert!(!crate::collector::test_support::is_live_for_tests(owner));
        assert!(!crate::collector::test_support::is_live_for_tests(boxed));
        assert!(!crate::collector::test_support::is_live_for_tests(child));
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn class_sidecar_contract_validation_seals_keys_for_every_alias() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let instance =
            unsafe { crate::memory::zirk_rt_alloc(crate::collector::HEADER_BYTES, WORD) };
        unsafe { (instance as *mut *mut c_void).write(descriptor.as_ptr() as *mut c_void) };
        let alias = instance;
        let name = key("name");
        assert_eq!(zirk_rt_object_set(instance, name, boxed_i64(7, 42)), 0);
        let requirements = [requirement("name", 7)];
        let mut failed_index = usize::MAX;
        assert_eq!(
            unsafe {
                zirk_rt_object_conform(
                    alias,
                    0,
                    requirements.as_ptr(),
                    requirements.len(),
                    &mut failed_index,
                )
            },
            0
        );
        assert_eq!(
            zirk_rt_object_set(instance, key("extra"), boxed_i64(7, 1)),
            1
        );
        assert_eq!(zirk_rt_object_length(alias), 1);
        crate::collector::test_support::reset_state_for_tests();
    }

    fn i64_payload(value: *mut c_void, tag: u32) -> i64 {
        let mut result = 0_i64;
        assert_eq!(
            unsafe {
                zirk_rt_dynamic_value_copy_to(
                    value,
                    tag,
                    (&mut result as *mut i64).cast(),
                    std::mem::size_of::<i64>(),
                )
            },
            0
        );
        result
    }

    #[test]
    fn preserves_insertion_order_and_replacement_position() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let a = key("a");
        let b = key("b");
        assert_eq!(zirk_rt_object_set(object, a, boxed_i64(1, 10)), 0);
        assert_eq!(zirk_rt_object_set(object, b, boxed_i64(2, 20)), 0);
        assert_eq!(zirk_rt_object_set(object, a, boxed_i64(3, 30)), 0);
        assert_eq!(zirk_rt_object_length(object), 2);
        let body = unsafe { &*object_body(object) };
        assert_eq!(
            body.properties
                .iter()
                .map(|p| p.key.as_ref())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        let first_value = body.properties[0].value as *mut c_void;
        assert_eq!(zirk_rt_dynamic_value_tag(first_value), 3);
        assert_eq!(i64_payload(first_value, 3), 30);
        let first_key = zirk_rt_object_key_at(object, 0);
        assert_eq!(
            unsafe { (*crate::string::payload_ptr(first_key)).as_str() },
            "a"
        );
        assert!(zirk_rt_object_key_at(object, 2).is_null());
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn lookup_distinguishes_missing_from_zero_and_sealing_blocks_shape_changes() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let a = key("a");
        let missing = key("missing");
        assert_eq!(zirk_rt_object_set(object, a, boxed_i64(7, 0)), 0);
        let mut value = std::ptr::null_mut();
        assert_eq!(unsafe { zirk_rt_object_get(object, a, &mut value) }, 1);
        assert_eq!(zirk_rt_dynamic_value_tag(value), 7);
        assert_eq!(i64_payload(value, 7), 0);
        assert_eq!(
            unsafe { zirk_rt_object_get(object, missing, &mut value) },
            0
        );
        assert_eq!(zirk_rt_object_remove(object, missing), 0);
        assert_eq!(zirk_rt_object_seal(object), 0);
        assert_eq!(zirk_rt_object_set(object, missing, boxed_i64(1, 1)), 1);
        assert_eq!(zirk_rt_object_set(object, a, boxed_i64(8, 9)), 0);
        assert_eq!(zirk_rt_object_remove(object, a), 2);
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn dynamic_values_preserve_wide_payload_bits_and_require_checked_tags() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let number = 0x1234_5678_9abc_def0_1357_9bdf_2468_ace0_u128;
        let value = unsafe {
            zirk_rt_dynamic_value_new(
                900,
                (&number as *const u128).cast(),
                std::mem::size_of::<u128>(),
                std::ptr::null(),
                0,
            )
        };
        let mut decoded = 0_u128;
        assert_eq!(
            unsafe {
                zirk_rt_dynamic_value_copy_to(
                    value,
                    900,
                    (&mut decoded as *mut u128).cast(),
                    std::mem::size_of::<u128>(),
                )
            },
            0
        );
        assert_eq!(decoded, number);
        assert_eq!(
            unsafe {
                zirk_rt_dynamic_value_copy_to(
                    value,
                    901,
                    (&mut decoded as *mut u128).cast(),
                    std::mem::size_of::<u128>(),
                )
            },
            1
        );
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn dynamic_object_unbox_checks_reference_tags_and_preserves_identity() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let boxed = unsafe {
            zirk_rt_dynamic_value_new(
                0x3_0000,
                (&object as *const *mut c_void).cast(),
                WORD,
                std::ptr::null(),
                0,
            )
        };
        let mut unboxed = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_dynamic_object_unbox(boxed, &mut unboxed) },
            0
        );
        assert_eq!(unboxed, object);

        let scalar = boxed_i64(0x107, 4);
        assert_eq!(
            unsafe { zirk_rt_dynamic_object_unbox(scalar, &mut unboxed) },
            1
        );
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn object_spread_and_rest_are_shallow_ordered_copies() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let source = zirk_rt_object_new();
        let nested = zirk_rt_object_new();
        let shared = unsafe {
            zirk_rt_dynamic_value_new(
                7,
                (&nested as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        };
        let first = boxed_i64(1, 10);
        assert_eq!(zirk_rt_object_set(source, key("first"), first), 0);
        assert_eq!(zirk_rt_object_set(source, key("nested"), shared), 0);
        assert_eq!(zirk_rt_object_set(source, key("last"), boxed_i64(2, 30)), 0);
        let self_reference = unsafe {
            zirk_rt_dynamic_value_new(
                7,
                (&source as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        };
        assert_eq!(zirk_rt_object_set(source, key("self"), self_reference), 0);

        let spread = zirk_rt_object_spread(source);
        assert_ne!(spread, source);
        assert_eq!(zirk_rt_object_length(spread), 4);
        assert_eq!(
            unsafe { &*object_body(spread) }.properties[0].value,
            first as usize
        );
        assert_eq!(
            unsafe { &*object_body(spread) }.properties[1].value,
            shared as usize
        );
        assert_eq!(
            unsafe { &*object_body(spread) }.properties[3].value,
            self_reference as usize
        );
        assert_eq!(
            unsafe { &*object_body(spread) }
                .properties
                .iter()
                .map(|property| property.key.as_ref())
                .collect::<Vec<_>>(),
            ["first", "nested", "last", "self"]
        );
        assert_eq!(
            zirk_rt_object_set(spread, key("first"), boxed_i64(1, 99)),
            0
        );
        assert_eq!(
            unsafe { &*object_body(spread) }.properties[0].key.as_ref(),
            "first"
        );
        assert_eq!(zirk_rt_object_length(source), 4);

        let excluded = [key("first"), key("last")];
        let rest = unsafe { zirk_rt_object_rest(source, excluded.as_ptr(), excluded.len()) };
        assert_eq!(zirk_rt_object_length(rest), 2);
        assert_eq!(
            unsafe { &*object_body(rest) }.properties[0].key.as_ref(),
            "nested"
        );
        assert_eq!(
            unsafe { &*object_body(rest) }.properties[0].value,
            shared as usize
        );
        assert_eq!(
            unsafe { &*object_body(rest) }.properties[1].key.as_ref(),
            "self"
        );
        assert_eq!(
            unsafe { &*object_body(rest) }.properties[1].value,
            self_reference as usize
        );

        assert_eq!(zirk_rt_object_set(spread, key("new"), boxed_i64(3, 1)), 0);
        assert_eq!(zirk_rt_object_length(source), 4);
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn collector_traces_nested_values_and_closures_in_properties() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let nested = zirk_rt_object_new();
        let leaf = crate::string::alloc_owned("only reachable through nested properties");
        let leaf_value = unsafe {
            zirk_rt_dynamic_value_new(
                43,
                (&leaf as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        };
        let nested_key = key("leaf");
        assert_eq!(zirk_rt_object_set(nested, nested_key, leaf_value), 0);

        let closure_descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let closure = unsafe {
            crate::collector::synthetic_object_for_tests(closure_descriptor.as_ptr().cast(), 0)
        };
        let closure_value = unsafe {
            zirk_rt_dynamic_value_new(
                44,
                (&closure as *const *mut c_void).cast(),
                WORD,
                [0usize].as_ptr(),
                1,
            )
        };
        assert_eq!(
            zirk_rt_object_set(object, key("child"), unsafe {
                zirk_rt_dynamic_value_new(
                    42,
                    (&nested as *const *mut c_void).cast(),
                    WORD,
                    [0usize].as_ptr(),
                    1,
                )
            }),
            0
        );
        assert_eq!(zirk_rt_object_set(object, key("handler"), closure_value), 0);

        let mut root = object;
        let mut roots = [&mut root as *mut *mut c_void as *mut c_void];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::test_support::collect_for_tests();
        for child in [object, nested, leaf, leaf_value, closure, closure_value] {
            assert!(crate::collector::test_support::is_live_for_tests(child));
        }
        crate::collector::zirk_rt_pop_frame();
        crate::collector::test_support::collect_for_tests();
        for child in [object, nested, leaf, leaf_value, closure, closure_value] {
            assert!(!crate::collector::test_support::is_live_for_tests(child));
        }
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn collector_does_not_trace_scalar_payloads_that_resemble_pointers() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let child = crate::string::alloc_owned("not a managed edge");
        let scalar = child as usize as i64;
        assert_eq!(
            zirk_rt_object_set(object, key("integer"), boxed_i64(3, scalar)),
            0
        );

        let mut root = object;
        let mut roots = [&mut root as *mut *mut c_void as *mut c_void];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        crate::collector::test_support::collect_for_tests();
        assert!(crate::collector::test_support::is_live_for_tests(object));
        assert!(!crate::collector::test_support::is_live_for_tests(child));
        crate::collector::zirk_rt_pop_frame();
        crate::collector::test_support::reset_state_for_tests();
    }

    #[test]
    fn contract_validation_is_atomic_and_seals_shared_object() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::test_support::reset_state_for_tests();
        crate::collector::test_support::set_threshold_for_tests(usize::MAX / 2);
        let object = zirk_rt_object_new();
        let alias = object;
        assert_eq!(zirk_rt_object_set(object, key("name"), boxed_i64(7, 11)), 0);
        let requirements = [requirement("name", 7), requirement("active", 8)];
        let mut failed_index = usize::MAX;
        assert_eq!(
            unsafe {
                zirk_rt_object_conform(
                    alias,
                    0,
                    requirements.as_ptr(),
                    requirements.len(),
                    &mut failed_index,
                )
            },
            1
        );
        assert_eq!(failed_index, 1);
        assert_eq!(
            zirk_rt_object_set(object, key("active"), boxed_i64(9, 0)),
            0
        );
        assert_eq!(
            unsafe {
                zirk_rt_object_conform(
                    object,
                    0,
                    requirements.as_ptr(),
                    requirements.len(),
                    &mut failed_index,
                )
            },
            2
        );
        assert_eq!(failed_index, 1);
        assert_eq!(
            zirk_rt_object_set(object, key("active"), boxed_i64(8, 1)),
            0
        );
        assert_eq!(
            unsafe {
                zirk_rt_object_conform(
                    object,
                    0,
                    requirements.as_ptr(),
                    requirements.len(),
                    &mut failed_index,
                )
            },
            0
        );
        assert_eq!(zirk_rt_object_length(alias), 2);
        assert_eq!(zirk_rt_object_set(object, key("extra"), boxed_i64(1, 1)), 1);
        assert_eq!(zirk_rt_object_length(alias), 2);
        assert_eq!(
            unsafe {
                zirk_rt_object_conform(
                    alias,
                    0,
                    requirements.as_ptr(),
                    requirements.len(),
                    &mut failed_index,
                )
            },
            0
        );
        crate::collector::test_support::reset_state_for_tests();
    }
}
