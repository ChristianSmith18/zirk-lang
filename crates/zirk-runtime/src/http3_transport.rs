//! Bounded HTTP/3 client transport over the reviewed Quinn and Hyperium h3
//! engines. Request policy and permissions remain owned by `std.http` and the
//! native runtime boundary.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Buf, Bytes};
use http::{HeaderMap, Request, Response, Version};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Endpoint, TransportConfig, VarInt};
use tokio::task::JoinHandle;
use zirk_runtime_core::http_values::{
    MAX_HTTP_BUFFERED_BODY_BYTES, MAX_HTTP_HEADER_BYTES, MAX_HTTP_HEADER_COUNT, Url,
};
use zirk_runtime_core::network_policy::{EffectiveNetworkGrant, NetworkOrigin};

use crate::network_transport::{NetworkTransportError, client_tls_config_for_http3};

const MAX_QUIC_STREAMS: u32 = 16;
#[allow(dead_code)] // Wired to the public listener lifecycle in the next step.
const MAX_HTTP3_REQUESTS_PER_CONNECTION: usize = 1_024;
const MAX_HTTP3_SERVER_CONNECTIONS: usize = 256;
// Keep slow QUIC handshakes from serializing admission for valid peers.
const MAX_HTTP3_SERVER_HANDSHAKES: usize = 16;
const MAX_HTTP3_SERVER_REQUESTS: usize = 65_536;
// Four in-flight requests can each hold a 16 MiB request and response body.
// Application body vectors are therefore capped at 128 MiB across this server,
// alongside Quinn's separately bounded transport windows.
const MAX_HTTP3_SERVER_CONCURRENT_REQUESTS: usize = 4;
const QUIC_STREAM_WINDOW_BYTES: u64 = MAX_HTTP_BUFFERED_BODY_BYTES as u64;
const QUIC_CONNECTION_WINDOW_BYTES: u64 = (MAX_HTTP_BUFFERED_BODY_BYTES as u64) * 4;
const HTTP3_CLOSE_REASON: &[u8] = b"request complete";
#[allow(dead_code)] // Wired to the public listener lifecycle in the next step.
const HTTP3_SERVER_IO_TIMEOUT: Duration = Duration::from_secs(30);

struct Http3ServerAdmission {
    max_requests: usize,
    admitted_requests: std::sync::atomic::AtomicUsize,
    request_limit: tokio::sync::watch::Sender<bool>,
    active_requests: std::sync::Arc<tokio::sync::Semaphore>,
}

impl Http3ServerAdmission {
    fn new(max_requests: usize) -> std::sync::Arc<Self> {
        let (request_limit, _) = tokio::sync::watch::channel(false);
        std::sync::Arc::new(Self {
            max_requests,
            admitted_requests: std::sync::atomic::AtomicUsize::new(0),
            request_limit,
            active_requests: std::sync::Arc::new(tokio::sync::Semaphore::new(
                MAX_HTTP3_SERVER_CONCURRENT_REQUESTS,
            )),
        })
    }

    fn subscribe_to_limit(&self) -> tokio::sync::watch::Receiver<bool> {
        self.request_limit.subscribe()
    }

    fn is_exhausted(&self) -> bool {
        self.admitted_requests
            .load(std::sync::atomic::Ordering::Acquire)
            >= self.max_requests
    }

    fn admit_request(&self) -> bool {
        let admitted = self.admitted_requests.fetch_update(
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
            |current| (current < self.max_requests).then_some(current + 1),
        );
        match admitted {
            Ok(previous) => {
                if previous + 1 == self.max_requests {
                    self.request_limit.send_replace(true);
                }
                true
            }
            Err(_) => false,
        }
    }
}

/// Response returned by the owner-task HTTP handler. A generated producer is
/// kept out of the reactor and is pumped only after the response headers and
/// bounded body receiver have been handed to the HTTP/3 connection task.
pub(crate) struct Http3ServerResponse {
    pub(crate) response: http::Response<Http3ResponseBody>,
    pub(crate) producer: Option<(
        crate::http_api::HttpServerBodyProducer,
        tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    )>,
}

pub(crate) enum Http3ResponseBody {
    Buffered(Vec<u8>),
    Stream(tokio::sync::mpsc::Receiver<Result<Bytes, std::io::Error>>),
}

/// Accepts one RFC 9220 WebSocket extended CONNECT request and bridges its
/// bidirectional HTTP/3 DATA stream to the bounded WebSocket codec.
///
/// The caller owns the returned cancellation handle for the lifetime of the
/// Zirk WebSocket session. Invalid path, handshake, or origin requests receive
/// a small status-only HTTP/3 response and return `Ok(None)`.
#[cfg(feature = "http3")]
pub(crate) async fn accept_websocket_request(
    resolver: h3::server::RequestResolver<h3_quinn::Connection, Bytes>,
    expected_path: &str,
    policy: zirk_runtime_core::http_websocket::WebSocketAcceptPolicy,
) -> Result<
    Option<(
        tokio::io::DuplexStream,
        Option<String>,
        tokio::task::JoinHandle<()>,
    )>,
    super::http_runtime::HttpRuntimeError,
> {
    use http::StatusCode;

    let (request, mut stream) =
        tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, resolver.resolve_request())
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;

    macro_rules! reject {
        ($status:expr) => {{
            let response = Response::builder()
                .status($status)
                .body(())
                .expect("fixed HTTP/3 WebSocket response is valid");
            let _ =
                tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, stream.send_response(response)).await;
            let _ = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, stream.finish()).await;
        }};
    }

    if request.method() != http::Method::CONNECT
        || request.version() != Version::HTTP_3
        || request
            .extensions()
            .get::<h3::ext::Protocol>()
            .is_none_or(|protocol| *protocol != h3::ext::Protocol::WEBSOCKET)
        || request.headers().len() > MAX_HTTP_HEADER_COUNT
    {
        reject!(StatusCode::BAD_REQUEST);
        return Ok(None);
    }
    if request.uri().path() != expected_path {
        reject!(StatusCode::NOT_FOUND);
        return Ok(None);
    }
    if request
        .headers()
        .get(http::header::SEC_WEBSOCKET_VERSION)
        .is_none_or(|version| version.as_bytes() != b"13")
        || request
            .headers()
            .contains_key(http::header::SEC_WEBSOCKET_EXTENSIONS)
    {
        reject!(StatusCode::BAD_REQUEST);
        return Ok(None);
    }

    let origins = request
        .headers()
        .get_all(http::header::ORIGIN)
        .iter()
        .map(http::HeaderValue::as_bytes)
        .collect::<Vec<_>>();
    let requested_protocols = request
        .headers()
        .get_all(http::header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .flat_map(|value| value.as_bytes().split(|byte| *byte == b','))
        .map(|value| std::str::from_utf8(value).map(str::trim))
        .collect::<Result<Vec<_>, _>>();
    let requested_protocols = match requested_protocols {
        Ok(protocols) => protocols,
        Err(_) => {
            reject!(StatusCode::BAD_REQUEST);
            return Ok(None);
        }
    };
    let selected = match policy.validate(&origins, &requested_protocols) {
        Ok(selected) => selected,
        Err(zirk_runtime_core::http_websocket::WebSocketPolicyError::OriginDenied) => {
            reject!(StatusCode::FORBIDDEN);
            return Ok(None);
        }
        Err(_) => {
            reject!(StatusCode::BAD_REQUEST);
            return Ok(None);
        }
    };

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(())
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    if let Some(protocol) = &selected {
        response.headers_mut().insert(
            http::header::SEC_WEBSOCKET_PROTOCOL,
            http::HeaderValue::from_str(protocol)
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?,
        );
    }
    tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, stream.send_response(response))
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;

    let (mut send, receive) = stream.split();
    let (websocket, transport) = tokio::io::duplex(64 * 1024);
    let (mut websocket_read, mut websocket_write) = tokio::io::split(transport);
    let driver = tokio::spawn(async move {
        let mut pumps = tokio::task::JoinSet::new();
        pumps.spawn(async move {
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let count =
                    match tokio::io::AsyncReadExt::read(&mut websocket_read, &mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                if send
                    .send_data(Bytes::copy_from_slice(&buffer[..count]))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            let _ = send.finish().await;
        });
        pumps.spawn(async move {
            let mut receive = receive;
            loop {
                let chunk = match receive.recv_data().await {
                    Ok(Some(mut chunk)) => {
                        let size = chunk.remaining();
                        chunk.copy_to_bytes(size)
                    }
                    Ok(None) | Err(_) => break,
                };
                if tokio::io::AsyncWriteExt::write_all(&mut websocket_write, &chunk)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            receive.stop_sending(h3::error::Code::H3_NO_ERROR);
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut websocket_write).await;
        });
        while pumps.join_next().await.is_some() {}
    });
    Ok(Some((websocket, selected, driver)))
}

