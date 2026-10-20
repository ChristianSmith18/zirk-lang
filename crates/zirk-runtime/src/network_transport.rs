//! Permission-checked TCP operations on Zirk's cooperative task stack.

use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{
    TcpListener as TokioTcpListener, TcpStream as TokioTcpStream, UdpSocket as TokioUdpSocket,
};
use zirk_runtime_core::http_values::Url;
use zirk_runtime_core::network_policy::{EffectiveNetworkGrant, NetworkOrigin, NetworkPolicyError};

const MAX_DNS_RESULTS: usize = 32;
const MAX_DNS_CACHE_ENTRIES: usize = 256;
const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
pub const DEFAULT_NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_TCP_READ_BYTES: usize = 1024 * 1024;
pub const MAX_TCP_WRITE_BYTES: usize = 1024 * 1024;
pub const MAX_UDP_IPV4_DATAGRAM_BYTES: usize = 65_507;
pub const MAX_UDP_IPV6_DATAGRAM_BYTES: usize = 65_527;

#[derive(Default)]
struct DnsCache {
    entries: VecDeque<((String, u16), Instant, Vec<SocketAddr>)>,
}

impl DnsCache {
    fn get(&mut self, key: &(String, u16), now: Instant) -> Option<Vec<SocketAddr>> {
        self.entries.retain(|(_, expires, _)| *expires > now);
        self.entries
            .iter()
            .find(|(candidate, _, _)| candidate == key)
            .map(|(_, _, addresses)| addresses.clone())
    }

    fn insert(&mut self, key: (String, u16), addresses: Vec<SocketAddr>, now: Instant) {
        self.entries.retain(|(candidate, _, _)| candidate != &key);
        while self.entries.len() >= MAX_DNS_CACHE_ENTRIES {
            self.entries.pop_front();
        }
        self.entries
            .push_back((key, now + DNS_CACHE_TTL, addresses));
    }
}

static DNS_CACHE: Mutex<DnsCache> = Mutex::new(DnsCache {
    entries: VecDeque::new(),
});

fn cached_dns(host: &str, port: u16) -> Option<Vec<SocketAddr>> {
    DNS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(host.to_owned(), port), Instant::now())
}

fn cache_dns(host: &str, port: u16, addresses: Vec<SocketAddr>) {
    DNS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert((host.to_owned(), port), addresses, Instant::now());
}

fn validate_dns_answers(
    origin: &str,
    addresses: &[SocketAddr],
    grant: &EffectiveNetworkGrant,
) -> Result<(), NetworkTransportError> {
    if addresses.is_empty() {
        return Err(NetworkTransportError::DnsFailure);
    }
    for address in addresses {
        grant
            .authorize_connect(origin, *address)
            .map_err(NetworkTransportError::from)?;
    }
    Ok(())
}

fn validate_and_cache_dns_answers(
    host: &str,
    port: u16,
    origin: &str,
    addresses: Vec<SocketAddr>,
    grant: &EffectiveNetworkGrant,
) -> Result<(), NetworkTransportError> {
    validate_dns_answers(origin, &addresses, grant)?;
    cache_dns(host, port, addresses);
    Ok(())
}

async fn resolve_candidates_with<F, Fut>(
    host: String,
    port: u16,
    origin: String,
    grant: Arc<EffectiveNetworkGrant>,
    resolver: F,
) -> Result<Vec<SocketAddr>, NetworkTransportError>
where
    F: FnOnce(String, u16) -> Fut,
    Fut: Future<Output = Result<Vec<SocketAddr>, NetworkTransportError>>,
{
    let candidates = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else if let Some(cached) = cached_dns(&host, port) {
        cached
    } else {
        let resolved = resolver(host.clone(), port).await?;
        if resolved.len() > MAX_DNS_RESULTS {
            return Err(NetworkTransportError::DnsResultLimit);
        }
        validate_and_cache_dns_answers(&host, port, &origin, resolved.clone(), &grant)?;
        resolved
    };

    // Cached and literal candidates are revalidated on every use. Fresh
    // answers are validated before insertion and before they reach a caller.
    validate_dns_answers(&origin, &candidates, &grant)?;
    Ok(candidates)
}

async fn resolve_system_dns(
    host: String,
    port: u16,
) -> Result<Vec<SocketAddr>, NetworkTransportError> {
    tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| NetworkTransportError::DnsFailure)
        .map(|resolved| resolved.take(MAX_DNS_RESULTS + 1).collect())
}

/// Resolves and authorizes every candidate before an HTTP/3 endpoint sends
/// its first UDP packet. The caller must still use each returned address
/// unchanged and keep the normalized origin for TLS identity validation.
#[cfg(feature = "http3")]
pub(crate) async fn resolve_http3_candidates(
    origin: NetworkOrigin,
    grant: Arc<EffectiveNetworkGrant>,
    timeout: Duration,
) -> Result<Vec<SocketAddr>, NetworkTransportError> {
    if origin.scheme() != "https" {
        return Err(NetworkTransportError::UnsupportedTransport);
    }
    grant
        .authorize_origin(origin.as_str())
        .map_err(NetworkTransportError::from)?;
    let host = origin.host().to_owned();
    let port = origin.port();
    let origin_text = origin.as_str().to_owned();
    tokio::time::timeout(
        timeout,
        resolve_candidates_with(host, port, origin_text, grant, resolve_system_dns),
    )
    .await
    .map_err(|_| NetworkTransportError::Timeout)?
}

/// Resolves a hostname only under an exact, already-approved network origin.
/// DNS results are bounded and every address is validated before it is
/// exposed; the cache is an optimization and never carries authority.
pub fn resolve_origin(
    endpoint: &str,
    grant: Arc<EffectiveNetworkGrant>,
    timeout: Duration,
) -> Result<Vec<String>, NetworkTransportError> {
    let origin = NetworkOrigin::parse(endpoint).map_err(NetworkTransportError::from)?;
    if !matches!(origin.scheme(), "http" | "https" | "tcp") {
        return Err(NetworkTransportError::UnsupportedTransport);
    }
    grant
        .authorize_origin(origin.as_str())
        .map_err(NetworkTransportError::from)?;
    let host = origin.host().to_owned();
    let port = origin.port();
    let origin_text = origin.as_str().to_owned();
    map_wait(crate::network_future::block_on_current(async move {
        tokio::time::timeout(timeout, async move {
            let addresses =
                resolve_candidates_with(host, port, origin_text, grant, resolve_system_dns).await?;
            Ok(addresses
                .into_iter()
                .map(|address| address.ip().to_string())
                .collect())
        })
        .await
        .map_err(|_| NetworkTransportError::Timeout)?
    }))?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTransportError {
    InvalidAddress,
    PermissionDenied,
    AddressNotAllowed,
    DnsFailure,
    DnsResultLimit,
    UnsupportedTransport,
    CertificateStore,
    InvalidServerName,
    InvalidServerIdentity,
    InvalidTlsConfiguration,
    TlsHandshake,
    BufferLimitExceeded,
    Io,
    Cancelled,
    CapacityExceeded,
    Timeout,
}

impl From<NetworkPolicyError> for NetworkTransportError {
    fn from(error: NetworkPolicyError) -> Self {
        match error {
            NetworkPolicyError::InvalidOrigin => Self::InvalidAddress,
            NetworkPolicyError::InvalidEndpoint => Self::InvalidAddress,
            NetworkPolicyError::PermissionDenied => Self::PermissionDenied,
            NetworkPolicyError::AddressNotAllowed => Self::AddressNotAllowed,
        }
    }
}

fn map_wait<T>(
    result: Result<T, crate::network_future::NetworkFutureError>,
) -> Result<T, NetworkTransportError> {
    result.map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => NetworkTransportError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => {
            NetworkTransportError::CapacityExceeded
        }
    })
}

/// An owned TCP stream. All async operations are polled on the owning Zirk
/// task stack and use bounded caller-visible buffers.
pub struct TcpStream {
    inner: TokioTcpStream,
    peer: SocketAddr,
}

impl TcpStream {
    pub fn connect(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_with_timeout(url, grant, DEFAULT_NETWORK_TIMEOUT)
    }

    pub fn connect_with_timeout(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        let origin = if let Ok(url) = Url::parse(url) {
            NetworkOrigin::parse(&url.origin())
        } else {
            NetworkOrigin::parse(url)
        }
        .map_err(NetworkTransportError::from)?;
        let (inner, peer) = connect_raw(origin, grant, false, timeout)?;
        Ok(Self { inner, peer })
    }

    pub fn peer_address(&self) -> SocketAddr {
        self.peer
    }

    /// Transfers the already-authorized socket into a protocol adapter.
    /// The returned socket remains owned by the caller and must be polled on
    /// the current Zirk task through `network_future::block_on_current`.
    pub(crate) fn into_inner(self) -> TokioTcpStream {
        self.inner
    }

    /// Returns `None` for orderly peer EOF. A returned vector is never larger
    /// than `MAX_TCP_READ_BYTES`.
    pub fn read(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, NetworkTransportError> {
        if max_bytes > MAX_TCP_READ_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if max_bytes == 0 {
            return Ok(Some(Vec::new()));
        }
        map_wait(crate::network_future::block_on_current(async {
            let mut buffer = vec![0; max_bytes];
            let count = self
                .inner
                .read(&mut buffer)
                .await
                .map_err(|_| NetworkTransportError::Io)?;
            if count == 0 {
                Ok(None)
            } else {
                buffer.truncate(count);
                Ok(Some(buffer))
            }
        }))?
    }

    /// Performs one bounded write and reports its partial byte count.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .write(bytes)
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .write_all(bytes)
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }

    pub fn shutdown(&mut self) -> Result<(), NetworkTransportError> {
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .shutdown()
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }
}

/// An owned UDP socket. Datagram operations are task-aware, destination
/// checked, bounded to the protocol maximum for the destination family, and
/// never silently report a truncated receive as a complete packet.
pub struct UdpDatagramSocket {
    inner: TokioUdpSocket,
    connected_peer: Option<SocketAddr>,
    multicast_memberships: HashSet<IpAddr>,
    grant: Arc<EffectiveNetworkGrant>,
}

