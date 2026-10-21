//! Compiler-owned ABI leaves for the public bundled `std.http` API.

use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime};

use zeroize::Zeroizing;

use crate::http_runtime::HttpResponseValues;
use crate::task::TaskId;
use sha2::{Digest, Sha256};
use zirk_runtime_core::http_cache::{CachedHttpResponse, HttpResponseCache};
use zirk_runtime_core::http_values::{Headers, HttpMethod, HttpRequestValues, HttpStatusCode, Url};

static HTTP_RESPONSE_CACHE: OnceLock<Mutex<HttpResponseCache>> = OnceLock::new();
static NEXT_CLIENT_HANDLE: AtomicU64 = AtomicU64::new(1);
static HTTP_CLIENTS: OnceLock<Mutex<HashMap<u64, Arc<OwnedHttpClient>>>> = OnceLock::new();
const MAX_HTTP_CLIENTS: usize = 256;
const HTTP_ERROR_INVALID_ARGUMENT: i32 = 15;
static HTTP_WEBSOCKETS: OnceLock<Mutex<HashMap<u64, OwnedHttpWebSocket>>> = OnceLock::new();
const MAX_HTTP_WEBSOCKETS: usize = 256;

struct OwnedHttpClient {
    executor_id: u64,
    project_id: String,
    manifest_digest: String,
    gate: crate::sync_primitives::CooperativeSemaphore,
    request_slots: crate::sync_primitives::CooperativeSemaphore,
    close_gate: crate::sync_primitives::CooperativeSemaphore,
    pool: Mutex<crate::http_runtime::HttpClientPool>,
    closed: AtomicBool,
}

enum PreparedHttpClientRequest {
    Complete(HttpResponseValues),
    Http2(
        crate::http_runtime::Http2RequestSender,
        crate::http_runtime::HttpTimeouts,
    ),
    #[cfg(feature = "http3")]
    Http3(
        Arc<crate::http3_transport::Http3ClientSession>,
        crate::http_runtime::HttpTimeouts,
        HttpRequestValues,
    ),
}

/// Rolls back the admission flag when cooperative cancellation interrupts
/// close before active requests drain and the pool/handle are released.
struct ClientCloseAttempt<'a> {
    closed: &'a AtomicBool,
    committed: bool,
}

impl ClientCloseAttempt<'_> {
    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for ClientCloseAttempt<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.closed.store(false, Ordering::Release);
        }
    }
}

struct OwnedHttpWebSocket {
    owner: TaskId,
    socket: crate::http_runtime::HttpWebSocket,
}

fn http_clients() -> &'static Mutex<HashMap<u64, Arc<OwnedHttpClient>>> {
    HTTP_CLIENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn client_by_handle(handle: u64) -> Option<Arc<OwnedHttpClient>> {
    http_clients()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .cloned()
}

fn client_identity_matches(client: &OwnedHttpClient) -> bool {
    crate::permissions::verified_grant().is_some_and(|verified| {
        client_identity_matches_values(
            client,
            crate::executor::current_executor_id(),
            verified.project_id(),
            verified.manifest_digest(),
        )
    })
}

fn client_identity_matches_values(
    client: &OwnedHttpClient,
    executor_id: u64,
    project_id: &str,
    manifest_digest: &str,
) -> bool {
    client.executor_id == executor_id
        && client.project_id == project_id
        && client.manifest_digest == manifest_digest
}

fn http_websockets() -> &'static Mutex<HashMap<u64, OwnedHttpWebSocket>> {
    HTTP_WEBSOCKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns zero for a valid route pattern and one for malformed input.
///
/// # Safety
///
/// `pattern` must be null or a live managed Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_route_pattern_validate(pattern: *const c_void) -> i32 {
    let Some(pattern) = (unsafe { crate::string::borrow(pattern) }) else {
        return 1;
    };
    zirk_runtime_core::http_router::validate_route_pattern(unsafe { pattern.as_str() })
        .map(|_| 0)
        .unwrap_or(1)
}

/// Returns zero for no conflict, one for a duplicate, two for an ambiguous
/// pair, and three for invalid inputs.
///
/// # Safety
///
/// Each argument must be null or a live managed Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_route_pattern_conflict(
    method: *const c_void,
    first: *const c_void,
    second: *const c_void,
) -> i32 {
    let (Some(method), Some(first), Some(second)) = (
        unsafe { crate::string::borrow(method) },
        unsafe { crate::string::borrow(first) },
        unsafe { crate::string::borrow(second) },
    ) else {
        return 3;
    };
    let method = match unsafe { method.as_str() } {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "CONNECT" => HttpMethod::Connect,
        "TRACE" => HttpMethod::Trace,
        "QUERY" => HttpMethod::Query,
        _ => return 3,
    };
    match zirk_runtime_core::http_router::validate_route_pair(
        method,
        unsafe { first.as_str() },
        unsafe { second.as_str() },
    ) {
        Ok(()) => 0,
        Err(zirk_runtime_core::http_router::HttpRouteError::DuplicateRoute) => 1,
        Err(zirk_runtime_core::http_router::HttpRouteError::AmbiguousRoute) => 2,
        Err(_) => 3,
    }
}

/// Returns an empty list for no match; a match is encoded as an empty marker
/// followed by alternating parameter names and decoded values.
///
/// # Safety
///
/// `pattern` and `path` must be null or live managed Zirk `String` handles;
/// `out` must be null or point to writable `HttpTextOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_route_match(
    pattern: *const c_void,
    path: *const c_void,
    out: *mut HttpTextOut,
) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        let Some(pattern) = (unsafe { crate::string::borrow(pattern) }) else {
            return Err(0);
        };
        let Some(path) = (unsafe { crate::string::borrow(path) }) else {
            return Err(0);
        };
        let values = match zirk_runtime_core::http_router::match_route_pattern(
            HttpMethod::Get,
            unsafe { pattern.as_str() },
            unsafe { path.as_str() },
        ) {
            Ok(Some(matched)) => {
                let mut values = Vec::with_capacity(1 + matched.parameters.len() * 2);
                values.push(String::new());
                for (name, value) in matched.parameters {
                    values.push(name);
                    values.push(value);
                }
                values
            }
            Ok(None) | Err(_) => Vec::new(),
        };
        let element_size = std::mem::size_of::<*mut c_void>();
        let element_align = std::mem::align_of::<*mut c_void>();
        let list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        let mut rooted_list = list;
        let mut current = std::ptr::null_mut();
        let mut roots = [
            (&mut rooted_list as *mut *mut c_void).cast(),
            (&mut current as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        for value in values {
            current = crate::string::alloc_owned(&value);
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_list,
                    (&current as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        crate::zirk_rt_pop_frame();
        Ok(rooted_list)
    })();
    let outcome = match result {
        Ok(value) => HttpTextOut {
            kind: 0,
            error: 0,
            value,
        },
        Err(error) => text_failure(error),
    };
    unsafe { *out = outcome };
}

/// Serializes a bounded, validated set of standard method names as one
/// response `Allow` field in the managed header-block encoding.
///
/// # Safety
///
/// `patterns` and `methods` must be null or live managed `List<String>`
/// handles with matching lengths, and `path` must be null or a live managed
/// Zirk `String` handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_route_allow_header(
    patterns: *mut c_void,
    methods: *mut c_void,
    path: *mut c_void,
) -> *mut c_void {
    const ORDERED_METHODS: [&str; 10] = [
        "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE", "QUERY",
    ];
    let encoded = (|| {
        if patterns.is_null() || methods.is_null() || path.is_null() {
            return None;
        }
        let count = unsafe { crate::list::zirk_rt_list_length(methods) };
        if count == 0
            || count > zirk_runtime_core::http_router::MAX_HTTP_ROUTES
            || unsafe { crate::list::zirk_rt_list_length(patterns) } != count
        {
            return None;
        }
        let path_value = unsafe { crate::string::borrow(path) }?;
        let mut selected = [false; ORDERED_METHODS.len()];
        for index in 0..count {
            let pattern_item = unsafe {
                crate::list::zirk_rt_list_element(
                    patterns,
                    index as i64,
                    std::mem::size_of::<usize>(),
                )
            };
            let pattern_handle = unsafe { *(pattern_item as *mut *mut c_void) };
            let pattern = unsafe { crate::string::borrow(pattern_handle) }?;
            if zirk_runtime_core::http_router::match_route_pattern(
                HttpMethod::Get,
                unsafe { pattern.as_str() },
                unsafe { path_value.as_str() },
            )
            .ok()?
            .is_none()
            {
                continue;
            }
            let method_item = unsafe {
                crate::list::zirk_rt_list_element(
                    methods,
                    index as i64,
                    std::mem::size_of::<usize>(),
                )
            };
            let handle = unsafe { *(method_item as *mut *mut c_void) };
            let value = unsafe { crate::string::borrow(handle) }?;
            let method = unsafe { value.as_str() };
            let index = ORDERED_METHODS
                .iter()
                .position(|candidate| *candidate == method)?;
            selected[index] = true;
        }
        if !selected.iter().any(|allowed| *allowed) {
            return None;
        }
        let value = ORDERED_METHODS
            .iter()
            .enumerate()
            .filter_map(|(index, method)| selected[index].then_some(*method))
            .collect::<Vec<_>>()
            .join(", ");
        let name = b"allow";
        let mut block = Vec::with_capacity(4 + 6 + name.len() + value.len());
        block.extend_from_slice(&1u32.to_le_bytes());
        block.extend_from_slice(&(name.len() as u16).to_le_bytes());
        block.extend_from_slice(&(value.len() as u32).to_le_bytes());
        block.extend_from_slice(name);
        block.extend_from_slice(value.as_bytes());
        Some(block)
    })()
    .unwrap_or_else(|| 0u32.to_le_bytes().to_vec());
    crate::array::alloc_byte_array(&encoded)
}

/// Encodes parallel, validated managed header name/value lists into the
/// bounded response-header block consumed by the server transport.
///
/// # Safety
/// Both arguments must be live managed `List<String>` handles for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_headers_encode(
    names: *mut c_void,
    values: *mut c_void,
) -> *mut c_void {
    let (mut rooted_names, mut rooted_values) = (names, values);
    let mut roots = [
        (&mut rooted_names as *mut *mut c_void).cast(),
        (&mut rooted_values as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let encoded = (|| {
        if rooted_names.is_null() || rooted_values.is_null() {
            return None;
        }
        let count = unsafe { crate::list::zirk_rt_list_length(rooted_names) };
        if count > zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT
            || unsafe { crate::list::zirk_rt_list_length(rooted_values) } != count
        {
            return None;
        }
        let mut block = Vec::with_capacity(4);
        let mut raw_bytes = 0usize;
        block.extend_from_slice(&(count as u32).to_le_bytes());
        for index in 0..count {
            let name_slot = unsafe {
                crate::list::zirk_rt_list_element(
                    rooted_names,
                    index as i64,
                    std::mem::size_of::<usize>(),
                )
            };
            let value_slot = unsafe {
                crate::list::zirk_rt_list_element(
                    rooted_values,
                    index as i64,
                    std::mem::size_of::<usize>(),
                )
            };
            let name_handle = unsafe { *name_slot.cast::<*mut c_void>() };
            let value_handle = unsafe { *value_slot.cast::<*mut c_void>() };
            let name = unsafe { crate::string::borrow(name_handle) }?;
            let value = unsafe { crate::string::borrow(value_handle) }?;
            let raw_name = unsafe { name.as_str() };
            // The source API prefixes explicitly sensitive custom names with
            // HTAB. Whitespace is forbidden in HTTP field names, so it is an
            // unambiguous internal transport marker.
            let sensitive = raw_name.starts_with('\t');
            let name_bytes = raw_name.strip_prefix('\t').unwrap_or(raw_name).as_bytes();
            let value_bytes = unsafe { value.as_str() }.as_bytes();
            let next_size = block
                .len()
                .checked_add(6 + name_bytes.len() + value_bytes.len())?;
            let next_raw_bytes = raw_bytes.checked_add(name_bytes.len() + value_bytes.len())?;
            if name_bytes.is_empty()
                || name_bytes.len() > 0x7fff
                || value_bytes.len() > u32::MAX as usize
                || next_raw_bytes > zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES
                || next_size > MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES
            {
                return None;
            }
            let encoded_name_len = name_bytes.len() as u16 | if sensitive { 0x8000 } else { 0 };
            block.extend_from_slice(&encoded_name_len.to_le_bytes());
            block.extend_from_slice(&(value_bytes.len() as u32).to_le_bytes());
            block.extend_from_slice(name_bytes);
            block.extend_from_slice(value_bytes);
            raw_bytes = next_raw_bytes;
        }
        Some(block)
    })();
    // A malformed block fails closed in `decode_all_headers`; an empty valid
    // block here would silently erase an encoder invariant violation.
    let block = encoded.unwrap_or_else(|| 1u32.to_le_bytes().to_vec());
    let result = crate::array::alloc_byte_array(&block);
    crate::zirk_rt_pop_frame();
    result
}

/// Parses a bounded form body into an alternating managed name/value list.
///
/// # Safety
/// `body` and `content_type` must be live Zirk values; `out` must point to a
/// writable `HttpTextOut` result slot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_form_parse(
    body: *mut c_void,
    content_type: *const c_void,
    out: *mut HttpTextOut,
) {
    if out.is_null() {
        return;
    }
    let (mut rooted_body, mut rooted_content_type, mut rooted_list, mut current) = (
        body,
        content_type.cast_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    let mut roots = [
        (&mut rooted_body as *mut *mut c_void).cast(),
        (&mut rooted_content_type as *mut *mut c_void).cast(),
        (&mut rooted_list as *mut *mut c_void).cast(),
        (&mut current as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let outcome = (|| {
        let Some(content_type) = (unsafe { crate::string::borrow(rooted_content_type) }) else {
            return text_failure(3);
        };
        let content_type = unsafe { content_type.as_str() };
        let media_type = match content_type.parse::<mime::Mime>() {
            Ok(value) => value,
            Err(_) => return text_failure(3),
        };
        if media_type.type_() != mime::APPLICATION
            || media_type.subtype().as_str() != "x-www-form-urlencoded"
            || media_type
                .get_param(mime::CHARSET)
                .is_some_and(|charset| !charset.as_str().eq_ignore_ascii_case("utf-8"))
        {
            return text_failure(3);
        }
        if rooted_body.is_null() {
            return text_failure(0);
        }
        let body_len = unsafe { crate::array::zirk_rt_array_length(rooted_body) };
        if body_len > zirk_runtime_core::http_form::MAX_FORM_BODY_BYTES {
            return text_failure(4);
        }
        let mut bytes = Vec::with_capacity(body_len);
        for index in 0..body_len {
            let slot = unsafe { crate::array::zirk_rt_array_element(rooted_body, index as i64, 1) };
            bytes.push(unsafe { *slot.cast::<u8>() });
        }
        let fields = match zirk_runtime_core::http_form::parse_form(&bytes) {
            Ok(fields) => fields,
            Err(zirk_runtime_core::http_form::FormError::BodyTooLarge) => return text_failure(4),
            Err(zirk_runtime_core::http_form::FormError::TooManyFields) => return text_failure(12),
            Err(zirk_runtime_core::http_form::FormError::InvalidEncoding) => {
                return text_failure(3);
            }
        };
        let flat = fields
            .into_iter()
            .flat_map(|(name, value)| [name, value])
            .collect::<Vec<_>>();
        let element_size = std::mem::size_of::<*mut c_void>();
        let element_align = std::mem::align_of::<*mut c_void>();
        rooted_list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        for value in flat {
            current = crate::string::alloc_owned(&value);
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_list,
                    (&current as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        HttpTextOut {
            kind: 0,
            error: 0,
            value: rooted_list,
        }
    })();
    crate::zirk_rt_pop_frame();
    unsafe { *out = outcome };
}

#[repr(C)]
pub struct HttpMultipartOut {
    kind: i32,
    error: i32,
    names: *mut c_void,
    filenames: *mut c_void,
    filename_present: *mut c_void,
    content_types: *mut c_void,
    content_type_present: *mut c_void,
    bodies: *mut c_void,
    upload_handles: *mut c_void,
}

/// Parses a multipart request into parallel managed lists. Optional text uses
/// a string list and parallel Boolean presence list, matching Zirk's inline
/// nullable ABI. Filenames remain untrusted metadata only.
///
/// # Safety
/// `body` and `content_type` must be live Zirk values; `out` must point to a
/// writable `HttpMultipartOut` result slot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_multipart_parse(
    body: *mut c_void,
    content_type: *const c_void,
    out: *mut HttpMultipartOut,
) {
    if out.is_null() {
        return;
    }
    let (
        mut rooted_body,
        mut rooted_content_type,
        mut rooted_names,
        mut rooted_filenames,
        mut rooted_filename_present,
        mut rooted_content_types,
        mut rooted_content_type_present,
        mut rooted_bodies,
        mut rooted_upload_handles,
        mut current_text,
        mut current_bytes,
    ) = (
        body,
        content_type.cast_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    let mut roots = [
        (&mut rooted_body as *mut *mut c_void).cast(),
        (&mut rooted_content_type as *mut *mut c_void).cast(),
        (&mut rooted_names as *mut *mut c_void).cast(),
        (&mut rooted_filenames as *mut *mut c_void).cast(),
        (&mut rooted_filename_present as *mut *mut c_void).cast(),
        (&mut rooted_content_types as *mut *mut c_void).cast(),
        (&mut rooted_content_type_present as *mut *mut c_void).cast(),
        (&mut rooted_bodies as *mut *mut c_void).cast(),
        (&mut rooted_upload_handles as *mut *mut c_void).cast(),
        (&mut current_text as *mut *mut c_void).cast(),
        (&mut current_bytes as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let outcome = (|| {
        let Some(content_type) = (unsafe { crate::string::borrow(rooted_content_type) }) else {
            return HttpMultipartOut::failure(3);
        };
        let media_type = match unsafe { content_type.as_str() }.parse::<mime::Mime>() {
            Ok(value) => value,
            Err(_) => return HttpMultipartOut::failure(3),
        };
        if media_type.type_() != mime::MULTIPART || media_type.subtype().as_str() != "form-data" {
            return HttpMultipartOut::failure(3);
        }
        let Some(boundary) = media_type
            .get_param("boundary")
            .map(|value| value.as_str().to_owned())
        else {
            return HttpMultipartOut::failure(3);
        };
        if rooted_body.is_null() {
            return HttpMultipartOut::failure(0);
        }
        let body_len = unsafe { crate::array::zirk_rt_array_length(rooted_body) };
        if body_len > zirk_runtime_core::http_form::MAX_FORM_BODY_BYTES {
            return HttpMultipartOut::failure(4);
        }
        let mut body_bytes = Vec::with_capacity(body_len);
        for index in 0..body_len {
            let slot = unsafe { crate::array::zirk_rt_array_element(rooted_body, index as i64, 1) };
            body_bytes.push(unsafe { *slot.cast::<u8>() });
        }
        let parts = match zirk_runtime_core::http_multipart::parse_multipart(&body_bytes, &boundary)
        {
            Ok(parts) => parts,
            Err(zirk_runtime_core::http_multipart::MultipartError::BodyTooLarge) => {
                return HttpMultipartOut::failure(4);
            }
            Err(zirk_runtime_core::http_multipart::MultipartError::TooManyParts) => {
                return HttpMultipartOut::failure(12);
            }
            Err(_) => return HttpMultipartOut::failure(3),
        };

        let mut created_uploads = Vec::new();
        let element_size = std::mem::size_of::<*mut c_void>();
        let element_align = std::mem::align_of::<*mut c_void>();
        rooted_names = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        rooted_filenames =
            unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        rooted_filename_present = unsafe { crate::list::zirk_rt_list_new(1, 1, false) };
        rooted_content_types =
            unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        rooted_content_type_present = unsafe { crate::list::zirk_rt_list_new(1, 1, false) };
        rooted_bodies = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        rooted_upload_handles = unsafe {
            crate::list::zirk_rt_list_new(
                std::mem::size_of::<u64>(),
                std::mem::align_of::<u64>(),
                false,
            )
        };
        for part in parts {
            let upload_handle = if part.filename.is_some() {
                match create_http_upload(&part.body, crate::executor::current_task()) {
                    Ok(handle) => {
                        created_uploads.push(handle);
                        handle
                    }
                    Err(()) => {
                        for handle in created_uploads {
                            close_http_upload(handle, crate::executor::current_task());
                        }
                        return HttpMultipartOut::failure(2);
                    }
                }
            } else {
                0
            };
            current_text = crate::string::alloc_owned(&part.name);
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_names,
                    (&current_text as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_upload_handles,
                    (&upload_handle as *const u64).cast(),
                    std::mem::size_of::<u64>(),
                    std::mem::align_of::<u64>(),
                    false,
                )
            };
            let filename_present = u8::from(part.filename.is_some());
            current_text = crate::string::alloc_owned(part.filename.as_deref().unwrap_or(""));
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_filenames,
                    (&current_text as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_filename_present,
                    (&filename_present as *const u8).cast(),
                    1,
                    1,
                    false,
                )
            };
            let content_type_present = u8::from(part.content_type.is_some());
            current_text = crate::string::alloc_owned(part.content_type.as_deref().unwrap_or(""));
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_content_types,
                    (&current_text as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_content_type_present,
                    (&content_type_present as *const u8).cast(),
                    1,
                    1,
                    false,
                )
            };
            current_bytes = crate::array::alloc_byte_array(&part.body);
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_bodies,
                    (&current_bytes as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        HttpMultipartOut {
            kind: 0,
            error: 0,
            names: rooted_names,
            filenames: rooted_filenames,
            filename_present: rooted_filename_present,
            content_types: rooted_content_types,
            content_type_present: rooted_content_type_present,
            bodies: rooted_bodies,
            upload_handles: rooted_upload_handles,
        }
    })();
    crate::zirk_rt_pop_frame();
    unsafe { *out = outcome };
}

impl HttpMultipartOut {
    fn failure(error: i32) -> Self {
        Self {
            kind: 1,
            error,
            names: std::ptr::null_mut(),
            filenames: std::ptr::null_mut(),
            filename_present: std::ptr::null_mut(),
            content_types: std::ptr::null_mut(),
            content_type_present: std::ptr::null_mut(),
            bodies: std::ptr::null_mut(),
            upload_handles: std::ptr::null_mut(),
        }
    }
}

/// Reads an explicit bounded range from a task-owned multipart temporary file.
///
/// # Safety
/// `out` must point to writable `HttpStreamReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_upload_read(
    handle: u64,
    offset: u64,
    max_bytes: u64,
    out: *mut HttpStreamReadOut,
) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        if max_bytes == 0 || max_bytes > 64 * 1024 {
            return Err(HTTP_ERROR_INVALID_ARGUMENT);
        }
        let owner = crate::executor::current_task();
        let mut uploads = http_uploads()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(upload) = uploads.get_mut(&handle) else {
            return Err(8);
        };
        if upload.owner != owner {
            return Err(1);
        }
        if offset > upload.size {
            return Err(HTTP_ERROR_INVALID_ARGUMENT);
        }
        if offset == upload.size {
            return Ok(HttpStreamReadOut {
                kind: 1,
                error: 0,
                bytes: std::ptr::null_mut(),
            });
        }
        let length = usize::try_from((upload.size - offset).min(max_bytes)).map_err(|_| 3)?;
        upload.file.seek(SeekFrom::Start(offset)).map_err(|_| 2)?;
        let mut bytes = vec![0; length];
        upload.file.read_exact(&mut bytes).map_err(|_| 2)?;
        Ok(HttpStreamReadOut {
            kind: 0,
            error: 0,
            bytes: crate::array::alloc_byte_array(&bytes),
        })
    })();
    let outcome = result.unwrap_or_else(|error| HttpStreamReadOut {
        kind: 2,
        error,
        bytes: std::ptr::null_mut(),
    });
    unsafe { out.write(outcome) };
}

/// Closes and deletes a task-owned multipart temporary file. Closing twice is
/// idempotent; a foreign task cannot close another task's upload.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_http_upload_close(handle: u64) -> bool {
    close_http_upload(handle, crate::executor::current_task())
}

/// Encodes a bounded alternating managed name/value list as
/// application/x-www-form-urlencoded bytes.
///
/// # Safety
/// `fields` must be a live managed `List<String>` handle and `out` must point
/// to writable `HttpTextOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_form_encode(fields: *mut c_void, out: *mut HttpTextOut) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        if fields.is_null() {
            return Err(3);
        }
        let count = unsafe { crate::list::zirk_rt_list_length(fields) };
        if count > zirk_runtime_core::http_form::MAX_FORM_FIELDS * 2 || count % 2 != 0 {
            return Err(12);
        }
        let element_size = std::mem::size_of::<*mut c_void>();
        let mut owned = Vec::with_capacity(count / 2);
        for index in (0..count).step_by(2) {
            let name_slot =
                unsafe { crate::list::zirk_rt_list_element(fields, index as i64, element_size) };
            let value_slot = unsafe {
                crate::list::zirk_rt_list_element(fields, index as i64 + 1, element_size)
            };
            let name_handle = unsafe { *name_slot.cast::<*mut c_void>() };
            let value_handle = unsafe { *value_slot.cast::<*mut c_void>() };
            let Some(name) = (unsafe { crate::string::borrow(name_handle) }) else {
                return Err(3);
            };
            let name = unsafe { name.as_str() }.to_owned();
            let Some(value) = (unsafe { crate::string::borrow(value_handle) }) else {
                return Err(3);
            };
            let value = unsafe { value.as_str() }.to_owned();
            owned.push((name, value));
        }
        let bytes = match zirk_runtime_core::http_form::encode_form(&owned) {
            Ok(bytes) => bytes,
            Err(zirk_runtime_core::http_form::FormError::BodyTooLarge) => return Err(4),
            Err(zirk_runtime_core::http_form::FormError::TooManyFields) => return Err(12),
            Err(zirk_runtime_core::http_form::FormError::InvalidEncoding) => return Err(3),
        };
        Ok(crate::array::alloc_byte_array(&bytes))
    })();
    let outcome = match result {
        Ok(bytes) => HttpTextOut {
            kind: 0,
            error: 0,
            value: bytes,
        },
        Err(error) => text_failure(error),
    };
    unsafe { *out = outcome };
}