/// Starts one HTTP/3 WebSocket session on an already bound endpoint. The
/// returned abort handle owns the QUIC connection driver and the per-stream
/// DATA pumps; dropping the Zirk WebSocket cancels the entire session.
#[cfg(feature = "http3")]
pub(crate) fn accept_websocket_endpoint(
    endpoint: Endpoint,
    expected_path: String,
    policy: zirk_runtime_core::http_websocket::WebSocketAcceptPolicy,
) -> Result<
    (
        tokio::io::DuplexStream,
        Option<String>,
        tokio::task::AbortHandle,
        tokio::sync::oneshot::Receiver<()>,
    ),
    super::http_runtime::HttpRuntimeError,
> {
    let (session_tx, session_rx) = tokio::sync::oneshot::channel();
    let task = crate::network_future::spawn_detached_on_reactor(async move {
        let result = async {
            let incoming = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, endpoint.accept())
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
                .ok_or(super::http_runtime::HttpRuntimeError::Cancelled)?;
            let connection = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, incoming)
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
            let mut builder = h3::server::builder();
            builder
                .max_field_section_size(MAX_HTTP_HEADER_BYTES as u64)
                .enable_extended_connect(true);
            let mut server = builder
                .build::<_, Bytes>(h3_quinn::Connection::new(connection))
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;

            for _ in 0..64 {
                let resolver = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, server.accept())
                    .await
                    .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
                    .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?
                    .ok_or(super::http_runtime::HttpRuntimeError::Cancelled)?;
                if let Some((stream, selected, bridge)) =
                    accept_websocket_request(resolver, &expected_path, policy.clone()).await?
                {
                    let (session_done_tx, session_done_rx) = tokio::sync::oneshot::channel();
                    if session_tx.send((stream, selected, session_done_rx)).is_err() {
                        return Err(super::http_runtime::HttpRuntimeError::Cancelled);
                    }
                    // Keep h3's control-stream driver alive for this session.
                    // Additional requests are rejected while the one-shot
                    // WebSocket callback owns the listener.
                    let bridge_abort = AbortHandleOnDrop::new(bridge.abort_handle());
                    tokio::pin!(bridge);
                    loop {
                        tokio::select! {
                            _ = &mut bridge => {
                                let _ = session_done_tx.send(());
                                drop(bridge_abort);
                                return Ok(());
                            }
                            accepted = server.accept() => {
                                let resolver = match accepted {
                                    Ok(Some(resolver)) => resolver,
                                    Ok(None) | Err(_) => {
                                        let _ = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, &mut bridge).await;
                                        let _ = session_done_tx.send(());
                                        drop(bridge_abort);
                                        return Ok(());
                                    }
                                };
                                let (_, mut stream) = resolver
                                    .resolve_request()
                                    .await
                                    .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
                                let response = Response::builder()
                                    .status(http::StatusCode::SERVICE_UNAVAILABLE)
                                    .body(())
                                    .expect("fixed HTTP/3 busy response is valid");
                                let _ = stream.send_response(response).await;
                                let _ = stream.finish().await;
                            }
                        }
                    }
                }
            }
            Err(super::http_runtime::HttpRuntimeError::Protocol)
        }
        .await;
        let _ = result;
    });
    let mut task_guard = AbortHandleOnDrop::new(task.clone());
    let session = crate::network_future::block_on_current(session_rx)
        .map_err(map_network_future_error)?
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    task_guard.disarm();
    Ok((session.0, session.1, task, session.2))
}

struct AbortHandleOnDrop(Option<tokio::task::AbortHandle>);

impl AbortHandleOnDrop {
    fn new(handle: tokio::task::AbortHandle) -> Self {
        Self(Some(handle))
    }

    fn disarm(&mut self) {
        self.0.take();
    }
}

impl Drop for AbortHandleOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

/// Binds one permission-checked QUIC endpoint for HTTP/3. The exact UDP bind
/// grant is checked before Quinn opens the socket.
#[allow(dead_code)] // Used by the compiler-owned listener ABI as it lands.
pub(crate) fn bind_server_endpoint(
    address: SocketAddr,
    grant: &EffectiveNetworkGrant,
    server_config: quinn::ServerConfig,
) -> Result<(Endpoint, SocketAddr), super::http_runtime::HttpRuntimeError> {
    grant
        .authorize_udp_bind(address)
        .map_err(|error| match error {
            zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                super::http_runtime::HttpRuntimeError::PermissionDenied
            }
            _ => super::http_runtime::HttpRuntimeError::InvalidRequest,
        })?;
    let endpoint = crate::network_future::block_on_current(
        crate::network_future::spawn_on_reactor(async move {
            Endpoint::server(server_config, address)
                .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)
        }),
    )
    .map_err(map_network_future_error)?
    .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)??;
    let local_address = endpoint
        .local_addr()
        .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?;
    Ok((endpoint, local_address))
}

/// Serves a finite HTTP/3 endpoint on its owner Zirk task. Accepted QUIC
/// connections receive separate Zirk tasks; a shared admission object caps
/// requests and in-flight body/handler resources across the listener.
#[allow(dead_code)] // Used by the compiler-owned listener ABI as it lands.
pub(crate) fn serve_endpoint(
    endpoint: Endpoint,
    max_requests: usize,
    max_connections: usize,
    handler: impl Fn(Request<Vec<u8>>) -> Http3ServerResponse + Clone + Send + 'static,
) -> Result<usize, super::http_runtime::HttpRuntimeError> {
    if max_requests == 0
        || max_requests > MAX_HTTP3_SERVER_REQUESTS
        || max_connections == 0
        || max_connections > MAX_HTTP3_SERVER_CONNECTIONS
    {
        endpoint.close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    let admission = Http3ServerAdmission::new(max_requests);
    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handshake_slots =
        std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_HTTP3_SERVER_HANDSHAKES));
    let mut accepted = 0;
    let mut connection_tasks = std::collections::VecDeque::new();
    while accepted < max_connections && !admission.is_exhausted() {
        let accept_endpoint = endpoint.clone();
        let request_limit = admission.subscribe_to_limit();
        let incoming =
            crate::network_future::block_on_current(crate::network_future::spawn_on_reactor(
                async move { accept_http3_connection(accept_endpoint, request_limit).await },
            ))
            .map_err(map_network_future_error)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)??;
        let Some(incoming) = incoming else { break };
        accepted += 1;
        let handler = handler.clone();
        let connection_admission = std::sync::Arc::clone(&admission);
        let connection_served = std::sync::Arc::clone(&served);
        let handshake_slots = std::sync::Arc::clone(&handshake_slots);
        let mut request_limit = admission.subscribe_to_limit();
        let task = crate::executor::spawn(move || {
            let handshake = crate::network_future::block_on_current(
                crate::network_future::spawn_on_reactor(async move {
                    if *request_limit.borrow() {
                        return None;
                    }
                    let _slot = tokio::select! {
                        slot = tokio::time::timeout(
                            HTTP3_SERVER_IO_TIMEOUT,
                            handshake_slots.acquire_owned(),
                        ) => match slot {
                            Ok(Ok(slot)) => slot,
                            _ => return None,
                        },
                        _ = request_limit.changed() => return None,
                    };
                    tokio::select! {
                        result = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, incoming) => {
                            Some(result)
                        }
                        _ = request_limit.changed() => None,
                    }
                }),
            );
            let connection = match handshake {
                Err(_error) => {
                    #[cfg(test)]
                    eprintln!("HTTP/3 test handshake task failed: {_error:?}");
                    return 0;
                }
                Ok(Err(_error)) => {
                    #[cfg(test)]
                    eprintln!("HTTP/3 test QUIC handshake timed out: {_error:?}");
                    return 0;
                }
                Ok(Ok(None)) => return 0,
                Ok(Ok(Some(Err(_error)))) => {
                    #[cfg(test)]
                    eprintln!("HTTP/3 test QUIC handshake rejected: {_error:?}");
                    return 0;
                }
                Ok(Ok(Some(Ok(Err(_error))))) => {
                    #[cfg(test)]
                    eprintln!("HTTP/3 test QUIC handshake rejected: {_error:?}");
                    return 0;
                }
                Ok(Ok(Some(Ok(Ok(connection))))) => connection,
            };
            match serve_connection(
                connection,
                MAX_HTTP3_REQUESTS_PER_CONNECTION,
                handler,
                connection_admission,
            ) {
                Ok(count) => {
                    connection_served.fetch_add(count, std::sync::atomic::Ordering::AcqRel);
                }
                Err(_error) => {
                    #[cfg(test)]
                    eprintln!("HTTP/3 test connection was rejected: {_error:?}");
                }
            }
            0
        });
        connection_tasks.push_back(task);
        crate::executor::yield_now();
    }

    while let Some(task) = connection_tasks.pop_front() {
        crate::http_runtime::await_server_handler_task(task);
    }
    endpoint.close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
    crate::network_future::block_on_current(crate::network_future::spawn_on_reactor(async move {
        endpoint.wait_idle().await
    }))
    .map_err(map_network_future_error)?
    .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?;
    Ok(served.load(std::sync::atomic::Ordering::Acquire))
}