impl UdpDatagramSocket {
    /// Creates an outbound socket on an ephemeral local port and connects it
    /// to one exact approved peer. The ephemeral local bind grants no inbound
    /// authority.
    pub fn connect(
        peer: SocketAddr,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        grant
            .authorize_udp_send(peer)
            .map_err(NetworkTransportError::from)?;
        let bind_address = if peer.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };
        let inner = map_wait(crate::network_future::block_on_current(async move {
            tokio::time::timeout(timeout, async move {
                let socket = TokioUdpSocket::bind(bind_address)
                    .await
                    .map_err(|_| NetworkTransportError::Io)?;
                socket
                    .connect(peer)
                    .await
                    .map_err(|_| NetworkTransportError::Io)?;
                Ok::<_, NetworkTransportError>(socket)
            })
            .await
            .map_err(|_| NetworkTransportError::Timeout)?
        }))??;
        Ok(Self {
            inner,
            connected_peer: Some(peer),
            multicast_memberships: HashSet::new(),
            grant,
        })
    }

    /// Binds a socket that can receive datagrams. Binding authority is
    /// separate from UDP destination authority and from TCP/HTTP listen.
    pub fn bind(
        address: SocketAddr,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        grant
            .authorize_udp_bind(address)
            .map_err(NetworkTransportError::from)?;
        let inner = map_wait(crate::network_future::block_on_current(async move {
            tokio::time::timeout(timeout, async move {
                TokioUdpSocket::bind(address)
                    .await
                    .map_err(|_| NetworkTransportError::Io)
            })
            .await
            .map_err(|_| NetworkTransportError::Timeout)?
        }))??;
        Ok(Self {
            inner,
            connected_peer: None,
            multicast_memberships: HashSet::new(),
            grant,
        })
    }

    pub fn local_address(&self) -> Result<SocketAddr, NetworkTransportError> {
        self.inner
            .local_addr()
            .map_err(|_| NetworkTransportError::Io)
    }

    /// Connects a previously bound socket to one authorized peer. Tokio's
    /// UDP connect filters incoming packets to this peer at the OS boundary.
    pub fn connect_peer(
        &mut self,
        peer: SocketAddr,
        timeout: Duration,
    ) -> Result<(), NetworkTransportError> {
        self.grant
            .authorize_udp_send(peer)
            .map_err(NetworkTransportError::from)?;
        map_wait(crate::network_future::block_on_current(async {
            tokio::time::timeout(timeout, self.inner.connect(peer))
                .await
                .map_err(|_| NetworkTransportError::Timeout)?
                .map_err(|_| NetworkTransportError::Io)
        }))??;
        self.connected_peer = Some(peer);
        Ok(())
    }

    pub fn send(&self, bytes: &[u8], timeout: Duration) -> Result<usize, NetworkTransportError> {
        let peer = self
            .connected_peer
            .ok_or(NetworkTransportError::InvalidAddress)?;
        let max_bytes = if peer.is_ipv4() {
            MAX_UDP_IPV4_DATAGRAM_BYTES
        } else {
            MAX_UDP_IPV6_DATAGRAM_BYTES
        };
        if bytes.len() > max_bytes {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        self.grant
            .authorize_udp_send(peer)
            .map_err(NetworkTransportError::from)?;
        let written = map_wait(crate::network_future::block_on_current(async {
            tokio::time::timeout(timeout, self.inner.send(bytes))
                .await
                .map_err(|_| NetworkTransportError::Timeout)?
                .map_err(|_| NetworkTransportError::Io)
        }))??;
        if written != bytes.len() {
            return Err(NetworkTransportError::Io);
        }
        Ok(written)
    }

    pub fn send_to(
        &self,
        bytes: &[u8],
        peer: SocketAddr,
        timeout: Duration,
    ) -> Result<usize, NetworkTransportError> {
        self.send_to_checked(bytes, peer, timeout)
    }

    /// Enables limited broadcast only after both the exact UDP destination and
    /// the separate broadcast grant have been verified.
    pub fn send_broadcast(
        &self,
        bytes: &[u8],
        destination: SocketAddr,
        timeout: Duration,
    ) -> Result<usize, NetworkTransportError> {
        if !matches!(destination.ip(), IpAddr::V4(address) if address.is_broadcast()) {
            return Err(NetworkTransportError::InvalidAddress);
        }
        self.grant
            .authorize_udp_broadcast(destination)
            .map_err(NetworkTransportError::from)?;
        self.grant
            .authorize_udp_send(destination)
            .map_err(NetworkTransportError::from)?;
        self.inner
            .set_broadcast(true)
            .map_err(|_| NetworkTransportError::Io)?;
        self.send_to_checked(bytes, destination, timeout)
    }

    /// Joins one explicitly approved multicast group. The socket is bound and
    /// task-owned; dropping it releases every membership at the OS boundary.
    pub fn join_multicast(&mut self, group: IpAddr) -> Result<(), NetworkTransportError> {
        if self.connected_peer.is_some() || !group.is_multicast() {
            return Err(NetworkTransportError::InvalidAddress);
        }
        self.grant
            .authorize_udp_multicast(group)
            .map_err(NetworkTransportError::from)?;
        if self.multicast_memberships.contains(&group) {
            return Err(NetworkTransportError::InvalidAddress);
        }
        match group {
            IpAddr::V4(group) => self
                .inner
                .join_multicast_v4(group, std::net::Ipv4Addr::UNSPECIFIED)
                .map_err(|_| NetworkTransportError::Io)?,
            IpAddr::V6(group) => self
                .inner
                .join_multicast_v6(&group, 0)
                .map_err(|_| NetworkTransportError::Io)?,
        }
        self.multicast_memberships.insert(group);
        Ok(())
    }

    /// Leaves a group previously joined by this socket.
    pub fn leave_multicast(&mut self, group: IpAddr) -> Result<(), NetworkTransportError> {
        self.grant
            .authorize_udp_multicast(group)
            .map_err(NetworkTransportError::from)?;
        if !self.multicast_memberships.contains(&group) {
            return Err(NetworkTransportError::InvalidAddress);
        }
        match group {
            IpAddr::V4(group) => self
                .inner
                .leave_multicast_v4(group, std::net::Ipv4Addr::UNSPECIFIED)
                .map_err(|_| NetworkTransportError::Io)?,
            IpAddr::V6(group) => self
                .inner
                .leave_multicast_v6(&group, 0)
                .map_err(|_| NetworkTransportError::Io)?,
        }
        self.multicast_memberships.remove(&group);
        Ok(())
    }

    /// Sends to an explicitly approved multicast address and port.
    pub fn send_multicast(
        &self,
        bytes: &[u8],
        destination: SocketAddr,
        timeout: Duration,
    ) -> Result<usize, NetworkTransportError> {
        if !destination.ip().is_multicast() {
            return Err(NetworkTransportError::InvalidAddress);
        }
        self.grant
            .authorize_udp_multicast(destination.ip())
            .map_err(NetworkTransportError::from)?;
        self.send_to_checked(bytes, destination, timeout)
    }

    fn send_to_checked(
        &self,
        bytes: &[u8],
        peer: SocketAddr,
        timeout: Duration,
    ) -> Result<usize, NetworkTransportError> {
        let max_bytes = if peer.is_ipv4() {
            MAX_UDP_IPV4_DATAGRAM_BYTES
        } else {
            MAX_UDP_IPV6_DATAGRAM_BYTES
        };
        if bytes.len() > max_bytes {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        self.grant
            .authorize_udp_send(peer)
            .map_err(NetworkTransportError::from)?;
        let written = map_wait(crate::network_future::block_on_current(async {
            tokio::time::timeout(timeout, self.inner.send_to(bytes, peer))
                .await
                .map_err(|_| NetworkTransportError::Timeout)?
                .map_err(|_| NetworkTransportError::Io)
        }))??;
        if written != bytes.len() {
            return Err(NetworkTransportError::Io);
        }
        Ok(written)
    }

    pub fn receive(
        &self,
        max_bytes: usize,
        timeout: Duration,
    ) -> Result<(Vec<u8>, SocketAddr), NetworkTransportError> {
        if max_bytes == 0 || max_bytes > MAX_UDP_IPV6_DATAGRAM_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        let mut buffer = vec![0; max_bytes + 1];
        let (length, peer) = map_wait(crate::network_future::block_on_current(async {
            tokio::time::timeout(timeout, self.inner.recv_from(&mut buffer))
                .await
                .map_err(|_| NetworkTransportError::Timeout)?
                .map_err(|_| NetworkTransportError::Io)
        }))??;
        if length > max_bytes {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        buffer.truncate(length);
        Ok((buffer, peer))
    }
}

fn connect_raw(
    origin: NetworkOrigin,
    grant: Arc<EffectiveNetworkGrant>,
    allow_tls: bool,
    timeout: Duration,
) -> Result<(TokioTcpStream, SocketAddr), NetworkTransportError> {
    connect_raw_with_resolver(origin, grant, allow_tls, timeout, resolve_system_dns)
}

fn connect_raw_with_resolver<F, Fut>(
    origin: NetworkOrigin,
    grant: Arc<EffectiveNetworkGrant>,
    allow_tls: bool,
    timeout: Duration,
    resolver: F,
) -> Result<(TokioTcpStream, SocketAddr), NetworkTransportError>
where
    F: FnOnce(String, u16) -> Fut + 'static,
    Fut: Future<Output = Result<Vec<SocketAddr>, NetworkTransportError>> + 'static,
{
    let allowed_scheme = if allow_tls {
        origin.scheme() == "https"
    } else {
        matches!(origin.scheme(), "http" | "tcp")
    };
    if !allowed_scheme {
        return Err(NetworkTransportError::UnsupportedTransport);
    }
    grant
        .authorize_origin(origin.as_str())
        .map_err(NetworkTransportError::from)?;
    let host = origin.host().to_owned();
    let port = origin.port();
    let origin = origin.as_str().to_owned();
    map_wait(crate::network_future::block_on_current(async move {
        tokio::time::timeout(timeout, async move {
            // Resolve and validate the complete answer set before creating a
            // socket, so a fresh private answer cannot win by DNS order.
            let candidates =
                resolve_candidates_with(host, port, origin.clone(), grant.clone(), resolver)
                    .await?;
            let mut last_error = NetworkTransportError::Io;
            for candidate in candidates {
                grant
                    .authorize_connect(&origin, candidate)
                    .map_err(NetworkTransportError::from)?;
                match TokioTcpStream::connect(candidate).await {
                    Ok(inner) => {
                        let peer = inner.peer_addr().map_err(|_| NetworkTransportError::Io)?;
                        return Ok((inner, peer));
                    }
                    Err(_error) => {
                        last_error = NetworkTransportError::Io;
                    }
                }
            }
            Err(last_error)
        })
        .await
        .map_err(|_| NetworkTransportError::Timeout)?
    }))?
}

pub(crate) type RustlsClientConfig = tokio_rustls::rustls::ClientConfig;
static TLS13_CLIENT_CONFIG: std::sync::OnceLock<
    Result<Arc<RustlsClientConfig>, NetworkTransportError>,
> = std::sync::OnceLock::new();
static TLS12_CLIENT_CONFIG: std::sync::OnceLock<
    Result<Arc<RustlsClientConfig>, NetworkTransportError>,
> = std::sync::OnceLock::new();

fn client_tls_config(allow_tls12: bool) -> Result<Arc<RustlsClientConfig>, NetworkTransportError> {
    let cell = if allow_tls12 {
        &TLS12_CLIENT_CONFIG
    } else {
        &TLS13_CLIENT_CONFIG
    };
    cell.get_or_init(|| {
        let loaded = rustls_native_certs::load_native_certs();
        build_client_tls_config(allow_tls12, loaded.certs, true)
    })
    .clone()
}

pub(crate) fn client_tls_config_for_http() -> Result<Arc<RustlsClientConfig>, NetworkTransportError>
{
    // HTTP interoperability includes TLS 1.2 peers. Rustls still prefers
    // TLS 1.3, while retaining TLS 1.2 as the standards-compliant fallback.
    client_tls_config(true)
}

/// TLS configuration for native HTTP/3: TLS 1.3 only, authenticated system
/// roots, and the single HTTP/3 ALPN. Quinn's early-data path is not enabled.
#[cfg(feature = "http3")]
pub(crate) fn client_tls_config_for_http3() -> Result<Arc<RustlsClientConfig>, NetworkTransportError>
{
    http3_tls_config(client_tls_config(false)?)
}

#[cfg(feature = "http3")]
fn http3_tls_config(
    config: Arc<RustlsClientConfig>,
) -> Result<Arc<RustlsClientConfig>, NetworkTransportError> {
    let mut config = (*config).clone();
    config.alpn_protocols = vec![b"h3".to_vec()];
    config.enable_early_data = false;
    Ok(Arc::new(config))
}

fn build_client_tls_config(
    allow_tls12: bool,
    certificates: Vec<tokio_rustls::rustls::pki_types::CertificateDer<'static>>,
    require_roots: bool,
) -> Result<Arc<RustlsClientConfig>, NetworkTransportError> {
    use tokio_rustls::rustls::{RootCertStore, client::ClientConfig, version};
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        let _ = roots.add(certificate);
    }
    if require_roots && roots.is_empty() {
        return Err(NetworkTransportError::CertificateStore);
    }
    let versions = if allow_tls12 {
        &[&version::TLS13, &version::TLS12][..]
    } else {
        &[&version::TLS13][..]
    };
    let config = ClientConfig::builder_with_protocol_versions(versions)
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

pub(crate) fn client_tls_config_with_trust_roots(
    roots_der: Vec<Vec<u8>>,
    allow_tls12: bool,
) -> Result<Arc<RustlsClientConfig>, NetworkTransportError> {
    use tokio_rustls::rustls::pki_types::CertificateDer;
    use tokio_rustls::rustls::{RootCertStore, client::ClientConfig, version};

    if roots_der.is_empty() {
        return Err(NetworkTransportError::CertificateStore);
    }
    let mut roots = RootCertStore::empty();
    for certificate in roots_der {
        roots
            .add(CertificateDer::from(certificate))
            .map_err(|_| NetworkTransportError::CertificateStore)?;
    }
    let versions = if allow_tls12 {
        &[&version::TLS13, &version::TLS12][..]
    } else {
        &[&version::TLS13][..]
    };
    Ok(Arc::new(
        ClientConfig::builder_with_protocol_versions(versions)
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

/// Builds an authenticated client configuration that presents a caller-owned
/// certificate chain. Private-key DER is erased after rustls constructs its
/// signer; the signer remains owned by the TLS configuration for the stream's
/// lifetime.
pub(crate) fn client_tls_config_with_identity(
    roots_der: Vec<Vec<u8>>,
    certificate_chain_der: Vec<Vec<u8>>,
    private_key_der: Vec<u8>,
    allow_tls12: bool,
) -> Result<Arc<RustlsClientConfig>, NetworkTransportError> {
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::{RootCertStore, client::ClientConfig, version};
    use zeroize::Zeroizing;

    const MAX_ROOT_CERTIFICATES: usize = 64;
    const MAX_IDENTITY_CERTIFICATES: usize = 64;
    const MAX_TLS_CERTIFICATE_BYTES: usize = 1024 * 1024;
    if certificate_chain_der.is_empty()
        || certificate_chain_der.len() > MAX_IDENTITY_CERTIFICATES
        || roots_der.len() > MAX_ROOT_CERTIFICATES
        || certificate_chain_der
            .iter()
            .chain(&roots_der)
            .try_fold(0usize, |total, certificate| {
                total.checked_add(certificate.len())
            })
            .is_none_or(|total| total > MAX_TLS_CERTIFICATE_BYTES)
    {
        return Err(NetworkTransportError::InvalidTlsConfiguration);
    }

    let mut roots = RootCertStore::empty();
    if roots_der.is_empty() {
        let loaded = rustls_native_certs::load_native_certs();
        for certificate in loaded.certs {
            let _ = roots.add(certificate);
        }
    } else {
        for certificate in roots_der {
            roots
                .add(CertificateDer::from(certificate))
                .map_err(|_| NetworkTransportError::CertificateStore)?;
        }
    }
    if roots.is_empty() {
        return Err(NetworkTransportError::CertificateStore);
    }

    let certificate_chain = certificate_chain_der
        .into_iter()
        .map(CertificateDer::from)
        .collect::<Vec<_>>();
    let private_key_der = Zeroizing::new(private_key_der);
    if private_key_der.is_empty() || private_key_der.len() > MAX_TLS_CERTIFICATE_BYTES {
        return Err(NetworkTransportError::InvalidTlsConfiguration);
    }
    let private_key = PrivateKeyDer::try_from(private_key_der.as_slice())
        .map_err(|_| NetworkTransportError::InvalidServerIdentity)?
        .clone_key();
    let versions = if allow_tls12 {
        &[&version::TLS13, &version::TLS12][..]
    } else {
        &[&version::TLS13][..]
    };
    let config = ClientConfig::builder_with_protocol_versions(versions)
        .with_root_certificates(roots)
        .with_client_auth_cert(certificate_chain, private_key)
        .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
    Ok(Arc::new(config))
}

#[cfg(test)]
pub(crate) fn loopback_test_client_tls_config() -> Arc<RustlsClientConfig> {
    use tokio_rustls::rustls::pki_types::CertificateDer;

    build_client_tls_config(
        false,
        vec![CertificateDer::from(
            include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
        )],
        true,
    )
    .expect("the loopback test root is valid")
}

/// Authenticated TLS client stream. TLS 1.3 is the default; TLS 1.2 is enabled
/// only by explicit caller configuration. Hostname verification and SNI are
/// provided by rustls and cannot be disabled through this API.
pub struct TlsClientStream {
    inner: tokio_rustls::client::TlsStream<TokioTcpStream>,
    peer: SocketAddr,
}

pub(crate) struct PreparedTlsClient {
    server_name: tokio_rustls::rustls::pki_types::ServerName<'static>,
    config: Arc<RustlsClientConfig>,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NegotiatedHttpProtocol {
    Http1,
    Http2,
}

impl TlsClientStream {
    /// Validates TLS policy for an already-connected TCP peer without
    /// consuming the caller's TCP resource.
    pub(crate) fn prepare_tcp_upgrade(
        endpoint: &str,
        peer: SocketAddr,
        grant: &EffectiveNetworkGrant,
        roots_der: Vec<Vec<u8>>,
        allow_tls12: bool,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<PreparedTlsClient, NetworkTransportError> {
        validate_tls_alpn_protocols(&alpn_protocols)?;
        let mut config = if roots_der.is_empty() {
            (*client_tls_config(allow_tls12)?).clone()
        } else {
            (*client_tls_config_with_trust_roots(roots_der, allow_tls12)?).clone()
        };
        config.alpn_protocols = alpn_protocols;
        let url = Url::parse(endpoint).map_err(|_| NetworkTransportError::InvalidAddress)?;
        // `Url::parse` already rejects embedded credentials at the shared
        // runtime-core boundary; this operation additionally requires HTTPS.
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        let origin = NetworkOrigin::parse(&url.origin()).map_err(NetworkTransportError::from)?;
        grant
            .authorize_connect(origin.as_str(), peer)
            .map_err(NetworkTransportError::from)?;
        Ok(PreparedTlsClient {
            server_name,
            config: Arc::new(config),
            timeout: DEFAULT_NETWORK_TIMEOUT,
        })
    }

    /// Transfers an existing TCP stream into the TLS state machine. Once the
    /// handshake starts, failures close the consumed TCP resource because its
    /// byte stream can no longer safely return to plaintext use.
    pub(crate) fn wrap_tcp(
        stream: TcpStream,
        prepared: PreparedTlsClient,
    ) -> Result<Self, NetworkTransportError> {
        let peer = stream.peer_address();
        let stream = stream.into_inner();
        let connector = tokio_rustls::TlsConnector::from(prepared.config);
        let inner = map_wait(crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                tokio::time::timeout(
                    prepared.timeout,
                    connector.connect(prepared.server_name, stream),
                )
                .await
                .map_err(|_| NetworkTransportError::Timeout)?
                .map_err(|_| NetworkTransportError::TlsHandshake)
            })
            .await
            .map_err(|_| NetworkTransportError::TlsHandshake)?
        }))??;
        Ok(Self { inner, peer })
    }

    /// Completes an HTTP/1.1 TLS handshake over an already-authorized TCP
    /// tunnel. The proxy socket is permission-checked by the caller; this
    /// handshake still validates the destination host and certificate.
    pub(crate) fn connect_for_http1_over_tunnel(
        url: &str,
        stream: TokioTcpStream,
        peer: SocketAddr,
        timeout: Duration,
        config: Arc<RustlsClientConfig>,
    ) -> Result<Self, NetworkTransportError> {
        let inner = Self::connect_tls_over_tunnel(url, stream, timeout, config)?;
        Ok(Self { inner, peer })
    }

    /// Authenticates an HTTP/1.1 TLS peer over an existing authorized stream.
    /// The generic transport also permits TLS-to-proxy followed by TLS-to-origin.
    pub(crate) fn connect_tls_over_tunnel<S>(
        url: &str,
        stream: S,
        timeout: Duration,
        config: Arc<RustlsClientConfig>,
    ) -> Result<tokio_rustls::client::TlsStream<S>, NetworkTransportError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        let mut client_config = (*config).clone();
        client_config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
        let inner = map_wait(crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                tokio::time::timeout(timeout, connector.connect(server_name, stream))
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(|_| NetworkTransportError::TlsHandshake)
            })
            .await
            .map_err(|_| NetworkTransportError::TlsHandshake)?
        }))??;
        Ok(inner)
    }

    /// Connects with HTTP ALPN preferences and returns the protocol selected
    /// by the authenticated peer. Servers without ALPN fall back to HTTP/1.1.
    pub(crate) fn connect_for_http(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
    ) -> Result<(Self, NegotiatedHttpProtocol), NetworkTransportError> {
        Self::connect_for_http_with_config(url, grant, timeout, client_tls_config_for_http()?)
    }

    pub(crate) fn connect_for_http_with_config(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
        config: Arc<RustlsClientConfig>,
    ) -> Result<(Self, NegotiatedHttpProtocol), NetworkTransportError> {
        Self::connect_for_http_with_protocols(
            url,
            grant,
            timeout,
            config,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()],
        )
    }

    pub(crate) fn connect_for_http1(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_for_http_with_protocols(
            url,
            grant,
            timeout,
            client_tls_config(false)?,
            vec![b"http/1.1".to_vec()],
        )
        .map(|(stream, _)| stream)
    }

    pub(crate) fn connect_for_http1_with_config(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
        config: Arc<RustlsClientConfig>,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_for_http_with_protocols(
            url,
            grant,
            timeout,
            config,
            vec![b"http/1.1".to_vec()],
        )
        .map(|(stream, _)| stream)
    }

    pub(crate) fn connect_for_http_with_protocols(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        timeout: Duration,
        config: Arc<RustlsClientConfig>,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<(Self, NegotiatedHttpProtocol), NetworkTransportError> {
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        let mut config = (*config).clone();
        config.alpn_protocols = alpn_protocols;
        let origin = NetworkOrigin::parse(&url.origin()).map_err(NetworkTransportError::from)?;
        let (stream, peer) = connect_raw(origin, grant, true, timeout)?;
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let inner = map_wait(crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                tokio::time::timeout(timeout, connector.connect(server_name, stream))
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(|_| NetworkTransportError::TlsHandshake)
            })
            .await
            .map_err(|_| NetworkTransportError::TlsHandshake)?
        }))??;
        let protocol = match inner.get_ref().1.alpn_protocol() {
            Some(b"h2") => NegotiatedHttpProtocol::Http2,
            _ => NegotiatedHttpProtocol::Http1,
        };
        Ok((Self { inner, peer }, protocol))
    }

    pub fn connect(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        allow_tls12: bool,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_with_timeout(url, grant, allow_tls12, DEFAULT_NETWORK_TIMEOUT)
    }

    pub fn connect_with_timeout(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        allow_tls12: bool,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        Self::connect_with_config_and_timeout(
            url,
            grant,
            server_name,
            client_tls_config(allow_tls12)?,
            timeout,
        )
    }

    /// Connects using an explicit set of trusted DER root certificates.
    /// Empty or malformed trust stores fail closed. TLS 1.2 can be enabled
    /// only through the explicit `allow_tls12` option.
    pub fn connect_with_trust_roots(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        roots_der: Vec<Vec<u8>>,
        allow_tls12: bool,
    ) -> Result<Self, NetworkTransportError> {
        let config = client_tls_config_with_trust_roots(roots_der, allow_tls12)?;
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        Self::connect_with_config_and_timeout(
            url,
            grant,
            server_name,
            config,
            DEFAULT_NETWORK_TIMEOUT,
        )
    }

    /// Connects with system trust roots and an explicit ALPN preference list.
    /// Protocol identifiers are limited to printable ASCII tokens and a
    /// bounded aggregate size before any socket is opened.
    pub fn connect_with_alpn(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_with_options(url, grant, Vec::new(), false, alpn_protocols)
    }

    /// Connects with explicit trust roots (or the platform store when empty),
    /// a TLS version policy, and an application protocol preference list.
    pub fn connect_with_options(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        roots_der: Vec<Vec<u8>>,
        allow_tls12: bool,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<Self, NetworkTransportError> {
        validate_tls_alpn_protocols(&alpn_protocols)?;
        let mut config = if roots_der.is_empty() {
            (*client_tls_config(allow_tls12)?).clone()
        } else {
            (*client_tls_config_with_trust_roots(roots_der, allow_tls12)?).clone()
        };
        config.alpn_protocols = alpn_protocols;
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        Self::connect_with_config_and_timeout(
            url,
            grant,
            server_name,
            Arc::new(config),
            DEFAULT_NETWORK_TIMEOUT,
        )
    }

    /// Connects with a client identity, explicit or platform trust roots, a
    /// TLS version policy, and a bounded ALPN preference list.
    pub fn connect_with_client_identity(
        url: &str,
        grant: Arc<EffectiveNetworkGrant>,
        roots_der: Vec<Vec<u8>>,
        certificate_chain_der: Vec<Vec<u8>>,
        private_key_der: Vec<u8>,
        allow_tls12: bool,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<Self, NetworkTransportError> {
        validate_tls_alpn_protocols(&alpn_protocols)?;
        let mut config = (*client_tls_config_with_identity(
            roots_der,
            certificate_chain_der,
            private_key_der,
            allow_tls12,
        )?)
        .clone();
        config.alpn_protocols = alpn_protocols;
        let url = Url::parse(url).map_err(|_| NetworkTransportError::InvalidAddress)?;
        if url.scheme() != "https" {
            return Err(NetworkTransportError::UnsupportedTransport);
        }
        let host = url
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let server_name = if let Ok(ip) = host.parse::<IpAddr>() {
            tokio_rustls::rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            tokio_rustls::rustls::pki_types::ServerName::try_from(host)
                .map_err(|_| NetworkTransportError::InvalidServerName)?
        };
        Self::connect_with_config_and_timeout(
            url,
            grant,
            server_name,
            Arc::new(config),
            DEFAULT_NETWORK_TIMEOUT,
        )
    }

    #[cfg(test)]
    fn connect_with_config(
        url: Url,
        grant: Arc<EffectiveNetworkGrant>,
        server_name: tokio_rustls::rustls::pki_types::ServerName<'static>,
        config: Arc<RustlsClientConfig>,
    ) -> Result<Self, NetworkTransportError> {
        Self::connect_with_config_and_timeout(
            url,
            grant,
            server_name,
            config,
            DEFAULT_NETWORK_TIMEOUT,
        )
    }

    fn connect_with_config_and_timeout(
        url: Url,
        grant: Arc<EffectiveNetworkGrant>,
        server_name: tokio_rustls::rustls::pki_types::ServerName<'static>,
        config: Arc<RustlsClientConfig>,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        let origin = NetworkOrigin::parse(&url.origin()).map_err(NetworkTransportError::from)?;
        let (stream, peer) = connect_raw(origin, grant, true, timeout)?;
        let connector = tokio_rustls::TlsConnector::from(config);
        let inner = map_wait(crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                tokio::time::timeout(timeout, connector.connect(server_name, stream))
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(|_| NetworkTransportError::TlsHandshake)
            })
            .await
            .map_err(|_| NetworkTransportError::TlsHandshake)?
        }))??;
        Ok(Self { inner, peer })
    }

    pub fn peer_address(&self) -> SocketAddr {
        self.peer
    }

    pub fn negotiated_alpn(&self) -> Option<String> {
        self.inner
            .get_ref()
            .1
            .alpn_protocol()
            .and_then(|protocol| std::str::from_utf8(protocol).ok())
            .map(str::to_owned)
    }

    pub fn peer_certificate_sha256(&self) -> Option<String> {
        self.inner
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|chain| chain.first())
            .map(|certificate| certificate_sha256(certificate.as_ref()))
    }

    /// Transfers the authenticated TLS stream into a protocol adapter.
    pub(crate) fn into_inner(self) -> tokio_rustls::client::TlsStream<TokioTcpStream> {
        self.inner
    }

    pub fn read(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, NetworkTransportError> {
        if max_bytes > MAX_TCP_READ_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if max_bytes == 0 {
            return Ok(Some(Vec::new()));
        }
        map_wait(crate::network_future::block_on_current(async {
            let mut buffer = vec![0; max_bytes];
            let count = self
                .inner
                .read(&mut buffer)
                .await
                .map_err(|_| NetworkTransportError::Io)?;
            if count == 0 {
                Ok(None)
            } else {
                buffer.truncate(count);
                Ok(Some(buffer))
            }
        }))?
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .write_all(bytes)
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }

    pub fn shutdown(&mut self) -> Result<(), NetworkTransportError> {
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .shutdown()
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }
}