/// Encodes bounded multipart metadata and binary payload lists.
///
/// # Safety
/// `metadata` must be a live List<String> with five entries per part,
/// `bodies` a live List<Array<Byte>>, `boundary` a live Zirk String, and `out`
/// writable HttpTextOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_multipart_encode(
    metadata: *mut c_void,
    bodies: *mut c_void,
    boundary: *const c_void,
    out: *mut HttpTextOut,
) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        if metadata.is_null() || bodies.is_null() {
            return Err(3);
        }
        let Some(boundary) = (unsafe { crate::string::borrow(boundary) }) else {
            return Err(3);
        };
        let boundary = unsafe { boundary.as_str() }.to_owned();
        let part_count = unsafe { crate::list::zirk_rt_list_length(bodies) };
        let metadata_count = unsafe { crate::list::zirk_rt_list_length(metadata) };
        if part_count > zirk_runtime_core::http_multipart::MAX_MULTIPART_PARTS {
            return Err(13);
        }
        if metadata_count != part_count.saturating_mul(5) {
            return Err(3);
        }
        let list_element_size = std::mem::size_of::<*mut c_void>();
        let mut parts = Vec::with_capacity(part_count);
        let mut retained_payload_bytes = 0usize;
        for index in 0..part_count {
            let get_text = |offset: usize| -> Result<String, i32> {
                let slot = unsafe {
                    crate::list::zirk_rt_list_element(
                        metadata,
                        (index * 5 + offset) as i64,
                        list_element_size,
                    )
                };
                let handle = unsafe { *slot.cast::<*mut c_void>() };
                let Some(value) = (unsafe { crate::string::borrow(handle) }) else {
                    return Err(3);
                };
                Ok(unsafe { value.as_str() }.to_owned())
            };
            let name = get_text(0)?;
            let filename = get_text(1)?;
            let filename_present = get_text(2)?;
            let content_type = get_text(3)?;
            let content_type_present = get_text(4)?;
            let filename = match filename_present.as_str() {
                "0" => None,
                "1" => Some(filename),
                _ => return Err(3),
            };
            let content_type = match content_type_present.as_str() {
                "0" => None,
                "1" => Some(content_type),
                _ => return Err(3),
            };
            let body_slot = unsafe {
                crate::list::zirk_rt_list_element(bodies, index as i64, list_element_size)
            };
            let body_handle = unsafe { *body_slot.cast::<*mut c_void>() };
            if body_handle.is_null() {
                return Err(3);
            }
            let body_length = unsafe { crate::array::zirk_rt_array_length(body_handle) };
            if body_length
                > zirk_runtime_core::http_multipart::MAX_MULTIPART_ENCODED_BYTES
                    .saturating_sub(retained_payload_bytes)
            {
                return Err(4);
            }
            retained_payload_bytes += body_length;
            let body =
                unsafe { crate::array::copy_byte_array(body_handle, body_length) }.ok_or(4)?;
            parts.push(zirk_runtime_core::http_multipart::MultipartPart {
                name,
                filename,
                content_type,
                body,
            });
        }
        let bytes = zirk_runtime_core::http_multipart::encode_multipart(&parts, &boundary)
            .map_err(|error| match error {
                zirk_runtime_core::http_multipart::MultipartError::BodyTooLarge => 4,
                zirk_runtime_core::http_multipart::MultipartError::TooManyParts => 13,
                _ => 3,
            })?;
        Ok(crate::array::alloc_byte_array(&bytes))
    })();
    let outcome = match result {
        Ok(bytes) => HttpTextOut {
            kind: 0,
            error: 0,
            value: bytes,
        },
        Err(error) => text_failure(error),
    };
    unsafe { out.write(outcome) };
}

/// Returns all values for one form field, preserving the parser's order.
///
/// # Safety
/// `fields` must be a live managed alternating `List<String>` and `name` a
/// live managed Zirk `String`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_form_values(
    fields: *mut c_void,
    name: *const c_void,
) -> *mut c_void {
    let (mut rooted_fields, mut rooted_name, mut rooted_values, mut current) = (
        fields,
        name.cast_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    let mut roots = [
        (&mut rooted_fields as *mut *mut c_void).cast(),
        (&mut rooted_name as *mut *mut c_void).cast(),
        (&mut rooted_values as *mut *mut c_void).cast(),
        (&mut current as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let element_size = std::mem::size_of::<*mut c_void>();
    let element_align = std::mem::align_of::<*mut c_void>();
    rooted_values = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
    (|| {
        if rooted_fields.is_null() || rooted_name.is_null() {
            return;
        }
        let count = unsafe { crate::list::zirk_rt_list_length(rooted_fields) };
        if count > zirk_runtime_core::http_form::MAX_FORM_FIELDS * 2 || count % 2 != 0 {
            return;
        }
        for index in (0..count).step_by(2) {
            let key_slot = unsafe {
                crate::list::zirk_rt_list_element(rooted_fields, index as i64, element_size)
            };
            let value_slot = unsafe {
                crate::list::zirk_rt_list_element(rooted_fields, index as i64 + 1, element_size)
            };
            let key_handle = unsafe { *key_slot.cast::<*mut c_void>() };
            let value_handle = unsafe { *value_slot.cast::<*mut c_void>() };
            let matches = {
                let Some(key) = (unsafe { crate::string::borrow(key_handle) }) else {
                    return;
                };
                let Some(requested) = (unsafe { crate::string::borrow(rooted_name) }) else {
                    return;
                };
                unsafe { key.as_str() == requested.as_str() }
            };
            if matches {
                let value = {
                    let Some(value) = (unsafe { crate::string::borrow(value_handle) }) else {
                        return;
                    };
                    unsafe { value.as_str().to_owned() }
                };
                current = crate::string::alloc_owned(&value);
                unsafe {
                    crate::list::zirk_rt_list_add(
                        rooted_values,
                        (&current as *const *mut c_void).cast(),
                        element_size,
                        element_align,
                        true,
                    )
                };
            }
        }
    })();
    crate::zirk_rt_pop_frame();
    rooted_values
}

/// Request values borrowed only for the duration of a generated HTTP handler
/// thunk. Every pointer names a rooted managed Zirk value.
#[repr(C)]
pub struct HttpServerRequest {
    pub method: *mut c_void,
    pub target: *mut c_void,
    pub headers: *mut c_void,
    pub body: *mut c_void,
}

/// Response fields written by the generated handler thunk. The returned
/// managed arrays remain rooted by the response object until copied.
#[repr(C)]
pub struct HttpServerResponse {
    pub status: i32,
    pub headers: *mut c_void,
    pub body: *mut c_void,
    pub producer_handle: u64,
    pub sse_heartbeat_ms: i32,
}

const MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES: usize =
    zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES
        + 4
        + zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT * 6;

pub type HttpServerCallback = unsafe extern "C-unwind" fn(
    request: *const HttpServerRequest,
    response: *mut HttpServerResponse,
    context: *mut c_void,
) -> i32;

pub type HttpWebSocketServerCallback =
    unsafe extern "C-unwind" fn(handle: u64, context: *mut c_void) -> i32;

/// Generated adapter for a source-level HTTP body producer. A null output is
/// EOF; otherwise it points to one managed `Array<Byte>` owned by the caller's
/// registered shadow frame.
pub type HttpUploadProducerCallback = unsafe extern "C" fn(
    context: *mut c_void,
    chunk_index: u64,
    out_chunk: *mut *mut c_void,
) -> i32;

const MAX_HTTP_SERVER_PRODUCERS: usize = 1_024;
pub(crate) const MAX_HTTP_SERVER_PRODUCER_CHUNK_BYTES: usize = 64 * 1024;

/// Stable two-word generated closure representation copied at registration.
/// The capture pointer is also retained through the collector root handle.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HttpServerProducerContext {
    pub(crate) function: usize,
    pub(crate) captures: usize,
    pub(crate) dispatch: usize,
}

struct RegisteredHttpServerProducer {
    owner: TaskId,
    callback: HttpUploadProducerCallback,
    context: HttpServerProducerContext,
    root_handle: u64,
}

static NEXT_HTTP_SERVER_PRODUCER: AtomicU64 = AtomicU64::new(1);
static HTTP_SERVER_PRODUCERS: OnceLock<Mutex<HashMap<u64, RegisteredHttpServerProducer>>> =
    OnceLock::new();

fn http_server_producers() -> &'static Mutex<HashMap<u64, RegisteredHttpServerProducer>> {
    HTTP_SERVER_PRODUCERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Producer removed from the registration table and owned by one server
/// response. It invokes generated code only from the registered Zirk task.
pub(crate) struct HttpServerBodyProducer {
    owner: TaskId,
    callback: HttpUploadProducerCallback,
    context: HttpServerProducerContext,
    root_handle: u64,
    next_index: u64,
    ended: bool,
    heartbeat_interval: Option<std::time::Duration>,
}

impl HttpServerBodyProducer {
    pub(crate) fn set_sse_heartbeat_ms(&mut self, heartbeat_ms: i32) -> Result<(), i32> {
        if heartbeat_ms == 0 {
            self.heartbeat_interval = None;
            return Ok(());
        }
        if !(1_000..=3_600_000).contains(&heartbeat_ms) {
            return Err(3);
        }
        self.heartbeat_interval = Some(std::time::Duration::from_millis(heartbeat_ms as u64));
        Ok(())
    }

    pub(crate) fn sse_heartbeat_interval(&self) -> Option<std::time::Duration> {
        self.heartbeat_interval
    }

    pub(crate) fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, i32> {
        if self.owner != crate::executor::current_task() {
            return Err(1);
        }
        if self.ended {
            return Ok(None);
        }
        let mut output = std::ptr::null_mut();
        let callback_status = unsafe {
            (self.callback)(
                (&self.context as *const HttpServerProducerContext)
                    .cast_mut()
                    .cast(),
                self.next_index,
                &mut output,
            )
        };
        self.next_index = self.next_index.checked_add(1).ok_or(4)?;
        if callback_status != 0 {
            self.ended = true;
            return Err(callback_status);
        }
        let Some(chunk) = (unsafe {
            crate::array::copy_byte_array(output, MAX_HTTP_SERVER_PRODUCER_CHUNK_BYTES)
        }) else {
            self.ended = true;
            return Err(4);
        };
        if chunk.is_empty() {
            self.ended = true;
            return Ok(None);
        }
        Ok(Some(chunk))
    }
}

impl Drop for HttpServerBodyProducer {
    fn drop(&mut self) {
        if self.root_handle != 0 {
            crate::zirk_rt_global_root_remove(self.root_handle);
            self.root_handle = 0;
        }
    }
}

/// Copies a generated producer closure into a bounded native registry and
/// roots its capture block until the server consumes or abandons the response.
///
/// # Safety
/// `context` must point to a live generated two-pointer callable and
/// `captures` must be null or a live managed object pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_server_producer_register(
    callback: Option<HttpUploadProducerCallback>,
    context: *const HttpServerProducerContext,
    captures: *mut c_void,
) -> u64 {
    let outcome = (|| {
        let callback = callback.ok_or(3)?;
        if context.is_null() {
            return Err(3);
        }
        let context = unsafe { context.read() };
        if context.captures != captures as usize {
            return Err(3);
        }
        let mut producers = http_server_producers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if producers.len() >= MAX_HTTP_SERVER_PRODUCERS {
            return Err(2);
        }
        let root_handle = if captures.is_null() {
            0
        } else {
            let root = unsafe { crate::zirk_rt_global_root_add(captures) };
            if root == 0 {
                return Err(2);
            }
            root
        };
        let handle = NEXT_HTTP_SERVER_PRODUCER.fetch_add(1, Ordering::Relaxed);
        if handle == 0 {
            if root_handle != 0 {
                crate::zirk_rt_global_root_remove(root_handle);
            }
            return Err(2);
        }
        producers.insert(
            handle,
            RegisteredHttpServerProducer {
                owner: crate::executor::current_task(),
                callback,
                context,
                root_handle,
            },
        );
        Ok(handle)
    })();
    outcome.unwrap_or(0)
}

pub(crate) fn take_http_server_producer(handle: u64) -> Result<HttpServerBodyProducer, i32> {
    let mut producers = http_server_producers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(producer) = producers.get(&handle) else {
        return Err(8);
    };
    if producer.owner != crate::executor::current_task() {
        return Err(1);
    }
    let producer = producers
        .remove(&handle)
        .expect("producer was just checked");
    Ok(HttpServerBodyProducer {
        owner: producer.owner,
        callback: producer.callback,
        context: producer.context,
        root_handle: producer.root_handle,
        next_index: 0,
        ended: false,
        heartbeat_interval: None,
    })
}

pub(crate) fn close_task_server_producers(owner: TaskId) {
    let removed = {
        let mut producers = http_server_producers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let handles = producers
            .iter()
            .filter_map(|(handle, producer)| (producer.owner == owner).then_some(*handle))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|handle| producers.remove(&handle))
            .collect::<Vec<_>>()
    };
    for producer in removed {
        if producer.root_handle != 0 {
            crate::zirk_rt_global_root_remove(producer.root_handle);
        }
    }
}

#[repr(C)]
pub struct HttpServerServeOut {
    pub kind: i32,
    pub error: i32,
}