async fn accept_http3_connection(
    endpoint: Endpoint,
    mut request_limit: tokio::sync::watch::Receiver<bool>,
) -> Result<Option<quinn::Incoming>, super::http_runtime::HttpRuntimeError> {
    if *request_limit.borrow() {
        return Ok(None);
    }
    tokio::select! {
        accepted = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, endpoint.accept()) => {
            accepted.map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)
        }
        _ = request_limit.changed() => Ok(None),
    }
}

/// Serves a bounded number of HTTP/3 request streams on an already
/// authenticated QUIC connection. Listener admission, permissions, and
/// Zirk handler invocation remain at the caller boundary.
#[allow(dead_code)] // Private adapter until the compiler-owned HTTP/3 server ABI lands.
fn serve_connection(
    connection: quinn::Connection,
    max_requests: usize,
    handler: impl Fn(Request<Vec<u8>>) -> Http3ServerResponse + Clone + Send + 'static,
    admission: std::sync::Arc<Http3ServerAdmission>,
) -> Result<usize, super::http_runtime::HttpRuntimeError> {
    if max_requests == 0 || max_requests > MAX_HTTP3_REQUESTS_PER_CONNECTION {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    let (request_tx, mut request_rx) =
        tokio::sync::mpsc::channel(MAX_HTTP3_SERVER_CONCURRENT_REQUESTS);
    let connection_admission = std::sync::Arc::clone(&admission);
    let server = crate::network_future::spawn_on_reactor(async move {
        serve_connection_inner(connection, max_requests, request_tx, connection_admission).await
    });
    let handler_failure = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mut active_tasks = std::collections::VecDeque::new();
    while let Some((request, response_tx)) =
        crate::network_future::block_on_current(request_rx.recv())
            .map_err(map_network_future_error)?
    {
        if active_tasks.len() >= MAX_HTTP3_SERVER_CONCURRENT_REQUESTS
            && let Some(task) = active_tasks.pop_front()
        {
            crate::http_runtime::await_server_handler_task(task);
        }
        let handler = handler.clone();
        let task_failure = std::sync::Arc::clone(&handler_failure);
        let task = crate::executor::spawn(move || {
            let mut response = match crate::http_runtime::run_callback_with_deadline(
                || handler(request),
                crate::http_runtime::DEFAULT_HTTP_SERVER_HANDLER_TIMEOUT,
            ) {
                Ok(response) => response,
                Err(error) => {
                    if error != super::http_runtime::HttpRuntimeError::Cancelled {
                        task_failure
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .get_or_insert(error);
                    }
                    return 0;
                }
            };
            let producer = response.producer.take();
            if response_tx.send(response.response).is_err() {
                return 0;
            }
            if let Some((producer, sender)) = producer
                && let Err(error) =
                    crate::http_runtime::pump_http_response_producer(producer, sender)
            {
                task_failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_or_insert(error);
            }
            1
        });
        active_tasks.push_back(task);
        crate::executor::yield_now();
    }
    while let Some(task) = active_tasks.pop_front() {
        crate::http_runtime::await_server_handler_task(task);
    }
    drop(request_rx);
    let served = crate::network_future::block_on_current(server)
        .map_err(map_network_future_error)?
        .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)??;
    if let Some(error) = handler_failure
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
    {
        return Err(error);
    }
    Ok(served)
}

type Http3ServerRequest = (
    Request<Vec<u8>>,
    tokio::sync::oneshot::Sender<Response<Http3ResponseBody>>,
);

#[allow(dead_code)] // Exercised by the local QUIC/H3 integration test.
async fn serve_connection_inner(
    connection: quinn::Connection,
    max_requests: usize,
    request_tx: tokio::sync::mpsc::Sender<Http3ServerRequest>,
    admission: std::sync::Arc<Http3ServerAdmission>,
) -> Result<usize, super::http_runtime::HttpRuntimeError> {
    let mut builder = h3::server::builder();
    builder.max_field_section_size(MAX_HTTP_HEADER_BYTES as u64);
    // RFC 9220 WebSocket sessions use extended CONNECT. The regular HTTP
    // request path still applies its own method and body limits; this setting
    // only advertises the protocol capability to the peer.
    builder.enable_extended_connect(true);
    let mut server = builder
        .build::<_, Bytes>(h3_quinn::Connection::new(connection))
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    let mut served = 0;
    let mut accepted = 0;
    let mut streams = tokio::task::JoinSet::new();
    let mut request_limit = admission.subscribe_to_limit();

    while accepted < max_requests && !admission.is_exhausted() {
        tokio::select! {
            accepted_stream = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, server.accept()),
                if streams.len() < MAX_HTTP3_SERVER_CONCURRENT_REQUESTS && !admission.is_exhausted() => {
                let resolver = accepted_stream
                    .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
                    .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
                let Some(resolver) = resolver else { break };
                accepted += 1;
                let request_tx = request_tx.clone();
                let admission = std::sync::Arc::clone(&admission);
                streams.spawn(async move {
                    serve_h3_request_stream(resolver, request_tx, admission).await
                });
            }
            completed = streams.join_next(), if !streams.is_empty() => {
                match completed {
                    Some(Ok(Ok(true))) => served += 1,
                    Some(Ok(Ok(false))) => {},
                    Some(Ok(Err(_))) => {},
                    Some(Err(_)) => return Err(super::http_runtime::HttpRuntimeError::Protocol),
                    None => {},
                }
            }
            _ = request_limit.changed() => {
                if *request_limit.borrow() {
                    break;
                }
            }
        }
    }
    while let Some(completed) = streams.join_next().await {
        match completed {
            Ok(Ok(true)) => served += 1,
            Ok(Ok(false)) => {}
            Ok(Err(_)) => {}
            Err(_) => return Err(super::http_runtime::HttpRuntimeError::Protocol),
        }
    }
    let _ = server.shutdown(0).await;
    // Keep the h3 connection driver alive until the peer has consumed the
    // response and closes its connection. Dropping `server` here closes QUIC
    // immediately and can discard response DATA that is still in flight.
    let _ = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, server.accept()).await;
    Ok(served)
}

async fn serve_h3_request_stream(
    resolver: h3::server::RequestResolver<h3_quinn::Connection, Bytes>,
    request_tx: tokio::sync::mpsc::Sender<Http3ServerRequest>,
    admission: std::sync::Arc<Http3ServerAdmission>,
) -> Result<bool, super::http_runtime::HttpRuntimeError> {
    let (request, mut stream) =
        tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, resolver.resolve_request())
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    if request.headers().len() > MAX_HTTP_HEADER_COUNT {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    if admission.is_exhausted() {
        reject_h3_request_stream(&mut stream).await?;
        return Ok(false);
    }
    let mut request_limit = admission.subscribe_to_limit();
    let _active_request = tokio::select! {
        permit = tokio::time::timeout(
            HTTP3_SERVER_IO_TIMEOUT,
            std::sync::Arc::clone(&admission.active_requests).acquire_owned(),
        ) => permit
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Cancelled)?,
        _ = request_limit.changed() => {
            if admission.is_exhausted() {
                reject_h3_request_stream(&mut stream).await?;
                return Ok(false);
            }
            return Err(super::http_runtime::HttpRuntimeError::Cancelled);
        }
    };
    if admission.is_exhausted() {
        reject_h3_request_stream(&mut stream).await?;
        return Ok(false);
    }

    let (parts, ()) = request.into_parts();
    let mut body = Vec::new();
    loop {
        let chunk = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, stream.recv_data())
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
        let Some(mut chunk) = chunk else { break };
        let size = chunk.remaining();
        if body
            .len()
            .checked_add(size)
            .is_none_or(|total| total > MAX_HTTP_BUFFERED_BODY_BYTES)
        {
            return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk.copy_to_bytes(size));
    }

    if !admission.admit_request() {
        reject_h3_request_stream(&mut stream).await?;
        return Ok(false);
    }

    let mut incoming = Request::from_parts(parts, body);
    *incoming.version_mut() = Version::HTTP_3;
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    request_tx
        .send((incoming, response_tx))
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Cancelled)?;
    let outgoing = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, response_rx)
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
        .map_err(|_| super::http_runtime::HttpRuntimeError::Cancelled)?;
    if outgoing.headers().len() > MAX_HTTP_HEADER_COUNT
        || outgoing
            .headers()
            .iter()
            .try_fold(0_usize, |total, (name, value)| {
                total
                    .checked_add(name.as_str().len())
                    .and_then(|size| size.checked_add(value.as_bytes().len()))
            })
            .is_none_or(|size| size > MAX_HTTP_HEADER_BYTES)
    {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    let (parts, body) = outgoing.into_parts();
    let mut response = Response::from_parts(parts, ());
    *response.version_mut() = Version::HTTP_3;
    stream
        .send_response(response)
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    match body {
        Http3ResponseBody::Buffered(body) => {
            if body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
                return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
            }
            if !body.is_empty() {
                stream
                    .send_data(Bytes::from(body))
                    .await
                    .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
            }
        }
        Http3ResponseBody::Stream(mut receiver) => loop {
            let chunk = tokio::time::timeout(HTTP3_SERVER_IO_TIMEOUT, receiver.recv())
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?;
            match chunk {
                Some(Ok(chunk)) => {
                    if chunk.len() > crate::http_api::MAX_HTTP_SERVER_PRODUCER_CHUNK_BYTES {
                        return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
                    }
                    stream
                        .send_data(chunk)
                        .await
                        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
                }
                Some(Err(_)) => return Err(super::http_runtime::HttpRuntimeError::Transport),
                None => break,
            }
        },
    }
    stream
        .finish()
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    Ok(true)
}

