//! Compiler-owned C ABI for the bundled `std.net::tcp` API.
//!
//! Handles stay owned by the creating Zirk task. Operations temporarily move
//! a stream out of the global handle map before parking on readiness, so no
//! scheduler thread is blocked behind a mutex while a task is suspended.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::network_transport::{
    NetworkTransportError, TcpListener, TcpStream, TlsClientStream, TlsListener, TlsServerConfig,
    TlsServerStream,
};
use crate::task::TaskId;
use zirk_runtime_core::network_policy::NetworkOrigin;

const MAX_NETWORK_HANDLES: usize = 1_024;
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static STREAMS: OnceLock<Mutex<HashMap<u64, OwnedTcpStream>>> = OnceLock::new();
static TLS_STREAMS: OnceLock<Mutex<HashMap<u64, OwnedTlsStream>>> = OnceLock::new();
static TLS_LISTENERS: OnceLock<Mutex<HashMap<u64, OwnedTlsListener>>> = OnceLock::new();
static TLS_SERVER_STREAMS: OnceLock<Mutex<HashMap<u64, OwnedTlsStream>>> = OnceLock::new();
static LISTENERS: OnceLock<Mutex<HashMap<u64, OwnedTcpListener>>> = OnceLock::new();

struct OwnedTcpStream {
    owner: TaskId,
    stream: TcpStream,
}

struct OwnedTlsStream {
    owner: TaskId,
    stream: TlsStreamKind,
}

enum TlsStreamKind {
    Client(TlsClientStream),
    Server(TlsServerStream),
}