#[repr(C)]
pub struct HttpServerServeHttp3Out {
    pub kind: i32,
    pub error: i32,
    pub count: u64,
}

/// Serves one HTTP/1.1 request on an owner-task TCP stream.
///
/// # Safety
/// `out` must point to writable storage and a non-null callback must satisfy
/// the [`HttpServerCallback`] ABI for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_http_server_serve_once(
    handle: u64,
    secure: i32,
    max_requests: u64,
    require_http2: i32,
    callback: Option<HttpServerCallback>,
    context: *mut c_void,
    out: *mut HttpServerServeOut,
) {
    let stream = if secure != 0 {
        crate::network_api::take_tls_server_stream_for_http(handle)
            .map(crate::http_runtime::HttpServerIo::from)
    } else {
        crate::network_api::take_tcp_stream_for_http(handle)
            .map(crate::http_runtime::HttpServerIo::from)
    };
    let outcome = match (callback, stream) {
        (Some(callback), Ok(stream)) => {
            match serve_once_with_callback(
                stream,
                secure != 0,
                max_requests,
                require_http2 != 0,
                callback,
                context,
            ) {
                Ok(()) => HttpServerServeOut { kind: 0, error: 0 },
                Err(error) => HttpServerServeOut {
                    kind: 1,
                    error: match error {
                        crate::http_runtime::HttpRuntimeError::InvalidRequest => 0,
                        crate::http_runtime::HttpRuntimeError::UnsupportedMethod => 14,
                        crate::http_runtime::HttpRuntimeError::PermissionDenied => 1,
                        crate::http_runtime::HttpRuntimeError::Transport => 2,
                        crate::http_runtime::HttpRuntimeError::Protocol => 3,
                        crate::http_runtime::HttpRuntimeError::BodyTooLarge => 4,
                        crate::http_runtime::HttpRuntimeError::InvalidText => 5,
                        crate::http_runtime::HttpRuntimeError::Timeout => 6,
                        crate::http_runtime::HttpRuntimeError::Cancelled => 7,
                        crate::http_runtime::HttpRuntimeError::BodyConsumed => 8,
                        crate::http_runtime::HttpRuntimeError::InvalidJson => 9,
                    },
                },
            }
        }
        (None, _) => HttpServerServeOut { kind: 1, error: 2 },
        (Some(_), Err(error)) => HttpServerServeOut {
            kind: 1,
            error: if error == 12 { 1 } else { 2 },
        },
    };
    if !out.is_null() {
        unsafe { out.write(outcome) };
    }
}

/// Binds and serves a finite HTTP/3 endpoint through the official Zirk router
/// callback. The effective UDP bind grant is checked before Quinn opens the
/// socket, and callback execution remains on the owning Zirk task.
///
/// # Safety
/// `endpoint`, certificate/key arrays, callback/context, and `out` must satisfy
/// their declared Zirk ABI layouts for the duration of this call.
#[unsafe(no_mangle)]
// The fallible body uses `?` in the HTTP/3 feature build; the disabled build
// compiles only its typed unsupported result, which makes Clippy see an
// otherwise redundant closure.
#[allow(clippy::redundant_closure_call)]
pub unsafe extern "C-unwind" fn zirk_http_server_serve_http3(
    endpoint: *const c_void,
    certificate_chain_der: *mut c_void,
    private_key_der: *mut c_void,
    max_requests: u64,
    max_connections: u64,
    callback: Option<HttpServerCallback>,
    context: *mut c_void,
    out: *mut HttpServerServeHttp3Out,
) {
    let result: Result<usize, crate::http_runtime::HttpRuntimeError> = (|| {
        #[cfg(not(feature = "http3"))]
        {
            let _ = (
                endpoint,
                certificate_chain_der,
                private_key_der,
                max_requests,
                max_connections,
                callback,
                context,
            );
            Err(crate::http_runtime::HttpRuntimeError::Protocol)
        }
        #[cfg(feature = "http3")]
        {
            let endpoint = unsafe { crate::string::borrow(endpoint.cast_mut()) }
                .ok_or(crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
            let address = crate::network_api::parse_udp_endpoint(unsafe { endpoint.as_str() })
                .map_err(|error| match error {
                    crate::network_transport::NetworkTransportError::PermissionDenied => {
                        crate::http_runtime::HttpRuntimeError::PermissionDenied
                    }
                    _ => crate::http_runtime::HttpRuntimeError::InvalidRequest,
                })?;
            let certificates = unsafe { copy_http_trust_roots(certificate_chain_der) }
                .ok_or(crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
            let private_key = unsafe { crate::array::copy_byte_array(private_key_der, 64 * 1024) }
                .filter(|key| !key.is_empty())
                .ok_or(crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
            let max_requests = usize::try_from(max_requests)
                .map_err(|_| crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
            let max_connections = usize::try_from(max_connections)
                .map_err(|_| crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
            let callback = callback.ok_or(crate::http_runtime::HttpRuntimeError::Protocol)?;
            let grant = crate::permissions::current_network_grant()
                .map_err(|_| crate::http_runtime::HttpRuntimeError::PermissionDenied)?;
            let server_config = crate::network_transport::TlsServerConfig::from_der(
                certificates,
                private_key,
                false,
            )
            .and_then(crate::network_transport::TlsServerConfig::into_http3_quic_config)
            .map_err(|_| crate::http_runtime::HttpRuntimeError::Protocol)?;
            let (endpoint, _) =
                crate::http3_transport::bind_server_endpoint(address, &grant, server_config)?;
            let context = context as usize;
            crate::http3_transport::serve_endpoint(
                endpoint,
                max_requests,
                max_connections,
                move |request| {
                    let request = match http3_request_values(request) {
                        Ok(request) => request,
                        Err(_) => return http3_error_response(400, b"invalid request"),
                    };
                    let response =
                        invoke_http_server_callback(request, callback, context as *mut c_void);
                    http3_response(response)
                },
            )
        }
    })();
    let outcome = match result {
        Ok(count) => HttpServerServeHttp3Out {
            kind: 0,
            error: 0,
            count: count as u64,
        },
        Err(error) => HttpServerServeHttp3Out {
            kind: 1,
            error: http_server_error_code(error),
            count: 0,
        },
    };
    if !out.is_null() {
        unsafe { out.write(outcome) };
    }
}

#[cfg(feature = "http3")]
fn http3_request_values(
    request: http::Request<Vec<u8>>,
) -> Result<HttpRequestValues, crate::http_runtime::HttpRuntimeError> {
    let method_token = request.method().as_str().to_owned();
    let method = match method_token.as_str() {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        "CONNECT" => HttpMethod::Connect,
        "OPTIONS" => HttpMethod::Options,
        "TRACE" => HttpMethod::Trace,
        "PATCH" => HttpMethod::Patch,
        "QUERY" => HttpMethod::Query,
        _ => HttpMethod::Other,
    };
    let uri = request.uri();
    let scheme = uri
        .scheme_str()
        .ok_or(crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
    if scheme != "https" {
        return Err(crate::http_runtime::HttpRuntimeError::Protocol);
    }
    let authority = uri
        .authority()
        .ok_or(crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
    let path = uri.path_and_query().map_or("/", |path| path.as_str());
    let url = Url::parse(&format!("https://{authority}{path}"))
        .map_err(|_| crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
    let mut values = HttpRequestValues::new(method, url);
    values.method_token = method_token;
    for (name, _) in request.headers().iter() {
        for value in request.headers().get_all(name).iter() {
            values
                .headers
                .append(name.as_str(), value.as_bytes())
                .map_err(|_| crate::http_runtime::HttpRuntimeError::InvalidRequest)?;
        }
    }
    values
        .set_body(request.into_body())
        .map_err(|_| crate::http_runtime::HttpRuntimeError::BodyTooLarge)?;
    Ok(values)
}

#[cfg(feature = "http3")]
fn http3_response(
    mut response: crate::http_runtime::HttpResponseValues,
) -> crate::http3_transport::Http3ServerResponse {
    if response.body_stream.is_some() && response.body_producer.is_some() {
        return http3_error_response(500, b"invalid streaming response");
    }
    let producer = if let Some(producer) = response.body_producer.take() {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        response.body_stream = Some(receiver);
        Some((producer, sender))
    } else {
        None
    };
    let body = match response.body_stream.take() {
        Some(receiver) => crate::http3_transport::Http3ResponseBody::Stream(receiver),
        None => {
            crate::http3_transport::Http3ResponseBody::Buffered(std::mem::take(&mut response.body))
        }
    };
    let mut builder = http::Response::builder().status(response.status.as_u16());
    for (name, _) in response.headers.iter() {
        if name.as_str() == "connection"
            || name.as_str() == "transfer-encoding"
            || name.as_str() == "upgrade"
            || name.as_str() == "keep-alive"
            || name.as_str() == "proxy-connection"
        {
            continue;
        }
        for value in response
            .headers
            .get_all(name.as_str())
            .into_iter()
            .flatten()
        {
            builder = builder.header(name.as_str(), value.as_bytes());
        }
    }
    match builder.body(body) {
        Ok(response) => crate::http3_transport::Http3ServerResponse { response, producer },
        Err(_) => http3_error_response(500, b"invalid response"),
    }
}

#[cfg(feature = "http3")]
fn http3_error_response(status: u16, body: &[u8]) -> crate::http3_transport::Http3ServerResponse {
    crate::http3_transport::Http3ServerResponse {
        response: http::Response::builder()
            .status(status)
            .body(crate::http3_transport::Http3ResponseBody::Buffered(
                body.to_vec(),
            ))
            .expect("fixed HTTP error response is valid"),
        producer: None,
    }
}

fn http_server_error_code(error: crate::http_runtime::HttpRuntimeError) -> i32 {
    match error {
        crate::http_runtime::HttpRuntimeError::InvalidRequest => 0,
        crate::http_runtime::HttpRuntimeError::UnsupportedMethod => 14,
        crate::http_runtime::HttpRuntimeError::PermissionDenied => 1,
        crate::http_runtime::HttpRuntimeError::Transport => 2,
        crate::http_runtime::HttpRuntimeError::Protocol => 3,
        crate::http_runtime::HttpRuntimeError::BodyTooLarge => 4,
        crate::http_runtime::HttpRuntimeError::InvalidText => 5,
        crate::http_runtime::HttpRuntimeError::Timeout => 6,
        crate::http_runtime::HttpRuntimeError::Cancelled => 7,
        crate::http_runtime::HttpRuntimeError::BodyConsumed => 8,
        crate::http_runtime::HttpRuntimeError::InvalidJson => 9,
    }
}

/// Serves one bounded HTTP/1.1 WebSocket session and invokes its generated
/// Zirk callback on the accepted stream's owner task.
///
/// # Safety
/// Strings/lists must be live managed values, `callback` must follow the
/// declared ABI, and `out` must point to writable `HttpServerServeOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_http_server_websocket_serve_once(
    stream_handle: u64,
    secure: i32,
    path: *mut c_void,
    allowed_origins: *mut c_void,
    allow_missing_origin: i32,
    protocols: *mut c_void,
    max_messages: u64,
    callback: Option<HttpWebSocketServerCallback>,
    context: *mut c_void,
    out: *mut HttpServerServeOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let callback = callback.ok_or(2)?;
        let path = unsafe { crate::string::borrow(path) }.ok_or(0)?;
        let path = unsafe { path.as_str() }.to_owned();
        let copy_list = |list: *mut c_void| -> Result<Vec<String>, i32> {
            if list.is_null() {
                return Err(0);
            }
            let count = unsafe { crate::list::zirk_rt_list_length(list) };
            if count > 64 {
                return Err(0);
            }
            let mut values = Vec::with_capacity(count);
            for index in 0..count {
                let slot = unsafe {
                    crate::list::zirk_rt_list_element(
                        list,
                        index as i64,
                        std::mem::size_of::<*mut c_void>(),
                    )
                };
                if slot.is_null() {
                    return Err(0);
                }
                let item = unsafe { *slot.cast::<*mut c_void>() };
                let item = unsafe { crate::string::borrow(item) }.ok_or(0)?;
                values.push(unsafe { item.as_str() }.to_owned());
            }
            Ok(values)
        };
        let origins = copy_list(allowed_origins)?;
        let protocols = copy_list(protocols)?;
        if max_messages == 0 || max_messages > 1_024 {
            return Err(0);
        }
        let origin_refs = origins.iter().map(String::as_str).collect::<Vec<_>>();
        let protocol_refs = protocols.iter().map(String::as_str).collect::<Vec<_>>();
        let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
            &origin_refs,
            allow_missing_origin != 0,
            &protocol_refs,
        )
        .map_err(|_| 0)?;
        let stream = if secure != 0 {
            crate::network_api::take_tls_server_stream_for_http(stream_handle)
                .map(crate::http_runtime::HttpServerIo::from)
        } else {
            crate::network_api::take_tcp_stream_for_http(stream_handle)
                .map(crate::http_runtime::HttpServerIo::from)
        }
        .map_err(|error| if error == 12 { 1 } else { 2 })?;
        crate::http_runtime::serve_http1_websocket_once(
            stream,
            &path,
            policy,
            max_messages as usize,
            move |socket| {
                let owner = crate::executor::current_task();
                let mut sockets = http_websockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if sockets.len() >= MAX_HTTP_WEBSOCKETS {
                    return Err(crate::http_runtime::HttpRuntimeError::Transport);
                }
                let mut handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                while sockets.contains_key(&handle) {
                    handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                }
                sockets.insert(handle, OwnedHttpWebSocket { owner, socket });
                drop(sockets);

                let callback_result = unsafe { callback(handle, context) };
                let socket = http_websockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&handle);
                if let Some(mut socket) = socket {
                    if socket.owner != owner {
                        return Err(crate::http_runtime::HttpRuntimeError::PermissionDenied);
                    }
                    socket.socket.close()?;
                }
                if callback_result == 0 {
                    Ok(())
                } else {
                    Err(crate::http_runtime::HttpRuntimeError::Protocol)
                }
            },
        )
        .map_err(error_code)
    })();
    let result = match outcome {
        Ok(()) => HttpServerServeOut { kind: 0, error: 0 },
        Err(error) => HttpServerServeOut { kind: 1, error },
    };
    unsafe { out.write(result) };
}

/// Binds one permission-checked HTTP/3 WebSocket endpoint and invokes its
/// generated Zirk callback on the owning task after an RFC 9220 handshake.
///
/// # Safety
/// Managed strings/lists, callback/context, and `out` must satisfy their Zirk
/// ABI layouts and remain live for the duration of this call.
#[unsafe(no_mangle)]
// Keep one result-producing body across feature configurations so that `?`
// maps directly to the public ABI error codes when HTTP/3 is enabled.
#[allow(clippy::redundant_closure_call)]
pub unsafe extern "C-unwind" fn zirk_http_server_websocket_serve_http3_once(
    endpoint: *const c_void,
    certificate_chain_der: *mut c_void,
    private_key_der: *mut c_void,
    path: *mut c_void,
    allowed_origins: *mut c_void,
    allow_missing_origin: i32,
    protocols: *mut c_void,
    max_messages: u64,
    idle_timeout_ms: u64,
    callback: Option<HttpWebSocketServerCallback>,
    context: *mut c_void,
    out: *mut HttpServerServeOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        #[cfg(not(feature = "http3"))]
        {
            let _ = (
                endpoint,
                certificate_chain_der,
                private_key_der,
                path,
                allowed_origins,
                allow_missing_origin,
                protocols,
                max_messages,
                idle_timeout_ms,
                callback,
                context,
            );
            Err(3)
        }
        #[cfg(feature = "http3")]
        {
            let callback = callback.ok_or(2)?;
            let endpoint_value = unsafe { crate::string::borrow(endpoint.cast_mut()) }.ok_or(0)?;
            let address =
                crate::network_api::parse_udp_endpoint(unsafe { endpoint_value.as_str() })
                    .map_err(|error| match error {
                        crate::network_transport::NetworkTransportError::PermissionDenied => 1,
                        _ => 0,
                    })?;
            let path_value = unsafe { crate::string::borrow(path) }.ok_or(0)?;
            let path = unsafe { path_value.as_str() }.to_owned();
            if path.is_empty()
                || !path.starts_with('/')
                || path.contains(['?', '#', '\r', '\n'])
                || max_messages == 0
                || max_messages > 1_024
                || idle_timeout_ms == 0
                || idle_timeout_ms > 600_000
            {
                return Err(0);
            }
            let copy_list = |list: *mut c_void| -> Result<Vec<String>, i32> {
                if list.is_null() {
                    return Err(0);
                }
                let count = unsafe { crate::list::zirk_rt_list_length(list) };
                if count > 64 {
                    return Err(0);
                }
                let mut values = Vec::with_capacity(count);
                for index in 0..count {
                    let slot = unsafe {
                        crate::list::zirk_rt_list_element(
                            list,
                            index as i64,
                            std::mem::size_of::<*mut c_void>(),
                        )
                    };
                    if slot.is_null() {
                        return Err(0);
                    }
                    let item = unsafe { *slot.cast::<*mut c_void>() };
                    let item = unsafe { crate::string::borrow(item) }.ok_or(0)?;
                    values.push(unsafe { item.as_str() }.to_owned());
                }
                Ok(values)
            };
            let origins = copy_list(allowed_origins)?;
            let protocols = copy_list(protocols)?;
            let origin_refs = origins.iter().map(String::as_str).collect::<Vec<_>>();
            let protocol_refs = protocols.iter().map(String::as_str).collect::<Vec<_>>();
            let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
                &origin_refs,
                allow_missing_origin != 0,
                &protocol_refs,
            )
            .map_err(|_| 0)?;
            let certificates = unsafe { copy_http_trust_roots(certificate_chain_der) }.ok_or(0)?;
            let private_key = unsafe { crate::array::copy_byte_array(private_key_der, 64 * 1024) }
                .filter(|key| !key.is_empty())
                .ok_or(0)?;
            let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
            let server_config = crate::network_transport::TlsServerConfig::from_der(
                certificates,
                private_key,
                false,
            )
            .and_then(crate::network_transport::TlsServerConfig::into_http3_quic_config)
            .map_err(|_| 3)?;
            let (endpoint, _) =
                crate::http3_transport::bind_server_endpoint(address, &grant, server_config)
                    .map_err(error_code)?;
            let (io, selected, driver, close_drain) =
                crate::http3_transport::accept_websocket_endpoint(endpoint, path, policy)
                    .map_err(error_code)?;
            let socket = crate::http_runtime::HttpWebSocket::from_http3_server_stream(
                io,
                selected,
                std::time::Duration::from_millis(idle_timeout_ms),
                max_messages as usize,
                driver,
                close_drain,
            )
            .map_err(error_code)?;
            let owner = crate::executor::current_task();
            let handle = {
                let mut sockets = http_websockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if sockets.len() >= MAX_HTTP_WEBSOCKETS {
                    return Err(2);
                }
                let mut handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                while sockets.contains_key(&handle) {
                    handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                }
                sockets.insert(handle, OwnedHttpWebSocket { owner, socket });
                handle
            };
            let callback_result = unsafe { callback(handle, context) };
            let socket = http_websockets()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&handle);
            if let Some(mut socket) = socket {
                if socket.owner != owner {
                    return Err(1);
                }
                socket.socket.close().map_err(error_code)?;
            }
            if callback_result != 0 {
                return Err(3);
            }
            Ok(())
        }
    })();
    let result = match outcome {
        Ok(()) => HttpServerServeOut { kind: 0, error: 0 },
        Err(error) => HttpServerServeOut { kind: 1, error },
    };
    unsafe { out.write(result) };
}

static NEXT_STREAM_HANDLE: AtomicU64 = AtomicU64::new(1);
static STREAMS: OnceLock<Mutex<HashMap<u64, OwnedHttpStream>>> = OnceLock::new();
const MAX_HTTP_STREAMS: usize = 1_024;