async fn reject_h3_request_stream<S>(
    stream: &mut h3::server::RequestStream<S, Bytes>,
) -> Result<(), super::http_runtime::HttpRuntimeError>
where
    S: h3::quic::SendStream<Bytes>,
{
    let response = Response::builder()
        .status(503)
        .body(())
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    stream
        .send_response(response)
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    stream
        .send_data(Bytes::from_static(b"HTTP/3 request limit reached"))
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    stream
        .finish()
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)
}

fn validate_request(
    request: &Request<Vec<u8>>,
) -> Result<(), super::http_runtime::HttpRuntimeError> {
    if request.body().len() > MAX_HTTP_BUFFERED_BODY_BYTES {
        return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
    }
    validate_request_headers(request.headers())
}

fn validate_request_headers(
    headers: &HeaderMap,
) -> Result<(), super::http_runtime::HttpRuntimeError> {
    if headers.len() > MAX_HTTP_HEADER_COUNT {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    let total = headers.iter().try_fold(0_usize, |total, (name, value)| {
        total
            .checked_add(name.as_str().len())
            .and_then(|size| size.checked_add(value.as_bytes().len()))
    });
    if total.is_none_or(|size| size > MAX_HTTP_HEADER_BYTES) {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    if headers.contains_key(http::header::HOST) {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    Ok(())
}

async fn connect_quic_endpoint(
    origin: &NetworkOrigin,
    grant: Arc<EffectiveNetworkGrant>,
    connect_timeout: Duration,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
) -> Result<(Endpoint, quinn::Connection), super::http_runtime::HttpRuntimeError> {
    let host = origin
        .host()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let candidates = crate::network_transport::resolve_http3_candidates(
        origin.clone(),
        Arc::clone(&grant),
        connect_timeout,
    )
    .await?;
    let tls = prepare_http3_tls(tls_config)?;
    let crypto = QuicClientConfig::try_from((*tls).clone())
        .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?;
    let mut quic_config = ClientConfig::new(Arc::new(crypto));
    let mut transport = TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(VarInt::from_u32(MAX_QUIC_STREAMS))
        .max_concurrent_uni_streams(VarInt::from_u32(MAX_QUIC_STREAMS))
        .stream_receive_window(
            VarInt::from_u64(QUIC_STREAM_WINDOW_BYTES)
                .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?,
        )
        .receive_window(
            VarInt::from_u64(QUIC_CONNECTION_WINDOW_BYTES)
                .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?,
        );
    quic_config.transport_config(Arc::new(transport));

    tokio::time::timeout(connect_timeout, async {
        let mut last_error = super::http_runtime::HttpRuntimeError::Transport;
        for candidate in candidates {
            let local = unspecified_address(candidate);
            let mut endpoint = Endpoint::client(local)
                .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?;
            endpoint.set_default_client_config(quic_config.clone());
            let connecting = match endpoint.connect(candidate, &host) {
                Ok(connecting) => connecting,
                Err(_) => {
                    last_error = super::http_runtime::HttpRuntimeError::Transport;
                    continue;
                }
            };
            match connecting.await {
                Ok(connection) => return Ok((endpoint, connection)),
                Err(_) => last_error = super::http_runtime::HttpRuntimeError::Transport,
            }
        }
        Err(last_error)
    })
    .await
    .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
}

#[allow(clippy::too_many_arguments)]
async fn perform_request(
    session: Arc<Http3ClientSession>,
    mut sender: h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
    url_text: String,
    origin: NetworkOrigin,
    method: http::Method,
    headers: HeaderMap,
    body: Vec<u8>,
    mut upload_body: Option<crate::http_runtime::HttpUploadBody>,
    grant: Arc<EffectiveNetworkGrant>,
    response_headers_timeout: Duration,
    response_idle_timeout: Duration,
) -> Result<Response<Vec<u8>>, super::http_runtime::HttpRuntimeError> {
    let connection = &session.connection;
    authorize_remote_path(connection, &origin, &grant)?;

    let mut outgoing = Request::builder()
        .method(method)
        .uri(&url_text)
        .version(Version::HTTP_3)
        .body(())
        .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
    *outgoing.headers_mut() = headers;
    authorize_remote_path(connection, &origin, &grant)?;
    let mut stream = sender.send_request(outgoing).await.map_err(|_error| {
        #[cfg(test)]
        eprintln!("HTTP/3 test request send failed: {_error:?}");
        super::http_runtime::HttpRuntimeError::Protocol
    })?;
    authorize_remote_path(connection, &origin, &grant)?;
    if let Some(upload_body) = upload_body.as_mut() {
        while let Some(chunk) = upload_body.next_chunk().await {
            authorize_remote_path(connection, &origin, &grant)?;
            stream
                .send_data(chunk)
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
        }
    } else if !body.is_empty() {
        authorize_remote_path(connection, &origin, &grant)?;
        stream
            .send_data(Bytes::from(body))
            .await
            .map_err(|_error| {
                #[cfg(test)]
                eprintln!("HTTP/3 test request body send failed: {_error:?}");
                super::http_runtime::HttpRuntimeError::Protocol
            })?;
        authorize_remote_path(connection, &origin, &grant)?;
    }
    stream.finish().await.map_err(|_error| {
        #[cfg(test)]
        eprintln!("HTTP/3 test request finish failed: {_error:?}");
        super::http_runtime::HttpRuntimeError::Protocol
    })?;
    authorize_remote_path(connection, &origin, &grant)?;

    let response = tokio::time::timeout(response_headers_timeout, stream.recv_response())
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
        .map_err(|_error| {
            #[cfg(test)]
            eprintln!("HTTP/3 test response headers failed: {_error:?}");
            super::http_runtime::HttpRuntimeError::Protocol
        })?;
    authorize_remote_path(connection, &origin, &grant)?;
    let (parts, ()) = response.into_parts();
    let mut body = Vec::new();
    loop {
        let next = tokio::time::timeout(response_idle_timeout, stream.recv_data())
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_error| {
                #[cfg(test)]
                eprintln!("HTTP/3 test response data failed: {_error:?}");
                super::http_runtime::HttpRuntimeError::Protocol
            })?;
        authorize_remote_path(connection, &origin, &grant)?;
        let Some(mut chunk) = next else { break };
        let size = chunk.remaining();
        if body
            .len()
            .checked_add(size)
            .is_none_or(|length| length > MAX_HTTP_BUFFERED_BODY_BYTES)
        {
            connection.close(VarInt::from_u32(0), b"response limit exceeded");
            return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk.copy_to_bytes(size));
    }

    let mut response = Response::builder()
        .status(parts.status)
        .version(Version::HTTP_3)
        .body(body)
        .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
    *response.headers_mut() = parts.headers;

    Ok(response)
}

/// Establishes an RFC 9220 extended CONNECT request and adapts its bounded
/// HTTP/3 DATA stream to an async byte stream for the existing WebSocket codec.
pub(crate) fn connect_websocket_transport(
    url: &Url,
    grant: Arc<EffectiveNetworkGrant>,
    connect_timeout: Duration,
    response_headers_timeout: Duration,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    origin_header: Option<http::HeaderValue>,
    subprotocols: &[String],
) -> Result<
    (
        tokio::io::DuplexStream,
        Option<String>,
        tokio::task::AbortHandle,
    ),
    super::http_runtime::HttpRuntimeError,
> {
    if url.scheme() != "https" {
        return Err(super::http_runtime::HttpRuntimeError::InvalidRequest);
    }
    let origin = NetworkOrigin::parse(&url.origin())
        .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
    grant
        .authorize_origin(origin.as_str())
        .map_err(NetworkTransportError::from)?;
    let url = url.as_str().to_owned();
    let subprotocols = subprotocols.to_vec();
    let grant_for_reactor = Arc::clone(&grant);
    crate::network_future::block_on_current(crate::network_future::spawn_on_reactor(async move {
        let (endpoint, connection) = connect_quic_endpoint(
            &origin,
            Arc::clone(&grant_for_reactor),
            connect_timeout,
            tls_config,
        )
        .await?;
        authorize_remote_path(&connection, &origin, &grant_for_reactor)?;

        let mut builder = h3::client::builder();
        builder
            .max_field_section_size(MAX_HTTP_HEADER_BYTES as u64)
            .enable_extended_connect(true);
        let (mut driver, mut sender) = builder
            .build::<_, _, Bytes>(h3_quinn::Connection::new(connection.clone()))
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
        let sender_keepalive = sender.clone();
        authorize_remote_path(&connection, &origin, &grant_for_reactor)?;
        let driver_task = tokio::spawn(async move {
            let _sender_keepalive = sender_keepalive;
            let _ = driver.wait_idle().await;
            endpoint.wait_idle().await
        });
        let driver_abort = driver_task.abort_handle();

        let mut request = Request::builder()
            .method(http::Method::CONNECT)
            .uri(&url)
            .version(Version::HTTP_3)
            .header(http::header::SEC_WEBSOCKET_VERSION, "13")
            .body(())
            .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        request
            .extensions_mut()
            .insert(h3::ext::Protocol::WEBSOCKET);
        if let Some(origin_header) = origin_header {
            request
                .headers_mut()
                .insert(http::header::ORIGIN, origin_header);
        }
        if !subprotocols.is_empty() {
            let value = http::HeaderValue::from_str(&subprotocols.join(", "))
                .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
            request
                .headers_mut()
                .insert(http::header::SEC_WEBSOCKET_PROTOCOL, value);
        }
        validate_request_headers(request.headers())?;
        authorize_remote_path(&connection, &origin, &grant_for_reactor)?;
        let mut stream = tokio::time::timeout(response_headers_timeout, async {
            sender
                .send_request(request)
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)
        })
        .await
        .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)??;
        authorize_remote_path(&connection, &origin, &grant_for_reactor)?;
        let response = tokio::time::timeout(response_headers_timeout, stream.recv_response())
            .await
            .map_err(|_| super::http_runtime::HttpRuntimeError::Timeout)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
        authorize_remote_path(&connection, &origin, &grant_for_reactor)?;
        if response.status() != http::StatusCode::OK
            || response
                .headers()
                .contains_key(http::header::SEC_WEBSOCKET_EXTENSIONS)
        {
            connection.close(VarInt::from_u32(0), b"websocket handshake rejected");
            return Err(super::http_runtime::HttpRuntimeError::Protocol);
        }
        let selected =
            super::http_runtime::websocket_selected_subprotocol(response.headers(), &subprotocols)?;
        let (mut send, mut receive) = stream.split();
        let (websocket, transport) = tokio::io::duplex(64 * 1024);
        let (mut websocket_read, mut websocket_write) = tokio::io::split(transport);
        let send_connection = connection.clone();
        let send_origin = origin.clone();
        let send_grant = Arc::clone(&grant_for_reactor);
        tokio::spawn(async move {
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let count =
                    match tokio::io::AsyncReadExt::read(&mut websocket_read, &mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                if authorize_remote_path(&send_connection, &send_origin, &send_grant).is_err()
                    || send
                        .send_data(Bytes::copy_from_slice(&buffer[..count]))
                        .await
                        .is_err()
                {
                    break;
                }
            }
            let _ = send.finish().await;
        });
        let receive_connection = connection.clone();
        let receive_grant = Arc::clone(&grant_for_reactor);
        tokio::spawn(async move {
            loop {
                if authorize_remote_path(&receive_connection, &origin, &receive_grant).is_err() {
                    break;
                }
                let chunk = match receive.recv_data().await {
                    Ok(Some(mut chunk)) => {
                        let size = chunk.remaining();
                        chunk.copy_to_bytes(size)
                    }
                    Ok(None) | Err(_) => break,
                };
                if tokio::io::AsyncWriteExt::write_all(&mut websocket_write, &chunk)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            receive.stop_sending(h3::error::Code::H3_NO_ERROR);
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut websocket_write).await;
        });
        Ok((websocket, selected, driver_abort))
    }))
    .map_err(map_network_future_error)?
    .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?
}