impl TlsStreamKind {
    fn read(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, NetworkTransportError> {
        match self {
            Self::Client(stream) => stream.read(max_bytes),
            Self::Server(stream) => stream.read(max_bytes),
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), NetworkTransportError> {
        match self {
            Self::Client(stream) => stream.write_all(bytes),
            Self::Server(stream) => stream.write_all(bytes),
        }
    }

    fn shutdown(&mut self) -> Result<(), NetworkTransportError> {
        match self {
            Self::Client(stream) => stream.shutdown(),
            Self::Server(stream) => stream.shutdown(),
        }
    }

    fn peer_address(&self) -> std::net::SocketAddr {
        match self {
            Self::Client(stream) => stream.peer_address(),
            Self::Server(stream) => stream.peer_address(),
        }
    }
}

struct OwnedTcpListener {
    owner: TaskId,
    // `None` means an accept operation temporarily owns the listener on its
    // Zirk task stack. Keeping the registry entry preserves owner checks
    // during that suspension without holding the global lock while pending.
    listener: Option<TcpListener>,
}

struct OwnedTlsListener {
    owner: TaskId,
    listener: Option<TlsListener>,
}

fn streams() -> &'static Mutex<HashMap<u64, OwnedTcpStream>> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tls_streams() -> &'static Mutex<HashMap<u64, OwnedTlsStream>> {
    TLS_STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn listeners() -> &'static Mutex<HashMap<u64, OwnedTcpListener>> {
    LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tls_listeners() -> &'static Mutex<HashMap<u64, OwnedTlsListener>> {
    TLS_LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tls_server_streams() -> &'static Mutex<HashMap<u64, OwnedTlsStream>> {
    TLS_SERVER_STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Publishes an owned network resource only while its registry is below the
/// per-kind ceiling. On failure `value` is dropped by the caller's scope,
/// closing any socket it owns instead of retaining an unbounded handle.
fn insert_network_handle<T>(registry: &Mutex<HashMap<u64, T>>, handle: u64, value: T) -> bool {
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if registry.len() >= MAX_NETWORK_HANDLES {
        return false;
    }
    registry.insert(handle, value);
    true
}

/// Drops any sockets still owned by a task when that task becomes terminal.
/// Explicit `TCPStream.close()` remains observable in source code, while this
/// fallback prevents abandoned handles from surviving task completion.
pub(crate) fn close_task_streams(owner: TaskId) {
    let mut registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry.retain(|_, entry| entry.owner != owner);
    tls_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    tls_server_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    tls_listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
}

#[repr(C)]
pub struct NetworkOut {
    /// `0 = success/value`, `1 = end`, `2 = error` (operation dependent).
    pub kind: i32,
    /// `TCPError` discriminant; meaningful only for an error.
    pub error: i32,
    /// UInt64 handle or partial-write count.
    pub scalar: u64,
    /// GC-managed `Array<Byte>` payload for a successful read, otherwise null.
    pub payload: *mut c_void,
}

/// Parses a plain IP literal and returns its canonical textual form.
/// Host names, scoped IPv6 literals, and ports are rejected; no DNS query is
/// performed.
///
/// # Safety
/// `value` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_ip_address_parse(value: *const c_void, out: *mut NetworkOut) {
    let Some(value) = (unsafe { crate::string::borrow(value) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let result = unsafe { value.as_str() }.parse::<std::net::IpAddr>();
    let outcome = match result {
        Ok(address) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&address.to_string()),
        },
        Err(_) => empty_out(1, 0),
    };
    unsafe { set_out(out, outcome) };
}

/// Normalizes a host and non-zero TCP port without consulting the system
/// resolver. Authority checks and DNS remain the responsibility of connect.
///
/// # Safety
/// `host` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_origin_normalize(
    host: *const c_void,
    port: u16,
    out: *mut NetworkOut,
) {
    let Some(host) = (unsafe { crate::string::borrow(host) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let outcome = match zirk_runtime_core::network_policy::normalize_tcp_origin(
        unsafe { host.as_str() },
        port,
    ) {
        Ok(origin) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&origin),
        },
        Err(_) => empty_out(1, 0),
    };
    unsafe { set_out(out, outcome) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TcpApiError {
    Closed,
    WrongOwner,
    Transport(NetworkTransportError),
}

impl From<NetworkTransportError> for TcpApiError {
    fn from(error: NetworkTransportError) -> Self {
        Self::Transport(error)
    }
}

fn error_code(error: TcpApiError) -> i32 {
    match error {
        TcpApiError::Closed => 11,
        TcpApiError::WrongOwner => 12,
        TcpApiError::Transport(error) => match error {
            NetworkTransportError::InvalidAddress => 0,
            NetworkTransportError::PermissionDenied => 1,
            NetworkTransportError::AddressNotAllowed => 2,
            NetworkTransportError::DnsFailure => 3,
            NetworkTransportError::DnsResultLimit => 4,
            NetworkTransportError::UnsupportedTransport => 5,
            NetworkTransportError::BufferLimitExceeded => 6,
            NetworkTransportError::Io => 7,
            NetworkTransportError::Cancelled => 8,
            NetworkTransportError::CapacityExceeded => 9,
            NetworkTransportError::Timeout => 10,
            NetworkTransportError::CertificateStore
            | NetworkTransportError::InvalidServerName
            | NetworkTransportError::InvalidServerIdentity
            | NetworkTransportError::TlsHandshake => 7,
        },
    }
}

fn tls_error_code(error: TcpApiError) -> i32 {
    match error {
        TcpApiError::Closed => 15,
        TcpApiError::WrongOwner => 16,
        TcpApiError::Transport(error) => match error {
            NetworkTransportError::InvalidAddress => 0,
            NetworkTransportError::PermissionDenied => 1,
            NetworkTransportError::AddressNotAllowed => 2,
            NetworkTransportError::DnsFailure => 3,
            NetworkTransportError::DnsResultLimit => 4,
            NetworkTransportError::UnsupportedTransport => 5,
            NetworkTransportError::CertificateStore => 6,
            NetworkTransportError::InvalidServerName => 7,
            NetworkTransportError::InvalidServerIdentity => 8,
            NetworkTransportError::TlsHandshake => 9,
            NetworkTransportError::BufferLimitExceeded => 10,
            NetworkTransportError::Io => 11,
            NetworkTransportError::Cancelled => 12,
            NetworkTransportError::CapacityExceeded => 13,
            NetworkTransportError::Timeout => 14,
        },
    }
}

fn with_stream<T>(
    handle: u64,
    operation: impl FnOnce(&mut TcpStream) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    let owner = crate::executor::current_task();
    let mut owned = {
        let mut registry = streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get(&handle) else {
            return Err(TcpApiError::Closed);
        };
        if entry.owner != owner {
            return Err(TcpApiError::WrongOwner);
        }
        registry
            .remove(&handle)
            .expect("the checked TCP handle exists")
    };

    let result = operation(&mut owned.stream);
    if !matches!(&result, Err(NetworkTransportError::Cancelled)) {
        streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(handle, owned);
    }
    result.map_err(TcpApiError::Transport)
}

/// Transfers one owner-task TCP stream into the HTTP server adapter. The
/// protocol adapter owns and closes it after serving its single request.
pub(crate) fn take_tcp_stream_for_http(handle: u64) -> Result<TcpStream, i32> {
    let owner = crate::executor::current_task();
    let mut registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = registry.get(&handle) else {
        return Err(11);
    };
    if entry.owner != owner {
        return Err(12);
    }
    Ok(registry.remove(&handle).expect("checked TCP handle").stream)
}

/// Transfers one owner-task accepted TLS stream into the HTTP adapter.
pub(crate) fn take_tls_server_stream_for_http(handle: u64) -> Result<TlsServerStream, i32> {
    let owner = crate::executor::current_task();
    let mut registry = tls_server_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = registry.get(&handle) else {
        return Err(11);
    };
    if entry.owner != owner {
        return Err(12);
    }
    let entry = registry.remove(&handle).expect("checked TLS server handle");
    match entry.stream {
        TlsStreamKind::Server(stream) => Ok(stream),
        TlsStreamKind::Client(_) => Err(11),
    }
}

fn with_tls_stream<T>(
    handle: u64,
    operation: impl FnOnce(&mut TlsStreamKind) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    let owner = crate::executor::current_task();
    let client_registry = tls_streams();
    let server_registry = tls_server_streams();
    let (registry, mut owned) = match take_owned_tls_stream(client_registry, handle, owner)? {
        Some(owned) => (client_registry, owned),
        None => match take_owned_tls_stream(server_registry, handle, owner)? {
            Some(owned) => (server_registry, owned),
            None => return Err(TcpApiError::Closed),
        },
    };

    let result = operation(&mut owned.stream);
    if !matches!(&result, Err(NetworkTransportError::Cancelled)) {
        registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(handle, owned);
    }
    result.map_err(TcpApiError::Transport)
}

fn take_owned_tls_stream(
    registry: &'static Mutex<HashMap<u64, OwnedTlsStream>>,
    handle: u64,
    owner: TaskId,
) -> Result<Option<OwnedTlsStream>, TcpApiError> {
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = registry.get(&handle) else {
        return Ok(None);
    };
    if entry.owner != owner {
        return Err(TcpApiError::WrongOwner);
    }
    Ok(registry.remove(&handle))
}

fn remove_owned_tls_stream(
    registry: &'static Mutex<HashMap<u64, OwnedTlsStream>>,
    handle: u64,
    owner: TaskId,
) -> Result<bool, TcpApiError> {
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match registry.get(&handle) {
        None => Ok(false),
        Some(entry) if entry.owner != owner => Err(TcpApiError::WrongOwner),
        Some(_) => {
            registry.remove(&handle);
            Ok(true)
        }
    }
}

/// Opens an authenticated TLS 1.3 client stream using system trust roots and
/// the exact `https://host:port` network grant.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable NetworkOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_connect(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    match TlsClientStream::connect(unsafe { endpoint.as_str() }, grant, false) {
        Ok(stream) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            let outcome = if handle == 0 {
                drop(stream);
                empty_out(1, 13)
            } else if insert_network_handle(
                tls_streams(),
                handle,
                OwnedTlsStream {
                    owner: crate::executor::current_task(),
                    stream: TlsStreamKind::Client(stream),
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 13)
            };
            unsafe { set_out(out, outcome) };
        }
        Err(error) => unsafe { set_out(out, empty_out(1, tls_error_code(error.into()))) },
    }
}