static NEXT_UPLOAD_HANDLE: AtomicU64 = AtomicU64::new(1);
static HTTP_UPLOADS: OnceLock<Mutex<HashMap<u64, OwnedHttpUpload>>> = OnceLock::new();
const MAX_HTTP_UPLOADS: usize = 256;

struct OwnedHttpUpload {
    owner: TaskId,
    file: File,
    path: PathBuf,
    size: u64,
}

impl Drop for OwnedHttpUpload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn http_uploads() -> &'static Mutex<HashMap<u64, OwnedHttpUpload>> {
    HTTP_UPLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn close_http_upload(handle: u64, owner: TaskId) -> bool {
    let mut uploads = http_uploads()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match uploads.get(&handle) {
        None => true,
        Some(upload) if upload.owner != owner => false,
        Some(_) => {
            uploads.remove(&handle);
            true
        }
    }
}

fn create_http_upload(bytes: &[u8], owner: TaskId) -> Result<u64, ()> {
    {
        let uploads = http_uploads()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if uploads.len() >= MAX_HTTP_UPLOADS {
            return Err(());
        }
    }

    let directory = std::env::temp_dir();
    let mut resource = None;
    for _ in 0..64 {
        let sequence = NEXT_UPLOAD_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        let path = directory.join(format!(
            "zirk-http-upload-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                resource = Some(OwnedHttpUpload {
                    owner,
                    file,
                    path,
                    size: bytes.len() as u64,
                });
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(()),
        }
    }
    let mut resource = resource.ok_or(())?;
    resource.file.write_all(bytes).map_err(|_| ())?;
    resource.file.seek(SeekFrom::Start(0)).map_err(|_| ())?;

    let mut uploads = http_uploads()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if uploads.len() >= MAX_HTTP_UPLOADS {
        return Err(());
    }
    let mut handle = NEXT_UPLOAD_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
    while handle == 0 || uploads.contains_key(&handle) {
        handle = NEXT_UPLOAD_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
    }
    uploads.insert(handle, resource);
    Ok(handle)
}

pub(crate) fn close_task_uploads(owner: TaskId) {
    http_uploads()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, upload| upload.owner != owner);
}

struct OwnedHttpStream {
    owner: TaskId,
    stream: crate::http_runtime::HttpStreamingResponse,
    stream_read_started: bool,
    sse_started: bool,
    sse_parser: Option<zirk_runtime_core::http_sse::SseParser>,
    sse_queue: std::collections::VecDeque<zirk_runtime_core::http_sse::SseEvent>,
}

fn streams() -> &'static Mutex<HashMap<u64, OwnedHttpStream>> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drops all response streams left open by a task before its completion is
/// observable. Dropping the runtime stream aborts its owned connection.
pub(crate) fn close_task_streams(owner: TaskId) {
    streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    http_websockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
}

/// Closes shareable HTTP clients after every task in their executor has
/// completed, preserving cross-task use while preventing process-global leaks.
pub(crate) fn close_executor_clients(executor_id: u64) {
    let clients = {
        let mut registered = http_clients()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let handles = registered
            .iter()
            .filter_map(|(handle, client)| (client.executor_id == executor_id).then_some(*handle))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|handle| registered.remove(&handle))
            .collect::<Vec<_>>()
    };
    for client in clients {
        client.closed.store(true, Ordering::Release);
        client
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .close();
    }
}

#[repr(C)]
pub struct HttpStreamOpenOut {
    pub kind: i32,
    pub error: i32,
    pub status: i32,
    pub handle: u64,
    pub headers: *mut c_void,
}

#[repr(C)]
pub struct HttpStreamReadOut {
    /// 0 means one chunk, 1 is clean EOF, 2 is a typed error.
    pub kind: i32,
    pub error: i32,
    pub bytes: *mut c_void,
}

#[repr(C)]
pub struct HttpGetOut {
    /// `0` is success; `1` is a typed failure.
    pub kind: i32,
    /// `HTTPError` discriminant, meaningful only when kind is 1.
    pub error: i32,
    pub status: i32,
    /// Managed `Array<Byte>` response body on success, null on failure.
    pub body: *mut c_void,
    /// Managed length-delimited response header block on success.
    pub headers: *mut c_void,
    /// Negotiated protocol version; zero means unknown or not applicable.
    pub protocol: i32,
}

#[repr(C)]
pub struct HttpClientRequestOut {
    pub kind: i32,
    pub error: i32,
    pub status: i32,
    pub body: *mut c_void,
    pub headers: *mut c_void,
    pub protocol: i32,
}

#[repr(C)]
pub struct HttpClientCreateOut {
    pub kind: i32,
    pub error: i32,
    pub handle: u64,
}

#[repr(C)]
pub struct HttpClientActionOut {
    pub kind: i32,
    pub error: i32,
}

#[repr(C)]
pub struct HttpUrlOut {
    pub kind: i32,
    pub error: i32,
    pub normalized: *mut c_void,
}

#[repr(C)]
pub struct HttpTextOut {
    pub kind: i32,
    pub error: i32,
    /// Owned Zirk `String` on success, null on failure.
    pub value: *mut c_void,
}

fn failure(code: i32) -> HttpGetOut {
    HttpGetOut {
        kind: 1,
        error: code,
        status: 0,
        body: std::ptr::null_mut(),
        headers: std::ptr::null_mut(),
        protocol: crate::http_runtime::HttpProtocolVersion::Unknown as i32,
    }
}

fn client_failure(code: i32) -> HttpClientRequestOut {
    HttpClientRequestOut {
        kind: 1,
        error: code,
        status: 0,
        body: std::ptr::null_mut(),
        headers: std::ptr::null_mut(),
        protocol: crate::http_runtime::HttpProtocolVersion::Unknown as i32,
    }
}

fn successful_client_response(
    status: u16,
    headers: &Headers,
    body: &[u8],
    protocol: crate::http_runtime::HttpProtocolVersion,
) -> Result<HttpClientRequestOut, i32> {
    let response = successful_response(status, headers, body, protocol)?;
    Ok(HttpClientRequestOut {
        kind: response.kind,
        error: response.error,
        status: response.status,
        body: response.body,
        headers: response.headers,
        protocol: protocol as i32,
    })
}

fn serialize_response_headers(headers: &Headers) -> Result<Vec<u8>, i32> {
    headers.validate_accept_query().map_err(|_| 3)?;
    serialize_headers(headers)
}

fn serialize_request_headers(headers: &Headers) -> Result<Vec<u8>, i32> {
    serialize_headers(headers)
}

fn serialize_headers(headers: &Headers) -> Result<Vec<u8>, i32> {
    let mut names = std::collections::BTreeSet::new();
    for (name, _) in headers.iter() {
        names.insert(name.as_str().to_owned());
    }
    let mut entries = Vec::new();
    for name in names {
        let values = headers.get_all(&name).ok_or(3)?;
        for value in values {
            let name_bytes = name.as_bytes();
            let value_bytes = value.as_bytes();
            let name_len = u16::try_from(name_bytes.len()).map_err(|_| 3)?;
            let value_len = u32::try_from(value_bytes.len()).map_err(|_| 3)?;
            entries.push((
                name_bytes.to_vec(),
                value_bytes.to_vec(),
                name_len,
                value_len,
            ));
        }
    }
    let count = u32::try_from(entries.len()).map_err(|_| 3)?;
    if entries.len() > zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT {
        return Err(3);
    }
    let total_bytes = entries
        .iter()
        .try_fold(4usize, |total, (name, value, _, _)| {
            total
                .checked_add(6)?
                .checked_add(name.len())?
                .checked_add(value.len())
        });
    let Some(total_bytes) = total_bytes else {
        return Err(3);
    };
    if total_bytes > zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES + 6 * entries.len() + 4 {
        return Err(3);
    }
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&count.to_le_bytes());
    for (name, value, name_len, value_len) in entries {
        encoded.extend_from_slice(&name_len.to_le_bytes());
        encoded.extend_from_slice(&value_len.to_le_bytes());
        encoded.extend_from_slice(&name);
        encoded.extend_from_slice(&value);
    }
    Ok(encoded)
}

fn successful_response(
    status: u16,
    headers: &Headers,
    body: &[u8],
    protocol: crate::http_runtime::HttpProtocolVersion,
) -> Result<HttpGetOut, i32> {
    let encoded_headers = serialize_response_headers(headers)?;
    let body = crate::array::alloc_byte_array(body);
    let mut rooted_body = body;
    let mut roots = [(&mut rooted_body as *mut *mut c_void).cast()];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let headers = crate::array::alloc_byte_array(&encoded_headers);
    crate::zirk_rt_pop_frame();
    Ok(HttpGetOut {
        kind: 0,
        error: 0,
        status: i32::from(status),
        body: rooted_body,
        headers,
        protocol: protocol as i32,
    })
}

/// Runs a generated Zirk handler after the transport has parsed and forwarded
/// the bounded request. HTTP/1.1 uses the caller's task; multiplexed HTTP/2
/// creates an independently cancelable task on the same executor. The handler
/// receives rooted managed request values and returns only bounded response
/// values through the C ABI.
pub(crate) fn serve_once_with_callback(
    stream: impl Into<crate::http_runtime::HttpServerIo>,
    secure: bool,
    max_requests: u64,
    require_http2: bool,
    callback: HttpServerCallback,
    context: *mut c_void,
) -> Result<(), crate::http_runtime::HttpRuntimeError> {
    if require_http2 {
        let context = context as usize;
        return crate::http_runtime::serve_http2_requests(
            stream,
            secure,
            max_requests,
            move |request| invoke_http_server_callback(request, callback, context as *mut c_void),
        );
    }
    crate::http_runtime::serve_once(stream, secure, move |request| {
        invoke_http_server_callback(request, callback, context)
    })
}