fn prepare_http3_tls(
    config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
) -> Result<Arc<crate::network_transport::RustlsClientConfig>, super::http_runtime::HttpRuntimeError>
{
    let mut tls = match config {
        Some(config) => (*config).clone(),
        None => (*client_tls_config_for_http3()?).clone(),
    };
    // QUIC requires TLS 1.3; Quinn's rustls adapter rejects any other version.
    // Keep the application protocol fixed to the HTTP/3 ALPN even if a custom
    // TCP/TLS client configuration also advertises HTTP/1.1 or HTTP/2.
    tls.alpn_protocols = vec![b"h3".to_vec()];
    tls.enable_early_data = false;
    Ok(Arc::new(tls))
}

fn authorize_remote_path(
    connection: &quinn::Connection,
    origin: &NetworkOrigin,
    grant: &EffectiveNetworkGrant,
) -> Result<(), super::http_runtime::HttpRuntimeError> {
    grant
        .authorize_connect(origin.as_str(), connection.remote_address())
        .map_err(NetworkTransportError::from)?;
    Ok(())
}

fn unspecified_address(candidate: SocketAddr) -> SocketAddr {
    match candidate.ip() {
        IpAddr::V4(_) => SocketAddr::from(([0, 0, 0, 0], 0)),
        IpAddr::V6(_) => SocketAddr::from(([0_u16; 8], 0)),
    }
}

struct AbortTask(JoinHandle<()>);

impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Reusable, origin-scoped HTTP/3 connection owned by an HTTP client pool.
/// The request sender is cloned for each stream, so concurrent Zirk tasks can
/// multiplex requests without serializing their bodies or responses.
pub(crate) struct Http3ClientSession {
    endpoint: Endpoint,
    connection: quinn::Connection,
    sender: std::sync::Mutex<h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>>,
    _driver_task: AbortTask,
}

impl Drop for Http3ClientSession {
    fn drop(&mut self) {
        self.connection
            .close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
        self.endpoint.close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
    }
}

impl Http3ClientSession {
    pub(crate) fn is_closed(&self) -> bool {
        self.connection.close_reason().is_some()
    }

    pub(crate) fn connect(
        origin: NetworkOrigin,
        grant: Arc<EffectiveNetworkGrant>,
        connect_timeout: Duration,
        tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    ) -> Result<Arc<Self>, super::http_runtime::HttpRuntimeError> {
        let session = crate::network_future::spawn_on_reactor(async move {
            let (endpoint, connection) =
                connect_quic_endpoint(&origin, Arc::clone(&grant), connect_timeout, tls_config)
                    .await?;
            authorize_remote_path(&connection, &origin, &grant)?;
            let h3_connection = h3_quinn::Connection::new(connection.clone());
            let (mut driver, sender) = h3::client::new(h3_connection)
                .await
                .map_err(|_| super::http_runtime::HttpRuntimeError::Protocol)?;
            let driver_task = tokio::spawn(async move {
                let _ = driver.wait_idle().await;
            });
            Ok::<_, super::http_runtime::HttpRuntimeError>((
                endpoint,
                connection,
                sender,
                AbortTask(driver_task),
            ))
        });
        let (endpoint, connection, sender, driver_task) =
            crate::network_future::block_on_current(session)
                .map_err(map_network_future_error)?
                .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)??;
        Ok(Arc::new(Self {
            endpoint,
            connection,
            sender: std::sync::Mutex::new(sender),
            _driver_task: driver_task,
        }))
    }

    pub(crate) fn request(
        self: &Arc<Self>,
        request: Request<Vec<u8>>,
        grant: Arc<EffectiveNetworkGrant>,
        response_headers_timeout: Duration,
        response_idle_timeout: Duration,
    ) -> Result<Response<Vec<u8>>, super::http_runtime::HttpRuntimeError> {
        let url = Url::parse(&request.uri().to_string())
            .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        if url.scheme() != "https" {
            return Err(super::http_runtime::HttpRuntimeError::Transport);
        }
        let origin = NetworkOrigin::parse(&url.origin())
            .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        grant
            .authorize_origin(origin.as_str())
            .map_err(NetworkTransportError::from)?;
        authorize_remote_path(&self.connection, &origin, &grant)?;
        validate_request(&request)?;

        let sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let url_text = url.as_str().to_owned();
        let method = request.method().clone();
        let headers = request.headers().clone();
        let body = request.into_body();
        let session = Arc::clone(self);
        let future = crate::network_future::spawn_on_reactor(async move {
            perform_request(
                session,
                sender,
                url_text,
                origin,
                method,
                headers,
                body,
                None,
                grant,
                response_headers_timeout,
                response_idle_timeout,
            )
            .await
        });
        crate::network_future::block_on_current(future)
            .map_err(map_network_future_error)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?
    }

    pub(crate) fn request_streaming(
        self: &Arc<Self>,
        request: Request<()>,
        grant: Arc<EffectiveNetworkGrant>,
        response_headers_timeout: Duration,
        response_idle_timeout: Duration,
        producer: &mut impl FnMut() -> Result<Option<Vec<u8>>, super::http_runtime::HttpRuntimeError>,
    ) -> Result<Response<Vec<u8>>, super::http_runtime::HttpRuntimeError> {
        let url = Url::parse(&request.uri().to_string())
            .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        if url.scheme() != "https" {
            return Err(super::http_runtime::HttpRuntimeError::Transport);
        }
        let origin = NetworkOrigin::parse(&url.origin())
            .map_err(|_| super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        grant
            .authorize_origin(origin.as_str())
            .map_err(NetworkTransportError::from)?;
        authorize_remote_path(&self.connection, &origin, &grant)?;
        validate_request_headers(request.headers())?;

        let first_chunk = producer()?;
        if crate::executor::current_task_cancel_requested() {
            return Err(super::http_runtime::HttpRuntimeError::Cancelled);
        }
        if first_chunk
            .as_ref()
            .is_some_and(|chunk| chunk.len() > super::http_runtime::HTTP_UPLOAD_CHUNK_LIMIT)
        {
            return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
        }

        let sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let url_text = url.as_str().to_owned();
        let method = request.method().clone();
        let headers = request.headers().clone();
        let owner = crate::executor::current_task();
        let (mut upload, body) = crate::http_runtime::HttpUploadBody::channel(owner);
        let session = Arc::clone(self);
        let future = crate::network_future::spawn_on_reactor(async move {
            perform_request(
                session,
                sender,
                url_text,
                origin,
                method,
                headers,
                Vec::new(),
                Some(body),
                grant,
                response_headers_timeout,
                response_idle_timeout,
            )
            .await
        });

        let mut next_chunk = first_chunk;
        loop {
            let chunk = match next_chunk.take() {
                Some(chunk) => chunk,
                None => {
                    let chunk = producer()?;
                    if crate::executor::current_task_cancel_requested() {
                        drop(future);
                        return Err(super::http_runtime::HttpRuntimeError::Cancelled);
                    }
                    match chunk {
                        Some(chunk) => chunk,
                        None => break,
                    }
                }
            };
            if chunk.is_empty() {
                break;
            }
            if chunk.len() > super::http_runtime::HTTP_UPLOAD_CHUNK_LIMIT {
                drop(future);
                return Err(super::http_runtime::HttpRuntimeError::BodyTooLarge);
            }
            if let Err(error) = upload.write_chunk(&chunk) {
                drop(future);
                return Err(error);
            }
        }
        upload.finish()?;
        crate::network_future::block_on_current(future)
            .map_err(map_network_future_error)?
            .map_err(|_| super::http_runtime::HttpRuntimeError::Transport)?
    }
}