fn copy_zirk_trust_roots(array: *mut c_void) -> Result<Vec<Vec<u8>>, TcpApiError> {
    copy_zirk_der_certificates(
        array,
        NetworkTransportError::CertificateStore,
        NetworkTransportError::CertificateStore,
    )
}

fn copy_zirk_certificate_chain(array: *mut c_void) -> Result<Vec<Vec<u8>>, TcpApiError> {
    copy_zirk_der_certificates(
        array,
        NetworkTransportError::InvalidServerIdentity,
        NetworkTransportError::BufferLimitExceeded,
    )
}

fn copy_zirk_der_certificates(
    array: *mut c_void,
    empty_error: NetworkTransportError,
    limit_error: NetworkTransportError,
) -> Result<Vec<Vec<u8>>, TcpApiError> {
    const MAX_CERTIFICATES: usize = 64;
    const MAX_TOTAL_CERTIFICATE_BYTES: usize = 1024 * 1024;
    if array.is_null() {
        return Err(TcpApiError::Transport(empty_error));
    }
    let count = unsafe { crate::array::zirk_rt_array_length(array) };
    if count == 0 {
        return Err(TcpApiError::Transport(empty_error));
    }
    if count > MAX_CERTIFICATES {
        return Err(TcpApiError::Transport(limit_error));
    }
    let pointer_size = std::mem::size_of::<*mut c_void>();
    let mut total_bytes = 0usize;
    let mut roots = Vec::with_capacity(count);
    for index in 0..count {
        let slot =
            unsafe { crate::array::zirk_rt_array_element(array, index as i64, pointer_size) };
        let root = unsafe { *slot.cast::<*mut c_void>() };
        let der = copy_zirk_bytes_limited(root, MAX_TOTAL_CERTIFICATE_BYTES - total_bytes)?;
        total_bytes = total_bytes
            .checked_add(der.len())
            .ok_or(TcpApiError::Transport(
                NetworkTransportError::BufferLimitExceeded,
            ))?;
        roots.push(der);
    }
    Ok(roots)
}