fn invoke_http_server_callback(
    request: zirk_runtime_core::http_values::HttpRequestValues,
    callback: HttpServerCallback,
    context: *mut c_void,
) -> crate::http_runtime::HttpResponseValues {
    let mut target = request.url.path().to_owned();
    if let Some(query) = request.url.query() {
        target.push('?');
        target.push_str(query);
    }
    let encoded_headers = match serialize_request_headers(&request.headers) {
        Ok(value) => value,
        Err(_) => return server_error_response(),
    };

    let mut rooted_method = crate::string::alloc_owned(request.method.as_str());
    let mut rooted_target = crate::string::alloc_owned(&target);
    let mut rooted_headers = crate::array::alloc_byte_array(&encoded_headers);
    let mut rooted_body = crate::array::alloc_byte_array(&request.body);
    let mut roots = [
        (&mut rooted_method as *mut *mut c_void).cast(),
        (&mut rooted_target as *mut *mut c_void).cast(),
        (&mut rooted_headers as *mut *mut c_void).cast(),
        (&mut rooted_body as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let borrowed = HttpServerRequest {
        method: rooted_method,
        target: rooted_target,
        headers: rooted_headers,
        body: rooted_body,
    };
    let mut response = HttpServerResponse {
        status: 500,
        headers: std::ptr::null_mut(),
        body: std::ptr::null_mut(),
        producer_handle: 0,
        sse_heartbeat_ms: 0,
    };
    let status = unsafe { callback(&borrowed, &mut response, context) };
    // A source-level handler may propagate a Throwable across the generated
    // callback ABI even when the public wrapper attempted to translate it.
    // Consume only an actual pending exception here; cooperative cancellation
    // and other callback status codes without an exception remain on their
    // existing error path.
    if status != 0 && crate::exceptions::zirk_rt_has_pending_exception() {
        let _ = crate::exceptions::zirk_rt_take_pending_exception();
        crate::zirk_rt_pop_frame();
        return internal_server_error_response();
    }
    // Take ownership immediately so every validation failure below drops the
    // producer and releases its capture root instead of leaving it registered
    // until task shutdown.
    let mut body_producer = if response.producer_handle == 0 {
        None
    } else {
        match take_http_server_producer(response.producer_handle) {
            Ok(producer) => Some(producer),
            Err(_) => {
                crate::zirk_rt_pop_frame();
                return server_error_response();
            }
        }
    };
    if let Some(producer) = body_producer.as_mut()
        && producer
            .set_sse_heartbeat_ms(response.sse_heartbeat_ms)
            .is_err()
    {
        crate::zirk_rt_pop_frame();
        return server_error_response();
    }
    let response_body = if status == 0 {
        unsafe {
            crate::array::copy_byte_array(
                response.body,
                zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES,
            )
        }
    } else {
        None
    };
    let response_headers = if status == 0 {
        unsafe {
            crate::array::copy_byte_array(response.headers, MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES)
        }
    } else {
        None
    };
    crate::zirk_rt_pop_frame();

    let (Some(body), Some(encoded_headers), Ok(status)) = (
        response_body,
        response_headers,
        u16::try_from(response.status)
            .ok()
            .and_then(|status| http::StatusCode::from_u16(status).ok())
            .map(|status| status.as_u16())
            .ok_or(()),
    ) else {
        return server_error_response();
    };
    let Ok(headers) = decode_all_headers(&encoded_headers) else {
        return server_error_response();
    };
    crate::http_runtime::HttpResponseValues {
        status: zirk_runtime_core::http_values::HttpStatusCode::new(status)
            .expect("validated HTTP status"),
        headers,
        body,
        body_stream: None,
        body_producer,
        protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
    }
}

fn server_error_response() -> crate::http_runtime::HttpResponseValues {
    crate::http_runtime::HttpResponseValues {
        status: zirk_runtime_core::http_values::HttpStatusCode::new(500)
            .expect("HTTP 500 is valid"),
        headers: Headers::new(),
        body: b"invalid handler response".to_vec(),
        body_stream: None,
        body_producer: None,
        protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
    }
}

fn internal_server_error_response() -> crate::http_runtime::HttpResponseValues {
    crate::http_runtime::HttpResponseValues {
        status: zirk_runtime_core::http_values::HttpStatusCode::new(500)
            .expect("HTTP 500 is valid"),
        headers: Headers::new(),
        body: b"Internal Server Error".to_vec(),
        body_stream: None,
        body_producer: None,
        protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
    }
}

fn decode_all_headers(encoded: &[u8]) -> Result<Headers, i32> {
    decode_headers(encoded, true)
}

fn decode_response_headers(encoded: &[u8]) -> Result<Headers, i32> {
    decode_headers(encoded, false)
}

fn decode_headers(encoded: &[u8], reject_framing: bool) -> Result<Headers, i32> {
    if encoded.len() < 4 {
        return Err(3);
    }
    let count = u32::from_le_bytes(encoded[..4].try_into().map_err(|_| 3)?) as usize;
    if count > zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT {
        return Err(3);
    }
    let mut cursor = 4usize;
    let mut headers = Headers::new();
    for _ in 0..count {
        let header = encoded.get(cursor..cursor + 6).ok_or(3)?;
        let encoded_name_len = u16::from_le_bytes(header[..2].try_into().map_err(|_| 3)?);
        let sensitive = encoded_name_len & 0x8000 != 0;
        let name_len = (encoded_name_len & 0x7fff) as usize;
        let value_len = u32::from_le_bytes(header[2..6].try_into().map_err(|_| 3)?) as usize;
        cursor += 6;
        let name_end = cursor.checked_add(name_len).ok_or(3)?;
        let value_end = name_end.checked_add(value_len).ok_or(3)?;
        let name = std::str::from_utf8(encoded.get(cursor..name_end).ok_or(3)?).map_err(|_| 3)?;
        let value = encoded.get(name_end..value_end).ok_or(3)?;
        if reject_framing
            && matches!(
                name.to_ascii_lowercase().as_str(),
                "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "keep-alive"
                    | "proxy-connection"
                    | "upgrade"
                    | "trailer"
                    | "te"
                    | "host"
            )
        {
            return Err(3);
        }
        if sensitive {
            headers.append_sensitive(name, value).map_err(|_| 3)?;
        } else {
            headers.append(name, value).map_err(|_| 3)?;
        }
        cursor = value_end;
    }
    if cursor != encoded.len() {
        return Err(3);
    }
    Ok(headers)
}

fn decode_header_values(encoded: &[u8], requested_name: &str) -> Result<Vec<String>, i32> {
    if http::HeaderName::from_bytes(requested_name.as_bytes()).is_err() || encoded.len() < 4 {
        return Err(0);
    }
    let count = u32::from_le_bytes(encoded[..4].try_into().map_err(|_| 3)?) as usize;
    if count > zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT {
        return Err(3);
    }
    let mut cursor = 4usize;
    let mut matches = Vec::new();
    for _ in 0..count {
        let header = encoded.get(cursor..cursor + 6).ok_or(3)?;
        let name_len = u16::from_le_bytes(header[..2].try_into().map_err(|_| 3)?) as usize;
        let value_len = u32::from_le_bytes(header[2..6].try_into().map_err(|_| 3)?) as usize;
        cursor += 6;
        let end_name = cursor.checked_add(name_len).ok_or(3)?;
        let name = encoded.get(cursor..end_name).ok_or(3)?;
        let end_value = end_name.checked_add(value_len).ok_or(3)?;
        let value = encoded.get(end_name..end_value).ok_or(3)?;
        cursor = end_value;
        let parsed_name = http::HeaderName::from_bytes(name).map_err(|_| 3)?;
        http::HeaderValue::from_bytes(value).map_err(|_| 3)?;
        if parsed_name.as_str().eq_ignore_ascii_case(requested_name) {
            matches.push(String::from_utf8(value.to_vec()).map_err(|_| 5)?);
        }
    }
    if cursor != encoded.len() {
        return Err(3);
    }
    Ok(matches)
}

fn text_failure(code: i32) -> HttpTextOut {
    HttpTextOut {
        kind: 1,
        error: code,
        value: std::ptr::null_mut(),
    }
}

fn copy_response_bytes(body: *const c_void) -> Result<Vec<u8>, i32> {
    if body.is_null() {
        return Err(0);
    }
    let body = body.cast_mut();
    let length = unsafe { crate::array::zirk_rt_array_length(body) };
    if length > zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES {
        return Err(4);
    }
    let mut bytes = Vec::with_capacity(length);
    for index in 0..length {
        let slot = unsafe { crate::array::zirk_rt_array_element(body, index as i64, 1) };
        bytes.push(unsafe { *slot.cast::<u8>() });
    }
    Ok(bytes)
}

fn decode_response_text(bytes: Vec<u8>) -> Result<String, i32> {
    String::from_utf8(bytes).map_err(|_| 5)
}

/// Decodes a managed HTTP response byte buffer as strict UTF-8.
///
/// # Safety
/// `body` is a live `Array<Byte>` and `out` points to writable `HttpTextOut`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_response_text(body: *mut c_void, out: *mut HttpTextOut) {
    let outcome = match copy_response_bytes(body).and_then(decode_response_text) {
        Ok(text) => HttpTextOut {
            kind: 0,
            error: 0,
            value: crate::string::alloc_owned(&text),
        },
        Err(error) => text_failure(error),
    };
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

fn error_code(error: crate::http_runtime::HttpRuntimeError) -> i32 {
    use crate::http_runtime::HttpRuntimeError as E;
    match error {
        E::InvalidRequest => 0,
        E::PermissionDenied => 1,
        E::Transport => 2,
        E::Protocol => 3,
        E::BodyTooLarge => 4,
        E::InvalidText => 5,
        E::Timeout => 6,
        E::Cancelled => 7,
        E::BodyConsumed => 8,
        E::InvalidJson => 9,
        E::UnsupportedMethod => 14,
    }
}

fn next_http_upload_chunk(
    callback: HttpUploadProducerCallback,
    context: *mut c_void,
    producer_errors: i32,
    chunk_index: &mut u64,
) -> Result<Option<Vec<u8>>, crate::http_runtime::HttpRuntimeError> {
    let mut chunk = std::ptr::null_mut();
    let status = unsafe { callback(context, *chunk_index, &mut chunk) };
    *chunk_index = chunk_index
        .checked_add(1)
        .ok_or(crate::http_runtime::HttpRuntimeError::BodyTooLarge)?;
    if crate::executor::current_task_cancel_requested() {
        return Err(crate::http_runtime::HttpRuntimeError::Cancelled);
    }
    if status != 0 {
        return Err(if producer_errors == 0 {
            crate::http_runtime::HttpRuntimeError::Cancelled
        } else {
            match status - 1 {
                0 => crate::http_runtime::HttpRuntimeError::Cancelled,
                1 => crate::http_runtime::HttpRuntimeError::Transport,
                _ => crate::http_runtime::HttpRuntimeError::Protocol,
            }
        });
    }
    if chunk.is_null() {
        return Ok(None);
    }
    unsafe { crate::array::copy_byte_array(chunk, crate::http_runtime::HTTP_UPLOAD_CHUNK_LIMIT) }
        .map(Some)
        .ok_or(crate::http_runtime::HttpRuntimeError::BodyTooLarge)
}

fn validate_client_request(request: &HttpRequestValues) -> Result<(), i32> {
    request
        .validate_client_request()
        .map_err(|error| match error {
            zirk_runtime_core::http_values::HttpValueError::UnsupportedMethod => 14,
            _ => 3,
        })
}

/// Creates a bounded reusable HTTP client owned by the current Zirk task.
/// The verified project and manifest identity are captured at creation.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientCreateOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create(max_origins: i32, out: *mut HttpClientCreateOut) {
    if out.is_null() {
        return;
    }
    let outcome = create_http_client(max_origins, None);
    unsafe { out.write(client_create_out(outcome)) };
}

/// Creates a reusable client with independently bounded timeout phases.
/// Timeout arguments are milliseconds: connection establishment, response
/// headers, and response-body idle time, in that order.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientCreateOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create_with_options(
    max_origins: i32,
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let timeouts = http_timeouts_from_millis(
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
        )?;
        create_http_client_config_with_timeouts(max_origins, None, None, timeouts)
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

/// Creates a reusable HTTP client with timeouts and an explicit protocol
/// preference. The legacy options entrypoint remains available to already
/// compiled callers and continues to select `Auto`.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientCreateOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create_with_protocol_options(
    max_origins: i32,
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
    protocol_code: i32,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let protocol = crate::http_runtime::HttpProtocolPreference::try_from(protocol_code)
            .map_err(error_code)?;
        let timeouts = http_timeouts_from_millis(
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
        )?;
        create_http_client_config_with_timeouts_and_protocol(
            max_origins,
            None,
            None,
            timeouts,
            protocol,
        )
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

fn http_timeouts_from_millis(
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
) -> Result<crate::http_runtime::HttpTimeouts, i32> {
    let timeout = |millis: u64| {
        (1..=3_600_000)
            .contains(&millis)
            .then(|| std::time::Duration::from_millis(millis))
            .ok_or(3)
    };
    Ok(crate::http_runtime::HttpTimeouts {
        connect: timeout(connect_timeout_ms)?,
        response_headers: timeout(response_headers_timeout_ms)?,
        response_idle: timeout(response_idle_timeout_ms)?,
    })
}

/// Creates a reusable client whose HTTPS connections trust exactly the
/// caller-supplied DER roots. The default constructor continues to use native
/// system roots.
///
/// # Safety
/// `roots` must be a live `Array<Array<Byte>>`; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create_with_trust_roots(
    max_origins: i32,
    roots: *mut c_void,
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
    protocol_code: i32,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let roots = unsafe { copy_http_trust_roots(roots) }.ok_or(3)?;
        let config = crate::network_transport::client_tls_config_with_trust_roots(roots, false)
            .map_err(|_| 3)?;
        let timeouts = http_timeouts_from_millis(
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
        )?;
        let protocol = crate::http_runtime::HttpProtocolPreference::try_from(protocol_code)
            .map_err(error_code)?;
        create_http_client_config_with_timeouts_and_protocol(
            max_origins,
            Some(config),
            None,
            timeouts,
            protocol,
        )
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

/// Creates a reusable HTTP client with an explicitly configured HTTP proxy.
///
/// # Safety
/// `proxy_url` must be a live managed Zirk `String`; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create_with_proxy(
    proxy_url: *mut c_void,
    proxy_credentials: *mut c_void,
    max_origins: i32,
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
    protocol_code: i32,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let proxy_url = unsafe { crate::string::borrow(proxy_url) }.ok_or(3)?;
        let proxy_credentials = unsafe { crate::string::borrow(proxy_credentials) }.ok_or(3)?;
        let proxy = zirk_runtime_core::http_proxy::HttpProxy::parse_with_credentials(
            unsafe { proxy_url.as_str() },
            match unsafe { proxy_credentials.as_str() }.is_empty() {
                true => None,
                false => Some(unsafe { proxy_credentials.as_str() }),
            },
        )
        .map_err(|_| 3)?;
        let timeouts = http_timeouts_from_millis(
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
        )?;
        let protocol = crate::http_runtime::HttpProtocolPreference::try_from(protocol_code)
            .map_err(error_code)?;
        create_http_client_config_with_timeouts_and_protocol(
            max_origins,
            None,
            Some(proxy),
            timeouts,
            protocol,
        )
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

/// Creates a reusable HTTP client with proxy routing and explicit destination
/// trust roots for both proxy TLS and destination TLS. Basic proxy credentials
/// are allowed only for loopback HTTP proxies or HTTPS proxies; remote
/// cleartext HTTP credentials are rejected.
///
/// # Safety
/// `proxy_url`, `proxy_credentials`, and `roots` must be live Zirk values;
/// `out` must point to writable `HttpClientCreateOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_create_with_proxy_and_trust_roots(
    proxy_url: *mut c_void,
    proxy_credentials: *mut c_void,
    max_origins: i32,
    roots: *mut c_void,
    connect_timeout_ms: u64,
    response_headers_timeout_ms: u64,
    response_idle_timeout_ms: u64,
    protocol_code: i32,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let proxy_url = unsafe { crate::string::borrow(proxy_url) }.ok_or(3)?;
        let proxy_credentials = unsafe { crate::string::borrow(proxy_credentials) }.ok_or(3)?;
        let proxy = zirk_runtime_core::http_proxy::HttpProxy::parse_with_credentials(
            unsafe { proxy_url.as_str() },
            match unsafe { proxy_credentials.as_str() }.is_empty() {
                true => None,
                false => Some(unsafe { proxy_credentials.as_str() }),
            },
        )
        .map_err(|_| 3)?;
        let roots = unsafe { copy_http_trust_roots(roots) }.ok_or(3)?;
        let config = crate::network_transport::client_tls_config_with_trust_roots(roots, false)
            .map_err(|_| 3)?;
        let timeouts = http_timeouts_from_millis(
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
        )?;
        let protocol = crate::http_runtime::HttpProtocolPreference::try_from(protocol_code)
            .map_err(error_code)?;
        create_http_client_config_with_timeouts_and_protocol(
            max_origins,
            Some(config),
            Some(proxy),
            timeouts,
            protocol,
        )
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

fn create_http_client(
    max_origins: i32,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
) -> Result<u64, i32> {
    create_http_client_config(max_origins, tls_config, None)
}

fn create_http_client_config(
    max_origins: i32,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    proxy: Option<zirk_runtime_core::http_proxy::HttpProxy>,
) -> Result<u64, i32> {
    create_http_client_config_with_timeouts(
        max_origins,
        tls_config,
        proxy,
        crate::http_runtime::HttpTimeouts::default(),
    )
}

fn create_http_client_config_with_timeouts(
    max_origins: i32,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    proxy: Option<zirk_runtime_core::http_proxy::HttpProxy>,
    timeouts: crate::http_runtime::HttpTimeouts,
) -> Result<u64, i32> {
    create_http_client_config_with_timeouts_and_protocol(
        max_origins,
        tls_config,
        proxy,
        timeouts,
        crate::http_runtime::HttpProtocolPreference::Auto,
    )
}

fn create_http_client_config_with_timeouts_and_protocol(
    max_origins: i32,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    proxy: Option<zirk_runtime_core::http_proxy::HttpProxy>,
    timeouts: crate::http_runtime::HttpTimeouts,
    protocol: crate::http_runtime::HttpProtocolPreference,
) -> Result<u64, i32> {
    (|| {
        let verified = crate::permissions::verified_grant().ok_or(1)?;
        let max_origins = usize::try_from(max_origins).map_err(|_| 0)?;
        let pool = crate::http_runtime::HttpClientPool::new_shared_with_protocol(
            max_origins,
            timeouts,
            protocol,
            tls_config,
            proxy,
        )
        .map_err(error_code)?;
        let gate = crate::sync_primitives::CooperativeSemaphore::new(1, 1).map_err(|_| 2)?;
        let request_slots =
            crate::sync_primitives::CooperativeSemaphore::new(8, 8).map_err(|_| 2)?;
        let close_gate = crate::sync_primitives::CooperativeSemaphore::new(1, 1).map_err(|_| 2)?;
        let mut clients = http_clients()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if clients.len() >= MAX_HTTP_CLIENTS {
            return Err(2);
        }
        let mut handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        while clients.contains_key(&handle) {
            handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        }
        clients.insert(
            handle,
            Arc::new(OwnedHttpClient {
                executor_id: crate::executor::current_executor_id(),
                project_id: verified.project_id().to_owned(),
                manifest_digest: verified.manifest_digest().to_owned(),
                gate,
                request_slots,
                close_gate,
                pool: Mutex::new(pool),
                closed: AtomicBool::new(false),
            }),
        );
        Ok(handle)
    })()
}

fn client_create_out(outcome: Result<u64, i32>) -> HttpClientCreateOut {
    match outcome {
        Ok(handle) => HttpClientCreateOut {
            kind: 0,
            error: 0,
            handle,
        },
        Err(error) => HttpClientCreateOut {
            kind: 1,
            error,
            handle: 0,
        },
    }
}

unsafe fn copy_http_trust_roots(roots: *mut c_void) -> Option<Vec<Vec<u8>>> {
    if roots.is_null() {
        return None;
    }
    let count = unsafe { crate::array::zirk_rt_array_length(roots) };
    if count == 0 || count > 64 {
        return None;
    }
    let mut total = 0usize;
    let mut copied = Vec::with_capacity(count);
    for index in 0..count {
        let slot = unsafe {
            crate::array::zirk_rt_array_element(
                roots,
                index as i64,
                std::mem::size_of::<*mut c_void>(),
            )
        };
        let certificate = unsafe { slot.cast::<*mut c_void>().read_unaligned() };
        let certificate = unsafe { crate::array::copy_byte_array(certificate, 64 * 1024) }?;
        total = total.checked_add(certificate.len())?;
        if certificate.is_empty() || total > 1024 * 1024 {
            return None;
        }
        copied.push(certificate);
    }
    Some(copied)
}

/// Enables the client's private, bounded memory-only cookie jar.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientActionOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_enable_cookies(
    handle: u64,
    out: *mut HttpClientActionOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = client_by_handle(handle).ok_or(0).and_then(|client| {
        if !client_identity_matches(&client) {
            return Err(1);
        }
        let _request_slot = client
            .request_slots
            .acquire()
            .map_err(|error| match error {
                crate::sync_primitives::SemaphoreError::Cancelled => 7,
                _ => 0,
            })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let _permit = client.gate.acquire().map_err(|error| match error {
            crate::sync_primitives::SemaphoreError::Cancelled => 7,
            _ => 0,
        })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        client
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .enable_cookies();
        Ok(())
    });
    let outcome = action_outcome(outcome);
    unsafe { out.write(outcome) };
}

/// Clears the client's private in-memory cookie jar and zeroizes removed values.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientActionOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_clear_cookies(
    handle: u64,
    out: *mut HttpClientActionOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = client_by_handle(handle).ok_or(0).and_then(|client| {
        if !client_identity_matches(&client) {
            return Err(1);
        }
        let _request_slot = client
            .request_slots
            .acquire()
            .map_err(|error| match error {
                crate::sync_primitives::SemaphoreError::Cancelled => 7,
                _ => 0,
            })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let _permit = client.gate.acquire().map_err(|error| match error {
            crate::sync_primitives::SemaphoreError::Cancelled => 7,
            _ => 0,
        })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        client
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear_cookies();
        Ok(())
    });
    unsafe { out.write(action_outcome(outcome)) };
}

/// Sends a bounded request through an owner-task reusable client.
///
/// # Safety
/// Pointer arguments must refer to live Zirk values, and `out` must be
/// writable `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_request(
    handle: u64,
    method: *const c_void,
    url: *const c_void,
    body: *const c_void,
    headers: *const c_void,
    validate_json: bool,
    out: *mut HttpClientRequestOut,
) {
    let outcome = (|| {
        let client = client_by_handle(handle).ok_or(0)?;
        if !client_identity_matches(&client) {
            return Err(1);
        }
        let Some(method) = (unsafe { crate::string::borrow(method) }) else {
            return Err(0);
        };
        let Some(url) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let method_name = unsafe { method.as_str() };
        let method = match method_name {
            "GET" => HttpMethod::Get,
            "HEAD" => HttpMethod::Head,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "PATCH" => HttpMethod::Patch,
            "DELETE" => HttpMethod::Delete,
            "OPTIONS" => HttpMethod::Options,
            "CONNECT" => HttpMethod::Connect,
            "TRACE" => HttpMethod::Trace,
            "QUERY" => HttpMethod::Query,
            _ if hyper::Method::from_bytes(method_name.as_bytes()).is_ok() => HttpMethod::Other,
            _ => return Err(HTTP_ERROR_INVALID_ARGUMENT),
        };
        let url = Url::parse(unsafe { url.as_str() }).map_err(|_| 0)?;
        let body = copy_response_bytes(body)?;
        if validate_json {
            validate_json_document(&body).map_err(error_code)?;
        }
        let header_block = unsafe {
            crate::array::copy_byte_array(headers.cast_mut(), MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES)
        }
        .ok_or(0)?;
        let mut request = HttpRequestValues::new(method, url);
        request.method_token = method_name.to_owned();
        request.headers = decode_all_headers(&header_block)?;
        request.set_body(body).map_err(|_| 4)?;
        validate_client_request(&request)?;
        let request_for_invalidation = request.clone();
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let _request_slot = client
            .request_slots
            .acquire()
            .map_err(|error| match error {
                crate::sync_primitives::SemaphoreError::Cancelled => 7,
                _ => 0,
            })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let origin = request.url.origin();
        let request_url = request.url.clone();
        let prepared = {
            let _pool_permit = client.gate.acquire().map_err(|error| match error {
                crate::sync_primitives::SemaphoreError::Cancelled => 7,
                _ => 0,
            })?;
            if client.closed.load(Ordering::Acquire) {
                return Err(0);
            }
            let mut pool = client
                .pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            #[cfg(feature = "http3")]
            if let Some(session) = pool
                .prepare_shared_http3(&mut request, &grant)
                .map_err(error_code)?
            {
                PreparedHttpClientRequest::Http3(session, pool.timeouts(), request.clone())
            } else {
                pool.ensure_shared_connection(&request, Arc::clone(&grant))
                    .map_err(error_code)?;
                match pool
                    .prepare_shared_http2(&mut request, &grant)
                    .map_err(error_code)?
                {
                    Some(sender) => PreparedHttpClientRequest::Http2(sender, pool.timeouts()),
                    None => PreparedHttpClientRequest::Complete(
                        pool.request(request.clone(), Arc::clone(&grant))
                            .map_err(error_code)?,
                    ),
                }
            }
            #[cfg(not(feature = "http3"))]
            {
                pool.ensure_shared_connection(&request, Arc::clone(&grant))
                    .map_err(error_code)?;
                match pool
                    .prepare_shared_http2(&mut request, &grant)
                    .map_err(error_code)?
                {
                    Some(sender) => PreparedHttpClientRequest::Http2(sender, pool.timeouts()),
                    None => PreparedHttpClientRequest::Complete(
                        pool.request(request.clone(), Arc::clone(&grant))
                            .map_err(error_code)?,
                    ),
                }
            }
        };
        let response = match prepared {
            PreparedHttpClientRequest::Complete(response) => response,
            PreparedHttpClientRequest::Http2(sender, timeouts) => {
                let response = sender.send(request, &grant, timeouts);
                let _pool_permit = client.gate.acquire().map_err(|error| match error {
                    crate::sync_primitives::SemaphoreError::Cancelled => 7,
                    _ => 0,
                })?;
                let mut pool = client
                    .pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match response {
                    Ok(response) => {
                        pool.finish_shared_http2(&origin, &request_url, &response);
                        response
                    }
                    Err(error) => {
                        if matches!(
                            error,
                            crate::http_runtime::HttpRuntimeError::Transport
                                | crate::http_runtime::HttpRuntimeError::Protocol
                        ) {
                            pool.discard_shared_http2(&origin);
                        }
                        return Err(error_code(error));
                    }
                }
            }
            #[cfg(feature = "http3")]
            PreparedHttpClientRequest::Http3(session, timeouts, request) => {
                let response = crate::http_runtime::HttpClientPool::request_http3_session(
                    request,
                    Arc::clone(&grant),
                    session,
                    timeouts,
                );
                let _pool_permit = client.gate.acquire().map_err(|error| match error {
                    crate::sync_primitives::SemaphoreError::Cancelled => 7,
                    _ => 0,
                })?;
                let mut pool = client
                    .pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match response {
                    Ok(response) => {
                        pool.finish_shared_http3(&request_url, &response);
                        response
                    }
                    Err(error) => {
                        if matches!(
                            error,
                            crate::http_runtime::HttpRuntimeError::Transport
                                | crate::http_runtime::HttpRuntimeError::Protocol
                        ) {
                            pool.discard_shared_http3(&origin);
                        }
                        return Err(error_code(error));
                    }
                }
            }
        };
        invalidate_cached_uris_after_write(
            &request_for_invalidation,
            response.status,
            &response.headers,
        );
        successful_client_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(client_failure);
    if !out.is_null() {
        unsafe { out.write(outcome) };
    }
}

/// Sends a one-pass producer body through a reusable HTTP client.
///
/// # Safety
/// `method`, `url`, and `headers` must be live managed values; `callback` and
/// `context` must follow the generated producer ABI; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_request_stream(
    handle: u64,
    method: *const c_void,
    url: *const c_void,
    headers: *mut c_void,
    callback: Option<HttpUploadProducerCallback>,
    context: *mut c_void,
    producer_errors: i32,
    out: *mut HttpClientRequestOut,
) {
    let outcome = (|| {
        let client = client_by_handle(handle).ok_or(0)?;
        if !client_identity_matches(&client) {
            return Err(1);
        }
        let Some(method) = (unsafe { crate::string::borrow(method) }) else {
            return Err(0);
        };
        let Some(url) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let method_name = unsafe { method.as_str() };
        if hyper::Method::from_bytes(method_name.as_bytes()).is_err() {
            return Err(HTTP_ERROR_INVALID_ARGUMENT);
        }
        let method = match method_name {
            "GET" => HttpMethod::Get,
            "HEAD" => HttpMethod::Head,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            "CONNECT" => HttpMethod::Connect,
            "OPTIONS" => HttpMethod::Options,
            "TRACE" => HttpMethod::Trace,
            "PATCH" => HttpMethod::Patch,
            "QUERY" => HttpMethod::Query,
            _ => HttpMethod::Other,
        };
        let url = Url::parse(unsafe { url.as_str() }).map_err(|_| 0)?;
        let encoded_headers =
            unsafe { crate::array::copy_byte_array(headers, MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES) }
                .ok_or(0)?;
        let mut request = HttpRequestValues::new(method, url);
        request.method_token = method_name.to_owned();
        request.headers = decode_all_headers(&encoded_headers)?;
        validate_client_request(&request)?;
        let callback = callback.ok_or(3)?;
        let request_for_cache_invalidation = request.clone();
        let origin = request.url.origin();
        let request_url = request.url.clone();
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let _request_slot = client
            .request_slots
            .acquire()
            .map_err(|error| match error {
                crate::sync_primitives::SemaphoreError::Cancelled => 7,
                _ => 0,
            })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let _pool_permit = client.gate.acquire().map_err(|error| match error {
            crate::sync_primitives::SemaphoreError::Cancelled => 7,
            _ => 0,
        })?;
        if client.closed.load(Ordering::Acquire) {
            return Err(0);
        }
        let mut chunk_index = 0u64;
        let prepared = {
            let mut pool = client
                .pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pool.ensure_shared_connection(&request, Arc::clone(&grant))
                .map_err(error_code)?;
            match pool
                .prepare_shared_http2(&mut request, &grant)
                .map_err(error_code)?
            {
                Some(sender) => Some((sender, pool.timeouts())),
                None => {
                    let response = pool
                        .request_streaming(request, Arc::clone(&grant), || {
                            next_http_upload_chunk(
                                callback,
                                context,
                                producer_errors,
                                &mut chunk_index,
                            )
                        })
                        .map_err(error_code)?;
                    invalidate_cached_uris_after_write(
                        &request_for_cache_invalidation,
                        response.status,
                        &response.headers,
                    );
                    return successful_client_response(
                        response.status.as_u16(),
                        &response.headers,
                        &response.body,
                        response.protocol,
                    );
                }
            }
        };
        let (sender, timeouts) = prepared.expect("HTTP/2 reusable upload preparation has a sender");
        drop(_pool_permit);
        let response = sender.send_streaming(request, &grant, timeouts, || {
            next_http_upload_chunk(callback, context, producer_errors, &mut chunk_index)
        });
        let _pool_permit = client.gate.acquire().map_err(|error| match error {
            crate::sync_primitives::SemaphoreError::Cancelled => 7,
            _ => 0,
        })?;
        let mut pool = client
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let response = match response {
            Ok(response) => {
                pool.finish_shared_http2(&origin, &request_url, &response);
                response
            }
            Err(error) => {
                if matches!(
                    error,
                    crate::http_runtime::HttpRuntimeError::Transport
                        | crate::http_runtime::HttpRuntimeError::Protocol
                ) {
                    pool.discard_shared_http2(&origin);
                }
                return Err(error_code(error));
            }
        };
        invalidate_cached_uris_after_write(
            &request_for_cache_invalidation,
            response.status,
            &response.headers,
        );
        successful_client_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })();
    let outcome = outcome.unwrap_or_else(client_failure);
    if !out.is_null() {
        unsafe { out.write(outcome) };
    }
}

/// Opens a permission-checked WebSocket client for one exact HTTP origin.
/// `origin` is empty when omitted; `subprotocols` is a managed List<String>.
///
/// # Safety
/// `url` and `origin` are live managed strings, `subprotocols` is a live
/// managed list of strings, and `out` points to writable create-result storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_connect(
    url: *mut c_void,
    origin: *mut c_void,
    subprotocols: *mut c_void,
    http3: bool,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = unsafe { connect_http_websocket(url, origin, subprotocols, None, false, http3) };
    unsafe { out.write(client_create_out(outcome)) };
}