fn map_network_future_error(
    error: crate::network_future::NetworkFutureError,
) -> super::http_runtime::HttpRuntimeError {
    match error {
        crate::network_future::NetworkFutureError::Cancelled => {
            super::http_runtime::HttpRuntimeError::Cancelled
        }
        crate::network_future::NetworkFutureError::CapacityExceeded => {
            super::http_runtime::HttpRuntimeError::Transport
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use zirk_runtime_core::network_policy::EffectiveNetworkGrant;

    fn test_http3_server_config() -> quinn::ServerConfig {
        crate::network_transport::TlsServerConfig::from_der(
            vec![include_bytes!("../testdata/tls-loopback-cert.der").to_vec()],
            include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
            false,
        )
        .unwrap()
        .into_http3_quic_config()
        .unwrap()
    }

    async fn test_http3_get(
        origin: NetworkOrigin,
        tls: Arc<crate::network_transport::RustlsClientConfig>,
        path: &str,
    ) -> Result<Vec<u8>, super::super::http_runtime::HttpRuntimeError> {
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.as_str().to_owned()], [])
                .map_err(|_| super::super::http_runtime::HttpRuntimeError::PermissionDenied)?,
        );
        let (endpoint, connection) =
            connect_quic_endpoint(&origin, grant, Duration::from_secs(5), Some(tls)).await?;
        let (mut driver, mut sender) =
            h3::client::new(h3_quinn::Connection::new(connection.clone()))
                .await
                .map_err(|_| super::super::http_runtime::HttpRuntimeError::Protocol)?;
        let driver_task = tokio::spawn(async move { driver.wait_idle().await });
        let request = Request::builder()
            .method(http::Method::GET)
            .uri(format!("{}{}", origin.as_str(), path))
            .version(Version::HTTP_3)
            .body(())
            .map_err(|_| super::super::http_runtime::HttpRuntimeError::InvalidRequest)?;
        let mut stream = sender
            .send_request(request)
            .await
            .map_err(|_| super::super::http_runtime::HttpRuntimeError::Protocol)?;
        stream
            .finish()
            .await
            .map_err(|_| super::super::http_runtime::HttpRuntimeError::Protocol)?;
        let response = stream
            .recv_response()
            .await
            .map_err(|_| super::super::http_runtime::HttpRuntimeError::Protocol)?;
        if response.status() != http::StatusCode::OK {
            return Err(super::super::http_runtime::HttpRuntimeError::Protocol);
        }
        let mut body = Vec::new();
        while let Some(mut chunk) = stream
            .recv_data()
            .await
            .map_err(|_| super::super::http_runtime::HttpRuntimeError::Protocol)?
        {
            let size = chunk.remaining();
            body.extend_from_slice(&chunk.copy_to_bytes(size));
        }
        connection.close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
        endpoint.close(VarInt::from_u32(0), HTTP3_CLOSE_REASON);
        let _ = tokio::time::timeout(Duration::from_secs(2), driver_task).await;
        Ok(body)
    }

    unsafe extern "C" fn streaming_test_producer(
        context: *mut std::ffi::c_void,
        chunk_index: u64,
        out: *mut *mut std::ffi::c_void,
    ) -> i32 {
        let context = unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
        assert!(crate::collector::is_live_for_tests(
            context.captures as *mut std::ffi::c_void
        ));
        let bytes: &[u8] = match chunk_index {
            0 => b"accepted:",
            1 => b"ping",
            _ => b"",
        };
        let value = crate::array::alloc_byte_array(bytes);
        if !out.is_null() {
            unsafe { out.write(value) };
        }
        0
    }

    #[test]
    fn http3_tls_forces_h3_alpn_and_disables_early_data() {
        let mut config =
            (*crate::network_transport::client_tls_config_for_http3().unwrap()).clone();
        config.alpn_protocols = vec![b"http/1.1".to_vec(), b"h2".to_vec()];
        config.enable_early_data = true;

        let config = prepare_http3_tls(Some(Arc::new(config))).unwrap();

        assert_eq!(config.alpn_protocols, [b"h3".to_vec()]);
        assert!(!config.enable_early_data);
    }

    #[test]
    fn http3_engine_exposes_the_rfc_9220_websocket_protocol_token() {
        use std::str::FromStr;

        let protocol = h3::ext::Protocol::from_str("websocket")
            .unwrap_or_else(|_| panic!("RFC 9220 Extended CONNECT protocol is supported"));
        assert_eq!(protocol, h3::ext::Protocol::WEBSOCKET);
        assert_eq!(protocol.as_str(), "websocket");
    }

    #[test]
    fn http3_websocket_uses_extended_connect_and_bidirectional_data_stream() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let address = reservation.local_addr().unwrap();
            drop(reservation);
            let server = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let endpoint = Endpoint::server(test_http3_server_config(), address).unwrap();
                    let incoming = endpoint.accept().await.unwrap();
                    let connection = incoming.await.unwrap();
                    let mut builder = h3::server::builder();
                    builder
                        .max_field_section_size(MAX_HTTP_HEADER_BYTES as u64)
                        .enable_extended_connect(true);
                    let mut server = builder
                        .build::<_, Bytes>(h3_quinn::Connection::new(connection))
                        .await
                        .unwrap();
                    let resolver = server.accept().await.unwrap().unwrap();
                    let origin = format!("https://{address}");
                    let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
                        &[&origin],
                        false,
                        &["echo.v1"],
                    )
                    .unwrap();
                    let (mut websocket_io, selected, driver) =
                        accept_websocket_request(resolver, "/socket", policy)
                            .await
                            .unwrap()
                            .unwrap();
                    assert_eq!(selected.as_deref(), Some("echo.v1"));
                    let mut frame = vec![0; 11];
                    tokio::io::AsyncReadExt::read_exact(&mut websocket_io, &mut frame)
                        .await
                        .unwrap();
                    assert_eq!(frame.len(), 11);
                    assert_eq!(frame[0], 0x81);
                    assert_eq!(frame[1], 0x85);
                    let mask = &frame[2..6];
                    let decoded = frame[6..]
                        .iter()
                        .enumerate()
                        .map(|(index, byte)| byte ^ mask[index % 4])
                        .collect::<Vec<_>>();
                    assert_eq!(decoded, b"hello");
                    tokio::io::AsyncWriteExt::write_all(&mut websocket_io, b"\x81\x05reply")
                        .await
                        .unwrap();
                    drop(websocket_io);
                    driver.abort();
                    let _ = tokio::time::timeout(Duration::from_secs(5), server.accept()).await;
                    Ok::<_, super::super::http_runtime::HttpRuntimeError>(())
                })
            });

            let origin = format!("https://{address}");
            let url = Url::parse(&format!("{origin}/socket")).unwrap();
            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap(),
            );
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let offered = vec!["echo.v1".to_owned()];
            let mut websocket = crate::http_runtime::HttpWebSocket::connect_http3_with_options(
                &url,
                grant,
                crate::http_runtime::HttpTimeouts {
                    connect: Duration::from_secs(5),
                    response_headers: Duration::from_secs(5),
                    response_idle: Duration::from_secs(5),
                },
                Some(&origin),
                &offered,
                Some(tls),
            )
            .unwrap();
            assert_eq!(websocket.negotiated_subprotocol(), Some("echo.v1"));
            websocket
                .send(crate::http_runtime::HttpWebSocketMessage::Text(
                    "hello".to_owned(),
                ))
                .unwrap();
            assert_eq!(
                websocket.receive(Duration::from_secs(5)).unwrap(),
                Some(crate::http_runtime::HttpWebSocketMessage::Text(
                    "reply".to_owned()
                ))
            );
            drop(websocket);
            server.join().unwrap().unwrap();
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http3_websocket_endpoint_owns_server_codec_and_cancellation_lifecycle() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let address = reservation.local_addr().unwrap();
            drop(reservation);
            let grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                [address.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap();
            let (endpoint, _) =
                bind_server_endpoint(address, &grant, test_http3_server_config()).unwrap();
            let origin = format!("https://{address}");
            let url = Url::parse(&format!("{origin}/socket")).unwrap();
            let client_grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap(),
            );
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let offered = vec!["echo.v1".to_owned()];
            let client_origin = origin.clone();
            let client = crate::executor::spawn(move || {
                let mut socket = crate::http_runtime::HttpWebSocket::connect_http3_with_options(
                    &url,
                    client_grant,
                    crate::http_runtime::HttpTimeouts {
                        connect: Duration::from_secs(5),
                        response_headers: Duration::from_secs(5),
                        response_idle: Duration::from_secs(5),
                    },
                    Some(&client_origin),
                    &offered,
                    Some(tls),
                )
                .unwrap();
                assert_eq!(socket.negotiated_subprotocol(), Some("echo.v1"));
                socket
                    .send(crate::http_runtime::HttpWebSocketMessage::Text(
                        "ping".to_owned(),
                    ))
                    .unwrap();
                assert_eq!(
                    socket.receive(Duration::from_secs(5)).unwrap(),
                    Some(crate::http_runtime::HttpWebSocketMessage::Text(
                        "pong".to_owned()
                    ))
                );
                socket.close().unwrap();
                0
            });

            let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
                &[&origin],
                false,
                &["echo.v1"],
            )
            .unwrap();
            let (io, selected, driver, close_drain) =
                accept_websocket_endpoint(endpoint, "/socket".to_owned(), policy).unwrap();
            let mut socket = crate::http_runtime::HttpWebSocket::from_http3_server_stream(
                io,
                selected,
                Duration::from_secs(5),
                8,
                driver,
                close_drain,
            )
            .unwrap();
            assert_eq!(
                socket.receive(Duration::from_secs(5)).unwrap(),
                Some(crate::http_runtime::HttpWebSocketMessage::Text(
                    "ping".to_owned()
                ))
            );
            socket
                .send(crate::http_runtime::HttpWebSocketMessage::Text(
                    "pong".to_owned(),
                ))
                .unwrap();
            assert!(matches!(
                socket.receive(Duration::from_secs(5)).unwrap(),
                Some(crate::http_runtime::HttpWebSocketMessage::Close { .. })
            ));
            socket.close().unwrap();
            crate::executor::await_task(client);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn auto_http3_streams_a_bounded_request_body_and_returns_a_bounded_response() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let requested_address = reservation.local_addr().unwrap();
            drop(reservation);
            let listen_grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                [requested_address.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap();
            let (endpoint, address) =
                bind_server_endpoint(requested_address, &listen_grant, test_http3_server_config())
                    .unwrap();
            let server_task = crate::executor::spawn(move || {
                let owner_thread = std::thread::current().id();
                let owner_task = crate::executor::current_task();
                let owner_executor = crate::executor::current_executor_id();
                let served = serve_endpoint(endpoint, 1, 2, move |request| {
                    assert_eq!(std::thread::current().id(), owner_thread);
                    assert_ne!(crate::executor::current_task(), owner_task);
                    assert_eq!(crate::executor::current_executor_id(), owner_executor);
                    assert_eq!(request.method(), http::Method::POST);
                    assert_eq!(request.body(), b"ping");
                    assert_eq!(
                        request.headers().get(http::header::USER_AGENT),
                        Some(&http::HeaderValue::from_static("Zirk-http-client/0.0.0"))
                    );
                    assert_eq!(
                        request.headers().get(http::header::ACCEPT_ENCODING),
                        Some(&http::HeaderValue::from_static("br, gzip, deflate"))
                    );
                    let capture = crate::string::alloc_owned("http3 response producer");
                    let context = crate::http_api::HttpServerProducerContext {
                        function: 0,
                        captures: capture as usize,
                        dispatch: 0,
                    };
                    let handle = unsafe {
                        crate::http_api::zirk_http_server_producer_register(
                            Some(streaming_test_producer),
                            &context,
                            capture,
                        )
                    };
                    assert_ne!(handle, 0);
                    crate::collector::collect_for_tests();
                    let producer = crate::http_api::take_http_server_producer(handle).unwrap();
                    let (sender, receiver) = tokio::sync::mpsc::channel(1);
                    Http3ServerResponse {
                        response: http::Response::builder()
                            .status(201)
                            .header("x-protocol", "h3")
                            .body(Http3ResponseBody::Stream(receiver))
                            .unwrap(),
                        producer: Some((producer, sender)),
                    }
                })
                .unwrap();
                assert_eq!(served, 1);
                served
            });

            let origin = format!("https://{}", address);
            // A peer can complete QUIC and disconnect without sending the H3
            // control stream. The listener must isolate it and continue
            // accepting the valid request within the configured bound.
            let raw_tls = prepare_http3_tls(Some(
                crate::network_transport::client_tls_config_with_trust_roots(
                    vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                    false,
                )
                .unwrap(),
            ))
            .unwrap();
            let crypto = QuicClientConfig::try_from((*raw_tls).clone()).unwrap();
            let mut raw_config = ClientConfig::new(Arc::new(crypto));
            let mut raw_transport = TransportConfig::default();
            raw_transport
                .max_concurrent_bidi_streams(VarInt::from_u32(MAX_QUIC_STREAMS))
                .max_concurrent_uni_streams(VarInt::from_u32(MAX_QUIC_STREAMS));
            raw_config.transport_config(Arc::new(raw_transport));
            crate::network_future::block_on_current(crate::network_future::spawn_on_reactor(
                async move {
                    let mut raw_endpoint =
                        Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
                    raw_endpoint.set_default_client_config(raw_config);
                    let raw_connection = raw_endpoint
                        .connect(address, "127.0.0.1")
                        .unwrap()
                        .await
                        .unwrap();
                    raw_connection.close(VarInt::from_u32(0), b"no HTTP/3 request");
                    drop(raw_connection);
                    drop(raw_endpoint);
                },
            ))
            .unwrap()
            .unwrap();

            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap(),
            );
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let mut request = zirk_runtime_core::http_values::HttpRequestValues::new(
                zirk_runtime_core::http_values::HttpMethod::Post,
                Url::parse(&format!("{origin}/echo")).unwrap(),
            );
            request.method_token = "POST".to_owned();
            request
                .headers
                .insert("content-type", b"text/plain")
                .unwrap();
            let mut client = crate::http_runtime::HttpClientPool::new_shared_with_protocol(
                1,
                crate::http_runtime::HttpTimeouts {
                    connect: Duration::from_secs(5),
                    response_headers: Duration::from_secs(5),
                    response_idle: Duration::from_secs(5),
                },
                crate::http_runtime::HttpProtocolPreference::Auto,
                Some(tls),
                None,
            )
            .unwrap();
            let mut chunks = std::collections::VecDeque::from([
                Some(b"pi".to_vec()),
                Some(b"ng".to_vec()),
                None,
            ]);
            let response = client
                .request_streaming(request, grant, || Ok(chunks.pop_front().unwrap()))
                .unwrap();

            assert_eq!(
                response.protocol,
                crate::http_runtime::HttpProtocolVersion::Http3
            );
            assert_eq!(response.status.as_u16(), 201);
            assert_eq!(
                response.headers.get("x-protocol").unwrap().as_bytes(),
                b"h3"
            );
            assert_eq!(response.body, b"accepted:ping");
            drop(client);
            assert_eq!(crate::executor::await_task(server_task), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http3_client_session_multiplexes_concurrent_requests_on_one_connection() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let requested_address = reservation.local_addr().unwrap();
            drop(reservation);
            let listen_grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                [requested_address.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap();
            let (endpoint, address) =
                bind_server_endpoint(requested_address, &listen_grant, test_http3_server_config())
                    .unwrap();
            let server_task = crate::executor::spawn(move || {
                serve_endpoint(endpoint, 2, 1, |request| {
                    let body = request.uri().path().as_bytes().to_vec();
                    Http3ServerResponse {
                        response: http::Response::builder()
                            .status(200)
                            .body(Http3ResponseBody::Buffered(body))
                            .unwrap(),
                        producer: None,
                    }
                })
                .unwrap()
            });

            let origin = format!("https://{address}");
            let network_origin = NetworkOrigin::parse(&origin).unwrap();
            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap(),
            );
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let session = Http3ClientSession::connect(
                network_origin,
                Arc::clone(&grant),
                Duration::from_secs(5),
                Some(tls),
            )
            .unwrap();
            let first_request = Request::builder()
                .method(http::Method::GET)
                .uri(format!("{origin}/first"))
                .version(Version::HTTP_3)
                .body(Vec::new())
                .unwrap();
            let second_request = Request::builder()
                .method(http::Method::GET)
                .uri(format!("{origin}/second"))
                .version(Version::HTTP_3)
                .body(Vec::new())
                .unwrap();

            let first_session = Arc::clone(&session);
            let first_grant = Arc::clone(&grant);
            let (first_tx, first_rx) = std::sync::mpsc::channel();
            let first = crate::executor::spawn(move || {
                let result = first_session.request(
                    first_request,
                    first_grant,
                    Duration::from_secs(5),
                    Duration::from_secs(5),
                );
                let _ = first_tx.send(result);
                0
            });
            let second_session = Arc::clone(&session);
            let second_grant = Arc::clone(&grant);
            let (second_tx, second_rx) = std::sync::mpsc::channel();
            let second = crate::executor::spawn(move || {
                let result = second_session.request(
                    second_request,
                    second_grant,
                    Duration::from_secs(5),
                    Duration::from_secs(5),
                );
                let _ = second_tx.send(result);
                0
            });

            crate::executor::await_task(first);
            crate::executor::await_task(second);
            let first = first_rx.recv().unwrap().unwrap();
            let second = second_rx.recv().unwrap().unwrap();
            assert_eq!(first.body(), b"/first");
            assert_eq!(second.body(), b"/second");
            drop(session);
            assert_eq!(crate::executor::await_task(server_task), 2);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http3_client_pool_reuses_one_connection_across_requests() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let requested_address = reservation.local_addr().unwrap();
            drop(reservation);
            let listen_grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                [requested_address.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap();
            let (endpoint, address) =
                bind_server_endpoint(requested_address, &listen_grant, test_http3_server_config())
                    .unwrap();
            let server_task = crate::executor::spawn(move || {
                serve_endpoint(endpoint, 2, 1, |request| Http3ServerResponse {
                    response: http::Response::builder()
                        .status(200)
                        .body(Http3ResponseBody::Buffered(
                            request.uri().path().as_bytes().to_vec(),
                        ))
                        .unwrap(),
                    producer: None,
                })
                .unwrap()
            });

            let origin = format!("https://{address}");
            let grant = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes([origin.clone()], []).unwrap(),
            );
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let mut client = crate::http_runtime::HttpClientPool::new_shared_with_protocol(
                1,
                crate::http_runtime::HttpTimeouts {
                    connect: Duration::from_secs(5),
                    response_headers: Duration::from_secs(5),
                    response_idle: Duration::from_secs(5),
                },
                crate::http_runtime::HttpProtocolPreference::Http3,
                Some(tls),
                None,
            )
            .unwrap();
            let request = |path: &str| {
                let mut request = zirk_runtime_core::http_values::HttpRequestValues::new(
                    zirk_runtime_core::http_values::HttpMethod::Get,
                    Url::parse(&format!("{origin}{path}")).unwrap(),
                );
                request.method_token = "GET".to_owned();
                request
            };
            let first = client
                .request(request("/first"), Arc::clone(&grant))
                .unwrap();
            let second = client.request(request("/second"), grant).unwrap();
            assert_eq!(
                first.protocol,
                crate::http_runtime::HttpProtocolVersion::Http3
            );
            assert_eq!(
                second.protocol,
                crate::http_runtime::HttpProtocolVersion::Http3
            );
            assert_eq!(first.body, b"/first");
            assert_eq!(second.body, b"/second");
            drop(client);
            assert_eq!(crate::executor::await_task(server_task), 2);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http3_server_dispatches_multiplexed_requests_to_bounded_owner_tasks() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let reservation = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let requested_address = reservation.local_addr().unwrap();
            drop(reservation);
            let listen_grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                [requested_address.to_string()],
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .unwrap();
            let (endpoint, address) =
                bind_server_endpoint(requested_address, &listen_grant, test_http3_server_config())
                    .unwrap();
            let owner_task = crate::executor::current_task();
            let owner_executor = crate::executor::current_executor_id();
            let owner_thread = std::thread::current().id();
            let second_started = Arc::new(AtomicUsize::new(0));
            let second_started_for_handler = Arc::clone(&second_started);
            let server_task = crate::executor::spawn(move || {
                let served = serve_endpoint(endpoint, 2, 3, move |request| {
                    assert_eq!(crate::executor::current_executor_id(), owner_executor);
                    assert_ne!(crate::executor::current_task(), owner_task);
                    assert_eq!(std::thread::current().id(), owner_thread);
                    let started = second_started_for_handler.fetch_add(1, Ordering::AcqRel) + 1;
                    if started == 1 {
                        let deadline = std::time::Instant::now() + Duration::from_secs(2);
                        while second_started_for_handler.load(Ordering::Acquire) < 2 {
                            assert!(std::time::Instant::now() < deadline);
                            crate::executor::yield_now();
                        }
                    }
                    Http3ServerResponse {
                        response: Response::builder()
                            .status(200)
                            .body(Http3ResponseBody::Buffered(
                                request.uri().path().as_bytes().to_vec(),
                            ))
                            .unwrap(),
                        producer: None,
                    }
                })
                .unwrap();
                assert_eq!(served, 2);
                served
            });

            let origin_text = format!("https://{address}");
            let origin_url = Url::parse(&origin_text).unwrap();
            let origin = NetworkOrigin::parse(&origin_url.origin()).unwrap();
            let tls = crate::network_transport::client_tls_config_with_trust_roots(
                vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                false,
            )
            .unwrap();
            let first_origin = origin.clone();
            let first_tls = Arc::clone(&tls);
            let second_origin = origin.clone();
            let second_tls = Arc::clone(&tls);
            let third_origin = origin;
            let third_tls = tls;
            let responses = crate::network_future::block_on_current(
                crate::network_future::spawn_on_reactor(async move {
                    tokio::join!(
                        test_http3_get(first_origin, first_tls, "/first"),
                        test_http3_get(second_origin, second_tls, "/second"),
                        test_http3_get(third_origin, third_tls, "/third"),
                    )
                }),
            )
            .unwrap()
            .unwrap();
            let successful_bodies = [responses.0, responses.1, responses.2]
                .into_iter()
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            assert_eq!(successful_bodies.len(), 2);
            assert!(successful_bodies.iter().all(|body| {
                [
                    b"/first".as_slice(),
                    b"/second".as_slice(),
                    b"/third".as_slice(),
                ]
                .contains(&body.as_slice())
            }));
            assert_eq!(second_started.load(Ordering::Acquire), 2);
            assert_eq!(crate::executor::await_task(server_task), 2);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http3_listener_checks_udp_bind_authority_before_opening_socket() {
        let address = "127.0.0.1:0".parse().unwrap();
        let error = bind_server_endpoint(
            address,
            &EffectiveNetworkGrant::default(),
            test_http3_server_config(),
        )
        .expect_err("an unapproved QUIC listener must be rejected");
        assert_eq!(
            error,
            super::super::http_runtime::HttpRuntimeError::PermissionDenied
        );
    }

    #[test]
    fn http3_request_requires_exact_origin_authority_before_network_access() {
        let mut request = zirk_runtime_core::http_values::HttpRequestValues::new(
            zirk_runtime_core::http_values::HttpMethod::Get,
            Url::parse("https://127.0.0.1/blocked").unwrap(),
        );
        request.method_token = "GET".to_owned();
        let grant = Arc::new(EffectiveNetworkGrant::from_verified_scopes([], []).unwrap());
        let mut client = crate::http_runtime::HttpClientPool::new_shared_with_protocol(
            1,
            crate::http_runtime::HttpTimeouts {
                connect: Duration::from_secs(1),
                response_headers: Duration::from_secs(1),
                response_idle: Duration::from_secs(1),
            },
            crate::http_runtime::HttpProtocolPreference::Http3,
            None,
            None,
        )
        .unwrap();
        assert!(matches!(
            client.request(request, grant),
            Err(super::super::http_runtime::HttpRuntimeError::PermissionDenied)
        ));
    }
}