/// Connects a TLS client using explicit DER trust roots.
///
/// # Safety
/// `endpoint` and `roots_der` must be live managed Zirk values of the declared
/// string and nested-array types, and `out` must point to writable
/// `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_connect_with_roots(
    endpoint: *const c_void,
    roots_der: *mut c_void,
    allow_tls12: bool,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let roots_der = match copy_zirk_trust_roots(roots_der) {
        Ok(roots) => roots,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    match TlsClientStream::connect_with_trust_roots(
        unsafe { endpoint.as_str() },
        grant,
        roots_der,
        allow_tls12,
    ) {
        Ok(stream) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            let outcome = if handle == 0 {
                drop(stream);
                empty_out(1, 13)
            } else if insert_network_handle(
                tls_streams(),
                handle,
                OwnedTlsStream {
                    owner: crate::executor::current_task(),
                    stream: TlsStreamKind::Client(stream),
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 13)
            };
            unsafe { set_out(out, outcome) };
        }
        Err(error) => unsafe { set_out(out, empty_out(1, tls_error_code(error.into()))) },
    }
}

/// Binds a TLS server with an explicit DER identity under the exact listen grant.
///
/// # Safety
/// `endpoint`, `certificate_chain`, and `private_key` must be live managed Zirk
/// values of the declared types, and `out` must point to writable
/// `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_listen(
    endpoint: *const c_void,
    certificate_chain: *mut c_void,
    private_key: *mut c_void,
    allow_tls12: bool,
    out: *mut NetworkOut,
) {
    unsafe {
        tls_listener_bind(
            endpoint,
            certificate_chain,
            private_key,
            allow_tls12,
            false,
            out,
        )
    }
}

/// Binds a TLS listener configured to negotiate HTTP/2 or HTTP/1.1 with ALPN.
///
/// # Safety
/// `endpoint`, `certificate_chain`, and `private_key` must be live managed Zirk
/// values of the declared types, and `out` must point to writable
/// `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_listen_http(
    endpoint: *const c_void,
    certificate_chain: *mut c_void,
    private_key: *mut c_void,
    allow_tls12: bool,
    out: *mut NetworkOut,
) {
    unsafe {
        tls_listener_bind(
            endpoint,
            certificate_chain,
            private_key,
            allow_tls12,
            true,
            out,
        )
    }
}