pub(crate) fn validate_tls_alpn_protocols(
    protocols: &[Vec<u8>],
) -> Result<(), NetworkTransportError> {
    const MAX_PROTOCOLS: usize = 16;
    const MAX_PROTOCOL_BYTES: usize = 1024;
    if protocols.is_empty() || protocols.len() > MAX_PROTOCOLS {
        return Err(NetworkTransportError::InvalidTlsConfiguration);
    }
    let mut total_bytes = 0usize;
    for (index, protocol) in protocols.iter().enumerate() {
        if protocol.is_empty()
            || protocol.len() > u8::MAX as usize
            || !protocol.iter().all(|byte| (0x21..=0x7e).contains(byte))
            || protocols[..index].contains(protocol)
        {
            return Err(NetworkTransportError::InvalidTlsConfiguration);
        }
        total_bytes = total_bytes
            .checked_add(protocol.len())
            .ok_or(NetworkTransportError::InvalidTlsConfiguration)?;
        if total_bytes > MAX_PROTOCOL_BYTES {
            return Err(NetworkTransportError::InvalidTlsConfiguration);
        }
    }
    Ok(())
}

/// Validated server identity and protocol policy. Identity material is
/// supplied explicitly as DER; the library never searches for implicit keys.
#[derive(Clone)]
pub struct TlsServerConfig(Arc<tokio_rustls::rustls::ServerConfig>);

