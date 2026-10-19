//! Permission-checked TCP operations on Zirk's cooperative task stack.

use std::collections::VecDeque;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream};
use zirk_runtime_core::http_values::Url;
use zirk_runtime_core::network_policy::{EffectiveNetworkGrant, NetworkOrigin, NetworkPolicyError};

const MAX_DNS_RESULTS: usize = 32;
const MAX_DNS_CACHE_ENTRIES: usize = 256;
const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
pub const DEFAULT_NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_TCP_READ_BYTES: usize = 1024 * 1024;
pub const MAX_TCP_WRITE_BYTES: usize = 1024 * 1024;

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
    client_tls_config(false)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NegotiatedHttpProtocol {
    Http1,
    Http2,
}

impl TlsClientStream {
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
        Self::connect_for_http_with_config(url, grant, timeout, client_tls_config(false)?)
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

    fn connect_for_http_with_protocols(
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

        if certificate_chain.is_empty() {
            return Err(NetworkTransportError::InvalidServerIdentity);
        }
        let certificates = certificate_chain
            .into_iter()
            .map(tokio_rustls::rustls::pki_types::CertificateDer::from)
            .collect();
        let key = PrivateKeyDer::try_from(private_key)
            .map_err(|_| NetworkTransportError::InvalidServerIdentity)?;
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

    /// Configures the protocols offered by an application protocol adapter.
    /// TLS itself defaults to no ALPN so generic transport users retain their
    /// existing negotiation behavior.
    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        let mut config = (*self.0).clone();
        config.alpn_protocols = protocols;
        self.0 = Arc::new(config);
        self
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

        fn negotiate(server_protocols: Vec<Vec<u8>>, expected: NegotiatedHttpProtocol) {
            let certificate = include_bytes!("../testdata/tls-loopback-cert.der").to_vec();
            let private_key = include_bytes!("../testdata/tls-loopback-key.der").to_vec();
            let server_config = TlsServerConfig::from_der(vec![certificate], private_key, false)
                .unwrap()
                .with_alpn_protocols(server_protocols.clone());
            let client_config = build_client_tls_config(
                false,
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

        negotiate(vec![b"h2".to_vec()], NegotiatedHttpProtocol::Http2);
        negotiate(vec![b"http/1.1".to_vec()], NegotiatedHttpProtocol::Http1);
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