unsafe fn tls_listener_bind(
    endpoint: *const c_void,
    certificate_chain: *mut c_void,
    private_key: *mut c_void,
    allow_tls12: bool,
    http_alpn: bool,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let origin = match NetworkOrigin::parse(unsafe { endpoint.as_str() }) {
        Ok(origin) if origin.scheme() == "tcp" => origin,
        _ => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let host = origin.host().trim_start_matches('[').trim_end_matches(']');
    let Ok(ip) = host.parse::<std::net::IpAddr>() else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let address = std::net::SocketAddr::new(ip, origin.port());
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    if let Err(error) = grant.authorize_listen(address) {
        let error = NetworkTransportError::from(error);
        unsafe {
            set_out(
                out,
                empty_out(1, tls_error_code(TcpApiError::Transport(error))),
            )
        };
        return;
    }
    let chain = match copy_zirk_certificate_chain(certificate_chain) {
        Ok(chain) => chain,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let private_key = match copy_zirk_bytes(private_key) {
        Ok(key) => key,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let config = match TlsServerConfig::from_der(chain, private_key, allow_tls12) {
        Ok(config) => config,
        Err(error) => {
            unsafe {
                set_out(
                    out,
                    empty_out(1, tls_error_code(TcpApiError::Transport(error))),
                )
            };
            return;
        }
    };
    let config = if http_alpn {
        config.with_alpn_protocols(vec![b"h2".to_vec(), b"http/1.1".to_vec()])
    } else {
        config
    };
    let outcome = match TlsListener::bind(address, &grant, config) {
        Ok(listener) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                drop(listener);
                empty_out(1, 13)
            } else if insert_network_handle(
                tls_listeners(),
                handle,
                OwnedTlsListener {
                    owner: crate::executor::current_task(),
                    listener: Some(listener),
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 13)
            }
        }
        Err(error) => empty_out(1, tls_error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Accepts a TLS peer on an owner-task listener.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_accept(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let listener = {
        let mut registry = tls_listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            unsafe { set_out(out, empty_out(1, 15)) };
            return;
        };
        if entry.owner != owner {
            unsafe { set_out(out, empty_out(1, 16)) };
            return;
        }
        let Some(listener) = entry.listener.take() else {
            unsafe { set_out(out, empty_out(1, 13)) };
            return;
        };
        listener
    };
    let accepted = listener.accept();
    let mut registry = tls_listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(&accepted, Err(NetworkTransportError::Cancelled)) {
        registry.remove(&handle);
    } else if let Some(entry) = registry.get_mut(&handle)
        && entry.owner == owner
    {
        entry.listener = Some(listener);
    }
    drop(registry);
    let outcome = match accepted {
        Ok((stream, _peer)) => {
            let stream_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if stream_handle == 0 {
                drop(stream);
                empty_out(1, 13)
            } else if insert_network_handle(
                tls_server_streams(),
                stream_handle,
                OwnedTlsStream {
                    owner,
                    stream: TlsStreamKind::Server(stream),
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: stream_handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 13)
            }
        }
        Err(error) => empty_out(1, tls_error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Closes an owner-task TLS listener; repeated close is harmless.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_close_listener(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let result = {
        let mut registry = tls_listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match registry.get(&handle) {
            None => Ok(()),
            Some(entry) if entry.owner != owner => Err(TcpApiError::WrongOwner),
            Some(_) => {
                registry.remove(&handle);
                Ok(())
            }
        }
    };
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_tls_listener_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    !tls_listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_some_and(|entry| entry.owner == owner)
}

/// Reads one bounded plaintext chunk from an owner-task TLS stream.
///
/// # Safety
/// `out` must point to writable NetworkOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_read(handle: u64, max_bytes: u64, out: *mut NetworkOut) {
    let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    let result = with_tls_stream(handle, |stream| stream.read(max_bytes));
    let outcome = match result {
        Ok(Some(bytes)) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::array::alloc_byte_array(&bytes),
        },
        Ok(None) => empty_out(1, 0),
        Err(error) => empty_out(2, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Writes all plaintext bytes over an owner-task TLS stream.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable NetworkOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_write_all(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result = copy_zirk_bytes(bytes)
        .and_then(|bytes| with_tls_stream(handle, |stream| stream.write_all(&bytes)));
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Completes the TLS close-notify handshake for the owner-task stream.
///
/// # Safety
/// `out` must point to writable NetworkOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_shutdown(handle: u64, out: *mut NetworkOut) {
    let result = with_tls_stream(handle, TlsStreamKind::shutdown);
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Drops an owner-task TLS stream; repeated close is harmless.
///
/// # Safety
/// `out` must point to writable NetworkOut storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_close(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let result = {
        remove_owned_tls_stream(tls_streams(), handle, owner).and_then(|removed| {
            if removed {
                Ok(())
            } else {
                remove_owned_tls_stream(tls_server_streams(), handle, owner).map(|_| ())
            }
        })
    };
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_tls_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    !tls_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_some_and(|entry| entry.owner == owner)
        && !tls_server_streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&handle)
            .is_some_and(|entry| entry.owner == owner)
}

/// Returns the numeric peer address of an owner-task TLS stream.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_peer_address(handle: u64, out: *mut NetworkOut) {
    let result = with_tls_stream(handle, |stream| Ok(stream.peer_address().to_string()));
    let outcome = match result {
        Ok(address) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&address),
        },
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Returns the numeric peer address of an owner-task TCP stream.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_peer_address(handle: u64, out: *mut NetworkOut) {
    let result = with_stream(handle, |stream| Ok(stream.peer_address().to_string()));
    let outcome = match result {
        Ok(address) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&address),
        },
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

unsafe fn set_out(out: *mut NetworkOut, value: NetworkOut) {
    if !out.is_null() {
        unsafe { *out = value };
    }
}

fn empty_out(kind: i32, error: i32) -> NetworkOut {
    NetworkOut {
        kind,
        error,
        scalar: 0,
        payload: std::ptr::null_mut(),
    }
}

/// Resolves an exact approved HTTP, HTTPS, or TCP origin to a bounded list of
/// validated IP-address strings. The list is a normal GC-managed Zirk value.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_dns_resolve(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    match crate::network_transport::resolve_origin(
        unsafe { endpoint.as_str() },
        grant,
        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
    ) {
        Ok(addresses) => {
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
            for address in addresses {
                current = crate::string::alloc_owned(&address);
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
            unsafe {
                set_out(
                    out,
                    NetworkOut {
                        kind: 0,
                        error: 0,
                        scalar: 0,
                        payload: rooted_list,
                    },
                )
            };
        }
        Err(error) => unsafe { set_out(out, empty_out(1, error_code(error.into()))) },
    }
}

/// Opens a TCP stream only for a canonical tcp origin covered by the verified
/// application's exact network grant.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_connect(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let text = unsafe { endpoint.as_str() };
    let origin = match NetworkOrigin::parse(text) {
        Ok(origin) if origin.scheme() == "tcp" => origin,
        _ => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let result = TcpStream::connect(origin.as_str(), grant);
    let outcome = match result {
        Ok(stream) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                drop(stream);
                empty_out(1, 9)
            } else if insert_network_handle(
                streams(),
                handle,
                OwnedTcpStream {
                    owner: crate::executor::current_task(),
                    stream,
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 9)
            }
        }
        Err(error) => empty_out(1, error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Binds a TCP listener under the separately approved `network.listen`
/// address scope. Only numeric IP addresses are accepted, so bind never
/// performs implicit DNS resolution.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_listen(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let origin = match NetworkOrigin::parse(unsafe { endpoint.as_str() }) {
        Ok(origin) if origin.scheme() == "tcp" => origin,
        _ => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let host = origin.host().trim_start_matches('[').trim_end_matches(']');
    let Ok(ip) = host.parse::<std::net::IpAddr>() else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let address = std::net::SocketAddr::new(ip, origin.port());
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let outcome = match TcpListener::bind(address, &grant) {
        Ok(listener) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                drop(listener);
                empty_out(1, 9)
            } else if insert_network_handle(
                listeners(),
                handle,
                OwnedTcpListener {
                    owner: crate::executor::current_task(),
                    listener: Some(listener),
                },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 9)
            }
        }
        Err(error) => empty_out(1, error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Accepts one connection on an owner-task listener and registers the
/// resulting stream to the same task.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_accept(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let listener = {
        let mut registry = listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            unsafe { set_out(out, empty_out(1, 11)) };
            return;
        };
        if entry.owner != owner {
            unsafe { set_out(out, empty_out(1, 12)) };
            return;
        }
        let Some(listener) = entry.listener.take() else {
            unsafe { set_out(out, empty_out(1, 9)) };
            return;
        };
        listener
    };
    let accepted = listener.accept();
    let mut registry = listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(&accepted, Err(NetworkTransportError::Cancelled)) {
        registry.remove(&handle);
    } else if let Some(entry) = registry.get_mut(&handle)
        && entry.owner == owner
    {
        entry.listener = Some(listener);
    }
    drop(registry);
    let outcome = match accepted {
        Ok((stream, _peer)) => {
            let stream_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if stream_handle == 0 {
                drop(stream);
                empty_out(1, 9)
            } else if insert_network_handle(
                streams(),
                stream_handle,
                OwnedTcpStream { owner, stream },
            ) {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: stream_handle,
                    payload: std::ptr::null_mut(),
                }
            } else {
                empty_out(1, 9)
            }
        }
        Err(error) => empty_out(1, error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Closes one owner-task listener. Repeated close is harmless.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_close_listener(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let result = {
        let mut registry = listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match registry.get(&handle) {
            None => Ok(()),
            Some(entry) if entry.owner != owner => Err(TcpApiError::WrongOwner),
            Some(_) => {
                registry.remove(&handle);
                Ok(())
            }
        }
    };
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Reports whether this task owns an open listener with `handle`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_tcp_listener_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    !listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_some_and(|entry| entry.owner == owner)
}

/// Reads at most `max_bytes`. The native result tags match `TCPReadResult`.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_read(handle: u64, max_bytes: u64, out: *mut NetworkOut) {
    let result = usize::try_from(max_bytes)
        .map_err(|_| TcpApiError::Transport(NetworkTransportError::BufferLimitExceeded))
        .and_then(|max_bytes| with_stream(handle, |stream| stream.read(max_bytes)));
    let outcome = match result {
        Ok(Some(bytes)) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::array::alloc_byte_array(&bytes),
        },
        Ok(None) => empty_out(1, 0),
        Err(error) => empty_out(2, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

fn copy_zirk_bytes(array: *mut c_void) -> Result<Vec<u8>, TcpApiError> {
    copy_zirk_bytes_limited(array, crate::network_transport::MAX_TCP_WRITE_BYTES)
}

fn copy_zirk_bytes_limited(array: *mut c_void, max_bytes: usize) -> Result<Vec<u8>, TcpApiError> {
    if array.is_null() {
        return Err(TcpApiError::Transport(
            NetworkTransportError::InvalidAddress,
        ));
    }
    let length = unsafe { crate::array::zirk_rt_array_length(array) };
    if length > max_bytes {
        return Err(TcpApiError::Transport(
            NetworkTransportError::BufferLimitExceeded,
        ));
    }
    let mut bytes = Vec::with_capacity(length);
    for index in 0..length {
        let slot = unsafe { crate::array::zirk_rt_array_element(array, index as i64, 1) };
        bytes.push(unsafe { *(slot.cast::<u8>()) });
    }
    Ok(bytes)
}

/// Performs one TCP write and reports the exact partial byte count.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_write(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result =
        copy_zirk_bytes(bytes).and_then(|bytes| with_stream(handle, |stream| stream.write(&bytes)));
    let outcome = match result {
        Ok(written) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: written as u64,
            payload: std::ptr::null_mut(),
        },
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Performs a bounded write-all with transport backpressure.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_write_all(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result = copy_zirk_bytes(bytes)
        .and_then(|bytes| with_stream(handle, |stream| stream.write_all(&bytes)));
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Half-closes the stream's write direction.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_shutdown(handle: u64, out: *mut NetworkOut) {
    let result = with_stream(handle, TcpStream::shutdown);
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Closes a TCP handle. Repeated close is successful and harmless.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_close(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let result = {
        let mut registry = streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match registry.get(&handle) {
            None => Ok(()),
            Some(entry) if entry.owner != owner => Err(TcpApiError::WrongOwner),
            Some(_) => {
                registry.remove(&handle);
                Ok(())
            }
        }
    };
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// An absent handle or a handle owned by another task is reported as closed.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_tcp_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    let registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    !registry
        .get(&handle)
        .is_some_and(|entry| entry.owner == owner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, await_task, spawn, yield_now};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use zirk_runtime_core::network_policy::EffectiveNetworkGrant;

    #[test]
    fn tcp_error_codes_stay_aligned_with_the_bundled_enum() {
        assert_eq!(error_code(TcpApiError::Closed), 11);
        assert_eq!(error_code(TcpApiError::WrongOwner), 12);
        assert_eq!(
            error_code(TcpApiError::Transport(NetworkTransportError::Timeout)),
            10
        );
    }

    #[test]
    fn tls_error_codes_stay_aligned_with_the_bundled_enum() {
        let expected = [
            (NetworkTransportError::InvalidAddress, 0),
            (NetworkTransportError::PermissionDenied, 1),
            (NetworkTransportError::AddressNotAllowed, 2),
            (NetworkTransportError::DnsFailure, 3),
            (NetworkTransportError::DnsResultLimit, 4),
            (NetworkTransportError::UnsupportedTransport, 5),
            (NetworkTransportError::CertificateStore, 6),
            (NetworkTransportError::InvalidServerName, 7),
            (NetworkTransportError::InvalidServerIdentity, 8),
            (NetworkTransportError::TlsHandshake, 9),
            (NetworkTransportError::BufferLimitExceeded, 10),
            (NetworkTransportError::Io, 11),
            (NetworkTransportError::Cancelled, 12),
            (NetworkTransportError::CapacityExceeded, 13),
            (NetworkTransportError::Timeout, 14),
        ];
        for (error, expected_code) in expected {
            assert_eq!(tls_error_code(TcpApiError::Transport(error)), expected_code);
        }
        assert_eq!(tls_error_code(TcpApiError::Closed), 15);
        assert_eq!(tls_error_code(TcpApiError::WrongOwner), 16);
    }

    #[test]
    fn network_handle_registry_rejects_and_drops_resources_at_its_bound() {
        struct DropProbe(Arc<AtomicBool>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let registry = Mutex::new(
            (0..MAX_NETWORK_HANDLES)
                .map(|id| (id as u64, DropProbe(Arc::clone(&dropped))))
                .collect(),
        );
        assert!(!insert_network_handle(
            &registry,
            u64::MAX,
            DropProbe(Arc::clone(&dropped)),
        ));
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(registry.lock().unwrap().len(), MAX_NETWORK_HANDLES);
    }

    #[test]
    fn tls_der_copy_checks_the_remaining_trust_store_budget_before_allocating() {
        let bytes = unsafe { crate::array::zirk_rt_array_new(4, 1, 1, false) };
        assert!(matches!(
            copy_zirk_bytes_limited(bytes, 3),
            Err(TcpApiError::Transport(
                NetworkTransportError::BufferLimitExceeded
            ))
        ));
    }

    #[test]
    fn network_origin_scope_rejects_http_and_https_as_raw_tcp_endpoints() {
        for endpoint in ["http://example.com:80", "https://example.com:443"] {
            assert_ne!(
                NetworkOrigin::parse(endpoint).unwrap().scheme(),
                "tcp",
                "the tcp API must enforce protocol-specific authority"
            );
        }
    }

    #[test]
    fn tcp_stream_handle_rejects_a_non_owner_task_without_consuming_it() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let origin = format!("tcp://127.0.0.1:{port}");
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [])
                .expect("literal TCP origin is valid authority"),
        );
        let published = Arc::new(AtomicU64::new(0));
        let release_owner = Arc::new(AtomicBool::new(false));

        Executor::new().run_with_root(move || {
            let owner_published = Arc::clone(&published);
            let owner_release = Arc::clone(&release_owner);
            let owner_grant = Arc::clone(&grant);
            let owner_origin = origin.clone();
            let owner = spawn(move || {
                let stream = TcpStream::connect(&owner_origin, owner_grant).unwrap();
                let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
                streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        handle,
                        OwnedTcpStream {
                            owner: crate::executor::current_task(),
                            stream,
                        },
                    );
                owner_published.store(handle, Ordering::Release);
                while !owner_release.load(Ordering::Acquire) {
                    yield_now();
                }
                assert_eq!(
                    with_stream(handle, |stream| Ok(stream.peer_address())).unwrap(),
                    listener.local_addr().unwrap()
                );
                1
            });

            let handle = loop {
                let handle = published.load(Ordering::Acquire);
                if handle != 0 {
                    break handle;
                }
                yield_now();
            };
            let foreign = spawn(move || {
                let read_rejected = usize::from(
                    with_stream(handle, |stream| Ok(stream.peer_address()))
                        == Err(TcpApiError::WrongOwner),
                );
                let mut close_out = NetworkOut {
                    kind: -1,
                    error: -1,
                    scalar: 0,
                    payload: std::ptr::null_mut(),
                };
                unsafe { zirk_network_tcp_close(handle, &mut close_out) };
                let close_rejected = usize::from(close_out.kind == 1 && close_out.error == 12);
                let still_foreign = usize::from(zirk_network_tcp_is_closed(handle));
                read_rejected + close_rejected + still_foreign
            });
            assert_eq!(await_task(foreign), 3);
            assert!(
                streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(&handle)
            );
            release_owner.store(true, Ordering::Release);
            assert_eq!(await_task(owner), 1);
            assert!(
                !streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(&handle)
            );
            1
        });
    }

    #[test]
    fn accepted_tls_server_streams_share_owner_checked_io_and_cleanup() {
        let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
        let server_config =
            TlsServerConfig::from_der(vec![certificate], private_key, false).unwrap();
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );

        let outcome = Executor::new().run_with_root(move || {
            let server_grant = Arc::clone(&grant);
            let server = spawn(move || {
                let listener = TlsListener::bind(address, &server_grant, server_config).unwrap();
                let (stream, _) = listener.accept().unwrap();
                let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
                let owner = crate::executor::current_task();
                tls_server_streams()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        handle,
                        OwnedTlsStream {
                            owner,
                            stream: TlsStreamKind::Server(stream),
                        },
                    );
                let foreign = spawn(move || {
                    usize::from(
                        with_tls_stream(handle, |stream| Ok(stream.peer_address()))
                            == Err(TcpApiError::WrongOwner),
                    )
                });
                assert_eq!(await_task(foreign), 1);
                let peer = with_tls_stream(handle, |stream| Ok(stream.peer_address())).unwrap();
                assert_eq!(
                    peer.ip(),
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                );
                assert_eq!(
                    with_tls_stream(handle, |stream| stream.read(16)).unwrap(),
                    Some(b"ping".to_vec())
                );
                with_tls_stream(handle, |stream| stream.write_all(b"pong")).unwrap();
                assert!(remove_owned_tls_stream(tls_server_streams(), handle, owner).unwrap());
                1
            });

            let client_grant = Arc::clone(&grant);
            let client = spawn(move || {
                let mut stream = TlsClientStream::connect_with_trust_roots(
                    &origin,
                    client_grant,
                    vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                    false,
                )
                .unwrap();
                stream.write_all(b"ping").unwrap();
                assert_eq!(stream.read(16).unwrap(), Some(b"pong".to_vec()));
                2
            });
            assert_eq!(await_task(client), 2);
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }
}