/// Opens a WebSocket using caller-supplied DER roots instead of platform
/// roots. Invalid or empty stores fail before network I/O.
///
/// # Safety
/// `url`, `origin`, `subprotocols`, and `roots` must be live managed Zirk
/// values of their declared types, and `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_connect_with_trust_roots(
    url: *mut c_void,
    origin: *mut c_void,
    subprotocols: *mut c_void,
    roots: *mut c_void,
    allow_tls12: bool,
    http3: bool,
    out: *mut HttpClientCreateOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = (|| {
        let roots = unsafe { copy_http_trust_roots(roots) }.ok_or(3)?;
        unsafe {
            connect_http_websocket(url, origin, subprotocols, Some(roots), allow_tls12, http3)
        }
    })();
    unsafe { out.write(client_create_out(outcome)) };
}

unsafe fn connect_http_websocket(
    url: *mut c_void,
    origin: *mut c_void,
    subprotocols: *mut c_void,
    roots_der: Option<Vec<Vec<u8>>>,
    allow_tls12: bool,
    force_http3: bool,
) -> Result<u64, i32> {
    (|| {
        let url = unsafe { crate::string::borrow(url) }.ok_or(0)?;
        let origin = unsafe { crate::string::borrow(origin) }.ok_or(0)?;
        if subprotocols.is_null() {
            return Err(0);
        }
        let count = unsafe { crate::list::zirk_rt_list_length(subprotocols) };
        if count > 64 {
            return Err(0);
        }
        let element_size = std::mem::size_of::<*mut c_void>();
        let mut protocols = Vec::with_capacity(count);
        for index in 0..count {
            let slot = unsafe {
                crate::list::zirk_rt_list_element(subprotocols, index as i64, element_size)
            };
            if slot.is_null() {
                return Err(0);
            }
            let protocol = unsafe { *slot.cast::<*mut c_void>() };
            let protocol = unsafe { crate::string::borrow(protocol) }.ok_or(0)?;
            protocols.push(unsafe { protocol.as_str() }.to_owned());
        }
        let url =
            zirk_runtime_core::http_values::Url::parse(unsafe { url.as_str() }).map_err(|_| 0)?;
        let grant = crate::permissions::current_network_grant().map_err(|error| match error {
            zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => 1,
            _ => 0,
        })?;
        let origin = match unsafe { origin.as_str() } {
            "" => None,
            value => Some(value),
        };
        let socket = if force_http3 {
            #[cfg(feature = "http3")]
            {
                if allow_tls12 {
                    return Err(3);
                }
                let tls_config = roots_der
                    .map(|roots| {
                        crate::network_transport::client_tls_config_with_trust_roots(roots, false)
                    })
                    .transpose()
                    .map_err(|_| 3)?;
                crate::http_runtime::HttpWebSocket::connect_http3_with_options(
                    &url,
                    grant,
                    crate::http_runtime::HttpTimeouts::default(),
                    origin,
                    &protocols,
                    tls_config,
                )
            }
            #[cfg(not(feature = "http3"))]
            {
                let _ = (roots_der, allow_tls12, url, grant, origin, protocols);
                return Err(error_code(crate::http_runtime::HttpRuntimeError::Protocol));
            }
        } else {
            match roots_der {
                Some(roots) => {
                    crate::http_runtime::HttpWebSocket::connect_with_options_and_trust_roots(
                        &url,
                        grant,
                        crate::http_runtime::HttpTimeouts::default(),
                        origin,
                        &protocols,
                        roots,
                        allow_tls12,
                    )
                }
                None => crate::http_runtime::HttpWebSocket::connect_with_options(
                    &url,
                    grant,
                    crate::http_runtime::HttpTimeouts::default(),
                    None,
                    origin,
                    &protocols,
                ),
            }
        }
        .map_err(error_code)?;
        let mut sockets = http_websockets()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sockets.len() >= MAX_HTTP_WEBSOCKETS {
            return Err(2);
        }
        let mut handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        while sockets.contains_key(&handle) {
            handle = NEXT_CLIENT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        }
        sockets.insert(
            handle,
            OwnedHttpWebSocket {
                owner: crate::executor::current_task(),
                socket,
            },
        );
        Ok(handle)
    })()
}

/// Sends one tagged, bounded WebSocket message record.
///
/// # Safety
/// `record` is a live managed byte array and `out` is writable action storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_send(
    handle: u64,
    record: *mut c_void,
    out: *mut HttpClientActionOut,
) {
    if out.is_null() {
        return;
    }
    let outcome = with_http_websocket(handle, |socket| {
        let record = copy_websocket_record(record)?;
        let Some((&kind, payload)) = record.split_first() else {
            return Err(0);
        };
        let message = match kind {
            0 => crate::http_runtime::HttpWebSocketMessage::Text(
                String::from_utf8(payload.to_vec()).map_err(|_| 5)?,
            ),
            1 => crate::http_runtime::HttpWebSocketMessage::Binary(payload.to_vec()),
            2 => crate::http_runtime::HttpWebSocketMessage::Ping(payload.to_vec()),
            3 => crate::http_runtime::HttpWebSocketMessage::Pong(payload.to_vec()),
            4 if payload.len() >= 2 => {
                let code = u16::from_be_bytes([payload[0], payload[1]]);
                let reason = String::from_utf8(payload[2..].to_vec()).map_err(|_| 5)?;
                crate::http_runtime::HttpWebSocketMessage::Close { code, reason }
            }
            _ => return Err(0),
        };
        socket.send(message).map_err(error_code)
    });
    unsafe { out.write(action_outcome(outcome)) };
}

/// Receives one complete bounded message record, EOF, or a typed error.
///
/// # Safety
/// `out` is null or points to writable `HttpStreamReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_receive(handle: u64, out: *mut HttpStreamReadOut) {
    if out.is_null() {
        return;
    }
    let outcome = with_http_websocket(handle, |socket| {
        let message = socket
            .receive(crate::http_runtime::HttpTimeouts::default().response_idle)
            .map_err(error_code)?;
        let Some(message) = message else {
            return Ok(Some(vec![5]));
        };
        let mut record = Vec::new();
        match message {
            crate::http_runtime::HttpWebSocketMessage::Text(text) => {
                record.push(0);
                record.extend_from_slice(text.as_bytes());
            }
            crate::http_runtime::HttpWebSocketMessage::Binary(bytes) => {
                record.push(1);
                record.extend_from_slice(&bytes);
            }
            crate::http_runtime::HttpWebSocketMessage::Ping(bytes) => {
                record.push(2);
                record.extend_from_slice(&bytes);
            }
            crate::http_runtime::HttpWebSocketMessage::Pong(bytes) => {
                record.push(3);
                record.extend_from_slice(&bytes);
            }
            crate::http_runtime::HttpWebSocketMessage::Close { code, reason } => {
                record.push(4);
                record.extend_from_slice(&code.to_be_bytes());
                record.extend_from_slice(reason.as_bytes());
            }
        }
        Ok(Some(record))
    });
    let value = match outcome {
        Ok(Some(record)) => HttpStreamReadOut {
            kind: 0,
            error: 0,
            bytes: crate::array::alloc_byte_array(&record),
        },
        Ok(None) => HttpStreamReadOut {
            kind: 0,
            error: 0,
            bytes: crate::array::alloc_byte_array(&[5]),
        },
        Err(error) => HttpStreamReadOut {
            kind: 2,
            error,
            bytes: std::ptr::null_mut(),
        },
    };
    unsafe { out.write(value) };
}

/// Closes a WebSocket handle and removes its registry entry.
///
/// # Safety
/// `out` is null or points to writable `HttpClientActionOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_close(handle: u64, out: *mut HttpClientActionOut) {
    if out.is_null() {
        return;
    }
    let mut sockets = http_websockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mut owned) = sockets.remove(&handle) else {
        unsafe { out.write(action_outcome(Err(8))) };
        return;
    };
    if owned.owner != crate::executor::current_task() {
        sockets.insert(handle, owned);
        unsafe { out.write(action_outcome(Err(1))) };
        return;
    }
    drop(sockets);
    let result = owned.socket.close().map_err(error_code);
    unsafe { out.write(action_outcome(result)) };
}

/// Sends a close status/reason and completes the WebSocket close handshake.
///
/// # Safety
/// `reason` is a live Zirk string and `out` points to writable action storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_websocket_close_with(
    handle: u64,
    code: u16,
    reason: *mut c_void,
    out: *mut HttpClientActionOut,
) {
    if out.is_null() {
        return;
    }
    let Some(reason) = (unsafe { crate::string::borrow(reason) }) else {
        unsafe { out.write(action_outcome(Err(0))) };
        return;
    };
    let reason = unsafe { reason.as_str() }.to_owned();
    let result = with_http_websocket(handle, |socket| {
        socket.close_with(code, reason).map_err(error_code)
    });
    unsafe { out.write(action_outcome(result)) };
}

fn copy_websocket_record(record: *mut c_void) -> Result<Vec<u8>, i32> {
    if record.is_null() {
        return Err(0);
    }
    let length = unsafe { crate::array::zirk_rt_array_length(record) };
    if length > 16 * 1024 * 1024 + 3 {
        return Err(4);
    }
    unsafe { crate::array::copy_byte_array(record, length) }.ok_or(0)
}

fn with_http_websocket<T>(
    handle: u64,
    operation: impl FnOnce(&mut crate::http_runtime::HttpWebSocket) -> Result<T, i32>,
) -> Result<T, i32> {
    let mut owned = {
        let mut sockets = http_websockets()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let owned = sockets.remove(&handle).ok_or(8)?;
        if owned.owner != crate::executor::current_task() {
            sockets.insert(handle, owned);
            return Err(1);
        }
        owned
    };
    let outcome = operation(&mut owned.socket);
    http_websockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(handle, owned);
    outcome
}

fn action_outcome(outcome: Result<(), i32>) -> HttpClientActionOut {
    match outcome {
        Ok(()) => HttpClientActionOut { kind: 0, error: 0 },
        Err(error) => HttpClientActionOut { kind: 1, error },
    }
}

/// Closes the client and all of its pooled connections.
///
/// # Safety
/// `out` must be null or point to writable `HttpClientActionOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_client_close(handle: u64, out: *mut HttpClientActionOut) {
    if out.is_null() {
        return;
    }
    let outcome = match client_by_handle(handle) {
        Some(client) => {
            let result = (|| {
                if !client_identity_matches(&client) {
                    return Err(1);
                }
                let _close_permit = client.close_gate.acquire().map_err(|error| match error {
                    crate::sync_primitives::SemaphoreError::Cancelled => 7,
                    _ => 0,
                })?;
                if client.closed.swap(true, Ordering::AcqRel) {
                    return Ok(());
                }
                let mut close_attempt = ClientCloseAttempt {
                    closed: &client.closed,
                    committed: false,
                };
                let mut request_permits = Vec::with_capacity(8);
                for _ in 0..8 {
                    request_permits.push(client.request_slots.acquire().map_err(
                        |error| match error {
                            crate::sync_primitives::SemaphoreError::Cancelled => 7,
                            _ => 0,
                        },
                    )?);
                }
                let _pool_permit = client.gate.acquire().map_err(|error| match error {
                    crate::sync_primitives::SemaphoreError::Cancelled => 7,
                    _ => 0,
                })?;
                client
                    .pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .close();
                let mut clients = http_clients()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if clients
                    .get(&handle)
                    .is_some_and(|registered| Arc::ptr_eq(registered, &client))
                {
                    clients.remove(&handle);
                }
                close_attempt.commit();
                Ok(())
            })();
            action_outcome(result)
        }
        // Closing an already-released opaque handle is idempotent. Source
        // objects intentionally keep only the immutable handle; runtime state
        // is the single authority for whether that handle remains open.
        None => HttpClientActionOut { kind: 0, error: 0 },
    };
    unsafe { out.write(outcome) };
}