impl TlsServerConfig {
    pub fn from_der(
        certificate_chain: Vec<Vec<u8>>,
        private_key: Vec<u8>,
        allow_tls12: bool,
    ) -> Result<Self, NetworkTransportError> {
        use tokio_rustls::rustls::{pki_types::PrivateKeyDer, server::ServerConfig, version};
        use zeroize::Zeroizing;

        if certificate_chain.is_empty()
            || certificate_chain.len() > 64
            || certificate_chain
                .iter()
                .try_fold(0usize, |total, certificate| {
                    total.checked_add(certificate.len())
                })
                .is_none_or(|total| total > 1024 * 1024)
            || private_key.len() > 1024 * 1024
        {
            return Err(NetworkTransportError::InvalidServerIdentity);
        }
        let certificates = certificate_chain
            .into_iter()
            .map(tokio_rustls::rustls::pki_types::CertificateDer::from)
            .collect();
        let private_key = Zeroizing::new(private_key);
        let key = PrivateKeyDer::try_from(private_key.as_slice())
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
        let key = key.clone_key();
        let versions = if allow_tls12 {
            &[&version::TLS13, &version::TLS12][..]
        } else {
            &[&version::TLS13][..]
        };
        let config = ServerConfig::builder_with_protocol_versions(versions)
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
        Ok(Self(Arc::new(config)))
    }

