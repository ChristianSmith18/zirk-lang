//! Compiler-owned C ABI for the bundled `std.net::tcp` API.
//!
//! Handles stay owned by the creating Zirk task. Operations temporarily move
//! a stream out of the global handle map before parking on readiness, so no
//! scheduler thread is blocked behind a mutex while a task is suspended.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::local_transport::{LocalListener, LocalStream, StaleEndpointPolicy};
use crate::network_transport::{
    NetworkTransportError, TcpListener, TcpStream, TlsClientStream, TlsListener, TlsServerConfig,
    TlsServerStream, UdpDatagramSocket,
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
static UDP_SOCKETS: OnceLock<Mutex<HashMap<u64, OwnedUdpSocket>>> = OnceLock::new();
static LOCAL_STREAMS: OnceLock<Mutex<HashMap<u64, OwnedLocalStream>>> = OnceLock::new();
static LOCAL_LISTENERS: OnceLock<Mutex<HashMap<u64, OwnedLocalListener>>> = OnceLock::new();

fn network_connect_grant(
    endpoint: &str,
) -> Option<std::sync::Arc<zirk_runtime_core::network_policy::EffectiveNetworkGrant>> {
    match canonical_connect_origin(endpoint) {
        Some(origin) => crate::permissions::network_grant_for("network.connect", &origin).ok(),
        None => None,
    }
}

fn canonical_connect_origin(endpoint: &str) -> Option<String> {
    if let Ok(origin) = NetworkOrigin::parse(endpoint) {
        return Some(origin.as_str().to_owned());
    }
    let parsed = url::Url::parse(endpoint).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.host_str().is_none()
    {
        return None;
    }
    let host = match parsed.host()? {
        url::Host::Domain(host) => host.to_ascii_lowercase(),
        url::Host::Ipv4(host) => host.to_string(),
        url::Host::Ipv6(host) => format!("[{host}]"),
    };
    let port = parsed.port_or_known_default()?;
    let canonical = format!("{}://{host}:{port}", parsed.scheme());
    NetworkOrigin::parse(&canonical)
        .ok()
        .map(|origin| origin.as_str().to_owned())
}

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

    fn negotiated_alpn(&self) -> Option<String> {
        match self {
            Self::Client(stream) => stream.negotiated_alpn(),
            Self::Server(stream) => stream.negotiated_alpn(),
        }
    }

    fn peer_certificate_sha256(&self) -> Option<String> {
        match self {
            Self::Client(stream) => stream.peer_certificate_sha256(),
            Self::Server(stream) => stream.peer_certificate_sha256(),
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

struct OwnedUdpSocket {
    owner: TaskId,
    socket: Option<UdpDatagramSocket>,
}

struct OwnedLocalStream {
    owner: TaskId,
    stream: Option<LocalStream>,
}

struct OwnedLocalListener {
    owner: TaskId,
    listener: Option<LocalListener>,
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

fn udp_sockets() -> &'static Mutex<HashMap<u64, OwnedUdpSocket>> {
    UDP_SOCKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn local_streams() -> &'static Mutex<HashMap<u64, OwnedLocalStream>> {
    LOCAL_STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn local_listeners() -> &'static Mutex<HashMap<u64, OwnedLocalListener>> {
    LOCAL_LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
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
    local_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    local_listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
    udp_sockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|_, entry| entry.owner != owner);
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

fn with_udp_socket<T>(
    handle: u64,
    operation: impl FnOnce(&UdpDatagramSocket) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    with_udp_socket_mut(handle, |socket| operation(socket))
}

fn with_udp_socket_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut UdpDatagramSocket) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    let owner = crate::executor::current_task();
    let mut socket = {
        let mut registry = udp_sockets()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            return Err(TcpApiError::Closed);
        };
        if entry.owner != owner {
            return Err(TcpApiError::WrongOwner);
        }
        let Some(socket) = entry.socket.take() else {
            return Err(TcpApiError::Closed);
        };
        socket
    };
    let result = operation(&mut socket);
    let cancelled = matches!(result, Err(NetworkTransportError::Cancelled));
    if !cancelled
        && let Some(entry) = udp_sockets()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(&handle)
    {
        entry.socket = Some(socket);
    }
    result.map_err(TcpApiError::Transport)
}

fn with_local_stream_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut LocalStream) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    let owner = crate::executor::current_task();
    let mut stream = {
        let mut registry = local_streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            return Err(TcpApiError::Closed);
        };
        if entry.owner != owner {
            return Err(TcpApiError::WrongOwner);
        }
        let Some(stream) = entry.stream.take() else {
            return Err(TcpApiError::Closed);
        };
        stream
    };
    let result = operation(&mut stream);
    if !matches!(result, Err(NetworkTransportError::Cancelled))
        && let Some(entry) = local_streams()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(&handle)
    {
        entry.stream = Some(stream);
    }
    result.map_err(TcpApiError::Transport)
}