/// Validates and normalizes a URL without performing DNS or network access.
///
/// # Safety
/// `url` must be a live Zirk String and `out` must point to writable
/// `HttpUrlOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_url_normalize(url: *const c_void, out: *mut HttpUrlOut) {
    let outcome = (|| {
        let Some(value) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let normalized = Url::parse(unsafe { value.as_str() }).map_err(|_| 0)?;
        Ok(HttpUrlOut {
            kind: 0,
            error: 0,
            normalized: crate::string::alloc_owned(normalized.as_str()),
        })
    })()
    .unwrap_or_else(|error| HttpUrlOut {
        kind: 1,
        error,
        normalized: std::ptr::null_mut(),
    });
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Resolves a relative reference against a validated HTTP or HTTPS base URL.
///
/// # Safety
/// `base` and `reference` must be live Zirk Strings and `out` must point to
/// writable `HttpUrlOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_url_resolve(
    base: *const c_void,
    reference: *const c_void,
    out: *mut HttpUrlOut,
) {
    let outcome = (|| {
        let Some(base) = (unsafe { crate::string::borrow(base) }) else {
            return Err(0);
        };
        let Some(reference) = (unsafe { crate::string::borrow(reference) }) else {
            return Err(0);
        };
        let base = Url::parse(unsafe { base.as_str() }).map_err(|_| 0)?;
        let resolved = base.resolve(unsafe { reference.as_str() }).map_err(|_| 0)?;
        Ok(HttpUrlOut {
            kind: 0,
            error: 0,
            normalized: crate::string::alloc_owned(resolved.as_str()),
        })
    })()
    .unwrap_or_else(|error| HttpUrlOut {
        kind: 1,
        error,
        normalized: std::ptr::null_mut(),
    });
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Executes one GET under the current task's verified network grant.
///
/// # Safety
/// `url` must be a live Zirk String and `out` must point to writable
/// `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_get(url: *const c_void, out: *mut HttpGetOut) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let request = HttpRequestValues::new(HttpMethod::Get, url);
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Executes an explicitly cache-enabled GET. The network grant is checked
/// before every cache lookup so a hit cannot bypass current origin authority.
///
/// # Safety
/// `url` must be a live Zirk String and `out` must point to writable
/// `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_get_cached(url: *const c_void, out: *mut HttpGetOut) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        grant.authorize_origin(&url.origin()).map_err(|_| 1)?;
        let verified = crate::permissions::verified_grant().ok_or(1)?;
        let partition = cache_partition(verified.project_id(), verified.manifest_digest());
        let request = HttpRequestValues::new(HttpMethod::Get, url);
        let response = send_cached(request, Arc::clone(&grant), &partition).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

fn cache_partition(project_id: &str, manifest_digest: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((project_id.len() as u64).to_be_bytes());
    digest.update(project_id.as_bytes());
    digest.update((manifest_digest.len() as u64).to_be_bytes());
    digest.update(manifest_digest.as_bytes());
    digest.finalize().into()
}

pub(crate) fn invalidate_cached_uris_after_write(
    request: &HttpRequestValues,
    status: HttpStatusCode,
    headers: &Headers,
) {
    if request.method.is_safe() || !(200..400).contains(&status.as_u16()) {
        return;
    }
    let Some(verified) = crate::permissions::verified_grant() else {
        return;
    };
    let partition = cache_partition(verified.project_id(), verified.manifest_digest());
    let origin = request.url.origin();
    let related_uris = ["location", "content-location"]
        .into_iter()
        .flat_map(|name| headers.get_all(name).into_iter().flatten())
        .filter_map(|value| value.to_str().ok())
        .filter_map(|reference| request.url.resolve(reference).ok())
        .filter(|url| url.origin() == origin)
        .collect::<Vec<_>>();
    let mut cache = HTTP_RESPONSE_CACHE
        .get_or_init(|| Mutex::new(HttpResponseCache::default()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.invalidate_uri(&request.url, &partition);
    for url in related_uris {
        cache.invalidate_uri(&url, &partition);
    }
}

fn send_cached(
    request: HttpRequestValues,
    grant: Arc<zirk_runtime_core::network_policy::EffectiveNetworkGrant>,
    partition: &[u8; 32],
) -> Result<HttpResponseValues, crate::http_runtime::HttpRuntimeError> {
    let cache = HTTP_RESPONSE_CACHE.get_or_init(|| Mutex::new(HttpResponseCache::default()));
    let now = Instant::now();
    let candidate = {
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(response) = cache.get(&request, partition, now) {
            return Ok(HttpResponseValues {
                status: response.status,
                headers: response.headers,
                body: response.body,
                body_stream: None,
                body_producer: None,
                protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
            });
        }
        cache.revalidation_candidate(&request, partition, now)
    };

    match zirk_runtime_core::http_cache::request_only_if_cached(&request) {
        Some(true) => {
            return Ok(HttpResponseValues {
                status: HttpStatusCode::new(504)
                    .expect("HTTP 504 is a valid gateway-timeout status"),
                headers: Headers::new(),
                body: Vec::new(),
                body_stream: None,
                body_producer: None,
                protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
            });
        }
        Some(false) => {}
        None => return Err(crate::http_runtime::HttpRuntimeError::InvalidRequest),
    }

    let mut outgoing = request.clone();
    let mut conditional_sent = false;
    if let Some((_, etag, last_modified)) = &candidate {
        if let Some(etag) = etag {
            conditional_sent = outgoing
                .headers
                .insert("if-none-match", etag.as_bytes())
                .is_ok();
        } else if let Some(last_modified) = last_modified {
            conditional_sent = outgoing
                .headers
                .insert("if-modified-since", last_modified.as_bytes())
                .is_ok();
        }
    }

    let request_started = Instant::now();
    let response = crate::http_runtime::send_once(outgoing, grant)?;
    let received_at = Instant::now();
    let received_wall = SystemTime::now();
    if response.status.as_u16() == 304
        && conditional_sent
        && let Some((mut cached, _, _)) = candidate
    {
        for (name, value) in response.headers.iter() {
            cached
                .headers
                .insert(name.as_str(), value.as_bytes())
                .map_err(|_| crate::http_runtime::HttpRuntimeError::Protocol)?;
        }
        let status = cached.status;
        let output = HttpResponseValues {
            status,
            headers: cached.headers.clone(),
            body: cached.body.clone(),
            body_stream: None,
            body_producer: None,
            protocol: crate::http_runtime::HttpProtocolVersion::Unknown,
        };
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.remove_request(&request, partition);
        let _ = cache.insert(
            &request,
            cached,
            partition,
            request_started,
            received_at,
            received_wall,
        );
        return Ok(output);
    }

    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.remove_request(&request, partition);
    let _ = cache.insert(
        &request,
        CachedHttpResponse {
            status: response.status,
            headers: response.headers.clone(),
            body: response.body.clone(),
        },
        partition,
        request_started,
        received_at,
        received_wall,
    );
    Ok(response)
}

/// Executes one caller-selected bounded HTTP request under the current task's
/// verified origin grant.
///
/// # Safety
/// `method` and `url` must be live Zirk strings, `body` and `headers` live
/// managed byte arrays, and `out` writable storage for `HttpGetOut`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_request(
    method: *const c_void,
    url: *const c_void,
    body: *const c_void,
    headers: *const c_void,
    validate_json: bool,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(method) = (unsafe { crate::string::borrow(method) }) else {
            return Err(3);
        };
        let Some(url) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let method_name = unsafe { method.as_str() };
        let method = match method_name {
            "GET" => HttpMethod::Get,
            "HEAD" => HttpMethod::Head,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "PATCH" => HttpMethod::Patch,
            "DELETE" => HttpMethod::Delete,
            "OPTIONS" => HttpMethod::Options,
            "CONNECT" => HttpMethod::Connect,
            "TRACE" => HttpMethod::Trace,
            "QUERY" => HttpMethod::Query,
            _ if hyper::Method::from_bytes(method_name.as_bytes()).is_ok() => HttpMethod::Other,
            _ => return Err(HTTP_ERROR_INVALID_ARGUMENT),
        };
        let url = Url::parse(unsafe { url.as_str() }).map_err(|_| 0)?;
        let body = copy_response_bytes(body)?;
        if validate_json {
            validate_json_document(&body).map_err(error_code)?;
        }
        let header_block = unsafe {
            crate::array::copy_byte_array(headers.cast_mut(), MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES)
        }
        .ok_or(3)?;
        let mut request = HttpRequestValues::new(method, url);
        request.method_token = method_name.to_owned();
        request.headers = decode_all_headers(&header_block)?;
        request.set_body(body).map_err(|_| 4)?;
        validate_client_request(&request)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Executes an explicit cache-enabled HTTP request. The cache itself accepts
/// only safe, eligible methods; all other methods still perform one network
/// request and never enter storage.
///
/// # Safety
/// Arguments follow the same live Zirk handle and writable output contract as
/// `zirk_http_request`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_request_cached(
    method: *const c_void,
    url: *const c_void,
    body: *const c_void,
    headers: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(method) = (unsafe { crate::string::borrow(method) }) else {
            return Err(3);
        };
        let Some(url) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let method_name = unsafe { method.as_str() };
        let method = match method_name {
            "GET" => HttpMethod::Get,
            "HEAD" => HttpMethod::Head,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "PATCH" => HttpMethod::Patch,
            "DELETE" => HttpMethod::Delete,
            "OPTIONS" => HttpMethod::Options,
            "CONNECT" => HttpMethod::Connect,
            "TRACE" => HttpMethod::Trace,
            "QUERY" => HttpMethod::Query,
            _ => return Err(HTTP_ERROR_INVALID_ARGUMENT),
        };
        let url = Url::parse(unsafe { url.as_str() }).map_err(|_| 0)?;
        let body = copy_response_bytes(body)?;
        let header_block = unsafe {
            crate::array::copy_byte_array(headers.cast_mut(), MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES)
        }
        .ok_or(3)?;
        let mut request = HttpRequestValues::new(method, url);
        request.method_token = method_name.to_owned();
        request.headers = decode_all_headers(&header_block)?;
        request.set_body(body).map_err(|_| 4)?;
        validate_client_request(&request)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        grant
            .authorize_origin(&request.url.origin())
            .map_err(|_| 1)?;
        let verified = crate::permissions::verified_grant().ok_or(1)?;
        let partition = cache_partition(verified.project_id(), verified.manifest_digest());
        let response = send_cached(request, Arc::clone(&grant), &partition).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends a request with one explicit secret-valued HTTPS header. The secret
/// is copied into an owned native buffer before any network suspension and is
/// never returned to the source program as a `String`.
///
/// # Safety
/// All arguments must be live Zirk strings/arrays and `out` must point to
/// writable `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_request_secret(
    method: *const c_void,
    url: *const c_void,
    body: *const c_void,
    headers: *const c_void,
    header_name: *const c_void,
    secret_value: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(method) = (unsafe { crate::string::borrow(method) }) else {
            return Err(3);
        };
        let Some(url) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let Some(header_name) = (unsafe { crate::string::borrow(header_name) }) else {
            return Err(3);
        };
        let Some(secret_value) = (unsafe { crate::string::borrow(secret_value) }) else {
            return Err(3);
        };
        let body = copy_response_bytes(body)?;
        let header_block = unsafe {
            crate::array::copy_byte_array(headers.cast_mut(), MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES)
        }
        .ok_or(3)?;
        let request = prepare_secret_header_request(
            unsafe { method.as_str() },
            unsafe { url.as_str() },
            body,
            &header_block,
            unsafe { header_name.as_str() },
            unsafe { secret_value.as_str() }.to_owned(),
        )?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

pub(crate) fn prepare_secret_header_request(
    method_name: &str,
    url: &str,
    body: Vec<u8>,
    header_block: &[u8],
    header_name: &str,
    secret_value: String,
) -> Result<HttpRequestValues, i32> {
    let secret_value = Zeroizing::new(secret_value);
    let method = match method_name {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "CONNECT" => HttpMethod::Connect,
        "TRACE" => HttpMethod::Trace,
        "QUERY" => HttpMethod::Query,
        _ if hyper::Method::from_bytes(method_name.as_bytes()).is_ok() => HttpMethod::Other,
        _ => return Err(HTTP_ERROR_INVALID_ARGUMENT),
    };
    let url = Url::parse(url).map_err(|_| 0)?;
    if url.scheme() != "https" {
        return Err(3);
    }
    let mut request = HttpRequestValues::new(method, url);
    request.method_token = method_name.to_owned();
    request.headers = decode_all_headers(header_block)?;
    if request.headers.get(header_name).is_some() {
        return Err(3);
    }
    request
        .headers
        .append_sensitive(header_name, secret_value.as_bytes())
        .map_err(|_| 3)?;
    request.set_body(body).map_err(|_| 4)?;
    validate_client_request(&request)?;
    Ok(request)
}

/// Opens a one-shot HTTP GET and returns its body as a task-owned stream.
///
/// # Safety
/// `url` must be a live Zirk String and `out` writable `HttpStreamOpenOut`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_get_stream(
    url: *const c_void,
    headers: *mut c_void,
    out: *mut HttpStreamOpenOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let encoded_headers =
            unsafe { crate::array::copy_byte_array(headers, MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES) }
                .ok_or(3)?;
        let mut request = HttpRequestValues::new(HttpMethod::Get, url);
        request.headers = decode_all_headers(&encoded_headers)?;
        let stream = crate::http_runtime::send_once_streaming(request, Arc::clone(&grant))
            .map_err(error_code)?;
        let status = i32::from(stream.status.as_u16());
        let headers_bytes = serialize_response_headers(&stream.headers)?;
        let headers = crate::array::alloc_byte_array(&headers_bytes);
        let mut registry = streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.len() >= MAX_HTTP_STREAMS {
            return Err(2);
        }
        let handle = loop {
            let candidate = NEXT_STREAM_HANDLE.fetch_add(1, Ordering::Relaxed);
            if candidate != 0 && !registry.contains_key(&candidate) {
                break candidate;
            }
        };
        registry.insert(
            handle,
            OwnedHttpStream {
                owner: crate::executor::current_task(),
                stream,
                stream_read_started: false,
                sse_started: false,
                sse_parser: None,
                sse_queue: std::collections::VecDeque::new(),
            },
        );
        Ok(HttpStreamOpenOut {
            kind: 0,
            error: 0,
            status,
            handle,
            headers,
        })
    })()
    .unwrap_or_else(|error| HttpStreamOpenOut {
        kind: 1,
        error,
        status: 0,
        handle: 0,
        headers: std::ptr::null_mut(),
    });
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Transfers a bounded buffered response into the task-owned chunk reader.
/// The bytes are already resident, but subsequent consumption follows the
/// same single-use, owner-checked resource lifecycle as live network streams.
///
/// # Safety
/// `body`, `headers`, and `out` must follow the generated managed-value ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_response_to_stream(
    status: i32,
    body: *mut c_void,
    headers: *mut c_void,
    out: *mut HttpStreamOpenOut,
) {
    let outcome = (|| {
        let status = u16::try_from(status).map_err(|_| 11)?;
        let status = HttpStatusCode::new(status).map_err(|_| 11)?;
        let body = unsafe {
            crate::array::copy_byte_array(
                body,
                zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES,
            )
        }
        .ok_or(4)?;
        let encoded_headers =
            unsafe { crate::array::copy_byte_array(headers, MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES) }
                .ok_or(3)?;
        let headers = decode_response_headers(&encoded_headers)?;
        let stream = crate::http_runtime::HttpStreamingResponse::from_buffered(
            status,
            headers.clone(),
            body,
            crate::executor::current_task(),
        );
        let headers_bytes = serialize_response_headers(&headers)?;
        let managed_headers = crate::array::alloc_byte_array(&headers_bytes);
        let mut registry = streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.len() >= MAX_HTTP_STREAMS {
            return Err(2);
        }
        let handle = loop {
            let candidate = NEXT_STREAM_HANDLE.fetch_add(1, Ordering::Relaxed);
            if candidate != 0 && !registry.contains_key(&candidate) {
                break candidate;
            }
        };
        registry.insert(
            handle,
            OwnedHttpStream {
                owner: crate::executor::current_task(),
                stream,
                stream_read_started: false,
                sse_started: false,
                sse_parser: None,
                sse_queue: std::collections::VecDeque::new(),
            },
        );
        Ok(HttpStreamOpenOut {
            kind: 0,
            error: 0,
            status: i32::from(status.as_u16()),
            handle,
            headers: managed_headers,
        })
    })()
    .unwrap_or_else(|error| HttpStreamOpenOut {
        kind: 1,
        error,
        status: 0,
        handle: 0,
        headers: std::ptr::null_mut(),
    });
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Opens a streaming HTTP request whose generated producer runs on the current
/// Zirk task. The protocol reactor receives only copied bounded chunks.
///
/// # Safety
/// `method`, `url`, `headers`, callback context, and `out` must follow the managed value
/// and generated callback ABI described above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_request_stream(
    method: *const c_void,
    url: *const c_void,
    headers: *mut c_void,
    callback: Option<HttpUploadProducerCallback>,
    context: *mut c_void,
    producer_errors: i32,
    out: *mut HttpStreamOpenOut,
) {
    let outcome = (|| {
        let Some(method_value) = (unsafe { crate::string::borrow(method) }) else {
            return Err(3);
        };
        let method_token = unsafe { method_value.as_str() };
        let _parsed_method = hyper::Method::from_bytes(method_token.as_bytes())
            .map_err(|_| HTTP_ERROR_INVALID_ARGUMENT)?;
        let method = match method_token {
            "GET" => HttpMethod::Get,
            "HEAD" => HttpMethod::Head,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            "CONNECT" => HttpMethod::Connect,
            "OPTIONS" => HttpMethod::Options,
            "TRACE" => HttpMethod::Trace,
            "PATCH" => HttpMethod::Patch,
            "QUERY" => HttpMethod::Query,
            _ => HttpMethod::Other,
        };
        let Some(url_value) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_value.as_str() }).map_err(|_| 0)?;
        let encoded_headers =
            unsafe { crate::array::copy_byte_array(headers, MAX_ENCODED_HTTP_HEADER_BLOCK_BYTES) }
                .ok_or(3)?;
        let headers = decode_all_headers(&encoded_headers)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(method, url);
        request.method_token = method_token.to_owned();
        request.headers = headers;
        let callback = callback.ok_or(3)?;
        let stream =
            crate::http_runtime::send_once_streaming_with_producer(request, Arc::clone(&grant), {
                let mut chunk_index = 0u64;
                move || {
                    let mut chunk = std::ptr::null_mut();
                    let status = unsafe { callback(context, chunk_index, &mut chunk) };
                    chunk_index = chunk_index
                        .checked_add(1)
                        .ok_or(crate::http_runtime::HttpRuntimeError::BodyTooLarge)?;
                    if status != 0 {
                        return Err(if producer_errors == 0 {
                            crate::http_runtime::HttpRuntimeError::Cancelled
                        } else {
                            match status - 1 {
                                0 => crate::http_runtime::HttpRuntimeError::Cancelled,
                                1 => crate::http_runtime::HttpRuntimeError::Transport,
                                _ => crate::http_runtime::HttpRuntimeError::Protocol,
                            }
                        });
                    }
                    if chunk.is_null() {
                        return Ok(None);
                    }
                    let bytes = unsafe {
                        crate::array::copy_byte_array(
                            chunk,
                            crate::http_runtime::HTTP_UPLOAD_CHUNK_LIMIT,
                        )
                    }
                    .ok_or(crate::http_runtime::HttpRuntimeError::BodyTooLarge)?;
                    Ok(Some(bytes))
                }
            })
            .map_err(error_code)?;
        let status = i32::from(stream.status.as_u16());
        let headers_bytes = serialize_response_headers(&stream.headers)?;
        let managed_headers = crate::array::alloc_byte_array(&headers_bytes);
        let mut registry = streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.len() >= MAX_HTTP_STREAMS {
            return Err(2);
        }
        let handle = loop {
            let candidate = NEXT_STREAM_HANDLE.fetch_add(1, Ordering::Relaxed);
            if candidate != 0 && !registry.contains_key(&candidate) {
                break candidate;
            }
        };
        registry.insert(
            handle,
            OwnedHttpStream {
                owner: crate::executor::current_task(),
                stream,
                stream_read_started: false,
                sse_started: false,
                sse_parser: None,
                sse_queue: std::collections::VecDeque::new(),
            },
        );
        Ok(HttpStreamOpenOut {
            kind: 0,
            error: 0,
            status,
            handle,
            headers: managed_headers,
        })
    })()
    .unwrap_or_else(|error| HttpStreamOpenOut {
        kind: 1,
        error,
        status: 0,
        handle: 0,
        headers: std::ptr::null_mut(),
    });
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Reads one bounded response-body chunk from the current task's stream.
///
/// # Safety
/// `out` must point to writable `HttpStreamReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_stream_read_chunk(
    handle: u64,
    max_bytes: u64,
    out: *mut HttpStreamReadOut,
) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        if max_bytes == 0 || max_bytes > 64 * 1024 {
            return Err(HTTP_ERROR_INVALID_ARGUMENT);
        }
        let owner = crate::executor::current_task();
        let mut entry = {
            let mut registry = streams()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(entry) = registry.get(&handle) else {
                return Err(8);
            };
            if entry.owner != owner {
                return Err(1);
            }
            if entry.sse_started {
                return Err(8);
            }
            registry.remove(&handle).ok_or(8)?
        };
        entry.stream_read_started = true;
        let chunk = entry
            .stream
            .read_chunk(max_bytes as usize)
            .map_err(error_code)?;
        match chunk {
            Some(bytes) => {
                let payload = crate::array::alloc_byte_array(&bytes);
                streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(handle, entry);
                Ok(HttpStreamReadOut {
                    kind: 0,
                    error: 0,
                    bytes: payload,
                })
            }
            None => Ok(HttpStreamReadOut {
                kind: 1,
                error: 0,
                bytes: std::ptr::null_mut(),
            }),
        }
    })();
    let outcome = result.unwrap_or_else(|error| HttpStreamReadOut {
        kind: 2,
        error,
        bytes: std::ptr::null_mut(),
    });
    unsafe { *out = outcome };
}

/// Consumes the remaining stream body into a bounded managed byte array.
///
/// # Safety
/// `out` must point to writable `HttpStreamReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_stream_read_all(handle: u64, out: *mut HttpStreamReadOut) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        let owner = crate::executor::current_task();
        let mut entry = {
            let mut registry = streams()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(entry) = registry.get(&handle) else {
                return Err(8);
            };
            if entry.owner != owner {
                return Err(1);
            }
            if entry.sse_started {
                return Err(8);
            }
            registry.remove(&handle).ok_or(8)?
        };
        entry.stream_read_started = true;
        let bytes = entry.stream.read_all().map_err(error_code)?;
        let bytes = crate::array::alloc_byte_array(&bytes);
        Ok(HttpStreamReadOut {
            kind: 0,
            error: 0,
            bytes,
        })
    })();
    let outcome = result.unwrap_or_else(|error| HttpStreamReadOut {
        kind: 1,
        error,
        bytes: std::ptr::null_mut(),
    });
    unsafe { *out = outcome };
}