    /// Builds a server identity that requires a client certificate validated
    /// against the caller's explicit DER trust roots. Empty roots are rejected
    /// so mutual authentication cannot be configured accidentally without a
    /// client trust policy.
    pub fn from_der_with_client_auth(
        certificate_chain: Vec<Vec<u8>>,
        private_key: Vec<u8>,
        client_roots_der: Vec<Vec<u8>>,
        allow_tls12: bool,
    ) -> Result<Self, NetworkTransportError> {
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
        use tokio_rustls::rustls::{RootCertStore, server::WebPkiClientVerifier, version};
        use zeroize::Zeroizing;

        const MAX_CERTIFICATES: usize = 64;
        const MAX_CERTIFICATE_BYTES: usize = 1024 * 1024;
        if certificate_chain.is_empty()
            || certificate_chain.len() > MAX_CERTIFICATES
            || client_roots_der.is_empty()
            || client_roots_der.len() > MAX_CERTIFICATES
            || certificate_chain
                .iter()
                .chain(&client_roots_der)
                .try_fold(0usize, |total, certificate| {
                    total.checked_add(certificate.len())
                })
                .is_none_or(|total| total > MAX_CERTIFICATE_BYTES)
            || private_key.is_empty()
            || private_key.len() > MAX_CERTIFICATE_BYTES
        {
            return Err(NetworkTransportError::InvalidTlsConfiguration);
        }

        let mut client_roots = RootCertStore::empty();
        for root in client_roots_der {
            client_roots
                .add(CertificateDer::from(root))
                .map_err(|_| NetworkTransportError::CertificateStore)?;
        }
        if client_roots.is_empty() {
            return Err(NetworkTransportError::CertificateStore);
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(client_roots))
            .build()
            .map_err(|_| NetworkTransportError::CertificateStore)?;

        let certificates = certificate_chain
            .into_iter()
            .map(CertificateDer::from)
            .collect();
        let private_key = Zeroizing::new(private_key);
        let key = PrivateKeyDer::try_from(private_key.as_slice())
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?
            .clone_key();
        let versions = if allow_tls12 {
            &[&version::TLS13, &version::TLS12][..]
        } else {
            &[&version::TLS13][..]
        };
        let config = tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(versions)
            .with_client_cert_verifier(verifier)
            .with_single_cert(certificates, key)
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
        Ok(Self(Arc::new(config)))
    }

    /// Configures the protocols offered by an application protocol adapter.
    /// TLS itself defaults to no ALPN so generic transport users retain their
    /// existing negotiation behavior.
    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        let mut config = (*self.0).clone();
        config.alpn_protocols = protocols;
        self.0 = Arc::new(config);
        self
    }

    /// Creates a QUIC server identity with TLS 1.3, `h3` ALPN, and early data
    /// disabled. HTTP/3 must not inherit a TCP listener's TLS 1.2 allowance.
    #[cfg(feature = "http3")]
    #[allow(dead_code)] // Used by the compiler-owned HTTP/3 listener ABI as it lands.
    pub(crate) fn into_http3_quic_config(
        self,
    ) -> Result<quinn::ServerConfig, NetworkTransportError> {
        use quinn::crypto::rustls::QuicServerConfig;

        let mut tls = (*self.0).clone();
        tls.alpn_protocols = vec![b"h3".to_vec()];
        tls.max_early_data_size = 0;
        let crypto = QuicServerConfig::try_from(tls)
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        let mut transport = quinn::TransportConfig::default();
        transport
            .max_concurrent_bidi_streams(quinn::VarInt::from_u32(16))
            .max_concurrent_uni_streams(quinn::VarInt::from_u32(16))
            .stream_receive_window(
                quinn::VarInt::from_u64(
                    zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES as u64,
                )
                .map_err(|_| NetworkTransportError::InvalidServerIdentity)?,
            )
            .receive_window(
                quinn::VarInt::from_u64(
                    (zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES as u64) * 4,
                )
                .map_err(|_| NetworkTransportError::InvalidServerIdentity)?,
            );
        server.transport_config(Arc::new(transport));
        Ok(server)
    }
}

pub struct TlsListener {
    inner: TokioTcpListener,
    config: TlsServerConfig,
}

impl TlsListener {
    pub fn bind(
        address: SocketAddr,
        grant: &EffectiveNetworkGrant,
        config: TlsServerConfig,
    ) -> Result<Self, NetworkTransportError> {
        let listener = TcpListener::bind(address, grant)?;
        Ok(Self {
            inner: listener.inner,
            config,
        })
    }

    pub fn local_address(&self) -> Result<SocketAddr, NetworkTransportError> {
        self.inner
            .local_addr()
            .map_err(|_| NetworkTransportError::Io)
    }

    pub fn accept(&self) -> Result<(TlsServerStream, SocketAddr), NetworkTransportError> {
        let (stream, peer) = map_wait(crate::network_future::block_on_current(async {
            self.inner
                .accept()
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))??;
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::clone(&self.config.0));
        let inner = map_wait(crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                tokio::time::timeout(DEFAULT_NETWORK_TIMEOUT, acceptor.accept(stream))
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(|_| NetworkTransportError::TlsHandshake)
            })
            .await
            .map_err(|_| NetworkTransportError::TlsHandshake)?
        }))??;
        Ok((TlsServerStream { inner, peer }, peer))
    }
}

pub struct TlsServerStream {
    inner: tokio_rustls::server::TlsStream<TokioTcpStream>,
    peer: SocketAddr,
}

impl TlsServerStream {
    pub fn peer_address(&self) -> SocketAddr {
        self.peer
    }

    pub fn negotiated_alpn(&self) -> Option<String> {
        self.inner
            .get_ref()
            .1
            .alpn_protocol()
            .and_then(|protocol| std::str::from_utf8(protocol).ok())
            .map(str::to_owned)
    }