fn with_local_listener_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut LocalListener) -> Result<T, NetworkTransportError>,
) -> Result<T, TcpApiError> {
    let owner = crate::executor::current_task();
    let mut listener = {
        let mut registry = local_listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            return Err(TcpApiError::Closed);
        };
        if entry.owner != owner {
            return Err(TcpApiError::WrongOwner);
        }
        let Some(listener) = entry.listener.take() else {
            return Err(TcpApiError::Closed);
        };
        listener
    };
    let result = operation(&mut listener);
    if !matches!(result, Err(NetworkTransportError::Cancelled))
        && let Some(entry) = local_listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(&handle)
    {
        entry.listener = Some(listener);
    }
    result.map_err(TcpApiError::Transport)
}

fn local_error_code(error: TcpApiError) -> i32 {
    match error {
        TcpApiError::Closed => 9,
        TcpApiError::WrongOwner => 10,
        TcpApiError::Transport(error) => match error {
            NetworkTransportError::InvalidAddress => 0,
            NetworkTransportError::PermissionDenied => 1,
            NetworkTransportError::AddressNotAllowed => 2,
            NetworkTransportError::UnsupportedTransport => 3,
            NetworkTransportError::BufferLimitExceeded => 4,
            NetworkTransportError::Cancelled => 6,
            NetworkTransportError::CapacityExceeded => 7,
            NetworkTransportError::Timeout => 8,
            NetworkTransportError::DnsFailure
            | NetworkTransportError::DnsResultLimit
            | NetworkTransportError::CertificateStore
            | NetworkTransportError::InvalidServerName
            | NetworkTransportError::InvalidServerIdentity
            | NetworkTransportError::InvalidTlsConfiguration
            | NetworkTransportError::TlsHandshake
            | NetworkTransportError::Io => 5,
        },
    }
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

/// Native result for one received UDP packet. `payload` and `peer` are live
/// GC-managed Zirk values on success and null otherwise.
#[repr(C)]
pub struct NetworkDatagramOut {
    pub kind: i32,
    pub error: i32,
    pub payload: *mut c_void,
    pub peer: *mut c_void,
}

fn empty_datagram_out(kind: i32, error: i32) -> NetworkDatagramOut {
    NetworkDatagramOut {
        kind,
        error,
        payload: std::ptr::null_mut(),
        peer: std::ptr::null_mut(),
    }
}

pub(crate) fn parse_udp_endpoint(
    text: &str,
) -> Result<std::net::SocketAddr, NetworkTransportError> {
    let address = text
        .strip_prefix("udp://")
        .ok_or(NetworkTransportError::InvalidAddress)?
        .parse::<std::net::SocketAddr>()
        .map_err(|_| NetworkTransportError::InvalidAddress)?;
    if address.port() == 0 || text != format!("udp://{address}") {
        return Err(NetworkTransportError::InvalidAddress);
    }
    Ok(address)
}

fn parse_udp_multicast_group(text: &str) -> Result<std::net::IpAddr, NetworkTransportError> {
    let group = text
        .parse::<std::net::IpAddr>()
        .map_err(|_| NetworkTransportError::InvalidAddress)?;
    if !group.is_multicast() || text != group.to_string() {
        return Err(NetworkTransportError::InvalidAddress);
    }
    Ok(group)
}

fn request_udp_send_grant(
    address: std::net::SocketAddr,
) -> Option<std::sync::Arc<zirk_runtime_core::network_policy::EffectiveNetworkGrant>> {
    if address.ip().is_multicast()
        && !crate::permissions::request_resource(
            "network.udp.send",
            &format!("udp-multicast://{}", address.ip()),
        )
    {
        return None;
    }
    crate::permissions::network_grant_for("network.udp.send", &format!("udp://{address}")).ok()
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
            | NetworkTransportError::InvalidTlsConfiguration
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
            NetworkTransportError::InvalidTlsConfiguration => 17,
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

fn take_tcp_stream_for_tls(handle: u64) -> Result<TcpStream, TcpApiError> {
    let owner = crate::executor::current_task();
    let mut registry = streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = registry.get(&handle) else {
        return Err(TcpApiError::Closed);
    };
    if entry.owner != owner {
        return Err(TcpApiError::WrongOwner);
    }
    Ok(registry.remove(&handle).expect("checked TCP handle").stream)
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
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
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

fn copy_zirk_optional_trust_roots(array: *mut c_void) -> Result<Vec<Vec<u8>>, TcpApiError> {
    if array.is_null() {
        return Err(TcpApiError::Transport(
            NetworkTransportError::InvalidTlsConfiguration,
        ));
    }
    if unsafe { crate::array::zirk_rt_array_length(array) } == 0 {
        return Ok(Vec::new());
    }
    copy_zirk_der_certificates(
        array,
        NetworkTransportError::InvalidTlsConfiguration,
        NetworkTransportError::BufferLimitExceeded,
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
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
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

/// Opens an authenticated TLS client stream using a bounded ALPN preference
/// list and system trust roots.
///
/// # Safety
/// `endpoint` and `protocols` must be live managed Zirk String and
/// `Array<String>` values, and `out` must point to writable `NetworkOut`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_connect_with_alpn(
    endpoint: *const c_void,
    roots_der: *mut c_void,
    allow_tls12: bool,
    protocols: *mut c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let roots_der = match copy_zirk_optional_trust_roots(roots_der) {
        Ok(roots) => roots,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let protocols = match unsafe { copy_zirk_alpn_protocols(protocols) } {
        Ok(protocols) => protocols,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    match TlsClientStream::connect_with_options(
        unsafe { endpoint.as_str() },
        grant,
        roots_der,
        allow_tls12,
        protocols,
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

/// Opens a mutual TLS client connection with bounded certificate and key DER.
/// The private key is zeroized after rustls constructs the signer.
///
/// # Safety
/// All Zirk pointers must reference live values of the declared types, and
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_connect_with_client_identity(
    endpoint: *const c_void,
    roots_der: *mut c_void,
    certificate_chain_der: *mut c_void,
    private_key_der: *mut c_void,
    allow_tls12: bool,
    protocols: *mut c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let roots_der = match copy_zirk_optional_trust_roots(roots_der) {
        Ok(roots) => roots,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let certificate_chain_der = match copy_zirk_certificate_chain(certificate_chain_der) {
        Ok(chain) => chain,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    const MAX_PRIVATE_KEY_BYTES: usize = 1024 * 1024;
    let private_key_der = match copy_zirk_bytes_limited(private_key_der, MAX_PRIVATE_KEY_BYTES) {
        Ok(key) if !key.is_empty() => zeroize::Zeroizing::new(key),
        Ok(_) => {
            unsafe {
                set_out(
                    out,
                    empty_out(
                        1,
                        tls_error_code(TcpApiError::Transport(
                            NetworkTransportError::InvalidServerIdentity,
                        )),
                    ),
                )
            };
            return;
        }
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let protocols = match unsafe { copy_zirk_alpn_protocols(protocols) } {
        Ok(protocols) => protocols,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    match TlsClientStream::connect_with_client_identity(
        unsafe { endpoint.as_str() },
        grant,
        roots_der,
        certificate_chain_der,
        private_key_der.to_vec(),
        allow_tls12,
        protocols,
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

/// Upgrades an owner-task TCP resource using the already-connected socket.
/// Validation and permission failures preserve the TCP handle; after the TLS
/// handshake begins, the plaintext handle is consumed even if the handshake
/// fails because its byte stream can no longer safely be reused.
///
/// # Safety
/// All Zirk pointers must reference live values of the declared types, and
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_upgrade_tcp(
    tcp_handle: u64,
    endpoint: *const c_void,
    roots_der: *mut c_void,
    allow_tls12: bool,
    protocols: *mut c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let roots_der = match copy_zirk_optional_trust_roots(roots_der) {
        Ok(roots) => roots,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let protocols = match unsafe { copy_zirk_alpn_protocols(protocols) } {
        Ok(protocols) => protocols,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let peer = match with_stream(tcp_handle, |stream| Ok(stream.peer_address())) {
        Ok(peer) => peer,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let prepared = match TlsClientStream::prepare_tcp_upgrade(
        unsafe { endpoint.as_str() },
        peer,
        &grant,
        roots_der,
        allow_tls12,
        protocols,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error.into()))) };
            return;
        }
    };
    let tcp_stream = match take_tcp_stream_for_tls(tcp_handle) {
        Ok(stream) => stream,
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    match TlsClientStream::wrap_tcp(tcp_stream, prepared) {
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

unsafe fn copy_zirk_alpn_protocols(array: *mut c_void) -> Result<Vec<Vec<u8>>, TcpApiError> {
    const MAX_PROTOCOLS: usize = 16;
    const MAX_PROTOCOL_BYTES: usize = 1024;
    if array.is_null() {
        return Err(TcpApiError::Transport(
            NetworkTransportError::InvalidTlsConfiguration,
        ));
    }
    let count = unsafe { crate::array::zirk_rt_array_length(array) };
    if count == 0 || count > MAX_PROTOCOLS {
        return Err(TcpApiError::Transport(
            NetworkTransportError::InvalidTlsConfiguration,
        ));
    }
    let mut protocols = Vec::with_capacity(count);
    let mut total = 0usize;
    let pointer_size = std::mem::size_of::<*mut c_void>();
    for index in 0..count {
        let slot =
            unsafe { crate::array::zirk_rt_array_element(array, index as i64, pointer_size) };
        let pointer = unsafe { *slot.cast::<*mut c_void>() };
        let Some(protocol) = (unsafe { crate::string::borrow(pointer) }) else {
            return Err(TcpApiError::Transport(
                NetworkTransportError::InvalidTlsConfiguration,
            ));
        };
        let bytes = unsafe { protocol.as_str() }.as_bytes();
        total = total
            .checked_add(bytes.len())
            .ok_or(TcpApiError::Transport(
                NetworkTransportError::InvalidTlsConfiguration,
            ))?;
        if total > MAX_PROTOCOL_BYTES {
            return Err(TcpApiError::Transport(
                NetworkTransportError::InvalidTlsConfiguration,
            ));
        }
        protocols.push(bytes.to_vec());
    }
    crate::network_transport::validate_tls_alpn_protocols(&protocols)
        .map_err(TcpApiError::Transport)?;
    Ok(protocols)
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
            std::ptr::null_mut(),
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
            std::ptr::null_mut(),
            allow_tls12,
            true,
            out,
        )
    }
}

/// Binds a TLS listener that requires a client identity from explicit roots.
///
/// # Safety
/// All Zirk pointers must reference live values of their declared types, and
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_listen_with_client_auth(
    endpoint: *const c_void,
    certificate_chain: *mut c_void,
    private_key: *mut c_void,
    client_roots_der: *mut c_void,
    allow_tls12: bool,
    out: *mut NetworkOut,
) {
    unsafe {
        tls_listener_bind(
            endpoint,
            certificate_chain,
            private_key,
            client_roots_der,
            allow_tls12,
            false,
            out,
        )
    }
}

unsafe fn tls_listener_bind(
    endpoint: *const c_void,
    certificate_chain: *mut c_void,
    private_key: *mut c_void,
    client_roots_der: *mut c_void,
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
    let Some(grant) =
        crate::permissions::network_grant_for("network.listen", &address.to_string()).ok()
    else {
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
        Ok(key) => zeroize::Zeroizing::new(key),
        Err(error) => {
            unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
            return;
        }
    };
    let config = if client_roots_der.is_null() {
        TlsServerConfig::from_der(chain, private_key.to_vec(), allow_tls12)
    } else {
        match copy_zirk_certificate_chain(client_roots_der) {
            Ok(client_roots) => TlsServerConfig::from_der_with_client_auth(
                chain,
                private_key.to_vec(),
                client_roots,
                allow_tls12,
            ),
            Err(error) => {
                unsafe { set_out(out, empty_out(1, tls_error_code(error))) };
                return;
            }
        }
    };
    let config = match config {
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

/// Accepts a TLS peer on an owner-task listener while also waiting for an
/// executor-scoped HTTP server shutdown control. A shutdown wake preserves the
/// listener handle so the owning HTTP server can close it after draining.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_accept_until_shutdown(
    handle: u64,
    control_handle: u64,
    out: *mut NetworkOut,
) {
    let owner = crate::executor::current_task();
    let shutdown = crate::http_api::http_server_control_shutdown_receiver(control_handle);
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
        let Some(shutdown) = shutdown else {
            unsafe { set_out(out, empty_out(1, 13)) };
            return;
        };
        let Some(listener) = entry.listener.take() else {
            unsafe { set_out(out, empty_out(1, 13)) };
            return;
        };
        (listener, shutdown)
    };
    let (listener, shutdown) = listener;
    let accepted = listener.accept_until_shutdown(shutdown);
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
        Ok(Some((stream, _peer))) => {
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
        Ok(None) => empty_out(1, 12),
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

/// Returns the ALPN protocol selected by the authenticated TLS handshake, if any.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_negotiated_alpn(handle: u64, out: *mut NetworkOut) {
    let result = with_tls_stream(handle, |stream| Ok(stream.negotiated_alpn()));
    let outcome = match result {
        Ok(Some(protocol)) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&protocol),
        },
        Ok(None) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: std::ptr::null_mut(),
        },
        Err(error) => empty_out(1, tls_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Returns the SHA-256 fingerprint of the authenticated peer's leaf
/// certificate. A null payload means that this TLS role has no peer identity.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tls_peer_certificate_sha256(
    handle: u64,
    out: *mut NetworkOut,
) {
    let result = with_tls_stream(handle, |stream| Ok(stream.peer_certificate_sha256()));
    let outcome = match result {
        Ok(Some(fingerprint)) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&fingerprint),
        },
        Ok(None) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: std::ptr::null_mut(),
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

/// Creates an owner-task UDP socket for a canonical `udp://IP:PORT` peer.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_connect(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let text = unsafe { endpoint.as_str() };
    let address = match parse_udp_endpoint(text) {
        Ok(address) => address,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) = request_udp_send_grant(address) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let outcome = match UdpDatagramSocket::connect(
        address,
        grant,
        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
    ) {
        Ok(socket) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                empty_out(1, 9)
            } else if insert_network_handle(
                udp_sockets(),
                handle,
                OwnedUdpSocket {
                    owner: crate::executor::current_task(),
                    socket: Some(socket),
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

/// Binds an owner-task UDP socket under the separate UDP listen grant.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_bind(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let address = match parse_udp_endpoint(unsafe { endpoint.as_str() }) {
        Ok(address) => address,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) =
        crate::permissions::network_grant_for("network.udp.listen", &address.to_string()).ok()
    else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let outcome = match UdpDatagramSocket::bind(
        address,
        grant,
        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
    ) {
        Ok(socket) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                empty_out(1, 9)
            } else if insert_network_handle(
                udp_sockets(),
                handle,
                OwnedUdpSocket {
                    owner: crate::executor::current_task(),
                    socket: Some(socket),
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

/// Sends one complete datagram through an owner-task connected UDP socket.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_send(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result =
        copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_UDP_IPV6_DATAGRAM_BYTES)
            .and_then(|bytes| {
                with_udp_socket(handle, |socket| {
                    let written =
                        socket.send(&bytes, crate::network_transport::DEFAULT_NETWORK_TIMEOUT)?;
                    if written != bytes.len() {
                        return Err(NetworkTransportError::Io);
                    }
                    Ok(written)
                })
            });
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

/// Sends one datagram to an explicitly supplied, exactly approved peer.
///
/// # Safety
/// Both inputs are live Zirk values and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_send_to(
    handle: u64,
    bytes: *mut c_void,
    endpoint: *const c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let peer = match parse_udp_endpoint(unsafe { endpoint.as_str() }) {
        Ok(peer) => peer,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) = request_udp_send_grant(peer) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let result =
        copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_UDP_IPV6_DATAGRAM_BYTES)
            .and_then(|bytes| {
                with_udp_socket_mut(handle, |socket| {
                    socket.replace_grant(grant);
                    socket.send_to(
                        &bytes,
                        peer,
                        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
                    )
                })
            });
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

/// Sends to the limited broadcast address under both explicit UDP grants.
///
/// # Safety
/// `bytes`, `endpoint`, and `out` must be valid Zirk ABI values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_send_broadcast(
    handle: u64,
    bytes: *mut c_void,
    endpoint: *const c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let destination = match parse_udp_endpoint(unsafe { endpoint.as_str() }) {
        Ok(address) => address,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) = request_udp_send_grant(destination) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let result =
        copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_UDP_IPV4_DATAGRAM_BYTES)
            .and_then(|bytes| {
                with_udp_socket_mut(handle, |socket| {
                    socket.replace_grant(grant);
                    socket.send_broadcast(
                        &bytes,
                        destination,
                        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
                    )
                })
            });
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

/// Sends to an explicitly authorized multicast group and port.
///
/// # Safety
/// `bytes`, `endpoint`, and `out` must be valid Zirk ABI values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_send_multicast(
    handle: u64,
    bytes: *mut c_void,
    endpoint: *const c_void,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let destination = match parse_udp_endpoint(unsafe { endpoint.as_str() }) {
        Ok(address) => address,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let Some(grant) = request_udp_send_grant(destination) else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let result =
        copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_UDP_IPV6_DATAGRAM_BYTES)
            .and_then(|bytes| {
                with_udp_socket_mut(handle, |socket| {
                    socket.replace_grant(grant);
                    socket.send_multicast(
                        &bytes,
                        destination,
                        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
                    )
                })
            });
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

/// Joins a multicast group after the exact group grant has been checked.
///
/// # Safety
/// `group` and `out` must be valid Zirk ABI values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_join_multicast(
    handle: u64,
    group: *const c_void,
    out: *mut NetworkOut,
) {
    let Some(group) = (unsafe { crate::string::borrow(group) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let group = match parse_udp_multicast_group(unsafe { group.as_str() }) {
        Ok(group) => group,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    if !crate::permissions::request_resource(
        "network.udp.send",
        &format!("udp-multicast://{group}"),
    ) {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    }
    let grant = crate::permissions::current_network_grant();
    let result = grant
        .map_err(|_| TcpApiError::Transport(NetworkTransportError::PermissionDenied))
        .and_then(|grant| {
            with_udp_socket_mut(handle, |socket| {
                socket.replace_grant(grant);
                socket.join_multicast(group)
            })
        });
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Leaves a multicast group previously joined by this socket.
///
/// # Safety
/// `group` and `out` must be valid Zirk ABI values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_leave_multicast(
    handle: u64,
    group: *const c_void,
    out: *mut NetworkOut,
) {
    let Some(group) = (unsafe { crate::string::borrow(group) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let group = match parse_udp_multicast_group(unsafe { group.as_str() }) {
        Ok(group) => group,
        Err(_) => {
            unsafe { set_out(out, empty_out(1, 0)) };
            return;
        }
    };
    let result = with_udp_socket_mut(handle, |socket| socket.leave_multicast(group));
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Returns the numeric local address of an owner-task UDP socket.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_local_address(handle: u64, out: *mut NetworkOut) {
    let result = with_udp_socket(handle, |socket| socket.local_address());
    let outcome = match result {
        Ok(address) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::string::alloc_owned(&format!("udp://{address}")),
        },
        Err(error) => empty_out(1, error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Receives one complete datagram and the canonical numeric peer endpoint.
///
/// # Safety
/// `out` must point to writable `NetworkDatagramOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_receive(
    handle: u64,
    max_bytes: u64,
    out: *mut NetworkDatagramOut,
) {
    let result = usize::try_from(max_bytes)
        .map_err(|_| TcpApiError::Transport(NetworkTransportError::BufferLimitExceeded))
        .and_then(|max_bytes| {
            with_udp_socket(handle, |socket| {
                socket.receive(max_bytes, crate::network_transport::DEFAULT_NETWORK_TIMEOUT)
            })
        });
    let outcome = match result {
        Ok((bytes, peer)) => NetworkDatagramOut {
            kind: 0,
            error: 0,
            payload: crate::array::alloc_byte_array(&bytes),
            peer: crate::string::alloc_owned(&format!("udp://{peer}")),
        },
        Err(error) => empty_datagram_out(1, error_code(error)),
    };
    if !out.is_null() {
        unsafe { *out = outcome };
    }
}

/// Closes and releases an owner-task UDP socket.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_udp_close(handle: u64, out: *mut NetworkOut) {
    let owner = crate::executor::current_task();
    let result = {
        let mut registry = udp_sockets()
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

/// Reports whether the current task owns a live UDP socket handle.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_udp_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    udp_sockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_none_or(|entry| entry.owner != owner || entry.socket.is_none())
}

/// Opens a permission-checked owner-task local IPC stream.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_connect(endpoint: *const c_void, out: *mut NetworkOut) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let Some(grant) = crate::permissions::network_grant_for("network.local.connect", unsafe {
        endpoint.as_str()
    })
    .ok() else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let outcome = match LocalStream::connect(
        unsafe { endpoint.as_str() },
        &grant,
        crate::network_transport::DEFAULT_NETWORK_TIMEOUT,
    ) {
        Ok(stream) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0
                || !insert_network_handle(
                    local_streams(),
                    handle,
                    OwnedLocalStream {
                        owner: crate::executor::current_task(),
                        stream: Some(stream),
                    },
                )
            {
                empty_out(1, 7)
            } else {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            }
        }
        Err(error) => empty_out(1, local_error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Binds a local IPC endpoint under its distinct listen grant.
///
/// # Safety
/// `endpoint` is a live Zirk String and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_bind(
    endpoint: *const c_void,
    remove_stale: bool,
    out: *mut NetworkOut,
) {
    let Some(endpoint) = (unsafe { crate::string::borrow(endpoint) }) else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let Some(grant) =
        crate::permissions::network_grant_for("network.local.listen", unsafe { endpoint.as_str() })
            .ok()
    else {
        unsafe { set_out(out, empty_out(1, 1)) };
        return;
    };
    let stale_policy = if remove_stale {
        StaleEndpointPolicy::RemoveStaleSocket
    } else {
        StaleEndpointPolicy::RejectExisting
    };
    let outcome = match LocalListener::bind(unsafe { endpoint.as_str() }, stale_policy, &grant) {
        Ok(listener) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0
                || !insert_network_handle(
                    local_listeners(),
                    handle,
                    OwnedLocalListener {
                        owner: crate::executor::current_task(),
                        listener: Some(listener),
                    },
                )
            {
                empty_out(1, 7)
            } else {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: handle,
                    payload: std::ptr::null_mut(),
                }
            }
        }
        Err(error) => empty_out(1, local_error_code(TcpApiError::Transport(error))),
    };
    unsafe { set_out(out, outcome) };
}

/// Accepts one bounded, cancellable local IPC connection.
///
/// # Safety
/// `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_accept(handle: u64, out: *mut NetworkOut) {
    let result = with_local_listener_mut(handle, |listener| {
        listener.accept(crate::network_transport::DEFAULT_NETWORK_TIMEOUT)
    });
    let outcome = match result {
        Ok(stream) => {
            let stream_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if stream_handle == 0
                || !insert_network_handle(
                    local_streams(),
                    stream_handle,
                    OwnedLocalStream {
                        owner: crate::executor::current_task(),
                        stream: Some(stream),
                    },
                )
            {
                empty_out(1, 7)
            } else {
                NetworkOut {
                    kind: 0,
                    error: 0,
                    scalar: stream_handle,
                    payload: std::ptr::null_mut(),
                }
            }
        }
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Reads a bounded local-stream chunk; kind 1 reports EOF.
///
/// # Safety
/// `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_read(
    handle: u64,
    max_bytes: u64,
    out: *mut NetworkOut,
) {
    let result = usize::try_from(max_bytes)
        .map_err(|_| TcpApiError::Transport(NetworkTransportError::BufferLimitExceeded))
        .and_then(|max_bytes| with_local_stream_mut(handle, |stream| stream.read(max_bytes)));
    let outcome = match result {
        Ok(Some(bytes)) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: crate::array::alloc_byte_array(&bytes),
        },
        Ok(None) => empty_out(1, 0),
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Writes one bounded local-stream chunk and returns its byte count.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_write(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result = copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_TCP_WRITE_BYTES)
        .and_then(|bytes| with_local_stream_mut(handle, |stream| stream.write(&bytes)));
    let outcome = match result {
        Ok(count) => NetworkOut {
            kind: 0,
            error: 0,
            scalar: count as u64,
            payload: std::ptr::null_mut(),
        },
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Writes all bounded bytes to a local stream.
///
/// # Safety
/// `bytes` is a live Zirk byte array and `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_write_all(
    handle: u64,
    bytes: *mut c_void,
    out: *mut NetworkOut,
) {
    let result = copy_zirk_bytes_limited(bytes, crate::network_transport::MAX_TCP_WRITE_BYTES)
        .and_then(|bytes| with_local_stream_mut(handle, |stream| stream.write_all(&bytes)));
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Half-closes a local stream's write side.
///
/// # Safety
/// `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_shutdown(handle: u64, out: *mut NetworkOut) {
    let outcome = match with_local_stream_mut(handle, LocalStream::shutdown) {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Releases an owner-task local stream; closing an already-closed handle succeeds.
///
/// # Safety
/// `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_close_stream(handle: u64, out: *mut NetworkOut) {
    let result = remove_owned_local_handle(local_streams(), handle);
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

/// Releases an owner-task local listener; closing an already-closed handle succeeds.
///
/// # Safety
/// `out` is writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_local_close_listener(handle: u64, out: *mut NetworkOut) {
    let result = remove_owned_local_handle(local_listeners(), handle);
    let outcome = match result {
        Ok(()) => empty_out(0, 0),
        Err(error) => empty_out(1, local_error_code(error)),
    };
    unsafe { set_out(out, outcome) };
}

fn remove_owned_local_handle<T>(
    registry: &Mutex<HashMap<u64, T>>,
    handle: u64,
) -> Result<(), TcpApiError>
where
    T: LocalHandleOwner,
{
    let owner = crate::executor::current_task();
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match registry.get(&handle) {
        None => Ok(()),
        Some(entry) if entry.local_owner() != owner => Err(TcpApiError::WrongOwner),
        Some(_) => {
            registry.remove(&handle);
            Ok(())
        }
    }
}

trait LocalHandleOwner {
    fn local_owner(&self) -> TaskId;
}

impl LocalHandleOwner for OwnedLocalStream {
    fn local_owner(&self) -> TaskId {
        self.owner
    }
}

impl LocalHandleOwner for OwnedLocalListener {
    fn local_owner(&self) -> TaskId {
        self.owner
    }
}

/// Reports whether the current task owns a live local stream.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_local_stream_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    local_streams()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_none_or(|entry| entry.owner != owner || entry.stream.is_none())
}

/// Reports whether the current task owns a live local listener.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_network_local_listener_is_closed(handle: u64) -> bool {
    let owner = crate::executor::current_task();
    local_listeners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&handle)
        .is_none_or(|entry| entry.owner != owner || entry.listener.is_none())
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
    let Some(grant) = network_connect_grant(unsafe { endpoint.as_str() }) else {
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

/// Returns a permission-checked immutable snapshot as rows of bounded strings.
/// The bundled Zirk module converts these private wire rows into its public
/// typed records.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_interfaces_snapshot(out: *mut NetworkOut) {
    if !crate::permissions::request_resource("network.inspect_interfaces", "network interfaces") {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    }
    let Some(grant) = crate::permissions::current_network_grant().ok() else {
        unsafe { set_out(out, empty_out(1, 0)) };
        return;
    };
    let snapshot = match crate::network_interfaces::snapshot_network_interfaces(&grant) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let code = match error {
                NetworkTransportError::PermissionDenied => 0,
                NetworkTransportError::BufferLimitExceeded => 1,
                NetworkTransportError::Io => 2,
                NetworkTransportError::UnsupportedTransport => 3,
                NetworkTransportError::Cancelled => 4,
                NetworkTransportError::CapacityExceeded => 5,
                _ => 2,
            };
            unsafe { set_out(out, empty_out(1, code)) };
            return;
        }
    };

    let element_size = std::mem::size_of::<*mut c_void>();
    let element_align = std::mem::align_of::<*mut c_void>();
    let mut rows = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
    let mut row = std::ptr::null_mut();
    let mut field = std::ptr::null_mut();
    let mut roots = [
        (&mut rows as *mut *mut c_void).cast(),
        (&mut row as *mut *mut c_void).cast(),
        (&mut field as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    for item in snapshot {
        row = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
        let index = item
            .index
            .map_or_else(String::new, |value| value.to_string());
        let fields = [
            item.name,
            index,
            item.operational_status.as_str().to_owned(),
            item.is_loopback.to_string(),
            item.is_point_to_point.to_string(),
            item.address.to_string(),
            item.prefix_length.to_string(),
            item.scope_id
                .map_or_else(String::new, |value| value.to_string()),
        ];
        for value in fields {
            field = crate::string::alloc_owned(&value);
            unsafe {
                crate::list::zirk_rt_list_add(
                    row,
                    (&field as *const *mut c_void).cast(),
                    element_size,
                    element_align,
                    true,
                )
            };
        }
        unsafe {
            crate::list::zirk_rt_list_add(
                rows,
                (&row as *const *mut c_void).cast(),
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
                payload: rows,
            },
        )
    };
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
    let Some(grant) =
        crate::permissions::network_grant_for("network.connect", origin.as_str()).ok()
    else {
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
    let Some(grant) =
        crate::permissions::network_grant_for("network.listen", &address.to_string()).ok()
    else {
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

/// Accepts a TCP peer while waiting on the HTTP server shutdown control.
/// Shutdown returns the existing typed `Cancelled` outcome but leaves the
/// listener registered so the server can close it deterministically.
///
/// # Safety
/// `out` must point to writable `NetworkOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_network_tcp_accept_until_shutdown(
    handle: u64,
    control_handle: u64,
    out: *mut NetworkOut,
) {
    let owner = crate::executor::current_task();
    let shutdown = crate::http_api::http_server_control_shutdown_receiver(control_handle);
    let listener = {
        let mut registry = listeners()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = registry.get_mut(&handle) else {
            unsafe { set_out(out, empty_out(1, 7)) };
            return;
        };
        if entry.owner != owner {
            unsafe { set_out(out, empty_out(1, 8)) };
            return;
        }
        let Some(shutdown) = shutdown else {
            unsafe { set_out(out, empty_out(1, 7)) };
            return;
        };
        let Some(listener) = entry.listener.take() else {
            unsafe { set_out(out, empty_out(1, 7)) };
            return;
        };
        (listener, shutdown)
    };
    let (listener, shutdown) = listener;
    let accepted = listener.accept_until_shutdown(shutdown);
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
        Ok(Some((stream, _peer))) => {
            let stream_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if stream_handle == 0 {
                drop(stream);
                empty_out(1, 7)
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
                empty_out(1, 7)
            }
        }
        Ok(None) => empty_out(1, 8),
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
    use crate::executor::{Executor, await_task, cancel_current, spawn, yield_now};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;
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
            (NetworkTransportError::InvalidTlsConfiguration, 17),
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
    fn udp_socket_handles_are_owner_checked_and_cleaned_when_the_task_exits() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let peer = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_network_scopes(
                std::iter::empty(),
                std::iter::empty(),
                [format!("udp://{peer}")],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );
        let published = Arc::new(AtomicU64::new(0));
        let release_owner = Arc::new(AtomicBool::new(false));

        Executor::new().run_with_root(move || {
            let owner_published = Arc::clone(&published);
            let owner_release = Arc::clone(&release_owner);
            let owner_grant = Arc::clone(&grant);
            let owner = spawn(move || {
                let socket = UdpDatagramSocket::connect(peer, owner_grant, Duration::from_secs(1))
                    .expect("approved UDP peer should connect");
                let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
                udp_sockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        handle,
                        OwnedUdpSocket {
                            owner: crate::executor::current_task(),
                            socket: Some(socket),
                        },
                    );
                owner_published.store(handle, Ordering::Release);
                while !owner_release.load(Ordering::Acquire) {
                    yield_now();
                }
                assert!(!zirk_network_udp_is_closed(handle));
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
                let operation_rejected = usize::from(
                    with_udp_socket(handle, |_| Ok(())) == Err(TcpApiError::WrongOwner),
                );
                let mut close_out = NetworkOut {
                    kind: -1,
                    error: -1,
                    scalar: 0,
                    payload: std::ptr::null_mut(),
                };
                unsafe { zirk_network_udp_close(handle, &mut close_out) };
                let close_rejected = usize::from(close_out.kind == 1 && close_out.error == 12);
                let still_foreign = usize::from(zirk_network_udp_is_closed(handle));
                operation_rejected + close_rejected + still_foreign
            });
            assert_eq!(await_task(foreign), 3);
            assert!(
                udp_sockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(&handle)
            );
            release_owner.store(true, Ordering::Release);
            assert_eq!(await_task(owner), 1);
            assert!(
                !udp_sockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(&handle)
            );
            1
        });
    }

    #[test]
    fn cancelling_a_pending_udp_receive_releases_its_owned_handle() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let peer = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_network_scopes(
                std::iter::empty(),
                std::iter::empty(),
                [format!("udp://{peer}")],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );
        let published = Arc::new(AtomicU64::new(0));
        let receiving = Arc::new(AtomicBool::new(false));

        let outcome = Executor::new().run_with_root(move || {
            let child_published = Arc::clone(&published);
            let child_receiving = Arc::clone(&receiving);
            let child_grant = Arc::clone(&grant);
            let receiver = spawn(move || {
                let socket = UdpDatagramSocket::connect(peer, child_grant, Duration::from_secs(30))
                    .expect("approved UDP peer should connect");
                let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
                udp_sockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        handle,
                        OwnedUdpSocket {
                            owner: crate::executor::current_task(),
                            socket: Some(socket),
                        },
                    );
                child_published.store(handle, Ordering::Release);
                child_receiving.store(true, Ordering::Release);
                let _ = with_udp_socket(handle, |socket| {
                    socket.receive(128, Duration::from_secs(30)).map(|_| ())
                });
                1
            });
            while !receiving.load(Ordering::Acquire) {
                yield_now();
            }
            // Give the network future a chance to park on receive readiness.
            for _ in 0..8 {
                yield_now();
            }
            cancel_current(receiver);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { await_task(receiver) }))
                    .is_err()
            );
            let handle = published.load(Ordering::Acquire);
            assert!(
                udp_sockets()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&handle)
                    .is_none_or(|entry| entry.owner != receiver)
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
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