/// Reads one complete SSE event from a task-owned HTTP stream and returns its
/// fields as `[event, data, last_event_id, retry_ms]`. An empty list means EOF.
/// The stream is advanced only until one event is ready, keeping consumption
/// demand-driven and bounded by the core SSE parser limits.
///
/// # Safety
/// `out` must point to writable `HttpStreamReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_stream_sse_next(handle: u64, out: *mut HttpStreamReadOut) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        let owner = crate::executor::current_task();
        let mut entry = {
            let mut registry = streams()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(entry) = registry.get(&handle) else {
                return Err(8);
            };
            if entry.owner != owner {
                return Err(1);
            }
            if entry.stream_read_started && !entry.sse_started {
                return Err(8);
            }
            registry.remove(&handle).ok_or(8)?
        };

        entry.sse_started = true;

        if entry.sse_parser.is_none() {
            crate::http_runtime::HttpSseResponse::validate_headers(&entry.stream.headers)
                .map_err(error_code)?;
            if entry.stream.status.as_u16() != 200 {
                return Err(3);
            }
            entry.sse_parser = Some(zirk_runtime_core::http_sse::SseParser::new());
        }

        let mut parser = entry.sse_parser.take().expect("initialized SSE parser");
        loop {
            if let Some(event) = entry.sse_queue.pop_front() {
                let fields = alloc_sse_event_fields(event);
                entry.sse_parser = Some(parser);
                streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(handle, entry);
                return Ok(HttpStreamReadOut {
                    kind: 0,
                    error: 0,
                    bytes: fields,
                });
            }
            match entry.stream.read_chunk(64 * 1024).map_err(error_code)? {
                Some(bytes) => {
                    let events = parser.feed(&bytes).map_err(|_| 3)?;
                    entry.sse_queue.extend(events);
                }
                None => {
                    parser.finish();
                    // Keep the completed parser reachable until the Zirk
                    // response wrapper explicitly closes its opaque handle.
                    // In particular, SSE heartbeat metadata remains queryable
                    // after EOF and across an automatic client reconnection.
                    entry.sse_parser = Some(parser);
                    streams()
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(handle, entry);
                    return Ok(HttpStreamReadOut {
                        kind: 0,
                        error: 0,
                        bytes: alloc_empty_string_list(),
                    });
                }
            }
        }
    })();
    let outcome = result.unwrap_or_else(|error| HttpStreamReadOut {
        kind: 1,
        error,
        bytes: std::ptr::null_mut(),
    });
    unsafe { *out = outcome };
}

/// Returns the number of SSE comment heartbeat lines observed on a stream.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_http_stream_sse_heartbeat_count(handle: u64) -> u64 {
    let owner = crate::executor::current_task();
    let registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry
        .get(&handle)
        .filter(|entry| entry.owner == owner && entry.sse_started)
        .and_then(|entry| entry.sse_parser.as_ref())
        .map_or(0, zirk_runtime_core::http_sse::SseParser::heartbeat_count)
}

fn alloc_empty_string_list() -> *mut c_void {
    unsafe {
        crate::list::zirk_rt_list_new(
            std::mem::size_of::<*mut c_void>(),
            std::mem::align_of::<*mut c_void>(),
            true,
        )
    }
}

fn alloc_sse_event_fields(event: zirk_runtime_core::http_sse::SseEvent) -> *mut c_void {
    let values = [
        event.event,
        event.data,
        event.last_event_id,
        event
            .retry_ms
            .map(|value| value.to_string())
            .unwrap_or_default(),
    ];
    let mut list = std::ptr::null_mut();
    let mut current = std::ptr::null_mut();
    let mut roots = [
        (&mut list as *mut *mut c_void).cast(),
        (&mut current as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let element_size = std::mem::size_of::<*mut c_void>();
    let element_align = std::mem::align_of::<*mut c_void>();
    list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
    for value in values {
        current = crate::string::alloc_owned(&value);
        unsafe {
            crate::list::zirk_rt_list_add(
                list,
                (&current as *const *mut c_void).cast(),
                element_size,
                element_align,
                true,
            );
        }
    }
    crate::zirk_rt_pop_frame();
    list
}

/// Closes a task-owned response body and aborts its connection if still open.
///
/// # Safety
/// This symbol follows the native ABI and must only be called with a handle
/// previously returned by `zirk_http_get_stream`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_stream_close(handle: u64) -> bool {
    let mut registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match registry.get(&handle) {
        None => true,
        Some(entry) if entry.owner != crate::executor::current_task() => false,
        Some(_) => {
            registry.remove(&handle);
            true
        }
    }
}

/// Executes one HTTP HEAD under the current task's verified network grant.
///
/// # Safety
/// `url` must be a live Zirk String and `out` must point to writable
/// `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_head(url: *const c_void, out: *mut HttpGetOut) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let request = HttpRequestValues::new(HttpMethod::Head, url);
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends one QUERY with an UTF-8 JSON document and the standardized query
/// media type. The request is permission-checked by the shared transport.
///
/// # Safety
/// `url` and `query` must be live Zirk strings; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_query(
    url: *const c_void,
    query: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let Some(query_string) = (unsafe { crate::string::borrow(query) }) else {
            return Err(9);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let body = unsafe { query_string.as_str() }.as_bytes().to_vec();
        validate_json_document(&body).map_err(error_code)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(HttpMethod::Query, url);
        request
            .headers
            .insert("content-type", b"application/json")
            .map_err(|_| 3)?;
        request.set_body(body).map_err(|_| 4)?;
        validate_client_request(&request)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends a QUERY request with a caller-selected content type.
///
/// # Safety
/// `url`, `query`, and `content_type` must be live Zirk strings; `out` must be
/// writable `HttpGetOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_query_with_content_type(
    url: *const c_void,
    query: *const c_void,
    content_type: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let Some(query_string) = (unsafe { crate::string::borrow(query) }) else {
            return Err(9);
        };
        let Some(content_type_string) = (unsafe { crate::string::borrow(content_type) }) else {
            return Err(3);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let body = unsafe { query_string.as_str() }.as_bytes().to_vec();
        let content_type = unsafe { content_type_string.as_str() };
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(HttpMethod::Query, url);
        request
            .headers
            .insert("content-type", content_type.as_bytes())
            .map_err(|_| 3)?;
        request.set_body(body).map_err(|_| 4)?;
        validate_client_request(&request)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends one bounded UTF-8 text POST under the current task's verified grant.
///
/// # Safety
/// `url` and `body` must be live Zirk strings; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_post(
    url: *const c_void,
    body: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let Some(body_string) = (unsafe { crate::string::borrow(body) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let body = unsafe { body_string.as_str() }.as_bytes().to_vec();
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(HttpMethod::Post, url);
        request
            .headers
            .insert("content-type", b"text/plain; charset=utf-8")
            .map_err(|_| 3)?;
        request.set_body(body).map_err(|_| 4)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends one bounded JSON POST after validating the representation locally.
///
/// # Safety
/// `url` and `body` must be live Zirk strings; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_post_json(
    url: *const c_void,
    body: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let Some(body_string) = (unsafe { crate::string::borrow(body) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let body = unsafe { body_string.as_str() }.as_bytes().to_vec();
        validate_json_document(&body).map_err(error_code)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(HttpMethod::Post, url);
        request
            .headers
            .insert("content-type", b"application/json")
            .map_err(|_| 3)?;
        request.set_body(body).map_err(|_| 4)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Sends one bounded opaque byte POST under the current task's verified grant.
///
/// # Safety
/// `url` must be a live Zirk string, `body` a live `Array<Byte>`, and `out`
/// writable storage for `HttpGetOut`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_post_bytes(
    url: *const c_void,
    body: *const c_void,
    out: *mut HttpGetOut,
) {
    let outcome = (|| {
        let Some(url_string) = (unsafe { crate::string::borrow(url) }) else {
            return Err(0);
        };
        let url = Url::parse(unsafe { url_string.as_str() }).map_err(|_| 0)?;
        let body = copy_response_bytes(body)?;
        let grant = crate::permissions::current_network_grant().map_err(|_| 1)?;
        let mut request = HttpRequestValues::new(HttpMethod::Post, url);
        request
            .headers
            .insert("content-type", b"application/octet-stream")
            .map_err(|_| 3)?;
        request.set_body(body).map_err(|_| 4)?;
        let response =
            crate::http_runtime::send_once(request, Arc::clone(&grant)).map_err(error_code)?;
        successful_response(
            response.status.as_u16(),
            &response.headers,
            &response.body,
            response.protocol,
        )
    })()
    .unwrap_or_else(failure);
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Builds the managed list returned by `HTTPResponse.header_values`.
///
/// # Safety
/// `headers` and `name` must be live Zirk values; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_http_response_header_values(
    headers: *const c_void,
    name: *const c_void,
    out: *mut HttpTextOut,
) {
    if out.is_null() {
        return;
    }
    let result = (|| {
        let Some(name) = (unsafe { crate::string::borrow(name) }) else {
            return Err(0);
        };
        let requested_name = unsafe { name.as_str() }.to_owned();
        let encoded = copy_response_bytes(headers)?;
        let values = decode_header_values(&encoded, &requested_name)?;

        let element_size = std::mem::size_of::<*mut c_void>();
        let element_align = std::mem::align_of::<*mut c_void>();
        let list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        let mut rooted_list = list;
        let mut current = std::ptr::null_mut();
        let mut roots = [
            (&mut rooted_list as *mut *mut c_void).cast(),
            (&mut current as *mut *mut c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        for value in values {
            current = crate::string::alloc_owned(&value);
            unsafe {
                crate::list::zirk_rt_list_add(
                    rooted_list,
                    (&current as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        crate::zirk_rt_pop_frame();
        Ok(rooted_list)
    })();
    let outcome = match result {
        Ok(list) => HttpTextOut {
            kind: 0,
            error: 0,
            value: list,
        },
        Err(error) => text_failure(error),
    };
    unsafe { *out = outcome };
}

fn validate_json_document(body: &[u8]) -> Result<(), crate::http_runtime::HttpRuntimeError> {
    serde_json::from_slice::<serde_json::Value>(body)
        .map(|_| ())
        .map_err(|_| crate::http_runtime::HttpRuntimeError::InvalidJson)
}

#[cfg(test)]
mod tests {
    use super::{
        ClientCloseAttempt, OwnedHttpClient, client_identity_matches_values, create_http_upload,
        decode_all_headers, decode_header_values, decode_response_text, http_uploads,
        serialize_request_headers, serialize_response_headers, validate_json_document,
        zirk_http_route_allow_header,
    };
    use crate::executor::Executor;
    use std::sync::{Mutex, atomic::AtomicBool};
    use zirk_runtime_core::http_values::Headers;

    #[test]
    fn interrupted_client_close_reopens_admission() {
        let closed = std::sync::atomic::AtomicBool::new(true);
        drop(ClientCloseAttempt {
            closed: &closed,
            committed: false,
        });
        assert!(!closed.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn committed_client_close_keeps_admission_closed() {
        let closed = std::sync::atomic::AtomicBool::new(true);
        let mut attempt = ClientCloseAttempt {
            closed: &closed,
            committed: false,
        };
        attempt.commit();
        drop(attempt);
        assert!(closed.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn reusable_http_client_identity_rejects_executor_project_and_manifest_mismatch() {
        let client = OwnedHttpClient {
            executor_id: 17,
            project_id: "project-a".into(),
            manifest_digest: "manifest-a".into(),
            gate: crate::sync_primitives::CooperativeSemaphore::new(1, 1).unwrap(),
            request_slots: crate::sync_primitives::CooperativeSemaphore::new(8, 8).unwrap(),
            close_gate: crate::sync_primitives::CooperativeSemaphore::new(1, 1).unwrap(),
            pool: Mutex::new(
                crate::http_runtime::HttpClientPool::new_shared(
                    1,
                    crate::http_runtime::HttpTimeouts::default(),
                )
                .unwrap(),
            ),
            closed: AtomicBool::new(false),
        };

        assert!(client_identity_matches_values(
            &client,
            17,
            "project-a",
            "manifest-a",
        ));
        assert!(!client_identity_matches_values(
            &client,
            18,
            "project-a",
            "manifest-a",
        ));
        assert!(!client_identity_matches_values(
            &client,
            17,
            "project-b",
            "manifest-a",
        ));
        assert!(!client_identity_matches_values(
            &client,
            17,
            "project-a",
            "manifest-b",
        ));
    }

    #[test]
    fn response_text_decoder_requires_valid_utf8() {
        assert_eq!(
            decode_response_text(b"hello \xE2\x9C\x93".to_vec()),
            Ok("hello ✓".into())
        );
        assert_eq!(decode_response_text(vec![0x66, 0x80]), Err(5));
    }

    #[test]
    fn configurable_http_timeouts_are_independent_and_bounded() {
        let timeouts = super::http_timeouts_from_millis(5, 12_000, 20_000).unwrap();
        assert_eq!(timeouts.connect, std::time::Duration::from_millis(5));
        assert_eq!(
            timeouts.response_headers,
            std::time::Duration::from_millis(12_000)
        );
        assert_eq!(
            timeouts.response_idle,
            std::time::Duration::from_millis(20_000)
        );
        assert!(super::http_timeouts_from_millis(0, 1, 1).is_err());
        assert!(super::http_timeouts_from_millis(1, 3_600_001, 1).is_err());
        assert!(super::http_timeouts_from_millis(1, 1, 0).is_err());
        assert_eq!(
            super::http_timeouts_from_millis(1, 1, 3_600_000)
                .unwrap()
                .response_idle,
            std::time::Duration::from_secs(3600)
        );
    }

    #[test]
    fn managed_multipart_uploads_delete_temporary_files_on_close_and_task_exit() {
        use std::sync::{Arc, Mutex};

        let task_exit_path = Arc::new(Mutex::new(None));
        let path_for_task = Arc::clone(&task_exit_path);
        let outcome = Executor::new().run_with_root(move || {
            let owner = crate::executor::current_task();
            let handle = create_http_upload(b"temporary upload", owner).unwrap();
            let path = http_uploads()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&handle)
                .unwrap()
                .path
                .clone();
            assert!(path.exists());
            assert!(super::close_http_upload(handle, owner));
            assert!(!path.exists());

            let task_owned_handle = create_http_upload(b"task cleanup", owner).unwrap();
            let task_path = http_uploads()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&task_owned_handle)
                .unwrap()
                .path
                .clone();
            *path_for_task
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(task_path);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let task_path = task_exit_path
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap();
        assert!(!task_path.exists());
    }

    #[test]
    fn json_document_validator_accepts_values_and_rejects_malformed_payloads() {
        assert!(validate_json_document(br#"{"filter":{"active":true}}"#).is_ok());
        assert!(validate_json_document(b"true").is_ok());
        assert!(validate_json_document(br#"{"filter":}"#).is_err());
        assert!(validate_json_document(b"").is_err());
    }

    #[test]
    fn response_header_decoder_preserves_repeated_values_case_insensitively() {
        let mut encoded = 3u32.to_le_bytes().to_vec();
        for (name, value) in [
            (b"set-cookie".as_slice(), b"first=1".as_slice()),
            (b"content-type".as_slice(), b"text/plain".as_slice()),
            (b"set-cookie".as_slice(), b"second=2".as_slice()),
        ] {
            encoded.extend_from_slice(&(name.len() as u16).to_le_bytes());
            encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
            encoded.extend_from_slice(name);
            encoded.extend_from_slice(value);
        }
        assert_eq!(
            decode_header_values(&encoded, "Set-Cookie").unwrap(),
            ["first=1", "second=2"]
        );
        assert!(decode_header_values(&encoded, "bad header").is_err());
    }

    #[test]
    fn explicitly_sensitive_custom_header_is_redacted_after_transport_decode() {
        let name = b"x-session-credential";
        let value = b"session-secret-private";
        let mut encoded = 1u32.to_le_bytes().to_vec();
        encoded.extend_from_slice(&((name.len() as u16) | 0x8000).to_le_bytes());
        encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
        encoded.extend_from_slice(name);
        encoded.extend_from_slice(value);

        let headers = decode_all_headers(&encoded).unwrap();
        assert_eq!(
            headers
                .get("x-session-credential")
                .unwrap()
                .to_str()
                .unwrap(),
            "session-secret-private"
        );
        assert!(!format!("{headers:?}").contains("session-secret-private"));
    }

    #[test]
    fn header_decoder_rejects_malformed_sensitive_markers_and_framing() {
        let mut empty_sensitive_name = 1u32.to_le_bytes().to_vec();
        empty_sensitive_name.extend_from_slice(&0x8000u16.to_le_bytes());
        empty_sensitive_name.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode_all_headers(&empty_sensitive_name), Err(3));

        let mut sensitive_framing_name = 1u32.to_le_bytes().to_vec();
        let name = b"content-length";
        sensitive_framing_name.extend_from_slice(&((name.len() as u16) | 0x8000).to_le_bytes());
        sensitive_framing_name.extend_from_slice(&1u32.to_le_bytes());
        sensitive_framing_name.extend_from_slice(name);
        sensitive_framing_name.push(b'0');
        assert_eq!(decode_all_headers(&sensitive_framing_name), Err(3));

        let mut trailing_data = 0u32.to_le_bytes().to_vec();
        trailing_data.push(0);
        assert_eq!(decode_all_headers(&trailing_data), Err(3));

        let too_many =
            (zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT as u32 + 1).to_le_bytes();
        assert_eq!(decode_all_headers(&too_many), Err(3));
    }

    #[test]
    fn response_header_decoder_rejects_truncated_and_invalid_utf8_values() {
        let mut truncated = 1u32.to_le_bytes().to_vec();
        truncated.extend_from_slice(&[1, 0, 2, 0, 0, 0, b'x']);
        assert_eq!(decode_header_values(&truncated, "x-test"), Err(3));

        let mut invalid_utf8 = 1u32.to_le_bytes().to_vec();
        invalid_utf8.extend_from_slice(&6u16.to_le_bytes());
        invalid_utf8.extend_from_slice(&1u32.to_le_bytes());
        invalid_utf8.extend_from_slice(b"x-test");
        invalid_utf8.push(0xff);
        assert_eq!(decode_header_values(&invalid_utf8, "x-test"), Err(5));
    }

    #[test]
    fn route_allow_header_matches_path_deduplicates_and_orders_standard_methods() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let element_size = std::mem::size_of::<*mut std::ffi::c_void>();
        let element_align = std::mem::align_of::<*mut std::ffi::c_void>();
        let mut patterns =
            unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        let mut methods =
            unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        let mut current = std::ptr::null_mut();
        let mut path = crate::string::alloc_owned("/items/special");
        let mut roots = [
            (&mut patterns as *mut *mut std::ffi::c_void).cast(),
            (&mut methods as *mut *mut std::ffi::c_void).cast(),
            (&mut current as *mut *mut std::ffi::c_void).cast(),
            (&mut path as *mut *mut std::ffi::c_void).cast(),
        ];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
        for (pattern, method) in [
            ("/items/:id", "GET"),
            ("/items/special", "GET"),
            ("/items/:id", "POST"),
            ("/other", "DELETE"),
        ] {
            current = crate::string::alloc_owned(pattern);
            unsafe {
                crate::list::zirk_rt_list_add(
                    patterns,
                    (&current as *const *mut std::ffi::c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
            current = crate::string::alloc_owned(method);
            unsafe {
                crate::list::zirk_rt_list_add(
                    methods,
                    (&current as *const *mut std::ffi::c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        let header = unsafe { zirk_http_route_allow_header(patterns, methods, path) };
        let encoded = unsafe {
            crate::array::copy_byte_array(
                header,
                zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES,
            )
        }
        .unwrap();
        assert_eq!(
            decode_header_values(&encoded, "allow").unwrap(),
            ["GET, POST"]
        );
        crate::zirk_rt_pop_frame();
    }

    #[test]
    fn response_header_abi_roundtrips_a_bounded_repeated_field_set() {
        let mut headers = Headers::new();
        for index in 0..zirk_runtime_core::http_values::MAX_HTTP_HEADER_COUNT {
            headers
                .append("set-cookie", format!("session={index}").as_bytes())
                .unwrap();
        }
        let encoded = serialize_response_headers(&headers).unwrap();
        assert_eq!(
            decode_header_values(&encoded, "SET-COOKIE").unwrap().len(),
            256
        );
        assert!(encoded.len() <= 64 * 1024 + 6 * 256 + 4);
    }

    #[test]
    fn accept_query_validation_applies_only_to_response_headers() {
        let mut headers = Headers::new();
        headers
            .insert("accept-query", b"application/json;q=0.5")
            .unwrap();

        assert!(serialize_request_headers(&headers).is_ok());
        assert_eq!(serialize_response_headers(&headers), Err(3));
    }
}