    pub fn peer_certificate_sha256(&self) -> Option<String> {
        self.inner
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|chain| chain.first())
            .map(|certificate| certificate_sha256(certificate.as_ref()))
    }

    /// Transfers an authenticated accepted stream to a protocol adapter.
    #[allow(dead_code)] // Used by the HTTP server adapter before public bindings land.
    pub(crate) fn into_inner(self) -> tokio_rustls::server::TlsStream<TokioTcpStream> {
        self.inner
    }

    pub fn read(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, NetworkTransportError> {
        if max_bytes > MAX_TCP_READ_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if max_bytes == 0 {
            return Ok(Some(Vec::new()));
        }
        map_wait(crate::network_future::block_on_current(async {
            let mut buffer = vec![0; max_bytes];
            let count = self
                .inner
                .read(&mut buffer)
                .await
                .map_err(|_| NetworkTransportError::Io)?;
            if count == 0 {
                Ok(None)
            } else {
                buffer.truncate(count);
                Ok(Some(buffer))
            }
        }))?
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .write_all(bytes)
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }

    pub fn shutdown(&mut self) -> Result<(), NetworkTransportError> {
        map_wait(crate::network_future::block_on_current(async {
            self.inner
                .shutdown()
                .await
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }
}

fn certificate_sha256(certificate_der: &[u8]) -> String {
    use sha2::Digest;

    sha2::Sha256::digest(certificate_der)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A permission-checked TCP listener with a separate listen authority scope.
pub struct TcpListener {
    inner: TokioTcpListener,
}

impl TcpListener {
    pub fn bind(
        address: SocketAddr,
        grant: &EffectiveNetworkGrant,
    ) -> Result<Self, NetworkTransportError> {
        grant
            .authorize_listen(address)
            .map_err(NetworkTransportError::from)?;
        map_wait(crate::network_future::block_on_current(async move {
            TokioTcpListener::bind(address)
                .await
                .map(|inner| Self { inner })
                .map_err(|_| NetworkTransportError::Io)
        }))?
    }

    pub fn local_address(&self) -> Result<SocketAddr, NetworkTransportError> {
        self.inner
            .local_addr()
            .map_err(|_| NetworkTransportError::Io)
    }

    pub fn accept(&self) -> Result<(TcpStream, SocketAddr), NetworkTransportError> {
        map_wait(crate::network_future::block_on_current(async {
            let (inner, peer) = self
                .inner
                .accept()
                .await
                .map_err(|_| NetworkTransportError::Io)?;
            Ok((TcpStream { inner, peer }, peer))
        }))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, await_task, cancel_current, spawn, yield_now};
    use crate::network_future::block_on_current;

    #[test]
    fn udp_connected_socket_preserves_datagrams_and_checks_destination_authority() {
        let server = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let peer = server.local_addr().unwrap();
        let destination = format!("udp://{peer}");
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_network_scopes(
                std::iter::empty(),
                std::iter::empty(),
                [destination],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );

        Executor::new().run_with_root(move || {
            let socket = UdpDatagramSocket::connect(peer, grant, Duration::from_secs(1)).unwrap();
            assert_eq!(socket.send(b"hello", Duration::from_secs(1)), Ok(5));
            let mut buffer = [0; 32];
            let (length, sender) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..length], b"hello");
            assert_eq!(sender, socket.local_address().unwrap());
            server.send_to(b"reply", sender).unwrap();
            assert_eq!(
                socket.receive(16, Duration::from_secs(1)),
                Ok((b"reply".to_vec(), peer))
            );
            0
        });
    }

    #[test]
    fn tls_alpn_protocols_are_unique_ascii_and_bounded() {
        assert_eq!(
            validate_tls_alpn_protocols(&[b"h2".to_vec(), b"http/1.1".to_vec()]),
            Ok(())
        );
        for protocols in [
            vec![],
            vec![b"h2".to_vec(), b"h2".to_vec()],
            vec![b"bad protocol".to_vec()],
            vec![vec![b'a'; 256]],
            (0..17)
                .map(|index| format!("p{index}").into_bytes())
                .collect(),
        ] {
            assert_eq!(
                validate_tls_alpn_protocols(&protocols),
                Err(NetworkTransportError::InvalidTlsConfiguration)
            );
        }
    }

    #[test]
    fn udp_denies_ungranted_destinations_and_rejects_oversized_receive() {
        let denied = Arc::new(
            EffectiveNetworkGrant::from_verified_network_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );
        let peer: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert_eq!(
            UdpDatagramSocket::connect(peer, denied, Duration::from_secs(1))
                .err()
                .unwrap(),
            NetworkTransportError::PermissionDenied
        );

        let server = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let peer = server.local_addr().unwrap();
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
        Executor::new().run_with_root(move || {
            let socket = UdpDatagramSocket::connect(peer, grant, Duration::from_secs(1)).unwrap();
            assert_eq!(socket.send(b"request", Duration::from_secs(1)), Ok(7));
            let mut request = [0; 32];
            let (length, sender) = server.recv_from(&mut request).unwrap();
            assert_eq!(&request[..length], b"request");
            server.send_to(b"larger-than-limit", sender).unwrap();
            assert_eq!(
                socket.receive(4, Duration::from_secs(1)),
                Err(NetworkTransportError::BufferLimitExceeded)
            );
            0
        });
    }

    #[test]
    fn udp_bound_socket_receives_sender_and_replies_to_an_approved_destination() {
        let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let local = probe.local_addr().unwrap();
        drop(probe);
        let sender_probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let sender = sender_probe.local_addr().unwrap();

        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_network_scopes(
                std::iter::empty(),
                std::iter::empty(),
                [format!("udp://{sender}")],
                [local.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );
        let sender_socket = sender_probe;
        sender_socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        Executor::new().run_with_root(move || {
            let socket = UdpDatagramSocket::bind(local, grant, Duration::from_secs(1)).unwrap();
            sender_socket.send_to(b"query", local).unwrap();
            let (message, peer) = socket.receive(32, Duration::from_secs(1)).unwrap();
            assert_eq!(message, b"query");
            assert_eq!(peer, sender);
            assert_eq!(
                socket.send_to(b"answer", peer, Duration::from_secs(1)),
                Ok(6)
            );
            let mut response = [0; 32];
            let (length, peer) = sender_socket.recv_from(&mut response).unwrap();
            assert_eq!(&response[..length], b"answer");
            assert_eq!(peer, local);
            0
        });
    }

    #[test]
    fn multicast_membership_and_send_require_their_separate_group_grant() {
        let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let local = reservation.local_addr().unwrap();
        drop(reservation);
        let destination: SocketAddr = "239.1.2.3:5000".parse().unwrap();
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                [format!("udp://{destination}")],
                [local.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap(),
        );
        Executor::new().run_with_root(move || {
            let mut socket = UdpDatagramSocket::bind(local, grant, Duration::from_secs(1)).unwrap();
            assert_eq!(
                socket.join_multicast(destination.ip()),
                Err(NetworkTransportError::PermissionDenied)
            );
            assert_eq!(
                socket.send_multicast(b"denied", destination, Duration::from_secs(1)),
                Err(NetworkTransportError::PermissionDenied)
            );
            assert!(socket.multicast_memberships.is_empty());
            0
        });
    }

    #[test]
    fn dns_cache_is_bounded_and_expires_entries() {
        let now = Instant::now();
        let mut cache = DnsCache::default();
        let address: SocketAddr = "93.184.216.34:443".parse().unwrap();
        cache.insert(("api.example.com".into(), 443), vec![address], now);
        assert_eq!(
            cache.get(&("api.example.com".into(), 443), now + DNS_CACHE_TTL / 2),
            Some(vec![address])
        );
        assert_eq!(
            cache.get(&("api.example.com".into(), 443), now + DNS_CACHE_TTL),
            None
        );

        for port in 0..=MAX_DNS_CACHE_ENTRIES as u16 {
            cache.insert(("bounded.example".into(), port), vec![address], now);
        }
        assert_eq!(cache.entries.len(), MAX_DNS_CACHE_ENTRIES);
    }

    #[test]
    fn dns_cache_accepts_only_a_fully_validated_answer_set() {
        let host = "zirk-dns-validation-cache.example";
        let origin = format!("https://{host}:443");
        let grant = EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap();
        let mixed = vec![
            "93.184.216.34:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap(),
        ];

        assert_eq!(
            validate_and_cache_dns_answers(host, 443, &origin, mixed, &grant),
            Err(NetworkTransportError::AddressNotAllowed)
        );
        assert_eq!(cached_dns(host, 443), None);

        let public = vec!["93.184.216.34:443".parse().unwrap()];
        assert_eq!(
            validate_and_cache_dns_answers(host, 443, &origin, public.clone(), &grant,),
            Ok(())
        );
        assert_eq!(cached_dns(host, 443), Some(public));
    }

    #[test]
    fn public_dns_resolution_requires_exact_origin_and_returns_validated_ip_literals() {
        let denied = EffectiveNetworkGrant::from_verified_scopes([], []).unwrap();
        assert_eq!(
            resolve_origin(
                "tcp://127.0.0.1:80",
                Arc::new(denied),
                Duration::from_secs(1)
            ),
            Err(NetworkTransportError::PermissionDenied)
        );

        let endpoint = "tcp://127.0.0.1:80".to_owned();
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([endpoint.clone()], []).unwrap());
        let outcome = Executor::new().run_with_root(move || {
            let addresses = resolve_origin(&endpoint, grant, Duration::from_secs(1)).unwrap();
            assert_eq!(addresses, ["127.0.0.1"]);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn public_dns_resolution_rejects_cached_private_rebinding_before_returning() {
        let endpoint = "https://zirk-rebind-check.example:443".to_owned();
        cache_dns(
            "zirk-rebind-check.example",
            443,
            vec!["127.0.0.1:443".parse().unwrap()],
        );
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([endpoint.clone()], []).unwrap());
        let outcome = Executor::new().run_with_root(move || {
            assert_eq!(
                resolve_origin(&endpoint, grant, Duration::from_secs(1)),
                Err(NetworkTransportError::AddressNotAllowed)
            );
            0
        });
        DNS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .retain(|(key, _, _)| key.0 != "zirk-rebind-check.example");
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tcp_connect_rejects_cached_private_dns_rebinding_before_socket_creation() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let host = format!("zirk-rebinding-connect-{}.example", address.port());
        let origin = format!("tcp://{host}:{}", address.port());
        cache_dns(&host, address.port(), vec![address]);
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap());
        let task_origin = origin.clone();
        let outcome = Executor::new().run_with_root(move || {
            assert_eq!(
                TcpStream::connect(&task_origin, grant).err(),
                Some(NetworkTransportError::AddressNotAllowed)
            );
            0
        });
        DNS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .retain(|(key, _, _)| key.0 != host);
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn tcp_connect_rejects_fresh_private_dns_rebinding_before_socket_creation() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let host = format!("zirk-fresh-rebinding-{}.example", address.port());
        let origin = format!("tcp://{host}:{}", address.port());
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap());
        let resolver_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_origin = origin.clone();
        let task_host = host.clone();
        let task_resolver_called = resolver_called.clone();

        let outcome = Executor::new().run_with_root(move || {
            let result = connect_raw_with_resolver(
                NetworkOrigin::parse(&task_origin).unwrap(),
                grant,
                false,
                Duration::from_secs(1),
                move |resolved_host, _port| async move {
                    assert_eq!(resolved_host, task_host);
                    task_resolver_called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(vec![address])
                },
            )
            .err();
            assert_eq!(result, Some(NetworkTransportError::AddressNotAllowed));
            0
        });

        DNS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .retain(|(key, _, _)| key.0 != host);
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(resolver_called.load(std::sync::atomic::Ordering::SeqCst));
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn tcp_connect_rejects_scoped_ipv6_dns_candidate_before_socket_creation() {
        let host = "zirk-scoped-ipv6-candidate.example";
        let origin = format!("https://{host}:443");
        let candidate = SocketAddr::V6(std::net::SocketAddrV6::new(
            "2606:4700:4700::1111".parse().unwrap(),
            443,
            0,
            7,
        ));
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap());
        let resolver_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_origin = origin.clone();
        let task_resolver_called = Arc::clone(&resolver_called);

        let outcome = Executor::new().run_with_root(move || {
            let result = connect_raw_with_resolver(
                NetworkOrigin::parse(&task_origin).unwrap(),
                grant,
                true,
                Duration::from_secs(1),
                move |resolved_host, port| async move {
                    assert_eq!(resolved_host, host);
                    assert_eq!(port, 443);
                    task_resolver_called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(vec![candidate])
                },
            )
            .err();
            assert_eq!(result, Some(NetworkTransportError::AddressNotAllowed));
            0
        });

        DNS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .retain(|(key, _, _)| key.0 != host);
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(resolver_called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(cached_dns(host, 443), None);
    }

    #[test]
    fn tcp_connect_rejects_oversized_dns_answer_set_before_cache_or_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let host = format!("zirk-oversized-dns-{}.example", address.port());
        let origin = format!("tcp://{host}:{}", address.port());
        let grant =
            Arc::new(EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap());
        let task_origin = origin.clone();
        let answers = vec![address; MAX_DNS_RESULTS + 1];

        let outcome = Executor::new().run_with_root(move || {
            let result = connect_raw_with_resolver(
                NetworkOrigin::parse(&task_origin).unwrap(),
                grant,
                false,
                Duration::from_secs(1),
                move |_host, _port| async move { Ok(answers) },
            )
            .err();
            assert_eq!(result, Some(NetworkTransportError::DnsResultLimit));
            0
        });

        let cached = cached_dns(&host, address.port());
        DNS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .retain(|(key, _, _)| key.0 != host);
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(cached, None);
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn tls_custom_trust_roots_reject_empty_and_invalid_der_before_connecting() {
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(["https://127.0.0.1:443".to_owned()], [])
                .unwrap(),
        );
        assert_eq!(
            TlsClientStream::connect_with_trust_roots(
                "https://127.0.0.1:443",
                Arc::clone(&grant),
                Vec::new(),
                false,
            )
            .err(),
            Some(NetworkTransportError::CertificateStore)
        );
        assert_eq!(
            TlsClientStream::connect_with_trust_roots(
                "https://127.0.0.1:443",
                grant,
                vec![vec![0, 1, 2]],
                false,
            )
            .err(),
            Some(NetworkTransportError::CertificateStore)
        );
    }

    #[test]
    fn tls_server_identity_rejects_empty_and_malformed_material() {
        let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();

        assert_eq!(
            TlsServerConfig::from_der(Vec::new(), private_key.clone(), false).err(),
            Some(NetworkTransportError::InvalidServerIdentity)
        );
        assert_eq!(
            TlsServerConfig::from_der(vec![vec![0, 1, 2]], private_key.clone(), false).err(),
            Some(NetworkTransportError::InvalidServerIdentity)
        );
        assert_eq!(
            TlsServerConfig::from_der(vec![certificate], vec![0, 1, 2], false).err(),
            Some(NetworkTransportError::InvalidServerIdentity)
        );
    }

    #[test]
    fn tls_uses_explicit_identity_trusts_the_chain_and_defaults_to_tls13() {
        use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};

        let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
        let server_config =
            TlsServerConfig::from_der(vec![certificate.clone()], private_key, false).unwrap();
        let trusted_client_config = build_client_tls_config(
            false,
            vec![CertificateDer::from(
                include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
            )],
            true,
        )
        .unwrap();
        let untrusted_client_config = build_client_tls_config(false, Vec::new(), false).unwrap();

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
            let server_task = spawn(move || {
                let listener = TlsListener::bind(address, &server_grant, server_config).unwrap();
                assert!(matches!(
                    listener.accept(),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                assert!(matches!(
                    listener.accept(),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                let (mut stream, _) = listener.accept().unwrap();
                assert_eq!(stream.read(16).unwrap(), Some(b"ping".to_vec()));
                stream.write_all(b"pong").unwrap();
                stream.shutdown().unwrap();
                31
            });

            let hostname_grant = Arc::clone(&grant);
            let hostname_origin = origin.clone();
            let wrong_name = ServerName::try_from("wrong.example".to_owned()).unwrap();
            let wrong_hostname_config = Arc::clone(&trusted_client_config);
            let wrong_hostname = spawn(move || {
                assert!(matches!(
                    TlsClientStream::connect_with_config(
                        Url::parse(&hostname_origin).unwrap(),
                        hostname_grant,
                        wrong_name,
                        wrong_hostname_config,
                    ),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                29
            });
            assert_eq!(await_task(wrong_hostname), 29);

            let untrusted_grant = Arc::clone(&grant);
            let untrusted_origin = origin.clone();
            let untrusted_client = spawn(move || {
                let name = ServerName::IpAddress("127.0.0.1".parse::<IpAddr>().unwrap().into());
                assert!(matches!(
                    TlsClientStream::connect_with_config(
                        Url::parse(&untrusted_origin).unwrap(),
                        untrusted_grant,
                        name,
                        untrusted_client_config,
                    ),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                19
            });
            assert_eq!(await_task(untrusted_client), 19);

            let good_grant = Arc::clone(&grant);
            let good_origin = origin.clone();
            let client = spawn(move || {
                let mut stream = TlsClientStream::connect_with_trust_roots(
                    &good_origin,
                    good_grant,
                    vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                    false,
                )
                .unwrap();
                assert_eq!(
                    stream.inner.get_ref().1.protocol_version(),
                    Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3)
                );
                stream.write_all(b"ping").unwrap();
                assert_eq!(stream.read(16).unwrap(), Some(b"pong".to_vec()));
                assert_eq!(stream.read(16).unwrap(), None);
                23
            });
            assert_eq!(await_task(client), 23);
            assert_eq!(await_task(server_task), 31);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tls_upgrade_wraps_an_existing_authorized_tcp_socket() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        reserved.set_nonblocking(true).unwrap();
        let address = reserved.local_addr().unwrap();
        let mut server_config = tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(
                    include_bytes!("../testdata/tls-loopback-cert.der").to_vec(),
                )],
                PrivateKeyDer::try_from(
                    include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
                )
                .unwrap(),
            )
            .unwrap();
        server_config.alpn_protocols = vec![b"zirk-runtime-upgrade/1".to_vec()];
        let server_config = Arc::new(server_config);
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(reserved).unwrap();
                let (socket, _) = listener.accept().await.unwrap();
                let acceptor = tokio_rustls::TlsAcceptor::from(server_config);
                let mut stream = acceptor.accept(socket).await.unwrap();
                assert_eq!(
                    stream.get_ref().1.alpn_protocol(),
                    Some(b"zirk-runtime-upgrade/1".as_slice())
                );
                let mut request = [0; 4];
                stream.read_exact(&mut request).await.unwrap();
                assert_eq!(&request, b"ping");
                stream.write_all(b"pong").await.unwrap();
            });
        });

        let tcp_origin = format!("tcp://127.0.0.1:{}", address.port());
        let https_origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [tcp_origin.clone(), https_origin.clone()],
                [address.to_string()],
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let tcp = TcpStream::connect(&tcp_origin, Arc::clone(&grant)).unwrap();
            let prepared = TlsClientStream::prepare_tcp_upgrade(
                &https_origin,
                tcp.peer_address(),
                &grant,
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
                vec![b"zirk-runtime-upgrade/1".to_vec()],
            )
            .unwrap();
            let mut tls = TlsClientStream::wrap_tcp(tcp, prepared).unwrap();
            assert_eq!(
                tls.negotiated_alpn().as_deref(),
                Some("zirk-runtime-upgrade/1")
            );
            tls.write_all(b"ping").unwrap();
            assert_eq!(tls.read(4).unwrap(), Some(b"pong".to_vec()));
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tls_client_identity_satisfies_mutual_authentication_and_negotiates_alpn() {
        let server_certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let server_private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
        let client_ca = include_bytes!("../testdata/mtls-client-ca.der").to_vec();
        let client_certificate = include_bytes!("../testdata/mtls-client.der").to_vec();
        let server_config = TlsServerConfig::from_der_with_client_auth(
            vec![server_certificate],
            server_private_key,
            vec![client_ca],
            false,
        )
        .unwrap()
        .with_alpn_protocols(vec![b"zirk-mtls-test/1".to_vec()]);

        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        reserved.set_nonblocking(true).unwrap();
        let address = reserved.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(reserved).unwrap();
                let (socket, _) = listener.accept().await.unwrap();
                let acceptor = tokio_rustls::TlsAcceptor::from(Arc::clone(&server_config.0));
                let mut stream = acceptor.accept(socket).await.unwrap();
                assert_eq!(
                    stream.get_ref().1.alpn_protocol(),
                    Some(b"zirk-mtls-test/1".as_slice())
                );
                assert_eq!(
                    stream
                        .get_ref()
                        .1
                        .peer_certificates()
                        .unwrap()
                        .first()
                        .unwrap()
                        .as_ref(),
                    client_certificate.as_slice()
                );
                let mut request = [0; 4];
                stream.read_exact(&mut request).await.unwrap();
                assert_eq!(&request, b"mTLS");
                stream.write_all(b"ok").await.unwrap();
            });
        });

        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let task_origin = origin.clone();
        let task_grant = Arc::clone(&grant);
        let outcome = Executor::new().run_with_root(move || {
            let mut stream = TlsClientStream::connect_with_client_identity(
                &task_origin,
                task_grant,
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                vec![include_bytes!("../testdata/mtls-client.der").to_vec()],
                include_bytes!("../testdata/mtls-client-key.der").to_vec(),
                false,
                vec![b"zirk-mtls-test/1".to_vec()],
            )
            .unwrap();
            assert_eq!(
                stream.inner.get_ref().1.protocol_version(),
                Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3)
            );
            assert_eq!(
                stream.negotiated_alpn(),
                Some("zirk-mtls-test/1".to_owned())
            );
            stream.write_all(b"mTLS").unwrap();
            assert_eq!(stream.read(8).unwrap(), Some(b"ok".to_vec()));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        server.join().unwrap();
    }

    #[test]
    fn tls_client_identity_rejects_malformed_private_key_der() {
        let error = client_tls_config_with_identity(
            vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
            vec![include_bytes!("../testdata/mtls-client.der").to_vec()],
            vec![0, 1, 2, 3],
            false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NetworkTransportError::InvalidServerIdentity
        ));
    }

    #[test]
    fn tls_server_mutual_auth_requires_explicit_valid_client_roots() {
        let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
        assert!(matches!(
            TlsServerConfig::from_der_with_client_auth(
                vec![certificate.clone()],
                key.clone(),
                Vec::new(),
                false,
            ),
            Err(NetworkTransportError::InvalidTlsConfiguration)
        ));
        assert!(matches!(
            TlsServerConfig::from_der_with_client_auth(
                vec![certificate],
                key,
                vec![vec![0, 1, 2]],
                false,
            ),
            Err(NetworkTransportError::CertificateStore)
        ));
    }

    #[test]
    fn tls12_requires_explicit_client_opt_in() {
        use tokio_rustls::rustls::{pki_types::PrivateKeyDer, version};

        let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
        let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
        let certificate = tokio_rustls::rustls::pki_types::CertificateDer::from(certificate);
        let key = PrivateKeyDer::try_from(private_key).unwrap();
        let server =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&version::TLS12])
                .with_no_client_auth()
                .with_single_cert(vec![certificate], key)
                .unwrap();
        let server_config = TlsServerConfig(Arc::new(server));

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
            let server_task = spawn(move || {
                let listener = TlsListener::bind(address, &server_grant, server_config).unwrap();
                assert!(matches!(
                    listener.accept(),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                let (mut stream, _) = listener.accept().unwrap();
                assert_eq!(
                    stream.inner.get_ref().1.protocol_version(),
                    Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_2)
                );
                assert_eq!(stream.read(16).unwrap(), Some(b"tls12".to_vec()));
                stream.write_all(b"ok").unwrap();
                1
            });

            let denied_grant = Arc::clone(&grant);
            let denied_origin = origin.clone();
            let denied_client = spawn(move || {
                assert!(matches!(
                    TlsClientStream::connect_with_trust_roots(
                        &denied_origin,
                        denied_grant,
                        vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                        false,
                    ),
                    Err(NetworkTransportError::TlsHandshake)
                ));
                2
            });
            assert_eq!(await_task(denied_client), 2);

            let allowed_grant = Arc::clone(&grant);
            let allowed_origin = origin.clone();
            let allowed_client = spawn(move || {
                let mut stream = TlsClientStream::connect_with_trust_roots(
                    &allowed_origin,
                    allowed_grant,
                    vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                    true,
                )
                .unwrap();
                stream.write_all(b"tls12").unwrap();
                assert_eq!(stream.read(16).unwrap(), Some(b"ok".to_vec()));
                3
            });
            assert_eq!(await_task(allowed_client), 3);
            assert_eq!(await_task(server_task), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http_tls_alpn_selects_h2_and_falls_back_to_http1() {
        use tokio_rustls::rustls::pki_types::CertificateDer;

        fn negotiate(
            server_protocols: Vec<Vec<u8>>,
            expected: NegotiatedHttpProtocol,
            allow_tls12: bool,
        ) {
            let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
            let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
            let server_config =
                TlsServerConfig::from_der(vec![certificate], private_key, allow_tls12)
                    .unwrap()
                    .with_alpn_protocols(server_protocols.clone());
            let client_config = build_client_tls_config(
                allow_tls12,
                vec![CertificateDer::from(
                    include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
                )],
                true,
            )
            .unwrap();
            let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = reserved.local_addr().unwrap();
            drop(reserved);
            let origin = format!("https://127.0.0.1:{}", address.port());
            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes(
                    [origin.clone()],
                    [address.to_string()],
                )
                .unwrap(),
            );

            let outcome = Executor::new().run_with_root(move || {
                let server_grant = Arc::clone(&grant);
                let server = spawn(move || {
                    let listener =
                        TlsListener::bind(address, &server_grant, server_config).unwrap();
                    let (stream, _) = listener.accept().unwrap();
                    assert_eq!(
                        stream.inner.get_ref().1.alpn_protocol(),
                        Some(expected_alpn(expected))
                    );
                    drop(stream);
                    1
                });

                let client_grant = Arc::clone(&grant);
                let client_origin = origin.clone();
                let mut client_config = (*client_config).clone();
                client_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
                let client = spawn(move || {
                    let (_, protocol) = TlsClientStream::connect_for_http_with_config(
                        &client_origin,
                        client_grant,
                        Duration::from_secs(2),
                        Arc::new(client_config),
                    )
                    .unwrap();
                    assert_eq!(protocol, expected);
                    1
                });
                assert_eq!(await_task(client), 1);
                assert_eq!(await_task(server), 1);
                0
            });
            assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        }

        fn expected_alpn(protocol: NegotiatedHttpProtocol) -> &'static [u8] {
            match protocol {
                NegotiatedHttpProtocol::Http1 => b"http/1.1",
                NegotiatedHttpProtocol::Http2 => b"h2",
            }
        }

        negotiate(vec![b"h2".to_vec()], NegotiatedHttpProtocol::Http2, false);
        negotiate(
            vec![b"http/1.1".to_vec()],
            NegotiatedHttpProtocol::Http1,
            false,
        );
        negotiate(
            vec![b"http/1.1".to_vec()],
            NegotiatedHttpProtocol::Http1,
            true,
        );
    }

    #[test]
    fn tls_client_handshake_obeys_its_timeout() {
        use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};

        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        reserved.set_nonblocking(true).unwrap();
        let address = reserved.local_addr().unwrap();
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], std::iter::empty())
                .unwrap(),
        );
        let config = build_client_tls_config(
            false,
            vec![CertificateDer::from(
                include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
            )],
            true,
        )
        .unwrap();
        let second_handshake_accepted = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let outcome = Executor::new().run_with_root(move || {
            let accepted = Arc::clone(&second_handshake_accepted);
            let server = spawn(move || {
                block_on_current(async move {
                    let listener = TokioTcpListener::from_std(reserved).unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    drop(stream);
                    let (mut stream, _) = listener.accept().await.unwrap();
                    accepted.store(true, std::sync::atomic::Ordering::Release);
                    let client_closed = tokio::time::timeout(Duration::from_millis(100), async {
                        let mut buffer = [0; 1024];
                        loop {
                            if stream.read(&mut buffer).await.unwrap() == 0 {
                                break;
                            }
                        }
                    })
                    .await;
                    assert!(
                        client_closed.is_ok(),
                        "canceled TLS client kept its TCP socket open"
                    );
                })
                .unwrap();
                7
            });
            let client_grant = Arc::clone(&grant);
            let timeout_config = Arc::clone(&config);
            let timeout_origin = origin.clone();
            let client = spawn(move || {
                let name = ServerName::IpAddress("127.0.0.1".parse::<IpAddr>().unwrap().into());
                match TlsClientStream::connect_with_config_and_timeout(
                    Url::parse(&timeout_origin).unwrap(),
                    client_grant,
                    name,
                    timeout_config,
                    Duration::from_millis(5),
                ) {
                    Err(NetworkTransportError::Timeout) => 11,
                    _ => 17,
                }
            });
            assert_eq!(await_task(client), 11, "TLS handshake should time out");

            let client_grant = Arc::clone(&grant);
            let cancellation_origin = origin.clone();
            let cancellation_config = Arc::clone(&config);
            let cancelled_client = spawn(move || {
                let name = ServerName::IpAddress("127.0.0.1".parse::<IpAddr>().unwrap().into());
                let _ = TlsClientStream::connect_with_config_and_timeout(
                    Url::parse(&cancellation_origin).unwrap(),
                    client_grant,
                    name,
                    cancellation_config,
                    Duration::from_secs(5),
                );
                13
            });
            while !second_handshake_accepted.load(std::sync::atomic::Ordering::Acquire) {
                yield_now();
            }
            cancel_current(cancelled_client);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    await_task(cancelled_client)
                }))
                .is_err()
            );
            assert_eq!(await_task(server), 7);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn authorized_tcp_listener_connect_read_write_and_eof_work_on_zirk_tasks() {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("tcp://127.0.0.1:{}", address.port())],
                [address.to_string()],
            )
            .unwrap(),
        );
        let server_grant = Arc::clone(&grant);
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let listener = TcpListener::bind(address, &server_grant).unwrap();
                let (mut stream, peer) = listener.accept().unwrap();
                assert_eq!(peer.ip(), "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
                assert_eq!(stream.read(16).unwrap(), Some(b"ping".to_vec()));
                assert_eq!(stream.read(16).unwrap(), None);
                stream.write_all(b"pong").unwrap();
                stream.shutdown().unwrap();
                23
            });
            let client_grant = Arc::clone(&grant);
            let client = spawn(move || {
                let mut stream = TcpStream::connect(
                    &format!("tcp://127.0.0.1:{}", address.port()),
                    client_grant,
                )
                .unwrap();
                assert_eq!(stream.write(b"ping").unwrap(), 4);
                stream.shutdown().unwrap();
                assert_eq!(stream.read(16).unwrap(), Some(b"pong".to_vec()));
                assert_eq!(stream.read(16).unwrap(), None);
                17
            });
            yield_now();
            assert_eq!(await_task(client), 17);
            assert_eq!(await_task(server), 23);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn cancelling_pending_tcp_accept_releases_listener_and_port() {
        use crate::executor::{await_task, cancel_current, spawn, yield_now};

        for _ in 0..8 {
            let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = reserved.local_addr().unwrap();
            drop(reserved);
            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes(
                    std::iter::empty::<String>(),
                    [address.to_string()],
                )
                .unwrap(),
            );
            let outcome = Executor::new().run_with_root(move || {
                let server_grant = Arc::clone(&grant);
                let server = spawn(move || {
                    let listener = TcpListener::bind(address, &server_grant).unwrap();
                    let _ = listener.accept();
                    1
                });
                // Let the listener bind and park its task in accept before
                // requesting cancellation from the owner.
                yield_now();
                cancel_current(server);
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        await_task(server)
                    }))
                    .is_err(),
                    "a cancelled accept task must not report successful completion"
                );
                let rebound = std::net::TcpListener::bind(address)
                    .expect("canceling a pending accept must release its listening socket");
                drop(rebound);
                0
            });
            assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        }
    }

    #[test]
    fn unauthorized_connect_and_listen_fail_before_socket_creation() {
        let grant = Arc::new(EffectiveNetworkGrant::default());
        let outcome = Executor::new().run_with_root(move || {
            assert!(matches!(
                TcpListener::bind("127.0.0.1:0".parse().unwrap(), &grant),
                Err(NetworkTransportError::PermissionDenied)
            ));
            assert!(matches!(
                TcpStream::connect("http://no-such-host.invalid:8080/", grant),
                Err(NetworkTransportError::PermissionDenied)
            ));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }
}
