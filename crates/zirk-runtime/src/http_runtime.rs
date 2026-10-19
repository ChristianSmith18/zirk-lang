//! Bounded one-shot HTTP/1.1 and negotiated HTTP/2 client foundation.
//!
//! HTTP/1.1 futures and server handlers run on their owning Zirk task. The
//! stack-heavy HTTP/2 client state machine runs on the native reactor thread
//! and returns data through bounded channels; it never calls generated code.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use async_compression::tokio::bufread::{
    BrotliDecoder, BrotliEncoder, GzipDecoder, GzipEncoder, ZlibDecoder, ZlibEncoder,
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full, combinators::UnsyncBoxBody};
use hyper::body::{Body, Frame, SizeHint};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zeroize::{Zeroize, Zeroizing};
use zirk_runtime_core::http_cookie::CookieJar;
use zirk_runtime_core::http_encoding::{AcceptEncoding, ContentCoding};
use zirk_runtime_core::http_proxy::HttpProxy;
use zirk_runtime_core::http_values::{
    Headers, HttpRequestValues, HttpStatusCode, MAX_HTTP_BUFFERED_BODY_BYTES,
};
use zirk_runtime_core::network_policy::EffectiveNetworkGrant;

use crate::network_transport::{
    NegotiatedHttpProtocol, NetworkTransportError, TcpStream, TlsClientStream, TlsServerStream,
};

type HttpClientRequestBody = UnsyncBoxBody<Bytes, std::convert::Infallible>;

fn buffered_client_request_body(bytes: Vec<u8>) -> HttpClientRequestBody {
    Full::new(Bytes::from(bytes)).boxed_unsync()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRuntimeError {
    InvalidRequest,
    UnsupportedMethod,
    PermissionDenied,
    Transport,
    Protocol,
    BodyTooLarge,
    BodyConsumed,
    InvalidText,
    InvalidJson,
    Timeout,
    Cancelled,
}

impl From<NetworkTransportError> for HttpRuntimeError {
    fn from(error: NetworkTransportError) -> Self {
        match error {
            NetworkTransportError::PermissionDenied | NetworkTransportError::AddressNotAllowed => {
                Self::PermissionDenied
            }
            NetworkTransportError::Timeout => Self::Timeout,
            NetworkTransportError::Cancelled => Self::Cancelled,
            _ => Self::Transport,
        }
    }
}

/// Independent limits for connection establishment, response headers, and
/// idle time while consuming a response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpTimeouts {
    pub connect: Duration,
    pub response_headers: Duration,
    pub response_idle: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            response_headers: Duration::from_secs(30),
            response_idle: Duration::from_secs(30),
        }
    }
}

const DEFAULT_HTTP_SERVER_TIMEOUTS: HttpTimeouts = HttpTimeouts {
    connect: Duration::from_secs(10),
    response_headers: Duration::from_secs(30),
    response_idle: Duration::from_secs(30),
};
const DEFAULT_HTTP_SERVER_HANDLER_TIMEOUT: Duration = Duration::from_secs(30);
/// Upper bound for protocol-level draining after the final admitted request.
/// The connection future owns all remaining Hyper streams; dropping it at the
/// deadline closes those streams and lets their cancellation guards notify the
/// Zirk owner task at its next cooperative safe point.
const DEFAULT_HTTP_SERVER_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

async fn await_http2_graceful_drain<F>(
    connection: F,
    timeout: Duration,
) -> Result<(), HttpRuntimeError>
where
    F: std::future::Future<Output = Result<(), hyper::Error>>,
{
    match tokio::time::timeout(timeout, connection).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if is_peer_disconnect(&error) => Ok(()),
        Ok(Err(_)) => Err(HttpRuntimeError::Protocol),
        // Dropping the connection future cancels every remaining stream. Any
        // Zirk handler suspended on I/O observes its cancellation through the
        // existing cooperative bridge.
        Err(_) => Err(HttpRuntimeError::Timeout),
    }
}

pub struct HttpResponseValues {
    pub status: HttpStatusCode,
    pub headers: Headers,
    pub body: Vec<u8>,
    /// Optional bounded producer channel. Hyper polls this on its transport
    /// task; the Zirk owner task remains responsible for producing chunks.
    pub body_stream: Option<tokio::sync::mpsc::Receiver<Result<Bytes, io::Error>>>,
    /// Generated source producer, consumed on its owning Zirk task. HTTP/2
    /// responses move it to a bounded channel before entering the reactor.
    pub(crate) body_producer: Option<crate::http_api::HttpServerBodyProducer>,
}

struct HttpChannelBody {
    receiver: tokio::sync::mpsc::Receiver<Result<Bytes, io::Error>>,
}

impl Body for HttpChannelBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.receiver.poll_recv(context) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.receiver.is_closed() && self.receiver.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

struct HttpZirkProducerBody {
    producer: Option<crate::http_api::HttpServerBodyProducer>,
}

impl Body for HttpZirkProducerBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let Some(producer) = self.producer.as_mut() else {
            return Poll::Ready(None);
        };
        match producer.next_chunk() {
            Ok(Some(chunk)) => Poll::Ready(Some(Ok(Frame::data(Bytes::from(chunk))))),
            Ok(None) => {
                self.producer.take();
                Poll::Ready(None)
            }
            Err(_) => {
                self.producer.take();
                Poll::Ready(Some(Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Zirk HTTP response producer failed",
                ))))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.producer.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

type HttpProducerInput = Result<Bytes, io::Error>;
type HttpProducerReserve = Pin<
    Box<
        dyn Future<
                Output = Result<
                    tokio::sync::mpsc::OwnedPermit<HttpProducerInput>,
                    tokio::sync::mpsc::error::SendError<()>,
                >,
            > + Send,
    >,
>;

struct HttpZirkCompressedProducerBody {
    producer: Option<crate::http_api::HttpServerBodyProducer>,
    input: Option<tokio::sync::mpsc::Sender<HttpProducerInput>>,
    reserve: Option<HttpProducerReserve>,
    output: tokio::sync::mpsc::Receiver<HttpProducerInput>,
}

impl HttpZirkCompressedProducerBody {
    fn new(producer: crate::http_api::HttpServerBodyProducer, coding: ContentCoding) -> Self {
        let (input, receiver) = tokio::sync::mpsc::channel(1);
        let (sender, output) = tokio::sync::mpsc::channel(2);
        spawn_http_response_encoder(
            HttpResponseChannelReader {
                receiver,
                pending: Bytes::new(),
            },
            coding,
            sender,
        );
        Self {
            producer: Some(producer),
            input: Some(input),
            reserve: None,
            output,
        }
    }
}

impl Body for HttpZirkCompressedProducerBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.output).poll_recv(context) {
            Poll::Ready(Some(Ok(bytes))) => {
                return Poll::Ready(Some(Ok(Frame::data(bytes))));
            }
            Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }
        let Some(input) = &this.input else {
            return Poll::Pending;
        };
        if this.reserve.is_none() {
            this.reserve = Some(Box::pin(input.clone().reserve_owned()));
        }
        let permit = match this
            .reserve
            .as_mut()
            .expect("reserve future initialized")
            .as_mut()
            .poll(context)
        {
            Poll::Ready(Ok(permit)) => permit,
            Poll::Ready(Err(_)) => {
                this.reserve.take();
                this.input.take();
                this.producer.take();
                return Poll::Pending;
            }
            Poll::Pending => return Poll::Pending,
        };
        this.reserve.take();
        let Some(producer) = this.producer.as_mut() else {
            drop(permit);
            this.input.take();
            return Poll::Pending;
        };
        match producer.next_chunk() {
            Ok(Some(chunk)) if !chunk.is_empty() => {
                permit.send(Ok(Bytes::from(chunk)));
            }
            Ok(Some(_)) | Ok(None) => {
                drop(permit);
                this.input.take();
                this.producer.take();
            }
            Err(_) => {
                permit.send(Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Zirk HTTP response producer failed",
                )));
                this.input.take();
                this.producer.take();
            }
        }
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        self.input.is_none() && self.output.is_closed() && self.output.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

/// A backpressured response stream. Dropping it before EOF aborts its owned
/// HTTP connection; attempting to read after EOF returns `None`.
pub struct HttpStreamingResponse {
    pub status: HttpStatusCode,
    pub headers: Headers,
    body: Option<HttpStreamingBody>,
    driver: Option<Vec<AbortOnDrop>>,
    pending: Bytes,
    owner: crate::task::TaskId,
    response_idle_timeout: Duration,
    body_consumed: bool,
    body_eof: bool,
    stream_started: bool,
}

/// A content-type-checked SSE view over one bounded HTTP response stream.
/// Parsing advances only when the caller asks for another event, preserving
/// the response's existing backpressure and owner-task rules.
pub struct HttpSseResponse {
    response: HttpStreamingResponse,
    parser: zirk_runtime_core::http_sse::SseParser,
    queued: VecDeque<zirk_runtime_core::http_sse::SseEvent>,
    ended: bool,
}

impl HttpSseResponse {
    pub fn from_response(response: HttpStreamingResponse) -> Result<Self, HttpRuntimeError> {
        Self::validate_headers(&response.headers)?;
        if response.status.as_u16() != 200 {
            return Err(HttpRuntimeError::Protocol);
        }
        Ok(Self {
            response,
            parser: zirk_runtime_core::http_sse::SseParser::new(),
            queued: VecDeque::new(),
            ended: false,
        })
    }

    pub(crate) fn validate_headers(headers: &Headers) -> Result<(), HttpRuntimeError> {
        let values = headers
            .get_all("content-type")
            .map(|values| values.collect::<Vec<_>>())
            .unwrap_or_default();
        if values.len() != 1 {
            return Err(HttpRuntimeError::Protocol);
        }
        let content_type = values[0]
            .to_str()
            .map_err(|_| HttpRuntimeError::Protocol)?
            .parse::<mime::Mime>()
            .map_err(|_| HttpRuntimeError::Protocol)?;
        if content_type.type_() != mime::TEXT
            || !content_type
                .subtype()
                .as_str()
                .eq_ignore_ascii_case("event-stream")
            || content_type
                .get_param(mime::CHARSET)
                .is_some_and(|charset| !charset.as_str().eq_ignore_ascii_case("utf-8"))
        {
            return Err(HttpRuntimeError::Protocol);
        }
        Ok(())
    }

    /// Waits for the next complete SSE event. The parser retains at most one
    /// bounded input fragment and queues no more than the core feed limit.
    pub fn next_event(
        &mut self,
    ) -> Result<Option<zirk_runtime_core::http_sse::SseEvent>, HttpRuntimeError> {
        if let Some(event) = self.queued.pop_front() {
            return Ok(Some(event));
        }
        if self.ended {
            return Ok(None);
        }
        loop {
            match self.response.read_chunk(64 * 1024)? {
                Some(bytes) => {
                    let events = self
                        .parser
                        .feed(&bytes)
                        .map_err(|_| HttpRuntimeError::Protocol)?;
                    self.queued.extend(events);
                    if let Some(event) = self.queued.pop_front() {
                        return Ok(Some(event));
                    }
                }
                None => {
                    self.parser.finish();
                    self.ended = true;
                    return Ok(None);
                }
            }
        }
    }

    pub fn retry_ms(&self) -> Option<u64> {
        self.parser.retry_ms()
    }

    pub fn heartbeat_count(&self) -> u64 {
        self.parser.heartbeat_count()
    }
}

enum HttpStreamingBody {
    Http1(hyper::body::Incoming),
    Chunks(tokio::sync::mpsc::Receiver<Result<Bytes, HttpRuntimeError>>),
    Buffered { bytes: Bytes, offset: usize },
}

const HTTP_COMPRESSED_RESPONSE_LIMIT: usize = MAX_HTTP_BUFFERED_BODY_BYTES;
const HTTP_EXPANDED_RESPONSE_LIMIT: usize = MAX_HTTP_BUFFERED_BODY_BYTES;
const HTTP_MAX_DECOMPRESSION_RATIO: usize = 1000;
const HTTP_CONTENT_CODING_CPU_BUDGET: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct HttpBodyReaderError(HttpRuntimeError);

impl fmt::Display for HttpBodyReaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "HTTP body read failed: {:?}", self.0)
    }
}

impl Error for HttpBodyReaderError {}

struct HttpBodyReader {
    body: HttpStreamingBody,
    pending: Bytes,
    bytes_read: usize,
    compressed_bytes: Arc<AtomicUsize>,
}

impl HttpBodyReader {
    fn new(body: HttpStreamingBody) -> (Self, Arc<AtomicUsize>) {
        let compressed_bytes = Arc::new(AtomicUsize::new(0));
        (
            Self {
                body,
                pending: Bytes::new(),
                bytes_read: 0,
                compressed_bytes: Arc::clone(&compressed_bytes),
            },
            compressed_bytes,
        )
    }
}

impl AsyncRead for HttpBodyReader {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if !this.pending.is_empty() {
                let count = output.remaining().min(this.pending.len());
                let Some(next_total) = this.bytes_read.checked_add(count) else {
                    return Poll::Ready(Err(body_reader_error(HttpRuntimeError::BodyTooLarge)));
                };
                if next_total > HTTP_COMPRESSED_RESPONSE_LIMIT {
                    return Poll::Ready(Err(body_reader_error(HttpRuntimeError::BodyTooLarge)));
                }
                output.put_slice(&this.pending.split_to(count));
                this.bytes_read = next_total;
                this.compressed_bytes.store(next_total, Ordering::Relaxed);
                return Poll::Ready(Ok(()));
            }

            let input = match &mut this.body {
                HttpStreamingBody::Http1(body) => match Pin::new(body).poll_frame(context) {
                    Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                        Ok(bytes) => Poll::Ready(Some(Ok(bytes))),
                        Err(_) => continue,
                    },
                    Poll::Ready(Some(Err(_))) => Poll::Ready(Some(Err(HttpRuntimeError::Protocol))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                },
                HttpStreamingBody::Chunks(chunks) => match Pin::new(chunks).poll_recv(context) {
                    Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(bytes))),
                    Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                },
                HttpStreamingBody::Buffered { bytes, offset } => {
                    if *offset >= bytes.len() {
                        Poll::Ready(None)
                    } else {
                        let end = offset.saturating_add(output.remaining()).min(bytes.len());
                        let chunk = bytes.slice(*offset..end);
                        *offset = end;
                        Poll::Ready(Some(Ok(chunk)))
                    }
                }
            };
            match input {
                Poll::Ready(Some(Ok(bytes))) => {
                    if this
                        .bytes_read
                        .checked_add(bytes.len())
                        .is_none_or(|total| total > HTTP_COMPRESSED_RESPONSE_LIMIT)
                    {
                        return Poll::Ready(Err(body_reader_error(HttpRuntimeError::BodyTooLarge)));
                    }
                    this.pending = bytes;
                }
                Poll::Ready(Some(Err(error))) => {
                    return Poll::Ready(Err(body_reader_error(error)));
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn body_reader_error(error: HttpRuntimeError) -> io::Error {
    io::Error::other(HttpBodyReaderError(error))
}

fn map_body_reader_error(error: &io::Error) -> HttpRuntimeError {
    error
        .get_ref()
        .and_then(|source| source.downcast_ref::<HttpBodyReaderError>())
        .map_or(HttpRuntimeError::Protocol, |source| source.0)
}

async fn read_with_cpu_budget(
    reader: &mut Pin<Box<dyn AsyncRead + Send + '_>>,
    bytes: &mut [u8],
    decoder_cpu_time: &mut Duration,
    cpu_budget: Duration,
) -> io::Result<usize> {
    std::future::poll_fn(|context| {
        let mut output = ReadBuf::new(bytes);
        let started = std::time::Instant::now();
        let result = match reader.as_mut().poll_read(context, &mut output) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(output.filled().len())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        };
        *decoder_cpu_time = decoder_cpu_time.saturating_add(started.elapsed());
        if *decoder_cpu_time > cpu_budget {
            Poll::Ready(Err(body_reader_error(HttpRuntimeError::Timeout)))
        } else {
            result
        }
    })
    .await
}

/// Moves content decoding to a Tokio worker and transfers decoded chunks back
/// through a two-item queue. One coding layer is accepted; malformed stacks
/// are rejected before the body is exposed to application code.
fn decode_response_body(
    mut headers: Headers,
    body: HttpStreamingBody,
    accepted: &AcceptEncoding,
) -> Result<(Headers, HttpStreamingBody, Option<AbortOnDrop>), HttpRuntimeError> {
    let values = headers
        .get_all("content-encoding")
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default();
    if values.is_empty() {
        return Ok((headers, body, None));
    }
    if values.len() != 1 {
        return Err(HttpRuntimeError::Protocol);
    }
    let coding = values[0]
        .to_str()
        .map_err(|_| HttpRuntimeError::Protocol)?
        .trim_matches([' ', '\t']);
    if coding.contains(',') {
        return Err(HttpRuntimeError::Protocol);
    }
    let coding = match coding.to_ascii_lowercase().as_str() {
        "br" => ContentCoding::Brotli,
        "gzip" => ContentCoding::Gzip,
        "deflate" => ContentCoding::Deflate,
        "identity" => ContentCoding::Identity,
        _ => return Err(HttpRuntimeError::Protocol),
    };
    if accepted.quality(coding) == 0 {
        return Err(HttpRuntimeError::Protocol);
    }
    if coding == ContentCoding::Identity {
        headers.remove("content-encoding");
        return Ok((headers, body, None));
    }
    let (input, compressed_bytes) = HttpBodyReader::new(body);
    let input = tokio::io::BufReader::with_capacity(8 * 1024, input);
    let mut decoder: Pin<Box<dyn AsyncRead + Send>> = match coding {
        ContentCoding::Brotli => Box::pin(BrotliDecoder::new(input)),
        ContentCoding::Gzip => Box::pin(GzipDecoder::new(input)),
        // HTTP's `deflate` content coding uses the zlib-wrapped format.
        ContentCoding::Deflate => Box::pin(ZlibDecoder::new(input)),
        ContentCoding::Identity => unreachable!("identity returned above"),
    };
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    let worker = tokio::spawn(async move {
        let mut expanded = 0usize;
        let mut decoder_cpu_time = Duration::ZERO;
        let mut bytes = vec![0u8; 64 * 1024];
        loop {
            let count = match read_with_cpu_budget(
                &mut decoder,
                &mut bytes,
                &mut decoder_cpu_time,
                HTTP_CONTENT_CODING_CPU_BUDGET,
            )
            .await
            {
                Ok(count) => count,
                Err(error) => {
                    let _ = sender.send(Err(map_body_reader_error(&error))).await;
                    return;
                }
            };
            if count == 0 {
                return;
            }
            let Some(next_expanded) = expanded.checked_add(count) else {
                let _ = sender.send(Err(HttpRuntimeError::BodyTooLarge)).await;
                return;
            };
            let compressed = compressed_bytes.load(Ordering::Relaxed);
            if next_expanded > HTTP_EXPANDED_RESPONSE_LIMIT
                || next_expanded
                    > compressed
                        .max(1)
                        .saturating_mul(HTTP_MAX_DECOMPRESSION_RATIO)
            {
                let _ = sender.send(Err(HttpRuntimeError::BodyTooLarge)).await;
                return;
            }
            expanded = next_expanded;
            if sender
                .send(Ok(Bytes::copy_from_slice(&bytes[..count])))
                .await
                .is_err()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    });
    headers.remove("content-encoding");
    headers.remove("content-length");
    Ok((
        headers,
        HttpStreamingBody::Chunks(receiver),
        Some(AbortOnDrop(worker.abort_handle())),
    ))
}

/// Negotiates and compresses buffered and streamed server representations.
/// Streaming codecs use bounded queues, and generated handler callbacks remain
/// on their owning Zirk task.
async fn prepare_server_response(
    mut response: HttpResponseValues,
    accepted: &AcceptEncoding,
) -> Result<HttpResponseValues, HttpRuntimeError> {
    const MINIMUM_COMPRESSIBLE_BODY_BYTES: usize = 512;
    let body_is_streamed = response.body_stream.is_some() || response.body_producer.is_some();
    if !body_is_streamed && response.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
        return Ok(simple_response(
            500,
            b"response body exceeds the configured limit",
        ));
    }
    let existing_codings = response
        .headers
        .get_all("content-encoding")
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default();
    if !existing_codings.is_empty() {
        if existing_codings.len() != 1 {
            return Ok(simple_response(500, b"invalid response content coding"));
        }
        let coding = existing_codings[0]
            .to_str()
            .map_err(|_| HttpRuntimeError::Protocol)?
            .trim_matches([' ', '\t'])
            .to_ascii_lowercase();
        if coding.contains(',') {
            return Ok(simple_response(500, b"nested response content coding"));
        }
        let supported = match coding.as_str() {
            "br" => Some(ContentCoding::Brotli),
            "gzip" => Some(ContentCoding::Gzip),
            "deflate" => Some(ContentCoding::Deflate),
            "identity" => Some(ContentCoding::Identity),
            _ => None,
        };
        let quality = supported.map_or_else(
            || accepted.quality_for_token(&coding),
            |supported| accepted.quality(supported),
        );
        if quality == 0 {
            return Ok(simple_response(406, b""));
        }
        append_vary_accept_encoding(&mut response.headers)?;
        return Ok(response);
    }

    let coding = accepted.negotiate(&[
        ContentCoding::Brotli,
        ContentCoding::Gzip,
        ContentCoding::Deflate,
    ]);
    if response.body_producer.is_none()
        && let Some(body_stream) = response.body_stream.take()
    {
        if let Some(coding) = coding {
            response.body_stream = Some(encode_response_stream(body_stream, coding));
            set_encoded_representation(&mut response.headers, coding)?;
            return Ok(response);
        }
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        response.body_stream = Some(body_stream);
        return Ok(response);
    }
    if response.body_producer.is_some() {
        if let Some(coding) = coding {
            set_encoded_representation(&mut response.headers, coding)?;
            return Ok(response);
        }
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        return Ok(response);
    }
    if body_is_streamed {
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        if coding.is_some() {
            append_vary_accept_encoding(&mut response.headers)?;
        }
        return Ok(response);
    }
    if response.body.is_empty() {
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        return Ok(response);
    }
    if response.body.len() < MINIMUM_COMPRESSIBLE_BODY_BYTES {
        return if accepted.quality(ContentCoding::Identity) == 0 {
            match coding {
                Some(coding) => {
                    let body = Arc::new(std::mem::take(&mut response.body));
                    let encoded = encode_response_body(body, coding).await?;
                    if encoded.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
                        return Ok(simple_response(406, b""));
                    }
                    set_encoded_representation(&mut response.headers, coding)?;
                    response.body = encoded;
                    Ok(response)
                }
                None => Ok(simple_response(406, b"")),
            }
        } else {
            if coding.is_some() {
                append_vary_accept_encoding(&mut response.headers)?;
            }
            Ok(response)
        };
    }

    let Some(coding) = coding else {
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        append_vary_accept_encoding(&mut response.headers)?;
        return Ok(response);
    };
    let original_body = Arc::new(std::mem::take(&mut response.body));
    let encoded = match encode_response_body(Arc::clone(&original_body), coding).await {
        Ok(encoded) => encoded,
        Err(HttpRuntimeError::BodyTooLarge) if accepted.quality(ContentCoding::Identity) > 0 => {
            response.body =
                Arc::try_unwrap(original_body).unwrap_or_else(|body| body.as_ref().clone());
            append_vary_accept_encoding(&mut response.headers)?;
            return Ok(response);
        }
        Err(HttpRuntimeError::BodyTooLarge) => return Ok(simple_response(406, b"")),
        Err(error) => return Err(error),
    };
    if encoded.len() > MAX_HTTP_BUFFERED_BODY_BYTES
        || (encoded.len() >= original_body.len() && accepted.quality(ContentCoding::Identity) > 0)
    {
        if accepted.quality(ContentCoding::Identity) == 0 {
            return Ok(simple_response(406, b""));
        }
        response.body = Arc::try_unwrap(original_body).unwrap_or_else(|body| body.as_ref().clone());
        append_vary_accept_encoding(&mut response.headers)?;
        return Ok(response);
    }
    set_encoded_representation(&mut response.headers, coding)?;
    response.body = encoded;
    Ok(response)
}

async fn encode_response_body(
    body: Arc<Vec<u8>>,
    coding: ContentCoding,
) -> Result<Vec<u8>, HttpRuntimeError> {
    let task = tokio::spawn(async move {
        let input =
            tokio::io::BufReader::with_capacity(8 * 1024, std::io::Cursor::new(body.as_slice()));
        let mut encoder: Pin<Box<dyn AsyncRead + Send + '_>> = match coding {
            ContentCoding::Brotli => Box::pin(BrotliEncoder::new(input)),
            ContentCoding::Gzip => Box::pin(GzipEncoder::new(input)),
            ContentCoding::Deflate => Box::pin(ZlibEncoder::new(input)),
            ContentCoding::Identity => return Err(HttpRuntimeError::InvalidRequest),
        };
        let mut encoded = Vec::new();
        let mut chunk = vec![0; 64 * 1024];
        let mut codec_cpu_time = Duration::ZERO;
        loop {
            let count = read_with_cpu_budget(
                &mut encoder,
                &mut chunk,
                &mut codec_cpu_time,
                HTTP_CONTENT_CODING_CPU_BUDGET,
            )
            .await
            .map_err(|error| map_body_reader_error(&error))?;
            if count == 0 {
                return Ok(encoded);
            }
            append_bounded_body(&mut encoded, &chunk[..count], MAX_HTTP_BUFFERED_BODY_BYTES)?;
            tokio::task::yield_now().await;
        }
    });
    let _abort_on_drop = AbortOnDrop(task.abort_handle());
    task.await.map_err(|_| HttpRuntimeError::Protocol)?
}

struct HttpResponseChannelReader {
    receiver: tokio::sync::mpsc::Receiver<Result<Bytes, io::Error>>,
    pending: Bytes,
}

impl AsyncRead for HttpResponseChannelReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if !self.pending.is_empty() {
                let count = self.pending.len().min(output.remaining());
                output.put_slice(&self.pending.split_to(count));
                return Poll::Ready(Ok(()));
            }
            match self.receiver.poll_recv(context) {
                Poll::Ready(Some(Ok(bytes))) => self.pending = bytes,
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn encode_response_stream(
    receiver: tokio::sync::mpsc::Receiver<Result<Bytes, io::Error>>,
    coding: ContentCoding,
) -> tokio::sync::mpsc::Receiver<Result<Bytes, io::Error>> {
    let (sender, encoded) = tokio::sync::mpsc::channel(2);
    spawn_http_response_encoder(
        HttpResponseChannelReader {
            receiver,
            pending: Bytes::new(),
        },
        coding,
        sender,
    );
    encoded
}

fn spawn_http_response_encoder(
    reader: impl AsyncRead + Send + 'static,
    coding: ContentCoding,
    sender: tokio::sync::mpsc::Sender<Result<Bytes, io::Error>>,
) {
    tokio::spawn(async move {
        let input = tokio::io::BufReader::with_capacity(8 * 1024, reader);
        let mut encoder: Pin<Box<dyn AsyncRead + Send>> = match coding {
            ContentCoding::Brotli => Box::pin(BrotliEncoder::new(input)),
            ContentCoding::Gzip => Box::pin(GzipEncoder::new(input)),
            ContentCoding::Deflate => Box::pin(ZlibEncoder::new(input)),
            ContentCoding::Identity => return,
        };
        let mut chunk = vec![0; 64 * 1024];
        let mut codec_cpu_time = Duration::ZERO;
        loop {
            let count = tokio::select! {
                _ = sender.closed() => return,
                result = read_with_cpu_budget(
                    &mut encoder,
                    &mut chunk,
                    &mut codec_cpu_time,
                HTTP_CONTENT_CODING_CPU_BUDGET,
                ) => match result {
                    Ok(count) => count,
                    Err(error) => {
                        let _ = sender.send(Err(error)).await;
                        return;
                    }
                }
            };
            if count == 0 {
                return;
            }
            if sender
                .send(Ok(Bytes::copy_from_slice(&chunk[..count])))
                .await
                .is_err()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    });
}

fn set_encoded_representation(
    headers: &mut Headers,
    coding: ContentCoding,
) -> Result<(), HttpRuntimeError> {
    let name = match coding {
        ContentCoding::Brotli => "br",
        ContentCoding::Gzip => "gzip",
        ContentCoding::Deflate => "deflate",
        ContentCoding::Identity => return Err(HttpRuntimeError::InvalidRequest),
    };
    headers.remove("content-length");
    headers.remove("etag");
    headers
        .insert("content-encoding", name.as_bytes())
        .map_err(|_| HttpRuntimeError::Protocol)?;
    append_vary_accept_encoding(headers)
}

fn append_vary_accept_encoding(headers: &mut Headers) -> Result<(), HttpRuntimeError> {
    let values = headers
        .get_all("vary")
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default();
    if values.iter().any(|value| {
        value.to_str().is_ok_and(|value| {
            value.split(',').any(|token| {
                token.trim_matches([' ', '\t']) == "*"
                    || token
                        .trim_matches([' ', '\t'])
                        .eq_ignore_ascii_case("accept-encoding")
            })
        })
    }) {
        return Ok(());
    }
    headers
        .append("vary", b"Accept-Encoding")
        .map_err(|_| HttpRuntimeError::Protocol)
}

const HTTP_UPLOAD_QUEUE_CAPACITY: usize = 1;
pub(crate) const HTTP_UPLOAD_CHUNK_LIMIT: usize = 64 * 1024;

/// Upload bytes cross from the Zirk owner task to a protocol driver through a
/// bounded channel. This keeps both the language callback and managed values
/// on their owner stack while the reactor only sees copied native bytes.
pub(crate) struct HttpUploadBody {
    receiver: tokio::sync::mpsc::Receiver<Bytes>,
}

pub(crate) struct HttpUploadWriter {
    owner: crate::task::TaskId,
    sender: Option<tokio::sync::mpsc::Sender<Bytes>>,
    total: usize,
}

impl HttpUploadBody {
    fn channel(owner: crate::task::TaskId) -> (HttpUploadWriter, Self) {
        let (sender, receiver) = tokio::sync::mpsc::channel(HTTP_UPLOAD_QUEUE_CAPACITY);
        (
            HttpUploadWriter {
                owner,
                sender: Some(sender),
                total: 0,
            },
            Self { receiver },
        )
    }
}

impl HttpUploadWriter {
    /// Copies one chunk and parks the owner task until the protocol driver has
    /// taken the previous chunk. The queue therefore retains at most 64 KiB.
    pub(crate) fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), HttpRuntimeError> {
        if crate::executor::current_task() != self.owner || self.sender.is_none() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        if bytes.len() > HTTP_UPLOAD_CHUNK_LIMIT {
            return Err(HttpRuntimeError::BodyTooLarge);
        }
        let total = self
            .total
            .checked_add(bytes.len())
            .filter(|total| *total <= MAX_HTTP_BUFFERED_BODY_BYTES)
            .ok_or(HttpRuntimeError::BodyTooLarge)?;
        let sender = self.sender.as_ref().expect("active upload sender");
        crate::network_future::block_on_current(sender.send(Bytes::copy_from_slice(bytes)))
            .map_err(|error| match error {
                crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
                crate::network_future::NetworkFutureError::CapacityExceeded => {
                    HttpRuntimeError::Transport
                }
            })?
            .map_err(|_| HttpRuntimeError::Cancelled)?;
        self.total = total;
        Ok(())
    }

    async fn write_chunk_async(&mut self, bytes: &[u8]) -> Result<(), HttpRuntimeError> {
        if crate::executor::current_task() != self.owner || self.sender.is_none() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        if bytes.len() > HTTP_UPLOAD_CHUNK_LIMIT {
            return Err(HttpRuntimeError::BodyTooLarge);
        }
        let total = self
            .total
            .checked_add(bytes.len())
            .filter(|total| *total <= MAX_HTTP_BUFFERED_BODY_BYTES)
            .ok_or(HttpRuntimeError::BodyTooLarge)?;
        self.sender
            .as_ref()
            .expect("active upload sender")
            .send(Bytes::copy_from_slice(bytes))
            .await
            .map_err(|_| HttpRuntimeError::Cancelled)?;
        self.total = total;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<(), HttpRuntimeError> {
        if crate::executor::current_task() != self.owner || self.sender.is_none() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        self.sender = None;
        Ok(())
    }
}

impl hyper::body::Body for HttpUploadBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        match self.receiver.poll_recv(context) {
            Poll::Ready(Some(bytes)) => Poll::Ready(Some(Ok(hyper::body::Frame::data(bytes)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.receiver.is_closed() && self.receiver.is_empty()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::default()
    }
}

impl HttpStreamingResponse {
    /// Wraps an already bounded response buffer in the same task-owned
    /// one-shot stream state used by live network bodies.
    pub(crate) fn from_buffered(
        status: HttpStatusCode,
        headers: Headers,
        body: Vec<u8>,
        owner: crate::task::TaskId,
    ) -> Self {
        Self {
            status,
            headers,
            body: Some(HttpStreamingBody::Buffered {
                bytes: Bytes::from(body),
                offset: 0,
            }),
            driver: None,
            pending: Bytes::new(),
            owner,
            response_idle_timeout: HttpTimeouts::default().response_idle,
            body_consumed: false,
            body_eof: false,
            stream_started: false,
        }
    }

    /// Reads at most `max_bytes` from the response. The next call continues at
    /// the following byte; body storage never grows with the full response.
    pub fn read_chunk(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, HttpRuntimeError> {
        if self.body_consumed {
            return Err(HttpRuntimeError::BodyConsumed);
        }
        if crate::executor::current_task() != self.owner || max_bytes == 0 {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        self.stream_started = true;
        self.read_chunk_inner(max_bytes)
    }

    fn read_chunk_inner(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, HttpRuntimeError> {
        if crate::executor::current_task() != self.owner {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        if max_bytes == 0 {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        if !self.pending.is_empty() {
            let count = max_bytes.min(self.pending.len());
            return Ok(Some(self.pending.split_to(count).to_vec()));
        }
        let Some(body) = self.body.as_mut() else {
            return Ok(None);
        };
        let next = crate::network_future::block_on_current(async {
            tokio::time::timeout(self.response_idle_timeout, async {
                match body {
                    HttpStreamingBody::Http1(body) => loop {
                        let Some(frame) = body.frame().await else {
                            return Ok::<Option<Bytes>, HttpRuntimeError>(None);
                        };
                        let frame = frame.map_err(|_| HttpRuntimeError::Protocol)?;
                        if let Ok(data) = frame.into_data() {
                            return Ok(Some(data));
                        }
                    },
                    HttpStreamingBody::Chunks(chunks) => chunks.recv().await.transpose(),
                    HttpStreamingBody::Buffered { bytes, offset } => {
                        if *offset >= bytes.len() {
                            return Ok(None);
                        }
                        let end = offset.saturating_add(max_bytes).min(bytes.len());
                        let chunk = bytes.slice(*offset..end);
                        *offset = end;
                        Ok(Some(chunk))
                    }
                }
            })
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })
        .and_then(|result| result);
        let next = match next {
            Ok(next) => next,
            Err(error) => {
                self.body = None;
                self.driver = None;
                self.body_eof = true;
                return Err(error);
            }
        };
        let Some(mut next) = next else {
            self.body = None;
            self.driver = None;
            self.body_eof = true;
            return Ok(None);
        };
        let count = max_bytes.min(next.len());
        self.pending = next.split_off(count);
        next.truncate(count);
        Ok(Some(next.to_vec()))
    }

    /// Consumes the response once into a bounded buffer.
    pub fn read_all(&mut self) -> Result<Vec<u8>, HttpRuntimeError> {
        if self.body_consumed || self.body_eof || self.stream_started {
            return Err(HttpRuntimeError::BodyConsumed);
        }
        self.body_consumed = true;
        let mut result = Vec::new();
        while let Some(chunk) = self.read_chunk_inner(64 * 1024)? {
            append_bounded_body(&mut result, &chunk, MAX_HTTP_BUFFERED_BODY_BYTES)?;
        }
        Ok(result)
    }

    /// Consumes the response as bounded bytes.
    pub fn read_bytes(&mut self) -> Result<Vec<u8>, HttpRuntimeError> {
        self.read_all()
    }

    /// Consumes the response as strict UTF-8 text.
    pub fn read_text(&mut self) -> Result<String, HttpRuntimeError> {
        decode_text(self.read_all()?)
    }

    /// Consumes and decodes the response as JSON. The response body limit is
    /// enforced before the JSON decoder runs.
    pub fn read_json<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, HttpRuntimeError> {
        decode_json(&self.read_all()?)
    }
}

enum HttpIo {
    Plain(tokio::net::TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    TlsOverTls(
        Box<
            tokio_rustls::client::TlsStream<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
        >,
    ),
    Upgraded(Box<TokioIo<hyper::upgrade::Upgraded>>),
}

/// A single-origin HTTP/1.1 connection that can carry sequential requests.
/// Its driver remains owned by the pool entry and is aborted when that entry
/// is evicted or its `HTTPClient` closes.
pub struct Http1ReusableConnection {
    sender: hyper::client::conn::http1::SendRequest<HttpClientRequestBody>,
    _driver: AbortOnDrop,
    origin: String,
    proxy_origin: Option<String>,
    proxy_authorization: Option<Zeroizing<String>>,
}

struct Http2ResponseData {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

struct Http2RequestCommand {
    request: Request<HttpClientRequestBody>,
    accepted: AcceptEncoding,
    timeouts: HttpTimeouts,
    response: tokio::sync::oneshot::Sender<Result<Http2ResponseData, HttpRuntimeError>>,
}

const MAX_HTTP2_CLIENT_STREAMS: usize = 8;

async fn run_http2_request(
    mut sender: hyper::client::conn::http2::SendRequest<HttpClientRequestBody>,
    command: Http2RequestCommand,
) {
    let mut response_tx = command.response;
    let result = async {
        tokio::select! {
            biased;
            _ = response_tx.closed() => return Err(HttpRuntimeError::Cancelled),
            ready = sender.ready() => ready.map_err(|_| HttpRuntimeError::Transport)?,
        }
        let response = tokio::select! {
            biased;
            _ = response_tx.closed() => return Err(HttpRuntimeError::Cancelled),
            response = tokio::time::timeout(
                command.timeouts.response_headers,
                sender.send_request(command.request),
            ) => response.map_err(|_| HttpRuntimeError::Timeout)?
                .map_err(|_| HttpRuntimeError::Transport)?,
        };
        let (parts, body) = response.into_parts();
        let mut headers = Headers::new();
        for (name, value) in parts.headers.iter() {
            headers
                .append(name.as_str(), value.as_bytes())
                .map_err(|_| HttpRuntimeError::Protocol)?;
        }
        headers
            .validate_accept_query()
            .map_err(|_| HttpRuntimeError::Protocol)?;
        let mut raw = Vec::new();
        let mut body = body;
        loop {
            let frame = tokio::select! {
                biased;
                _ = response_tx.closed() => return Err(HttpRuntimeError::Cancelled),
                frame = tokio::time::timeout(command.timeouts.response_idle, body.frame()) => {
                    frame.map_err(|_| HttpRuntimeError::Timeout)?
                }
            };
            let Some(frame) = frame else {
                break;
            };
            let frame = frame.map_err(|_| HttpRuntimeError::Protocol)?;
            if let Ok(chunk) = frame.into_data() {
                if raw.len().saturating_add(chunk.len()) > MAX_HTTP_BUFFERED_BODY_BYTES {
                    return Err(HttpRuntimeError::BodyTooLarge);
                }
                raw.extend_from_slice(&chunk);
            }
        }
        let (headers, mut body, _decoder) = decode_response_body(
            headers,
            HttpStreamingBody::Buffered {
                bytes: Bytes::from(raw),
                offset: 0,
            },
            &command.accepted,
        )?;
        let mut decoded = Vec::new();
        while let HttpStreamingBody::Chunks(ref mut chunks) = body {
            let chunk = tokio::select! {
                biased;
                _ = response_tx.closed() => return Err(HttpRuntimeError::Cancelled),
                chunk = chunks.recv() => chunk,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk = chunk?;
            if decoded.len().saturating_add(chunk.len()) > MAX_HTTP_BUFFERED_BODY_BYTES {
                return Err(HttpRuntimeError::BodyTooLarge);
            }
            decoded.extend_from_slice(&chunk);
        }
        if let HttpStreamingBody::Buffered { bytes, .. } = body {
            decoded = bytes.to_vec();
        }
        Ok(Http2ResponseData {
            status: parts.status.as_u16(),
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_string(), value.as_bytes().to_vec()))
                .collect(),
            body: decoded,
        })
    }
    .await;
    let _ = response_tx.send(result);
}

/// A reusable HTTP/2 connection whose protocol state machine remains on the
/// native reactor. The bounded command channel keeps Zirk task stacks and
/// managed references out of the HTTP/2 worker.
struct Http2ReusableConnection {
    sender: tokio::sync::mpsc::Sender<Http2RequestCommand>,
    _worker: crate::network_future::ReactorTask<()>,
    origin: String,
}

#[derive(Clone)]
pub(crate) struct Http2RequestSender {
    sender: tokio::sync::mpsc::Sender<Http2RequestCommand>,
    origin: String,
}

impl Http2ReusableConnection {
    fn connect(
        io: HttpIo,
        origin: String,
        timeouts: HttpTimeouts,
    ) -> Result<Self, HttpRuntimeError> {
        let (sender, mut requests) =
            tokio::sync::mpsc::channel::<Http2RequestCommand>(MAX_HTTP2_CLIENT_STREAMS);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let worker = crate::network_future::spawn_on_reactor(async move {
            let handshake = tokio::time::timeout(
                timeouts.response_headers,
                hyper::client::conn::http2::handshake(
                    hyper_util::rt::TokioExecutor::new(),
                    TokioIo::new(io),
                ),
            )
            .await
            .map_err(|_| HttpRuntimeError::Timeout)
            .and_then(|result| result.map_err(|_| HttpRuntimeError::Protocol));
            let (sender, connection) = match handshake {
                Ok(pair) => {
                    let _ = ready_tx.send(Ok(()));
                    pair
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let driver = tokio::spawn(connection);
            let _driver = AbortOnDrop(driver.abort_handle());
            let mut in_flight = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    biased;
                    result = in_flight.join_next(), if !in_flight.is_empty() => {
                        let _ = result;
                    }
                    command = requests.recv(), if in_flight.len() < MAX_HTTP2_CLIENT_STREAMS => {
                        let Some(command) = command else { break; };
                        in_flight.spawn(run_http2_request(sender.clone(), command));
                    }
                    else => break,
                }
            }
        });
        let ready = crate::network_future::block_on_current(ready_rx)
            .map_err(|_| HttpRuntimeError::Transport)?
            .map_err(|_| HttpRuntimeError::Transport)?;
        ready?;
        Ok(Self {
            sender,
            _worker: worker,
            origin,
        })
    }

    fn request_sender(&self) -> Http2RequestSender {
        Http2RequestSender {
            sender: self.sender.clone(),
            origin: self.origin.clone(),
        }
    }

    fn send(
        &self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        self.request_sender().send(request, grant, timeouts)
    }
}

impl Http2RequestSender {
    pub(crate) fn send(
        &self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if request.url.origin() != self.origin
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        validate_client_request(&request)?;
        request
            .headers
            .validate_message_framing()
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let accepted = AcceptEncoding::parse(&request.headers)
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        if request.headers.get("content-length").is_some()
            || request.headers.get("transfer-encoding").is_some()
            || request.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES
        {
            return Err(if request.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
                HttpRuntimeError::BodyTooLarge
            } else {
                HttpRuntimeError::InvalidRequest
            });
        }
        grant
            .authorize_origin(&request.url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;
        let expected_authority = authority(&request.url);
        if request.headers.get_all("host").is_some_and(|values| {
            let values = values.collect::<Vec<_>>();
            !values.is_empty()
                && (values.len() != 1
                    || values
                        .iter()
                        .any(|value| value.as_bytes() != expected_authority.as_bytes()))
        }) {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let mut builder = Request::builder()
            .method(
                hyper::Method::from_bytes(request.method_token.as_bytes())
                    .map_err(|_| HttpRuntimeError::InvalidRequest)?,
            )
            .uri(request_target(&request.url))
            .header(hyper::header::HOST, expected_authority);
        for (name, value) in request.headers.iter() {
            if name != hyper::header::HOST && name != hyper::header::CONNECTION {
                builder = builder.header(name, value);
            }
        }
        if request.headers.get("accept-encoding").is_none() {
            builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
        }
        let outgoing = builder
            .body(buffered_client_request_body(request.body))
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let (response, result_rx) = tokio::sync::oneshot::channel();
        let command = Http2RequestCommand {
            request: outgoing,
            accepted,
            timeouts,
            response,
        };
        let result = crate::network_future::block_on_current(async {
            tokio::time::timeout(timeouts.response_headers, self.sender.send(command))
                .await
                .map_err(|_| HttpRuntimeError::Timeout)?
                .map_err(|_| HttpRuntimeError::Transport)?;
            tokio::time::timeout(timeouts.response_headers, result_rx)
                .await
                .map_err(|_| HttpRuntimeError::Timeout)?
                .map_err(|_| HttpRuntimeError::Transport)?
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })??;
        let mut headers = Headers::new();
        for (name, value) in result.headers {
            headers
                .append(&name, &value)
                .map_err(|_| HttpRuntimeError::Protocol)?;
        }
        Ok(HttpResponseValues {
            status: HttpStatusCode::new(result.status).map_err(|_| HttpRuntimeError::Protocol)?,
            headers,
            body: result.body,
            body_stream: None,
            body_producer: None,
        })
    }

    pub(crate) fn send_streaming(
        &self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
        mut producer: impl FnMut() -> Result<Option<Vec<u8>>, HttpRuntimeError>,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if request.url.origin() != self.origin
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
            || !request.body.is_empty()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        validate_client_request(&request)?;
        request
            .headers
            .validate_message_framing()
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let accepted = AcceptEncoding::parse(&request.headers)
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        if request.headers.get("content-length").is_some()
            || request.headers.get("transfer-encoding").is_some()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        grant
            .authorize_origin(&request.url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;
        let expected_authority = authority(&request.url);
        if request.headers.get_all("host").is_some_and(|values| {
            let values = values.collect::<Vec<_>>();
            !values.is_empty()
                && (values.len() != 1
                    || values
                        .iter()
                        .any(|value| value.as_bytes() != expected_authority.as_bytes()))
        }) {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let owner = crate::executor::current_task();
        let (mut upload, body) = HttpUploadBody::channel(owner);
        let mut builder = Request::builder()
            .method(
                hyper::Method::from_bytes(request.method_token.as_bytes())
                    .map_err(|_| HttpRuntimeError::InvalidRequest)?,
            )
            .uri(request_target(&request.url))
            .header(hyper::header::HOST, expected_authority);
        for (name, value) in request.headers.iter() {
            if name != hyper::header::HOST && name != hyper::header::CONNECTION {
                builder = builder.header(name, value);
            }
        }
        if request.headers.get("accept-encoding").is_none() {
            builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
        }
        let outgoing = builder
            .body(body.boxed_unsync())
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let command = Http2RequestCommand {
            request: outgoing,
            accepted,
            timeouts,
            response: response_tx,
        };
        let result = crate::network_future::block_on_current(async move {
            let exchange = async {
                tokio::time::timeout(timeouts.response_headers, self.sender.send(command))
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Transport)?;
                tokio::time::timeout(timeouts.response_headers, response_rx)
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Transport)?
            };
            let upload_future = async {
                loop {
                    if crate::executor::current_task_cancel_requested() {
                        return Err(HttpRuntimeError::Cancelled);
                    }
                    match producer()? {
                        None => break,
                        Some(chunk) if chunk.is_empty() => break,
                        Some(chunk) => {
                            if chunk.len() > HTTP_UPLOAD_CHUNK_LIMIT {
                                return Err(HttpRuntimeError::BodyTooLarge);
                            }
                            tokio::time::timeout(
                                timeouts.response_idle,
                                upload.write_chunk_async(&chunk),
                            )
                            .await
                            .map_err(|_| HttpRuntimeError::Timeout)??;
                        }
                    }
                }
                upload.finish()?;
                Ok::<(), HttpRuntimeError>(())
            };
            let (response, ()) = tokio::try_join!(exchange, upload_future)?;
            Ok::<_, HttpRuntimeError>(response)
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })??;
        let mut headers = Headers::new();
        for (name, value) in result.headers {
            headers
                .append(&name, &value)
                .map_err(|_| HttpRuntimeError::Protocol)?;
        }
        Ok(HttpResponseValues {
            status: HttpStatusCode::new(result.status).map_err(|_| HttpRuntimeError::Protocol)?,
            headers,
            body: result.body,
            body_stream: None,
            body_producer: None,
        })
    }
}

enum HttpReusableConnection {
    Http1(Http1ReusableConnection),
    Http2(Http2ReusableConnection),
}

impl HttpReusableConnection {
    fn http2_sender(&self) -> Option<Http2RequestSender> {
        match self {
            Self::Http1(_) => None,
            Self::Http2(connection) => Some(connection.request_sender()),
        }
    }

    fn send(
        &mut self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        match self {
            Self::Http1(connection) => connection.send(request, grant, timeouts),
            Self::Http2(connection) => connection.send(request, grant, timeouts),
        }
    }

    fn send_streaming(
        &mut self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
        producer: impl FnMut() -> Result<Option<Vec<u8>>, HttpRuntimeError>,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        match self {
            Self::Http1(connection) => {
                connection.send_streaming(request, grant, timeouts, producer)
            }
            Self::Http2(connection) => connection
                .request_sender()
                .send_streaming(request, grant, timeouts, producer),
        }
    }

    fn connect(
        url: &zirk_runtime_core::http_values::Url,
        grant: Arc<EffectiveNetworkGrant>,
        timeouts: HttpTimeouts,
        tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
        proxy: Option<&HttpProxy>,
    ) -> Result<Self, HttpRuntimeError> {
        if url.scheme() == "https" && proxy.is_none() {
            let (stream, protocol) = match tls_config {
                Some(config) => TlsClientStream::connect_for_http_with_config(
                    url.as_str(),
                    Arc::clone(&grant),
                    timeouts.connect,
                    config,
                ),
                None => TlsClientStream::connect_for_http(
                    url.as_str(),
                    Arc::clone(&grant),
                    timeouts.connect,
                ),
            }
            .map_err(HttpRuntimeError::from)?;
            let io = HttpIo::Tls(Box::new(stream.into_inner()));
            return match protocol {
                NegotiatedHttpProtocol::Http2 => Ok(Self::Http2(Http2ReusableConnection::connect(
                    io,
                    url.origin(),
                    timeouts,
                )?)),
                NegotiatedHttpProtocol::Http1 => Ok(Self::Http1(Http1ReusableConnection::from_io(
                    io,
                    url.origin(),
                    None,
                    None,
                    timeouts,
                )?)),
            };
        }
        Ok(Self::Http1(Http1ReusableConnection::connect(
            url, grant, timeouts, tls_config, proxy,
        )?))
    }
}

impl Http1ReusableConnection {
    pub fn connect(
        url: &zirk_runtime_core::http_values::Url,
        grant: Arc<EffectiveNetworkGrant>,
        timeouts: HttpTimeouts,
        tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
        proxy: Option<&HttpProxy>,
    ) -> Result<Self, HttpRuntimeError> {
        if timeouts.connect.is_zero() || timeouts.response_headers.is_zero() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        grant
            .authorize_origin(&url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;
        let io = if let Some(proxy) = proxy {
            let proxy_origin = proxy.origin();
            grant
                .authorize_origin(&proxy_origin)
                .map_err(|error| match error {
                    zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                        HttpRuntimeError::PermissionDenied
                    }
                    _ => HttpRuntimeError::InvalidRequest,
                })?;
            if proxy.as_str().starts_with("https://") {
                let proxy_tls_config = tls_config.clone().unwrap_or_else(|| {
                    crate::network_transport::client_tls_config_for_http()
                        .expect("system TLS roots are initialized for HTTP clients")
                });
                let proxy_tls = TlsClientStream::connect_for_http1_with_config(
                    proxy.as_str(),
                    Arc::clone(&grant),
                    timeouts.connect,
                    proxy_tls_config,
                )
                .map_err(HttpRuntimeError::from)?;
                let stream = proxy_tls.into_inner();
                if url.scheme() == "https" {
                    let stream = establish_http_connect_tunnel(
                        stream,
                        &authority(url),
                        proxy.authorization_header(),
                        timeouts.response_headers,
                    )?;
                    let config = tls_config.unwrap_or_else(|| {
                        crate::network_transport::client_tls_config_for_http()
                            .expect("system TLS roots are initialized for HTTP clients")
                    });
                    let stream = TlsClientStream::connect_tls_over_tunnel(
                        url.as_str(),
                        stream,
                        timeouts.connect,
                        config,
                    )
                    .map_err(HttpRuntimeError::from)?;
                    HttpIo::TlsOverTls(Box::new(stream))
                } else {
                    HttpIo::Tls(Box::new(stream))
                }
            } else {
                let proxy_stream = TcpStream::connect_with_timeout(
                    &proxy_origin,
                    Arc::clone(&grant),
                    timeouts.connect,
                )
                .map_err(HttpRuntimeError::from)?;
                let peer = proxy_stream.peer_address();
                let stream = proxy_stream.into_inner();
                if url.scheme() == "https" {
                    let stream = establish_http_connect_tunnel(
                        stream,
                        &authority(url),
                        proxy.authorization_header(),
                        timeouts.response_headers,
                    )?;
                    let config = tls_config.unwrap_or_else(|| {
                        crate::network_transport::client_tls_config_for_http()
                            .expect("system TLS roots are initialized for HTTP clients")
                    });
                    let stream = TlsClientStream::connect_for_http1_over_tunnel(
                        url.as_str(),
                        stream,
                        peer,
                        timeouts.connect,
                        config,
                    )
                    .map_err(HttpRuntimeError::from)?;
                    HttpIo::Tls(Box::new(stream.into_inner()))
                } else {
                    HttpIo::Plain(stream)
                }
            }
        } else if url.scheme() == "https" {
            let stream = match tls_config {
                Some(config) => TlsClientStream::connect_for_http1_with_config(
                    url.as_str(),
                    grant,
                    timeouts.connect,
                    config,
                ),
                None => TlsClientStream::connect_for_http1(url.as_str(), grant, timeouts.connect),
            }
            .map_err(HttpRuntimeError::from)?;
            HttpIo::Tls(Box::new(stream.into_inner()))
        } else {
            let stream = TcpStream::connect_with_timeout(url.as_str(), grant, timeouts.connect)
                .map_err(HttpRuntimeError::from)?;
            HttpIo::Plain(stream.into_inner())
        };
        Self::from_io(
            io,
            url.origin(),
            proxy.map(HttpProxy::origin),
            proxy
                .and_then(HttpProxy::authorization_header)
                .map(|value| Zeroizing::new(value.to_owned())),
            timeouts,
        )
    }

    fn from_io(
        io: HttpIo,
        origin: String,
        proxy_origin: Option<String>,
        proxy_authorization: Option<Zeroizing<String>>,
        timeouts: HttpTimeouts,
    ) -> Result<Self, HttpRuntimeError> {
        let (sender, driver) = crate::network_future::block_on_current(async move {
            let (sender, connection) = tokio::time::timeout(
                timeouts.response_headers,
                hyper::client::conn::http1::handshake(TokioIo::new(io)),
            )
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
            .map_err(|_| HttpRuntimeError::Protocol)?;
            let driver = tokio::spawn(connection);
            Ok::<_, HttpRuntimeError>((sender, AbortOnDrop(driver.abort_handle())))
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })??;
        Ok(Self {
            sender,
            _driver: driver,
            origin,
            proxy_origin,
            proxy_authorization,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn send(
        &mut self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if request.url.origin() != self.origin
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        validate_client_request(&request)?;
        request
            .headers
            .validate_message_framing()
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let accepted = AcceptEncoding::parse(&request.headers)
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        if request.headers.get("content-length").is_some()
            || request.headers.get("transfer-encoding").is_some()
            || request.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES
        {
            return Err(if request.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
                HttpRuntimeError::BodyTooLarge
            } else {
                HttpRuntimeError::InvalidRequest
            });
        }
        grant
            .authorize_origin(&request.url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;

        let expected_authority = authority(&request.url);
        if request.headers.get_all("host").is_some_and(|values| {
            let values = values.collect::<Vec<_>>();
            !values.is_empty()
                && (values.len() != 1
                    || values
                        .iter()
                        .any(|value| value.as_bytes() != expected_authority.as_bytes()))
        }) {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let mut builder = Request::builder()
            .method(
                hyper::Method::from_bytes(request.method_token.as_bytes())
                    .map_err(|_| HttpRuntimeError::InvalidRequest)?,
            )
            .uri(
                if self.proxy_origin.is_some() && request.url.scheme() == "http" {
                    request.url.as_str().to_owned()
                } else {
                    request_target(&request.url)
                },
            )
            .header(hyper::header::HOST, expected_authority);
        if self.proxy_origin.is_some()
            && request.url.scheme() == "http"
            && let Some(proxy_authorization) = &self.proxy_authorization
        {
            builder = builder.header("proxy-authorization", proxy_authorization.as_str());
        }
        for (name, value) in request.headers.iter() {
            if name != hyper::header::HOST && name != hyper::header::CONNECTION {
                builder = builder.header(name, value);
            }
        }
        if request.headers.get("accept-encoding").is_none() {
            builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
        }
        let outgoing = builder
            .body(buffered_client_request_body(request.body))
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let sender = &mut self.sender;
        crate::network_future::block_on_current(async move {
            let response =
                tokio::time::timeout(timeouts.response_headers, sender.send_request(outgoing))
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Transport)?;
            let (parts, body) = response.into_parts();
            let status = HttpStatusCode::new(parts.status.as_u16())
                .map_err(|_| HttpRuntimeError::Protocol)?;
            let mut headers = Headers::new();
            for (name, value) in parts.headers.iter() {
                headers
                    .append(name.as_str(), value.as_bytes())
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            }
            headers
                .validate_accept_query()
                .map_err(|_| HttpRuntimeError::Protocol)?;
            let (headers, mut body, _decoder) =
                decode_response_body(headers, HttpStreamingBody::Http1(body), &accepted)?;
            let mut bytes = Vec::new();
            loop {
                let chunk = tokio::time::timeout(timeouts.response_idle, async {
                    match &mut body {
                        HttpStreamingBody::Http1(body) => loop {
                            let Some(frame) = body.frame().await else {
                                return Ok(None);
                            };
                            let frame = frame.map_err(|_| HttpRuntimeError::Protocol)?;
                            if let Ok(data) = frame.into_data() {
                                return Ok(Some(data));
                            }
                        },
                        HttpStreamingBody::Chunks(receiver) => receiver.recv().await.transpose(),
                        HttpStreamingBody::Buffered { bytes, offset } => {
                            if *offset >= bytes.len() {
                                return Ok(None);
                            }
                            let end = offset.saturating_add(64 * 1024).min(bytes.len());
                            let chunk = bytes.slice(*offset..end);
                            *offset = end;
                            Ok(Some(chunk))
                        }
                    }
                })
                .await
                .map_err(|_| HttpRuntimeError::Timeout)??;
                let Some(chunk) = chunk else { break };
                if bytes.len().saturating_add(chunk.len()) > MAX_HTTP_BUFFERED_BODY_BYTES {
                    return Err(HttpRuntimeError::BodyTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(HttpResponseValues {
                status,
                headers,
                body: bytes,
                body_stream: None,
                body_producer: None,
            })
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?
    }

    fn send_streaming(
        &mut self,
        request: HttpRequestValues,
        grant: &EffectiveNetworkGrant,
        timeouts: HttpTimeouts,
        mut producer: impl FnMut() -> Result<Option<Vec<u8>>, HttpRuntimeError>,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if request.url.origin() != self.origin
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
            || !request.body.is_empty()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        validate_client_request(&request)?;
        request
            .headers
            .validate_message_framing()
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let accepted = AcceptEncoding::parse(&request.headers)
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        if request.headers.get("content-length").is_some()
            || request.headers.get("transfer-encoding").is_some()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        grant
            .authorize_origin(&request.url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;
        let expected_authority = authority(&request.url);
        if request.headers.get_all("host").is_some_and(|values| {
            let values = values.collect::<Vec<_>>();
            !values.is_empty()
                && (values.len() != 1
                    || values
                        .iter()
                        .any(|value| value.as_bytes() != expected_authority.as_bytes()))
        }) {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let owner = crate::executor::current_task();
        let (mut upload, body) = HttpUploadBody::channel(owner);
        let mut builder = Request::builder()
            .method(
                hyper::Method::from_bytes(request.method_token.as_bytes())
                    .map_err(|_| HttpRuntimeError::InvalidRequest)?,
            )
            .uri(
                if self.proxy_origin.is_some() && request.url.scheme() == "http" {
                    request.url.as_str().to_owned()
                } else {
                    request_target(&request.url)
                },
            )
            .header(hyper::header::HOST, expected_authority);
        if self.proxy_origin.is_some()
            && request.url.scheme() == "http"
            && let Some(proxy_authorization) = &self.proxy_authorization
        {
            builder = builder.header("proxy-authorization", proxy_authorization.as_str());
        }
        for (name, value) in request.headers.iter() {
            if name != hyper::header::HOST && name != hyper::header::CONNECTION {
                builder = builder.header(name, value);
            }
        }
        if request.headers.get("accept-encoding").is_none() {
            builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
        }
        let outgoing = builder
            .body(body.boxed_unsync())
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        let sender = &mut self.sender;
        let response = crate::network_future::block_on_current(async move {
            let response_future = async {
                tokio::time::timeout(timeouts.response_headers, sender.send_request(outgoing))
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Transport)
            };
            let upload_future = async {
                loop {
                    if crate::executor::current_task_cancel_requested() {
                        return Err(HttpRuntimeError::Cancelled);
                    }
                    match producer()? {
                        None => break,
                        Some(chunk) if chunk.is_empty() => break,
                        Some(chunk) => {
                            if chunk.len() > HTTP_UPLOAD_CHUNK_LIMIT {
                                return Err(HttpRuntimeError::BodyTooLarge);
                            }
                            tokio::time::timeout(
                                timeouts.response_idle,
                                upload.write_chunk_async(&chunk),
                            )
                            .await
                            .map_err(|_| HttpRuntimeError::Timeout)??;
                        }
                    }
                }
                upload.finish()?;
                Ok::<(), HttpRuntimeError>(())
            };
            let (response, ()) = tokio::try_join!(response_future, upload_future)?;
            let (parts, body) = response.into_parts();
            let status = HttpStatusCode::new(parts.status.as_u16())
                .map_err(|_| HttpRuntimeError::Protocol)?;
            let mut headers = Headers::new();
            for (name, value) in parts.headers.iter() {
                headers
                    .append(name.as_str(), value.as_bytes())
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            }
            headers
                .validate_accept_query()
                .map_err(|_| HttpRuntimeError::Protocol)?;
            let (headers, mut body, _decoder) =
                decode_response_body(headers, HttpStreamingBody::Http1(body), &accepted)?;
            let mut bytes = Vec::new();
            loop {
                let chunk = tokio::time::timeout(timeouts.response_idle, async {
                    match &mut body {
                        HttpStreamingBody::Http1(body) => loop {
                            let Some(frame) = body.frame().await else {
                                return Ok(None);
                            };
                            let frame = frame.map_err(|_| HttpRuntimeError::Protocol)?;
                            if let Ok(data) = frame.into_data() {
                                return Ok(Some(data));
                            }
                        },
                        HttpStreamingBody::Chunks(receiver) => receiver.recv().await.transpose(),
                        HttpStreamingBody::Buffered { bytes, offset } => {
                            if *offset >= bytes.len() {
                                return Ok(None);
                            }
                            let end = offset.saturating_add(64 * 1024).min(bytes.len());
                            let chunk = bytes.slice(*offset..end);
                            *offset = end;
                            Ok(Some(chunk))
                        }
                    }
                })
                .await
                .map_err(|_| HttpRuntimeError::Timeout)??;
                let Some(chunk) = chunk else { break };
                if bytes.len().saturating_add(chunk.len()) > MAX_HTTP_BUFFERED_BODY_BYTES {
                    return Err(HttpRuntimeError::BodyTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok::<_, HttpRuntimeError>(HttpResponseValues {
                status,
                headers,
                body: bytes,
                body_stream: None,
                body_producer: None,
            })
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })??;
        Ok(response)
    }
}

fn establish_http_connect_tunnel<S>(
    mut stream: S,
    destination_authority: &str,
    proxy_authorization: Option<&str>,
    timeout: Duration,
) -> Result<S, HttpRuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut request = match proxy_authorization {
        Some(value) => format!(
            "CONNECT {destination_authority} HTTP/1.1\r\nHost: {destination_authority}\r\nProxy-Authorization: {value}\r\nProxy-Connection: keep-alive\r\n\r\n"
        ),
        None => format!(
            "CONNECT {destination_authority} HTTP/1.1\r\nHost: {destination_authority}\r\nProxy-Connection: keep-alive\r\n\r\n"
        ),
    };
    crate::network_future::block_on_current(async move {
        let handshake = tokio::time::timeout(timeout, async {
            let write_result = stream
                .write_all(request.as_bytes())
                .await
                .map_err(|_| HttpRuntimeError::Transport);
            request.zeroize();
            write_result?;
            let mut response = Vec::with_capacity(256);
            while !response.ends_with(b"\r\n\r\n") {
                if response.len() >= 64 * 1024 {
                    return Err(HttpRuntimeError::Protocol);
                }
                let mut byte = [0u8; 1];
                let count = stream
                    .read(&mut byte)
                    .await
                    .map_err(|_| HttpRuntimeError::Transport)?;
                if count == 0 {
                    return Err(HttpRuntimeError::Protocol);
                }
                response.push(byte[0]);
            }
            let response =
                std::str::from_utf8(&response).map_err(|_| HttpRuntimeError::Protocol)?;
            let mut lines = response.split("\r\n");
            let status = lines.next().ok_or(HttpRuntimeError::Protocol)?;
            if status != "HTTP/1.1 200"
                && status != "HTTP/1.0 200"
                && !status.starts_with("HTTP/1.1 200 ")
                && !status.starts_with("HTTP/1.0 200 ")
            {
                return Err(HttpRuntimeError::Transport);
            }
            let mut content_length_seen = false;
            for line in lines.take_while(|line| !line.is_empty()) {
                let Some((name, value)) = line.split_once(':') else {
                    return Err(HttpRuntimeError::Protocol);
                };
                if name.eq_ignore_ascii_case("transfer-encoding") {
                    return Err(HttpRuntimeError::Protocol);
                }
                if name.eq_ignore_ascii_case("content-length") {
                    if content_length_seen || value.trim() != "0" {
                        return Err(HttpRuntimeError::Protocol);
                    }
                    content_length_seen = true;
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| HttpRuntimeError::Timeout)?;
        handshake?;
        Ok::<_, HttpRuntimeError>(stream)
    })
    .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })?
}

/// Owner-task HTTP/1.1 connection pool for a single client configuration.
/// Each normalized origin retains at most one connection, and the number of
/// origins is bounded when the pool is created. This pool uses system TLS
/// roots and no proxy; those immutable settings are therefore shared by every
/// entry in this instance.
pub struct HttpClientPool {
    owner: Option<crate::task::TaskId>,
    max_origins: usize,
    timeouts: HttpTimeouts,
    tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    proxy: Option<HttpProxy>,
    sequence: u64,
    connections: HashMap<String, (HttpReusableConnection, u64)>,
    cookies: Option<CookieJar>,
}

impl HttpClientPool {
    pub const MAX_ORIGINS: usize = 32;

    /// Creates a client pool owned by the current Zirk task. A client cannot
    /// be moved to another task; dropping it deterministically closes sockets.
    pub fn new(max_origins: usize, timeouts: HttpTimeouts) -> Result<Self, HttpRuntimeError> {
        Self::new_for_owner(max_origins, timeouts, Some(crate::executor::current_task()))
    }

    /// Creates a pool whose synchronization is provided by its owning shared
    /// client handle. Callers must serialize access to this mutable pool.
    pub fn new_shared(
        max_origins: usize,
        timeouts: HttpTimeouts,
    ) -> Result<Self, HttpRuntimeError> {
        Self::new_for_owner(max_origins, timeouts, None)
    }

    fn new_for_owner(
        max_origins: usize,
        timeouts: HttpTimeouts,
        owner: Option<crate::task::TaskId>,
    ) -> Result<Self, HttpRuntimeError> {
        if !(1..=Self::MAX_ORIGINS).contains(&max_origins)
            || timeouts.connect.is_zero()
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        Ok(Self {
            owner,
            max_origins,
            timeouts,
            tls_config: None,
            proxy: None,
            sequence: 0,
            connections: HashMap::new(),
            cookies: None,
        })
    }

    pub fn new_with_tls_config(
        max_origins: usize,
        timeouts: HttpTimeouts,
        tls_config: Arc<crate::network_transport::RustlsClientConfig>,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new(max_origins, timeouts)?;
        pool.tls_config = Some(tls_config);
        Ok(pool)
    }

    pub fn new_shared_with_tls_config(
        max_origins: usize,
        timeouts: HttpTimeouts,
        tls_config: Arc<crate::network_transport::RustlsClientConfig>,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new_shared(max_origins, timeouts)?;
        pool.tls_config = Some(tls_config);
        Ok(pool)
    }

    pub fn new_with_proxy(
        max_origins: usize,
        timeouts: HttpTimeouts,
        proxy: HttpProxy,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new(max_origins, timeouts)?;
        pool.proxy = Some(proxy);
        Ok(pool)
    }

    pub fn new_shared_with_proxy(
        max_origins: usize,
        timeouts: HttpTimeouts,
        proxy: HttpProxy,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new_shared(max_origins, timeouts)?;
        pool.proxy = Some(proxy);
        Ok(pool)
    }

    pub fn new_with_tls_config_and_proxy(
        max_origins: usize,
        timeouts: HttpTimeouts,
        tls_config: Arc<crate::network_transport::RustlsClientConfig>,
        proxy: HttpProxy,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new_with_proxy(max_origins, timeouts, proxy)?;
        pool.tls_config = Some(tls_config);
        Ok(pool)
    }

    pub fn new_shared_with_tls_config_and_proxy(
        max_origins: usize,
        timeouts: HttpTimeouts,
        tls_config: Arc<crate::network_transport::RustlsClientConfig>,
        proxy: HttpProxy,
    ) -> Result<Self, HttpRuntimeError> {
        let mut pool = Self::new_shared_with_proxy(max_origins, timeouts, proxy)?;
        pool.tls_config = Some(tls_config);
        Ok(pool)
    }

    /// Enables an isolated, memory-only cookie jar for this client instance.
    pub fn enable_cookies(&mut self) {
        self.cookies.get_or_insert_with(CookieJar::new);
    }

    /// Removes the client cookie jar and zeroizes stored cookie values.
    pub fn clear_cookies(&mut self) {
        if let Some(cookies) = &mut self.cookies {
            cookies.clear();
        }
    }

    /// Establishes and stores a shared client's connection without sending a
    /// request. The caller can then drop its cooperative pool gate before
    /// dispatching the first HTTP/2 stream.
    pub(crate) fn ensure_shared_connection(
        &mut self,
        request: &HttpRequestValues,
        grant: Arc<EffectiveNetworkGrant>,
    ) -> Result<(), HttpRuntimeError> {
        if self.owner.is_some() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let origin = request.url.origin();
        if self.connections.contains_key(&origin) {
            return Ok(());
        }
        if let Err(error) = grant.authorize_origin(&origin) {
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        if let Some(proxy) = &self.proxy
            && let Err(error) = grant.authorize_origin(&proxy.origin())
        {
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        let connection = HttpReusableConnection::connect(
            &request.url,
            Arc::clone(&grant),
            self.timeouts,
            self.tls_config.clone(),
            self.proxy.as_ref(),
        )?;
        self.sequence = self.sequence.wrapping_add(1).max(1);
        if self.connections.len() >= self.max_origins
            && let Some(oldest) = self
                .connections
                .iter()
                .min_by_key(|(_, (_, last_used))| *last_used)
                .map(|(origin, _)| origin.clone())
        {
            self.connections.remove(&oldest);
        }
        self.connections.insert(origin, (connection, self.sequence));
        Ok(())
    }

    /// Sends one bounded request. The exact origin grant is checked for every
    /// call, including calls reusing an already-connected socket.
    pub fn request(
        &mut self,
        mut request: HttpRequestValues,
        grant: Arc<EffectiveNetworkGrant>,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if self
            .owner
            .is_some_and(|owner| crate::executor::current_task() != owner)
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let origin = request.url.origin();
        let request_url = request.url.clone();
        if let Err(error) = grant.authorize_origin(&origin) {
            self.connections.remove(&origin);
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        if let Some(proxy) = &self.proxy
            && let Err(error) = grant.authorize_origin(&proxy.origin())
        {
            self.connections.remove(&origin);
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        let now = SystemTime::now();
        if let Some(cookies) = &mut self.cookies
            && request.headers.get("cookie").is_none()
            && let Some(cookie_header) = cookies.cookie_header(
                &request.url,
                Some(&request.url),
                false,
                request.method.is_safe(),
                now,
            )
        {
            request
                .headers
                .insert("cookie", cookie_header.as_str().as_bytes())
                .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        }
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let sequence = self.sequence;
        let timeouts = self.timeouts;
        if let Some((mut connection, _)) = self.connections.remove(&origin) {
            let response = connection.send(request, &grant, timeouts)?;
            self.store_response_cookies(&response.headers, &request_url, SystemTime::now());
            if !response_connection_closes(&response.headers) {
                self.connections.insert(origin, (connection, sequence));
            }
            return Ok(response);
        }

        let mut connection = HttpReusableConnection::connect(
            &request.url,
            Arc::clone(&grant),
            timeouts,
            self.tls_config.clone(),
            self.proxy.as_ref(),
        )?;
        let response = connection.send(request, &grant, timeouts)?;
        self.store_response_cookies(&response.headers, &request_url, SystemTime::now());
        if !response_connection_closes(&response.headers) {
            if self.connections.len() >= self.max_origins
                && let Some(oldest) = self
                    .connections
                    .iter()
                    .min_by_key(|(_, (_, last_used))| *last_used)
                    .map(|(origin, _)| origin.clone())
            {
                self.connections.remove(&oldest);
            }
            self.connections.insert(origin, (connection, sequence));
        }
        Ok(response)
    }

    /// Sends a bounded one-pass producer body through this client's pooled
    /// HTTP/1.1 or HTTP/2 connection. Producer callbacks stay on the current
    /// Zirk task; the transport sees only copied chunks.
    pub fn request_streaming(
        &mut self,
        mut request: HttpRequestValues,
        grant: Arc<EffectiveNetworkGrant>,
        mut producer: impl FnMut() -> Result<Option<Vec<u8>>, HttpRuntimeError>,
    ) -> Result<HttpResponseValues, HttpRuntimeError> {
        if self
            .owner
            .is_some_and(|owner| crate::executor::current_task() != owner)
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let origin = request.url.origin();
        let request_url = request.url.clone();
        grant
            .authorize_origin(&origin)
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;
        if let Some(proxy) = &self.proxy {
            grant
                .authorize_origin(&proxy.origin())
                .map_err(|error| match error {
                    zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                        HttpRuntimeError::PermissionDenied
                    }
                    _ => HttpRuntimeError::InvalidRequest,
                })?;
        }
        if let Some(cookies) = &mut self.cookies
            && request.headers.get("cookie").is_none()
            && let Some(cookie_header) = cookies.cookie_header(
                &request.url,
                Some(&request.url),
                false,
                request.method.is_safe(),
                SystemTime::now(),
            )
        {
            request
                .headers
                .insert("cookie", cookie_header.as_str().as_bytes())
                .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        }
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let sequence = self.sequence;
        let timeouts = self.timeouts;
        let mut connection = match self.connections.remove(&origin) {
            Some((connection, _)) => connection,
            None => HttpReusableConnection::connect(
                &request.url,
                Arc::clone(&grant),
                timeouts,
                self.tls_config.clone(),
                self.proxy.as_ref(),
            )?,
        };
        let response = connection.send_streaming(request, &grant, timeouts, &mut producer)?;
        self.store_response_cookies(&response.headers, &request_url, SystemTime::now());
        if !response_connection_closes(&response.headers) {
            if self.connections.len() >= self.max_origins
                && let Some(oldest) = self
                    .connections
                    .iter()
                    .min_by_key(|(_, (_, last_used))| *last_used)
                    .map(|(origin, _)| origin.clone())
            {
                self.connections.remove(&oldest);
            }
            self.connections.insert(origin, (connection, sequence));
        }
        Ok(response)
    }

    /// Prepares a request on an established HTTP/2 connection so the caller
    /// can release the pool lock before awaiting its multiplexed stream.
    /// Returns None for a cold origin or HTTP/1.1 connection.
    pub(crate) fn prepare_shared_http2(
        &mut self,
        request: &mut HttpRequestValues,
        grant: &EffectiveNetworkGrant,
    ) -> Result<Option<Http2RequestSender>, HttpRuntimeError> {
        if self.owner.is_some() {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let origin = request.url.origin();
        if let Err(error) = grant.authorize_origin(&origin) {
            self.connections.remove(&origin);
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        if let Some(proxy) = &self.proxy
            && let Err(error) = grant.authorize_origin(&proxy.origin())
        {
            self.connections.remove(&origin);
            return Err(match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            });
        }
        let Some(sender) = self
            .connections
            .get(&origin)
            .and_then(|(connection, _)| connection.http2_sender())
        else {
            return Ok(None);
        };
        if let Some(cookies) = &mut self.cookies
            && request.headers.get("cookie").is_none()
            && let Some(cookie_header) = cookies.cookie_header(
                &request.url,
                Some(&request.url),
                false,
                request.method.is_safe(),
                SystemTime::now(),
            )
        {
            request
                .headers
                .insert("cookie", cookie_header.as_str().as_bytes())
                .map_err(|_| HttpRuntimeError::InvalidRequest)?;
        }
        self.sequence = self.sequence.wrapping_add(1).max(1);
        if let Some((_, last_used)) = self.connections.get_mut(&origin) {
            *last_used = self.sequence;
        }
        Ok(Some(sender))
    }

    /// Applies response policy after an HTTP/2 stream completes outside the
    /// client pool lock.
    pub(crate) fn finish_shared_http2(
        &mut self,
        origin: &str,
        request_url: &zirk_runtime_core::http_values::Url,
        response: &HttpResponseValues,
    ) {
        self.store_response_cookies(&response.headers, request_url, SystemTime::now());
        if response_connection_closes(&response.headers) {
            self.connections.remove(origin);
        }
    }

    /// Removes a shared HTTP/2 connection after a failed stream exchange.
    pub(crate) fn discard_shared_http2(&mut self, origin: &str) {
        self.connections.remove(origin);
    }

    /// Closes every idle pooled connection immediately.
    pub fn close(&mut self) {
        self.connections.clear();
    }

    pub fn pooled_origin_count(&self) -> usize {
        self.connections.len()
    }

    pub fn timeouts(&self) -> HttpTimeouts {
        self.timeouts
    }

    fn store_response_cookies(
        &mut self,
        headers: &Headers,
        request_url: &zirk_runtime_core::http_values::Url,
        now: SystemTime,
    ) {
        let Some(jar) = &mut self.cookies else {
            return;
        };
        for value in headers.get_all("set-cookie").into_iter().flatten() {
            let _ = jar.store_set_cookie(request_url, value.as_bytes(), now);
        }
    }
}

fn response_connection_closes(headers: &Headers) -> bool {
    headers
        .get_all("connection")
        .into_iter()
        .flatten()
        .filter_map(|value| std::str::from_utf8(value.as_bytes()).ok())
        .flat_map(|value| value.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case("close"))
}

#[allow(dead_code)] // Internal server adapter until compiler-owned bindings land.
pub(crate) enum HttpServerIo {
    Plain(tokio::net::TcpStream),
    Tls(Box<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>),
}

impl HttpServerIo {
    fn negotiated_http2(&self) -> bool {
        matches!(
            self,
            Self::Tls(stream)
                if stream.get_ref().1.alpn_protocol() == Some(b"h2".as_slice())
        )
    }
}

impl From<TcpStream> for HttpServerIo {
    fn from(stream: TcpStream) -> Self {
        Self::Plain(stream.into_inner())
    }
}

impl From<tokio::net::TcpStream> for HttpServerIo {
    fn from(stream: tokio::net::TcpStream) -> Self {
        Self::Plain(stream)
    }
}

impl From<TlsServerStream> for HttpServerIo {
    fn from(stream: TlsServerStream) -> Self {
        Self::Tls(Box::new(stream.into_inner()))
    }
}

impl AsyncRead for HttpServerIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buffer),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_read(cx, buffer),
        }
    }
}

/// Holds incoming bytes until the original header block is validated. Hyper
/// intentionally normalizes some wire headers (for example removing
/// `Content-Length` when `Transfer-Encoding` is present); inspecting only its
/// normalized map would miss an ambiguous raw message before dispatch.
struct FramingGuardIo {
    inner: HttpServerIo,
    rejected: Arc<std::sync::atomic::AtomicBool>,
    websocket_upgrade: Arc<std::sync::atomic::AtomicBool>,
    probe: Vec<u8>,
    pending: Vec<u8>,
    offset: usize,
    validated: bool,
}

impl FramingGuardIo {
    fn new(inner: HttpServerIo, rejected: Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self::new_with_websocket_flag(
            inner,
            rejected,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
    }

    fn new_with_websocket_flag(
        inner: HttpServerIo,
        rejected: Arc<std::sync::atomic::AtomicBool>,
        websocket_upgrade: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            inner,
            rejected,
            websocket_upgrade,
            probe: Vec::new(),
            pending: Vec::new(),
            offset: 0,
            validated: false,
        }
    }
}

impl AsyncRead for FramingGuardIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.offset < self.pending.len() {
            let count = output
                .remaining()
                .min(self.pending.len().saturating_sub(self.offset));
            output.put_slice(&self.pending[self.offset..self.offset + count]);
            self.offset += count;
            return Poll::Ready(Ok(()));
        }
        if self.validated {
            return Pin::new(&mut self.inner).poll_read(cx, output);
        }

        loop {
            let mut raw = [0_u8; 8192];
            let mut read_buffer = ReadBuf::new(&mut raw);
            match Pin::new(&mut self.inner).poll_read(cx, &mut read_buffer) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(())) => {
                    let received = read_buffer.filled();
                    if received.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                    self.probe.extend_from_slice(received);
                    let Some(end) = self.probe.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                    else {
                        if self.probe.len() > zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES
                        {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "HTTP header block exceeds the configured bound",
                            )));
                        }
                        // The transport returned data, so it may not have registered this
                        // task's waker for the next read. Poll it again instead of waking
                        // ourselves and spinning while a slow client sends the header.
                        continue;
                    };
                    if end + 4 > zirk_runtime_core::http_values::MAX_HTTP_HEADER_BYTES {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "HTTP header block exceeds the configured bound",
                        )));
                    }
                    if let Err(error) = validate_raw_framing(&self.probe[..end + 2]) {
                        self.rejected
                            .store(true, std::sync::atomic::Ordering::Release);
                        return Poll::Ready(Err(error));
                    }
                    self.websocket_upgrade.store(
                        raw_has_websocket_upgrade(&self.probe[..end + 2]),
                        std::sync::atomic::Ordering::Release,
                    );
                    self.pending = std::mem::take(&mut self.probe);
                    self.validated = true;
                    self.offset = 0;
                    let count = output.remaining().min(self.pending.len());
                    output.put_slice(&self.pending[..count]);
                    self.offset = count;
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

impl AsyncWrite for FramingGuardIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, bytes)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

fn validate_raw_framing(headers: &[u8]) -> io::Result<()> {
    let mut lines = headers.split(|byte| *byte == b'\n');
    let _request_line = lines.next();
    let mut has_transfer_encoding = false;
    let mut content_length = None;
    for line in lines {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        let Some(colon) = line.iter().position(|byte| *byte == b':') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid HTTP header",
            ));
        };
        let name = &line[..colon];
        let value = line[colon + 1..]
            .iter()
            .copied()
            .skip_while(u8::is_ascii_whitespace)
            .collect::<Vec<_>>();
        if name.eq_ignore_ascii_case(b"transfer-encoding") {
            has_transfer_encoding = true;
        } else if name.eq_ignore_ascii_case(b"content-length") {
            let parsed = std::str::from_utf8(&value)
                .ok()
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                })
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length")
                })?;
            if content_length.is_some_and(|previous| previous != parsed) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "conflicting Content-Length values",
                ));
            }
            content_length = Some(parsed);
        }
    }
    if has_transfer_encoding && content_length.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ambiguous Transfer-Encoding and Content-Length",
        ));
    }
    Ok(())
}

fn raw_has_websocket_upgrade(headers: &[u8]) -> bool {
    let mut lines = headers.split(|byte| *byte == b'\n');
    let request_line = lines.next().unwrap_or_default();
    let request_line = request_line.strip_suffix(b"\r").unwrap_or(request_line);
    let mut request_parts = request_line.split(|byte| *byte == b' ');
    if request_parts.next() != Some(b"GET".as_slice())
        || request_parts.next().is_none()
        || request_parts.next() != Some(b"HTTP/1.1".as_slice())
    {
        return false;
    }
    let mut connection_upgrade = false;
    let mut websocket_upgrade = false;
    for line in lines {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        let Some(colon) = line.iter().position(|byte| *byte == b':') else {
            return false;
        };
        let name = &line[..colon];
        let value = &line[colon + 1..];
        if name.eq_ignore_ascii_case(b"connection") {
            connection_upgrade |= value
                .split(|byte| *byte == b',')
                .any(|token| trim_ascii(token).eq_ignore_ascii_case(b"upgrade"));
        } else if name.eq_ignore_ascii_case(b"upgrade") {
            if websocket_upgrade {
                return false;
            }
            websocket_upgrade = trim_ascii(value).eq_ignore_ascii_case(b"websocket");
        }
    }
    connection_upgrade && websocket_upgrade
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

impl AsyncWrite for HttpServerIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, bytes),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_write(cx, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for HttpIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buffer),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buffer),
            Self::TlsOverTls(stream) => Pin::new(stream.as_mut()).poll_read(cx, buffer),
            Self::Upgraded(stream) => Pin::new(stream.as_mut()).poll_read(cx, buffer),
        }
    }
}

impl AsyncWrite for HttpIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, bytes),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, bytes),
            Self::TlsOverTls(stream) => Pin::new(stream.as_mut()).poll_write(cx, bytes),
            Self::Upgraded(stream) => Pin::new(stream.as_mut()).poll_write(cx, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
            Self::TlsOverTls(stream) => Pin::new(stream.as_mut()).poll_flush(cx),
            Self::Upgraded(stream) => Pin::new(stream.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::TlsOverTls(stream) => Pin::new(stream.as_mut()).poll_shutdown(cx),
            Self::Upgraded(stream) => Pin::new(stream.as_mut()).poll_shutdown(cx),
        }
    }
}

const HTTP_WEBSOCKET_MESSAGE_LIMIT: usize = 16 * 1024 * 1024;
const HTTP_WEBSOCKET_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

/// A WebSocket message retains its wire-level text/binary distinction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpWebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close { code: u16, reason: String },
}

/// Owner-task WebSocket client over an already-authorized HTTP origin.
/// Tungstenite validates the RFC 6455 handshake, masking, fragmentation,
/// control-frame sizes, and UTF-8; both frames and complete messages are
/// bounded before bytes are exposed to Zirk.
pub struct HttpWebSocket {
    stream: tokio_tungstenite::WebSocketStream<HttpIo>,
    // HTTP/2 connection drivers own the multiplexed transport behind an
    // extended CONNECT stream. Keep that driver alive exactly as long as the
    // WebSocket and abort it when the stream closes or is dropped.
    h2_driver: Option<AbortOnDrop>,
    owner: crate::task::TaskId,
    idle_timeout: Duration,
    negotiated_subprotocol: Option<String>,
    closed: bool,
    receive_limit: usize,
    received_messages: usize,
}

impl HttpWebSocket {
    pub fn connect(
        url: &zirk_runtime_core::http_values::Url,
        grant: Arc<EffectiveNetworkGrant>,
        timeouts: HttpTimeouts,
        tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
    ) -> Result<Self, HttpRuntimeError> {
        Self::connect_with_options(url, grant, timeouts, tls_config, None, &[])
    }

    pub fn connect_with_options(
        url: &zirk_runtime_core::http_values::Url,
        grant: Arc<EffectiveNetworkGrant>,
        timeouts: HttpTimeouts,
        tls_config: Option<Arc<crate::network_transport::RustlsClientConfig>>,
        origin: Option<&str>,
        subprotocols: &[String],
    ) -> Result<Self, HttpRuntimeError> {
        if !matches!(url.scheme(), "http" | "https")
            || timeouts.connect.is_zero()
            || timeouts.response_headers.is_zero()
            || timeouts.response_idle.is_zero()
            || origin.is_some_and(|origin| normalize_websocket_origin(origin).is_none())
            || subprotocols.len() > 64
        {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let mut protocol_bytes = 0usize;
        for (index, protocol) in subprotocols.iter().enumerate() {
            if protocol.is_empty()
                || protocol.len() > 128
                || !protocol.bytes().all(is_http_token_byte)
                || subprotocols[..index]
                    .iter()
                    .any(|previous| previous == protocol)
            {
                return Err(HttpRuntimeError::InvalidRequest);
            }
            protocol_bytes = protocol_bytes
                .checked_add(protocol.len())
                .and_then(|size| size.checked_add(2))
                .ok_or(HttpRuntimeError::InvalidRequest)?;
        }
        if protocol_bytes > 8192 {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let origin_header = match origin {
            Some(origin) => {
                let origin =
                    normalize_websocket_origin(origin).ok_or(HttpRuntimeError::InvalidRequest)?;
                Some(
                    http::HeaderValue::from_str(&origin)
                        .map_err(|_| HttpRuntimeError::InvalidRequest)?,
                )
            }
            None => None,
        };
        let subprotocol_header = if subprotocols.is_empty() {
            None
        } else {
            let value = http::HeaderValue::from_str(&subprotocols.join(", "))
                .map_err(|_| HttpRuntimeError::InvalidRequest)?;
            Some(value)
        };
        grant
            .authorize_origin(&url.origin())
            .map_err(|error| match error {
                zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                    HttpRuntimeError::PermissionDenied
                }
                _ => HttpRuntimeError::InvalidRequest,
            })?;

        let (stream, negotiated_subprotocol, h2_driver) = if url.scheme() == "https" {
            let (tls, protocol) = match tls_config {
                Some(config) => TlsClientStream::connect_for_http_with_config(
                    url.as_str(),
                    Arc::clone(&grant),
                    timeouts.connect,
                    config,
                ),
                None => TlsClientStream::connect_for_http(
                    url.as_str(),
                    Arc::clone(&grant),
                    timeouts.connect,
                ),
            }
            .map_err(HttpRuntimeError::from)?;
            let io = HttpIo::Tls(Box::new(tls.into_inner()));
            match protocol {
                NegotiatedHttpProtocol::Http2 => {
                    let (stream, selected, driver) = connect_http2_websocket(
                        io,
                        url.as_str().to_owned(),
                        origin_header,
                        subprotocols,
                        timeouts.response_headers,
                    )?;
                    (stream, selected, Some(driver))
                }
                NegotiatedHttpProtocol::Http1 => {
                    let (stream, selected) = connect_http1_websocket(
                        io,
                        url.as_str(),
                        origin_header,
                        subprotocol_header,
                        subprotocols,
                        timeouts.response_headers,
                    )?;
                    (stream, selected, None)
                }
            }
        } else {
            let io = TcpStream::connect_with_timeout(url.as_str(), grant, timeouts.connect)
                .map_err(HttpRuntimeError::from)?;
            let (stream, selected) = connect_http1_websocket(
                HttpIo::Plain(io.into_inner()),
                url.as_str(),
                origin_header,
                subprotocol_header,
                subprotocols,
                timeouts.response_headers,
            )?;
            (stream, selected, None)
        };
        Ok(Self {
            stream,
            h2_driver,
            owner: crate::executor::current_task(),
            idle_timeout: timeouts.response_idle,
            negotiated_subprotocol,
            closed: false,
            receive_limit: usize::MAX,
            received_messages: 0,
        })
    }

    /// Connects a WebSocket using an explicit bounded DER trust store. The
    /// supplied roots replace the platform store and are validated before
    /// opening a network connection.
    pub fn connect_with_options_and_trust_roots(
        url: &zirk_runtime_core::http_values::Url,
        grant: Arc<EffectiveNetworkGrant>,
        timeouts: HttpTimeouts,
        origin: Option<&str>,
        subprotocols: &[String],
        roots_der: Vec<Vec<u8>>,
        allow_tls12: bool,
    ) -> Result<Self, HttpRuntimeError> {
        let config =
            crate::network_transport::client_tls_config_with_trust_roots(roots_der, allow_tls12)
                .map_err(|_| HttpRuntimeError::Protocol)?;
        Self::connect_with_options(url, grant, timeouts, Some(config), origin, subprotocols)
    }

    pub fn negotiated_subprotocol(&self) -> Option<&str> {
        self.negotiated_subprotocol.as_deref()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn send(&mut self, message: HttpWebSocketMessage) -> Result<(), HttpRuntimeError> {
        self.check_owner()?;
        if self.closed {
            return Err(HttpRuntimeError::BodyConsumed);
        }
        let message = match message {
            HttpWebSocketMessage::Text(text) => {
                if text.len() > HTTP_WEBSOCKET_MESSAGE_LIMIT {
                    return Err(HttpRuntimeError::BodyTooLarge);
                }
                tokio_tungstenite::tungstenite::Message::Text(text.into())
            }
            HttpWebSocketMessage::Binary(bytes) => {
                if bytes.len() > HTTP_WEBSOCKET_MESSAGE_LIMIT {
                    return Err(HttpRuntimeError::BodyTooLarge);
                }
                tokio_tungstenite::tungstenite::Message::Binary(bytes.into())
            }
            HttpWebSocketMessage::Ping(bytes) => {
                if bytes.len() > 125 {
                    return Err(HttpRuntimeError::InvalidRequest);
                }
                tokio_tungstenite::tungstenite::Message::Ping(bytes.into())
            }
            HttpWebSocketMessage::Pong(bytes) => {
                if bytes.len() > 125 {
                    return Err(HttpRuntimeError::InvalidRequest);
                }
                tokio_tungstenite::tungstenite::Message::Pong(bytes.into())
            }
            HttpWebSocketMessage::Close { code, reason } => {
                if reason.len() > 123 {
                    return Err(HttpRuntimeError::InvalidRequest);
                }
                let code =
                    tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(code);
                if !is_valid_websocket_close_code(code) {
                    return Err(HttpRuntimeError::InvalidRequest);
                }
                tokio_tungstenite::tungstenite::Message::Close(Some(
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code,
                        reason: reason.into(),
                    },
                ))
            }
        };
        let idle_timeout = self.idle_timeout;
        crate::network_future::block_on_current(async {
            tokio::time::timeout(idle_timeout, self.stream.send(message))
                .await
                .map_err(|_| HttpRuntimeError::Timeout)?
                .map_err(map_websocket_error)
        })
        .map_err(map_network_future_error)?
    }

    pub fn receive(
        &mut self,
        idle_timeout: Duration,
    ) -> Result<Option<HttpWebSocketMessage>, HttpRuntimeError> {
        self.check_owner()?;
        if self.closed {
            return Ok(None);
        }
        if self.received_messages >= self.receive_limit {
            self.close_with(1001, "message limit reached".to_owned())?;
            return Ok(None);
        }
        let message = crate::network_future::block_on_current(async {
            tokio::time::timeout(idle_timeout, self.stream.next())
                .await
                .map_err(|_| HttpRuntimeError::Timeout)?
                .transpose()
                .map_err(map_websocket_error)
        })
        .map_err(map_network_future_error)??;
        let Some(message) = message else {
            self.closed = true;
            self.h2_driver.take();
            return Ok(None);
        };
        let message = match message {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                HttpWebSocketMessage::Text(text.to_string())
            }
            tokio_tungstenite::tungstenite::Message::Binary(bytes) => {
                HttpWebSocketMessage::Binary(bytes.to_vec())
            }
            tokio_tungstenite::tungstenite::Message::Ping(bytes) => {
                HttpWebSocketMessage::Ping(bytes.to_vec())
            }
            tokio_tungstenite::tungstenite::Message::Pong(bytes) => {
                HttpWebSocketMessage::Pong(bytes.to_vec())
            }
            tokio_tungstenite::tungstenite::Message::Close(frame) => {
                // Tungstenite queues the close reply while reading the peer's
                // close frame. Flush it before marking the handle closed.
                let idle_timeout = self.idle_timeout;
                crate::network_future::block_on_current(async {
                    tokio::time::timeout(idle_timeout, self.stream.flush())
                        .await
                        .map_err(|_| HttpRuntimeError::Timeout)?
                        .map_err(map_websocket_error)?;
                    match tokio::time::timeout(idle_timeout, self.stream.next())
                        .await
                        .map_err(|_| HttpRuntimeError::Timeout)?
                    {
                        None
                        | Some(Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed)) => {
                            Ok(())
                        }
                        Some(Err(error)) => Err(map_websocket_error(error)),
                        Some(Ok(_)) => Err(HttpRuntimeError::Protocol),
                    }
                })
                .map_err(map_network_future_error)??;
                self.closed = true;
                self.h2_driver.take();
                let (code, reason) = frame
                    .map(|frame| (u16::from(frame.code), frame.reason.to_string()))
                    .unwrap_or((1005, String::new()));
                HttpWebSocketMessage::Close { code, reason }
            }
            tokio_tungstenite::tungstenite::Message::Frame(_) => {
                return Err(HttpRuntimeError::Protocol);
            }
        };
        if matches!(message, HttpWebSocketMessage::Ping(_)) {
            let idle_timeout = self.idle_timeout;
            crate::network_future::block_on_current(async {
                tokio::time::timeout(idle_timeout, self.stream.flush())
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(map_websocket_error)
            })
            .map_err(map_network_future_error)??;
        }
        self.received_messages = self.received_messages.saturating_add(1);
        Ok(Some(message))
    }

    pub fn close(&mut self) -> Result<(), HttpRuntimeError> {
        self.close_with_frame(None)
    }

    pub fn close_with(&mut self, code: u16, reason: String) -> Result<(), HttpRuntimeError> {
        if reason.len() > 123 {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        let code = tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(code);
        if !is_valid_websocket_close_code(code) {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        self.close_with_frame(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
            code,
            reason: reason.into(),
        }))
    }

    fn close_with_frame(
        &mut self,
        close_frame: Option<tokio_tungstenite::tungstenite::protocol::CloseFrame>,
    ) -> Result<(), HttpRuntimeError> {
        self.check_owner()?;
        if self.closed {
            return Ok(());
        }
        let result = crate::network_future::block_on_current(async {
            tokio::time::timeout(HTTP_WEBSOCKET_CLOSE_TIMEOUT, async {
                match self
                    .stream
                    .send(tokio_tungstenite::tungstenite::Message::Close(close_frame))
                    .await
                {
                    Ok(()) | Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed) => {}
                    Err(error) => return Err(map_websocket_error(error)),
                }
                while let Some(message) = self.stream.next().await {
                    match message {
                        Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                        Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed) => break,
                        Err(error) => return Err(map_websocket_error(error)),
                        Ok(_) => continue,
                    }
                }
                Ok::<_, HttpRuntimeError>(())
            })
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
        })
        .map_err(map_network_future_error)?;
        self.closed = true;
        self.h2_driver.take();
        result
    }

    fn check_owner(&self) -> Result<(), HttpRuntimeError> {
        if self.owner == crate::executor::current_task() {
            Ok(())
        } else {
            Err(HttpRuntimeError::InvalidRequest)
        }
    }
}

fn connect_http1_websocket(
    io: HttpIo,
    url: &str,
    origin_header: Option<http::HeaderValue>,
    subprotocol_header: Option<http::HeaderValue>,
    subprotocols: &[String],
    response_timeout: Duration,
) -> Result<(tokio_tungstenite::WebSocketStream<HttpIo>, Option<String>), HttpRuntimeError> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let websocket_url = if url.starts_with("https://") {
        url.replacen("https://", "wss://", 1)
    } else {
        url.replacen("http://", "ws://", 1)
    };
    let mut request = websocket_url
        .into_client_request()
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    if let Some(origin) = origin_header {
        request.headers_mut().insert(http::header::ORIGIN, origin);
    }
    if let Some(value) = subprotocol_header {
        request
            .headers_mut()
            .insert(http::header::SEC_WEBSOCKET_PROTOCOL, value);
    }
    let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_frame_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_write_buffer_size = HTTP_WEBSOCKET_MESSAGE_LIMIT + 64 * 1024;
    let (stream, response) = crate::network_future::block_on_current(async move {
        tokio::time::timeout(
            response_timeout,
            tokio_tungstenite::client_async_with_config(request, io, Some(config)),
        )
        .await
        .map_err(|_| HttpRuntimeError::Timeout)?
        .map_err(map_websocket_error)
    })
    .map_err(map_network_future_error)??;
    if response.status() != http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(HttpRuntimeError::Protocol);
    }
    let negotiated = websocket_selected_subprotocol(response.headers(), subprotocols)?;
    Ok((stream, negotiated))
}

fn connect_http2_websocket(
    io: HttpIo,
    url: String,
    origin_header: Option<http::HeaderValue>,
    subprotocols: &[String],
    response_timeout: Duration,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<HttpIo>,
        Option<String>,
        AbortOnDrop,
    ),
    HttpRuntimeError,
> {
    use http::header::{ORIGIN, SEC_WEBSOCKET_EXTENSIONS, SEC_WEBSOCKET_PROTOCOL};

    let subprotocol_header = if subprotocols.is_empty() {
        None
    } else {
        Some(
            http::HeaderValue::from_str(&subprotocols.join(", "))
                .map_err(|_| HttpRuntimeError::InvalidRequest)?,
        )
    };
    let offered_subprotocols = subprotocols.to_vec();
    let (upgraded, negotiated_subprotocol, driver) =
        crate::network_future::block_on_current(async move {
            crate::network_future::spawn_on_reactor(async move {
                let (mut sender, connection) = tokio::time::timeout(
                    response_timeout,
                    hyper::client::conn::http2::handshake(
                        hyper_util::rt::TokioExecutor::new(),
                        TokioIo::new(io),
                    ),
                )
                .await
                .map_err(|_| HttpRuntimeError::Timeout)?
                .map_err(|_| HttpRuntimeError::Protocol)?;

                // RFC 8441 requires the peer to opt in before the client sends
                // :protocol. Poll the connection while waiting so its SETTINGS
                // frame is observed before the extended CONNECT is constructed.
                let mut connection = Box::pin(connection);
                tokio::time::timeout(
                    response_timeout,
                    std::future::poll_fn(|cx| {
                        if connection.as_mut().poll(cx).is_ready() {
                            return Poll::Ready(Err(HttpRuntimeError::Protocol));
                        }
                        if connection.is_extended_connect_protocol_enabled() {
                            Poll::Ready(Ok(()))
                        } else {
                            Poll::Pending
                        }
                    }),
                )
                .await
                .map_err(|_| HttpRuntimeError::Timeout)??;

                let driver = AbortOnDrop(tokio::spawn(connection).abort_handle());
                let mut request = Request::builder()
                    .method(http::Method::CONNECT)
                    .version(http::Version::HTTP_2)
                    .uri(url)
                    .header(http::header::SEC_WEBSOCKET_VERSION, "13")
                    .body(Full::new(Bytes::new()))
                    .map_err(|_| HttpRuntimeError::InvalidRequest)?;
                if let Some(origin) = origin_header {
                    request.headers_mut().insert(ORIGIN, origin);
                }
                if let Some(subprotocols) = subprotocol_header {
                    request
                        .headers_mut()
                        .insert(SEC_WEBSOCKET_PROTOCOL, subprotocols);
                }
                request
                    .extensions_mut()
                    .insert(hyper::ext::Protocol::from_static("websocket"));

                let mut response = tokio::time::timeout(response_timeout, async {
                    sender
                        .ready()
                        .await
                        .map_err(|_| HttpRuntimeError::Transport)?;
                    sender
                        .send_request(request)
                        .await
                        .map_err(|_| HttpRuntimeError::Protocol)
                })
                .await
                .map_err(|_| HttpRuntimeError::Timeout)??;
                if response.status() != http::StatusCode::OK
                    || response.headers().contains_key(SEC_WEBSOCKET_EXTENSIONS)
                {
                    return Err(HttpRuntimeError::Protocol);
                }
                let negotiated =
                    websocket_selected_subprotocol(response.headers(), &offered_subprotocols)?;
                let upgraded =
                    tokio::time::timeout(response_timeout, hyper::upgrade::on(&mut response))
                        .await
                        .map_err(|_| HttpRuntimeError::Timeout)?
                        .map_err(|_| HttpRuntimeError::Protocol)?;
                Ok::<_, HttpRuntimeError>((upgraded, negotiated, driver))
            })
            .await
            .map_err(|_| HttpRuntimeError::Protocol)?
        })
        .map_err(map_network_future_error)??;

    let io = HttpIo::Upgraded(Box::new(TokioIo::new(upgraded)));
    let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_frame_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_write_buffer_size = HTTP_WEBSOCKET_MESSAGE_LIMIT + 64 * 1024;
    let stream = crate::network_future::block_on_current(
        tokio_tungstenite::WebSocketStream::from_raw_socket(
            io,
            tokio_tungstenite::tungstenite::protocol::Role::Client,
            Some(config),
        ),
    )
    .map_err(map_network_future_error)?;
    Ok((stream, negotiated_subprotocol, driver))
}

fn websocket_selected_subprotocol(
    headers: &http::HeaderMap,
    offered: &[String],
) -> Result<Option<String>, HttpRuntimeError> {
    match headers
        .get_all(http::header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => Ok(None),
        [selected] => {
            let selected = selected.to_str().map_err(|_| HttpRuntimeError::Protocol)?;
            if !offered.iter().any(|protocol| protocol == selected) {
                return Err(HttpRuntimeError::Protocol);
            }
            Ok(Some(selected.to_owned()))
        }
        _ => Err(HttpRuntimeError::Protocol),
    }
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn normalize_websocket_origin(origin: &str) -> Option<String> {
    if origin == "null" {
        return Some(origin.to_owned());
    }
    let parsed = url::Url::parse(origin).ok()?;
    (matches!(parsed.scheme(), "http" | "https")
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.path() == "/"
        && parsed.query().is_none()
        && parsed.fragment().is_none())
    .then(|| parsed.origin().ascii_serialization())
}

fn is_valid_websocket_close_code(
    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode,
) -> bool {
    // IANA registered 1014 after Tungstenite's close-code table was written.
    // Accept that assigned value while retaining the crate's validation for
    // other protocol and application ranges.
    matches!(
        code,
        tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Bad(1014)
    ) || code.is_allowed()
}

fn map_websocket_error(error: tokio_tungstenite::tungstenite::Error) -> HttpRuntimeError {
    use tokio_tungstenite::tungstenite::Error as E;
    match error {
        E::Capacity(_) => HttpRuntimeError::BodyTooLarge,
        E::Protocol(_) | E::Utf8(_) | E::AttackAttempt => HttpRuntimeError::Protocol,
        E::Io(_) | E::ConnectionClosed | E::AlreadyClosed => HttpRuntimeError::Transport,
        _ => HttpRuntimeError::Transport,
    }
}

fn map_network_future_error(error: crate::network_future::NetworkFutureError) -> HttpRuntimeError {
    match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    }
}

struct HttpResponseStart {
    status: u16,
    headers: hyper::http::HeaderMap,
    chunks: tokio::sync::mpsc::Receiver<Result<Bytes, HttpRuntimeError>>,
    worker: AbortOnDrop,
}

/// Runs the stack-heavy Hyper HTTP/2 state machine on Tokio's native reactor
/// thread. The Zirk task only awaits bounded channels and remains the owner of
/// the public response value; this keeps the language's 128 KiB task stacks
/// suitable for ordinary tasks.
async fn start_http2_on_reactor<B>(
    io: HttpIo,
    request: Request<B>,
    response_headers_timeout: Duration,
    response_idle_timeout: Duration,
) -> Result<HttpResponseStart, HttpRuntimeError>
where
    B: hyper::body::Body<Data = Bytes> + Send + Unpin + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let mut start_tx = Some(start_tx);
        let result = async {
            let (mut sender, connection) = tokio::time::timeout(
                response_headers_timeout,
                hyper::client::conn::http2::handshake(
                    hyper_util::rt::TokioExecutor::new(),
                    TokioIo::new(io),
                ),
            )
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
            .map_err(|_| HttpRuntimeError::Protocol)?;
            let driver = tokio::spawn(connection);
            let _driver_guard = AbortOnDrop(driver.abort_handle());
            let response = tokio::time::timeout(response_headers_timeout, async {
                sender
                    .ready()
                    .await
                    .map_err(|_| HttpRuntimeError::Protocol)?;
                sender
                    .send_request(request)
                    .await
                    .map_err(|_| HttpRuntimeError::Protocol)
            })
            .await
            .map_err(|_| HttpRuntimeError::Timeout)??;
            let (parts, mut body) = response.into_parts();
            let (chunks_tx, chunks_rx) = tokio::sync::mpsc::channel(2);
            if start_tx
                .take()
                .expect("response-start sender is present before response headers")
                .send(Ok((parts.status.as_u16(), parts.headers, chunks_rx)))
                .is_err()
            {
                return Ok::<(), HttpRuntimeError>(());
            }
            loop {
                let frame = match tokio::time::timeout(response_idle_timeout, body.frame()).await {
                    Ok(Some(Ok(frame))) => frame,
                    Ok(Some(Err(_))) => {
                        let _ = chunks_tx.send(Err(HttpRuntimeError::Protocol)).await;
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {
                        let _ = chunks_tx.send(Err(HttpRuntimeError::Timeout)).await;
                        break;
                    }
                };
                if let Ok(data) = frame.into_data() {
                    let mut pending = data;
                    while !pending.is_empty() {
                        let count = pending.len().min(64 * 1024);
                        let chunk = pending.split_to(count);
                        if chunks_tx.send(Ok(chunk)).await.is_err() {
                            return Ok::<(), HttpRuntimeError>(());
                        }
                    }
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = result
            && let Some(start_tx) = start_tx
        {
            let _ = start_tx.send(Err(error));
        }
    });
    let worker_guard = AbortOnDrop(worker.abort_handle());
    let (status, headers, chunks) = start_rx.await.map_err(|_| HttpRuntimeError::Transport)??;
    Ok(HttpResponseStart {
        status,
        headers,
        chunks,
        worker: worker_guard,
    })
}

/// Runs a streaming HTTP/1.1 exchange away from the Zirk stack. The body
/// driver consumes the single-slot upload channel and forwards response
/// chunks through a separate bounded channel.
async fn start_http1_on_reactor<B>(
    io: HttpIo,
    request: Request<B>,
    response_headers_timeout: Duration,
    response_idle_timeout: Duration,
) -> Result<HttpResponseStart, HttpRuntimeError>
where
    B: hyper::body::Body<Data = Bytes> + Send + Unpin + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let mut start_tx = Some(start_tx);
        let result = async {
            let (mut sender, connection) = tokio::time::timeout(
                response_headers_timeout,
                hyper::client::conn::http1::handshake(TokioIo::new(io)),
            )
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
            .map_err(|_| HttpRuntimeError::Protocol)?;
            let driver = tokio::spawn(connection);
            let _driver_guard = AbortOnDrop(driver.abort_handle());
            let response =
                tokio::time::timeout(response_headers_timeout, sender.send_request(request))
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            let (parts, mut body) = response.into_parts();
            let (chunks_tx, chunks_rx) = tokio::sync::mpsc::channel(2);
            if start_tx
                .take()
                .expect("HTTP/1 response-start sender exists")
                .send(Ok((parts.status.as_u16(), parts.headers, chunks_rx)))
                .is_err()
            {
                return Ok::<(), HttpRuntimeError>(());
            }
            loop {
                let frame = match tokio::time::timeout(response_idle_timeout, body.frame()).await {
                    Ok(Some(Ok(frame))) => frame,
                    Ok(Some(Err(_))) => {
                        let _ = chunks_tx.send(Err(HttpRuntimeError::Protocol)).await;
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {
                        let _ = chunks_tx.send(Err(HttpRuntimeError::Timeout)).await;
                        break;
                    }
                };
                if let Ok(data) = frame.into_data() {
                    let mut pending = data;
                    while !pending.is_empty() {
                        let chunk = pending.split_to(pending.len().min(64 * 1024));
                        if chunks_tx.send(Ok(chunk)).await.is_err() {
                            return Ok::<(), HttpRuntimeError>(());
                        }
                    }
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = result
            && let Some(start_tx) = start_tx
        {
            let _ = start_tx.send(Err(error));
        }
    });
    let worker_guard = AbortOnDrop(worker.abort_handle());
    let (status, headers, chunks) = start_rx.await.map_err(|_| HttpRuntimeError::Transport)??;
    Ok(HttpResponseStart {
        status,
        headers,
        chunks,
        worker: worker_guard,
    })
}

/// Executes one bounded request, preferring HTTP/2 when HTTPS ALPN negotiates
/// it and otherwise using HTTP/1.1. There are deliberately no implicit
/// redirects, retries, cookies, caches, or retained connections in this layer.
pub fn send_once(
    request: HttpRequestValues,
    grant: Arc<EffectiveNetworkGrant>,
) -> Result<HttpResponseValues, HttpRuntimeError> {
    let mut response = send_once_streaming(request, grant)?;
    let body = response.read_all()?;
    let response = HttpResponseValues {
        status: response.status,
        headers: response.headers,
        body,
        body_stream: None,
        body_producer: None,
    };
    Ok(response)
}

/// Executes one negotiated HTTP request and returns a single-consumption
/// response stream. It applies backpressure as the caller asks for chunks.
pub fn send_once_streaming(
    request: HttpRequestValues,
    grant: Arc<EffectiveNetworkGrant>,
) -> Result<HttpStreamingResponse, HttpRuntimeError> {
    send_once_streaming_with_timeouts(request, grant, HttpTimeouts::default())
}

/// Executes a bounded HTTP request with independently configurable phase
/// limits. A zero phase timeout is rejected before any network operation.
pub fn send_once_streaming_with_timeouts(
    request: HttpRequestValues,
    grant: Arc<EffectiveNetworkGrant>,
    timeouts: HttpTimeouts,
) -> Result<HttpStreamingResponse, HttpRuntimeError> {
    send_once_streaming_with_tls_config(request, grant, timeouts, None)
}

fn send_once_streaming_with_tls_config(
    request: HttpRequestValues,
    grant: Arc<EffectiveNetworkGrant>,
    timeouts: HttpTimeouts,
    tls_config: Option<Arc<tokio_rustls::rustls::ClientConfig>>,
) -> Result<HttpStreamingResponse, HttpRuntimeError> {
    let request_for_cache_invalidation = request.clone();
    if timeouts.connect.is_zero()
        || timeouts.response_headers.is_zero()
        || timeouts.response_idle.is_zero()
    {
        return Err(HttpRuntimeError::InvalidRequest);
    }
    validate_client_request(&request)?;
    request
        .headers
        .validate_message_framing()
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    let accepted =
        AcceptEncoding::parse(&request.headers).map_err(|_| HttpRuntimeError::InvalidRequest)?;
    if request.headers.get("content-length").is_some()
        || request.headers.get("transfer-encoding").is_some()
    {
        // Hyper owns framing for the fixed body. Accepting caller framing
        // fields would make it possible to disagree with the serialized body.
        return Err(HttpRuntimeError::InvalidRequest);
    }
    let expected_authority = authority(&request.url);
    if request.headers.get_all("host").is_some_and(|values| {
        let values = values.collect::<Vec<_>>();
        !values.is_empty()
            && (values.len() != 1 || values.iter().any(|v| *v != expected_authority.as_str()))
    }) {
        return Err(HttpRuntimeError::InvalidRequest);
    }
    if request.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
        return Err(HttpRuntimeError::BodyTooLarge);
    }
    let url = request.url.clone();
    grant
        .authorize_origin(&url.origin())
        .map_err(|error| match error {
            zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                HttpRuntimeError::PermissionDenied
            }
            _ => HttpRuntimeError::InvalidRequest,
        })?;
    let owner = crate::executor::current_task();
    let (io, use_http2) = if url.scheme() == "https" {
        let (stream, protocol) = match tls_config {
            Some(config) => TlsClientStream::connect_for_http_with_config(
                url.as_str(),
                grant,
                timeouts.connect,
                config,
            ),
            None => TlsClientStream::connect_for_http(url.as_str(), grant, timeouts.connect),
        }
        .map_err(HttpRuntimeError::from)?;
        (
            HttpIo::Tls(Box::new(stream.into_inner())),
            protocol == NegotiatedHttpProtocol::Http2,
        )
    } else {
        let stream = TcpStream::connect_with_timeout(url.as_str(), grant, timeouts.connect)
            .map_err(HttpRuntimeError::from)?;
        (HttpIo::Plain(stream.into_inner()), false)
    };

    let mut builder = Request::builder()
        .method(
            hyper::Method::from_bytes(request.method_token.as_bytes())
                .map_err(|_| HttpRuntimeError::InvalidRequest)?,
        )
        .uri(request_target(&url));
    for (name, value) in request.headers.iter() {
        if name != hyper::header::HOST && name != hyper::header::CONNECTION {
            builder = builder.header(name, value);
        }
    }
    if request.headers.get("accept-encoding").is_none() {
        builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
    }
    if !use_http2 {
        builder = builder.header(hyper::header::HOST, expected_authority);
        // One-shot calls cannot retain a pooled socket or hidden client state.
        builder = builder.header(hyper::header::CONNECTION, "close");
    }
    let request = builder
        .body(Full::new(Bytes::from(request.body)))
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;

    let task = crate::network_future::block_on_current(async move {
        let (status, response_headers, body, driver_guard) = if use_http2 {
            let response = start_http2_on_reactor(
                io,
                request,
                timeouts.response_headers,
                timeouts.response_idle,
            )
            .await?;
            let status =
                HttpStatusCode::new(response.status).map_err(|_| HttpRuntimeError::Protocol)?;
            let mut headers = Headers::new();
            for (name, value) in response.headers.iter() {
                headers
                    .append(name.as_str(), value.as_bytes())
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            }
            headers
                .validate_accept_query()
                .map_err(|_| HttpRuntimeError::Protocol)?;
            (
                status,
                headers,
                HttpStreamingBody::Chunks(response.chunks),
                response.worker,
            )
        } else {
            let (mut sender, connection) = tokio::time::timeout(
                timeouts.response_headers,
                hyper::client::conn::http1::handshake(TokioIo::new(io)),
            )
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?
            .map_err(|_| HttpRuntimeError::Protocol)?;
            let driver = tokio::spawn(connection);
            let driver_guard = AbortOnDrop(driver.abort_handle());
            let response =
                tokio::time::timeout(timeouts.response_headers, sender.send_request(request))
                    .await
                    .map_err(|_| HttpRuntimeError::Timeout)?
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            let (parts, body) = response.into_parts();
            let status = HttpStatusCode::new(parts.status.as_u16())
                .map_err(|_| HttpRuntimeError::Protocol)?;
            let mut headers = Headers::new();
            for (name, value) in parts.headers.iter() {
                headers
                    .append(name.as_str(), value.as_bytes())
                    .map_err(|_| HttpRuntimeError::Protocol)?;
            }
            headers
                .validate_accept_query()
                .map_err(|_| HttpRuntimeError::Protocol)?;
            (
                status,
                headers,
                HttpStreamingBody::Http1(body),
                driver_guard,
            )
        };
        let (response_headers, body, decoder) =
            decode_response_body(response_headers, body, &accepted)?;
        let mut drivers = vec![driver_guard];
        if let Some(decoder) = decoder {
            drivers.push(decoder);
        }
        Ok::<_, HttpRuntimeError>(HttpStreamingResponse {
            status,
            headers: response_headers,
            body: Some(body),
            driver: Some(drivers),
            pending: Bytes::new(),
            owner,
            body_consumed: false,
            body_eof: false,
            stream_started: false,
            response_idle_timeout: timeouts.response_idle,
        })
    })
    .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })??;
    crate::http_api::invalidate_cached_uris_after_write(
        &request_for_cache_invalidation,
        task.status,
        &task.headers,
    );
    Ok(task)
}

/// Streams a one-shot request body from a producer running on the current
/// Zirk task. The protocol driver receives copied chunks through a single-slot
/// channel and cannot call back into Zirk or inspect managed memory.
pub(crate) fn send_once_streaming_with_producer(
    request: HttpRequestValues,
    grant: Arc<EffectiveNetworkGrant>,
    mut producer: impl FnMut() -> Result<Option<Vec<u8>>, HttpRuntimeError>,
) -> Result<HttpStreamingResponse, HttpRuntimeError> {
    let request_for_cache_invalidation = request.clone();
    let timeouts = HttpTimeouts::default();
    if timeouts.connect.is_zero()
        || timeouts.response_headers.is_zero()
        || timeouts.response_idle.is_zero()
        || !request.body.is_empty()
    {
        return Err(HttpRuntimeError::InvalidRequest);
    }
    request
        .headers
        .validate_message_framing()
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    let accepted =
        AcceptEncoding::parse(&request.headers).map_err(|_| HttpRuntimeError::InvalidRequest)?;
    if request.headers.get("content-length").is_some()
        || request.headers.get("transfer-encoding").is_some()
    {
        return Err(HttpRuntimeError::InvalidRequest);
    }
    let expected_authority = authority(&request.url);
    if request.headers.get_all("host").is_some_and(|values| {
        let values = values.collect::<Vec<_>>();
        !values.is_empty()
            && (values.len() != 1
                || values
                    .iter()
                    .any(|value| value.as_bytes() != expected_authority.as_bytes()))
    }) {
        return Err(HttpRuntimeError::InvalidRequest);
    }

    let url = request.url.clone();
    grant
        .authorize_origin(&url.origin())
        .map_err(|error| match error {
            zirk_runtime_core::network_policy::NetworkPolicyError::PermissionDenied => {
                HttpRuntimeError::PermissionDenied
            }
            _ => HttpRuntimeError::InvalidRequest,
        })?;
    // QUERY requires Content-Type even when its request content is empty, so
    // reject missing or conflicting media metadata before invoking the producer.
    validate_client_request(&request)?;
    // Pull one chunk after permission validation and local request validation.
    let first_chunk = producer()?;
    // A producer can cooperatively resume from a cancelled language-level
    // suspension and return an empty chunk; cancellation must not be mistaken
    // for its normal EOF marker.
    if crate::executor::current_task_cancel_requested() {
        return Err(HttpRuntimeError::Cancelled);
    }
    if let Some(chunk) = &first_chunk
        && !chunk.is_empty()
        && chunk.len() > HTTP_UPLOAD_CHUNK_LIMIT
    {
        return Err(HttpRuntimeError::BodyTooLarge);
    }
    let owner = crate::executor::current_task();
    let (io, use_http2) = if url.scheme() == "https" {
        let (stream, protocol) =
            TlsClientStream::connect_for_http(url.as_str(), grant, timeouts.connect)
                .map_err(HttpRuntimeError::from)?;
        (
            HttpIo::Tls(Box::new(stream.into_inner())),
            protocol == NegotiatedHttpProtocol::Http2,
        )
    } else {
        let stream = TcpStream::connect_with_timeout(url.as_str(), grant, timeouts.connect)
            .map_err(HttpRuntimeError::from)?;
        (HttpIo::Plain(stream.into_inner()), false)
    };

    let (mut upload, body) = HttpUploadBody::channel(owner);
    let mut builder = Request::builder()
        .method(
            hyper::Method::from_bytes(request.method_token.as_bytes())
                .map_err(|_| HttpRuntimeError::InvalidRequest)?,
        )
        .uri(request_target(&url));
    for (name, value) in request.headers.iter() {
        if name != hyper::header::HOST && name != hyper::header::CONNECTION {
            builder = builder.header(name, value);
        }
    }
    if request.headers.get("accept-encoding").is_none() {
        builder = builder.header(hyper::header::ACCEPT_ENCODING, "br, gzip, deflate");
    }
    if !use_http2 {
        builder = builder
            .header(hyper::header::HOST, expected_authority)
            .header(hyper::header::CONNECTION, "close");
    }
    let request = builder
        .body(body)
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    let protocol_worker = crate::network_future::spawn_on_reactor(async move {
        let started = if use_http2 {
            start_http2_on_reactor(
                io,
                request,
                timeouts.response_headers,
                timeouts.response_idle,
            )
            .await
        } else {
            start_http1_on_reactor(
                io,
                request,
                timeouts.response_headers,
                timeouts.response_idle,
            )
            .await
        };
        let _ = start_tx.send(started);
    });

    let mut next_chunk = first_chunk;
    loop {
        let chunk = match next_chunk.take() {
            Some(chunk) => chunk,
            None => {
                let produced = producer()?;
                if crate::executor::current_task_cancel_requested() {
                    return Err(HttpRuntimeError::Cancelled);
                }
                match produced {
                    Some(chunk) => chunk,
                    None => break,
                }
            }
        };
        if chunk.is_empty() {
            break;
        }
        if chunk.len() > HTTP_UPLOAD_CHUNK_LIMIT {
            return Err(HttpRuntimeError::InvalidRequest);
        }
        upload.write_chunk(&chunk)?;
    }
    if crate::executor::current_task_cancel_requested() {
        return Err(HttpRuntimeError::Cancelled);
    }
    upload.finish()?;

    let start = crate::network_future::block_on_current(async {
        start_rx.await.map_err(|_| HttpRuntimeError::Transport)?
    })
    .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })??;
    let worker_result =
        crate::network_future::block_on_current(protocol_worker).map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?;
    worker_result.map_err(|_| HttpRuntimeError::Transport)?;
    let status = HttpStatusCode::new(start.status).map_err(|_| HttpRuntimeError::Protocol)?;
    let mut headers = Headers::new();
    for (name, value) in start.headers.iter() {
        headers
            .append(name.as_str(), value.as_bytes())
            .map_err(|_| HttpRuntimeError::Protocol)?;
    }
    headers
        .validate_accept_query()
        .map_err(|_| HttpRuntimeError::Protocol)?;
    let (headers, body, decoder) = crate::network_future::block_on_current(async move {
        decode_response_body(headers, HttpStreamingBody::Chunks(start.chunks), &accepted)
    })
    .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })??;
    crate::http_api::invalidate_cached_uris_after_write(
        &request_for_cache_invalidation,
        status,
        &headers,
    );
    let mut drivers = vec![start.worker];
    if let Some(decoder) = decoder {
        drivers.push(decoder);
    }
    Ok(HttpStreamingResponse {
        status,
        headers,
        body: Some(body),
        driver: Some(drivers),
        pending: Bytes::new(),
        owner,
        response_idle_timeout: timeouts.response_idle,
        body_consumed: false,
        body_eof: false,
        stream_started: false,
    })
}

/// Negotiates the accepted TLS stream's HTTP/2 ALPN when available, otherwise
/// serves HTTP/1.1. It bounds and consumes a request body before dispatch,
/// invokes the handler on the current Zirk task, writes a response, then closes
/// the connection. Routing and long-lived admission belong to the API layer.
#[allow(dead_code)] // Internal server adapter until compiler-owned bindings land.
pub(crate) fn serve_once(
    stream: impl Into<HttpServerIo>,
    secure: bool,
    handler: impl FnOnce(HttpRequestValues) -> HttpResponseValues,
) -> Result<(), HttpRuntimeError> {
    let stream = stream.into();
    if stream.negotiated_http2() {
        return serve_http2_offloaded(stream, secure, handler);
    }
    serve_once_with_timeouts(stream, secure, DEFAULT_HTTP_SERVER_TIMEOUTS, handler)
}

/// Accepts one HTTP/1.1 WebSocket upgrade at an exact path and dispatches
/// bounded messages on the caller's Zirk task. Hyper owns the HTTP handshake;
/// the WebSocket stream is exposed only after its upgrade future resolves.
#[allow(dead_code)] // Internal server transport until the public Zirk callback ABI lands.
pub(crate) fn serve_http1_websocket_once(
    stream: impl Into<HttpServerIo>,
    path: &str,
    policy: zirk_runtime_core::http_websocket::WebSocketAcceptPolicy,
    max_messages: usize,
    handler: impl FnOnce(HttpWebSocket) -> Result<(), HttpRuntimeError>,
) -> Result<(), HttpRuntimeError> {
    let stream = stream.into();
    if stream.negotiated_http2() {
        return serve_http2_websocket_once(stream, path, policy, max_messages, handler);
    }
    use http::StatusCode;
    use http::header::{CONNECTION, ORIGIN, SEC_WEBSOCKET_PROTOCOL, UPGRADE};
    use tokio_tungstenite::tungstenite::protocol::Role;

    if path.is_empty()
        || !path.starts_with('/')
        || path.contains(['?', '#', '\r', '\n'])
        || max_messages == 0
        || max_messages > 1_024
    {
        return Err(HttpRuntimeError::InvalidRequest);
    }

    let (upgrade_tx, upgrade_rx) = tokio::sync::oneshot::channel();
    let upgrade_tx = std::sync::Mutex::new(Some(upgrade_tx));
    let websocket_upgrade = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let websocket_upgrade_for_service = Arc::clone(&websocket_upgrade);
    let expected_path = path.to_owned();
    let service = hyper::service::service_fn(move |mut request: Request<hyper::body::Incoming>| {
        let websocket_upgrade_for_request = Arc::clone(&websocket_upgrade_for_service);
        let sender = upgrade_tx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let policy = policy.clone();
        let expected_path = expected_path.clone();
        async move {
            let reject = |status| {
                Response::builder()
                    .status(status)
                    .header(http::header::CONNECTION, "close")
                    .body(Full::new(Bytes::new()))
                    .expect("fixed WebSocket rejection response is valid")
            };
            if request.uri().path() != expected_path {
                if let Some(sender) = sender {
                    let _ = sender.send(None);
                }
                return Ok::<_, std::convert::Infallible>(reject(StatusCode::NOT_FOUND));
            }
            if !websocket_upgrade_for_request.load(std::sync::atomic::Ordering::Acquire) {
                if let Some(sender) = sender {
                    let _ = sender.send(None);
                }
                return Ok(reject(StatusCode::BAD_REQUEST));
            }
            let declared_body = request
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            if declared_body > 0
                || request
                    .headers()
                    .contains_key(http::header::TRANSFER_ENCODING)
            {
                if let Some(sender) = sender {
                    let _ = sender.send(None);
                }
                return Ok(reject(StatusCode::BAD_REQUEST));
            }

            let origin_fields = request
                .headers()
                .get_all(ORIGIN)
                .iter()
                .map(|value| value.as_bytes())
                .collect::<Vec<_>>();
            let protocols = match request
                .headers()
                .get_all(SEC_WEBSOCKET_PROTOCOL)
                .iter()
                .try_fold(Vec::new(), |mut values, header| {
                    let value = header.to_str().map_err(|_| ())?;
                    for token in value.split(',') {
                        let token = token.trim_matches([' ', '\t']);
                        if token.is_empty() {
                            return Err(());
                        }
                        values.push(token.to_owned());
                    }
                    Ok::<_, ()>(values)
                }) {
                Ok(protocols) => protocols,
                Err(()) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::BAD_REQUEST));
                }
            };
            let protocol_refs = protocols.iter().map(String::as_str).collect::<Vec<_>>();
            let selected_protocol = match policy.validate(&origin_fields, &protocol_refs) {
                Ok(protocol) => protocol,
                Err(zirk_runtime_core::http_websocket::WebSocketPolicyError::OriginDenied) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::FORBIDDEN));
                }
                Err(_) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::BAD_REQUEST));
                }
            };

            // Tungstenite validates GET, HTTP/1.1, Connection/Upgrade, version,
            // and the 16-byte base64 key, then computes Sec-WebSocket-Accept.
            let on_upgrade = hyper::upgrade::on(&mut request);
            let (parts, body) = request.into_parts();
            drop(body);
            let mut handshake_request = Request::from_parts(parts, ());
            // Hyper removes hop-by-hop Connection fields after parsing. The
            // framing guard above has already required the original raw field
            // to contain the Upgrade token.
            handshake_request
                .headers_mut()
                .insert(CONNECTION, http::HeaderValue::from_static("Upgrade"));
            let mut response =
                match tokio_tungstenite::tungstenite::handshake::server::create_response(
                    &handshake_request,
                ) {
                    Ok(response) => response,
                    Err(_) => {
                        if let Some(sender) = sender {
                            let _ = sender.send(None);
                        }
                        return Ok(reject(StatusCode::BAD_REQUEST));
                    }
                };
            if let Some(protocol) = selected_protocol.as_ref() {
                let value = match http::HeaderValue::from_str(protocol) {
                    Ok(value) => value,
                    Err(_) => {
                        if let Some(sender) = sender {
                            let _ = sender.send(None);
                        }
                        return Ok(reject(StatusCode::BAD_REQUEST));
                    }
                };
                response.headers_mut().insert(SEC_WEBSOCKET_PROTOCOL, value);
            }
            // Keep the response's mandatory upgrade fields explicit in this
            // adapter so future response-header filtering cannot strip them.
            response
                .headers_mut()
                .insert(CONNECTION, http::HeaderValue::from_static("Upgrade"));
            response
                .headers_mut()
                .insert(UPGRADE, http::HeaderValue::from_static("websocket"));
            if let Some(sender) = sender {
                let _ = sender.send(Some((on_upgrade, selected_protocol)));
            } else {
                return Ok(reject(StatusCode::SERVICE_UNAVAILABLE));
            }
            Ok(response.map(|_| Full::new(Bytes::new())))
        }
    });

    let rejected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(Some(DEFAULT_HTTP_SERVER_TIMEOUTS.response_headers))
        .max_headers(256);
    let connection = builder
        .serve_connection(
            TokioIo::new(FramingGuardIo::new_with_websocket_flag(
                stream,
                Arc::clone(&rejected),
                Arc::clone(&websocket_upgrade),
            )),
            service,
        )
        .with_upgrades();
    let connection_task = crate::network_future::spawn_on_reactor(async move {
        connection.await.map_err(|_| HttpRuntimeError::Protocol)
    });
    let (on_upgrade, selected_protocol) =
        match crate::network_future::block_on_current(upgrade_rx).map_err(map_network_future)? {
            Ok(Some(on_upgrade)) => on_upgrade,
            Ok(None) | Err(_) => {
                crate::network_future::block_on_current(connection_task)
                    .map_err(map_network_future)?
                    .map_err(|_| HttpRuntimeError::Transport)??;
                return if rejected.load(std::sync::atomic::Ordering::Acquire) {
                    Err(HttpRuntimeError::Protocol)
                } else {
                    Ok(())
                };
            }
        };
    let upgraded = crate::network_future::block_on_current(on_upgrade)
        .map_err(map_network_future)?
        .map_err(|_| HttpRuntimeError::Protocol)?;
    let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_frame_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    let websocket_stream = crate::network_future::block_on_current(
        tokio_tungstenite::WebSocketStream::from_raw_socket(
            HttpIo::Upgraded(Box::new(TokioIo::new(upgraded))),
            Role::Server,
            Some(config),
        ),
    )
    .map_err(map_network_future)?;

    let websocket = HttpWebSocket {
        stream: websocket_stream,
        h2_driver: None,
        owner: crate::executor::current_task(),
        idle_timeout: DEFAULT_HTTP_SERVER_TIMEOUTS.response_idle,
        negotiated_subprotocol: selected_protocol,
        closed: false,
        receive_limit: max_messages,
        received_messages: 0,
    };

    handler(websocket)?;
    crate::network_future::block_on_current(connection_task)
        .map_err(map_network_future)?
        .map_err(|_| HttpRuntimeError::Transport)??;
    Ok(())
}

/// Accepts one RFC 8441 extended CONNECT WebSocket stream. Hyper keeps HTTP/2
/// framing and flow control on the reactor; only the upgraded stream crosses
/// to the owning Zirk task, through a one-slot channel.
fn is_http2_websocket_extended_connect<B>(request: &Request<B>, expected_path: &str) -> bool {
    request.method() == http::Method::CONNECT
        && request.version() == http::Version::HTTP_2
        && request.uri().path() == expected_path
        && request
            .extensions()
            .get::<hyper::ext::Protocol>()
            .is_some_and(|protocol| protocol.as_str() == "websocket")
}

fn serve_http2_websocket_once(
    stream: HttpServerIo,
    path: &str,
    policy: zirk_runtime_core::http_websocket::WebSocketAcceptPolicy,
    max_messages: usize,
    handler: impl FnOnce(HttpWebSocket) -> Result<(), HttpRuntimeError>,
) -> Result<(), HttpRuntimeError> {
    use http::StatusCode;
    use http::header::{ORIGIN, SEC_WEBSOCKET_PROTOCOL};
    use tokio_tungstenite::tungstenite::protocol::Role;

    if !stream.negotiated_http2()
        || path.is_empty()
        || !path.starts_with('/')
        || path.contains(['?', '#', '\r', '\n'])
        || max_messages == 0
        || max_messages > 1_024
    {
        return Err(HttpRuntimeError::InvalidRequest);
    }

    let (upgrade_tx, upgrade_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let upgrade_tx = std::sync::Mutex::new(Some(upgrade_tx));
    let expected_path = path.to_owned();
    let service = hyper::service::service_fn(move |mut request: Request<hyper::body::Incoming>| {
        let sender = upgrade_tx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let expected_path = expected_path.clone();
        let policy = policy.clone();
        async move {
            let reject = |status| {
                Response::builder()
                    .status(status)
                    .body(Full::new(Bytes::new()))
                    .expect("fixed HTTP/2 WebSocket rejection response is valid")
            };
            if !is_http2_websocket_extended_connect(&request, &expected_path) {
                if let Some(sender) = sender {
                    let _ = sender.send(None);
                }
                return Ok::<_, std::convert::Infallible>(reject(StatusCode::BAD_REQUEST));
            }
            let origins = request
                .headers()
                .get_all(ORIGIN)
                .iter()
                .map(|value| value.as_bytes())
                .collect::<Vec<_>>();
            let protocols = match request
                .headers()
                .get_all(SEC_WEBSOCKET_PROTOCOL)
                .iter()
                .try_fold(Vec::new(), |mut values, header| {
                    let value = header.to_str().map_err(|_| ())?;
                    for token in value.split(',') {
                        let token = token.trim_matches([' ', '\t']);
                        if token.is_empty() {
                            return Err(());
                        }
                        values.push(token.to_owned());
                    }
                    Ok::<_, ()>(values)
                }) {
                Ok(protocols) => protocols,
                Err(()) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::BAD_REQUEST));
                }
            };
            let protocol_refs = protocols.iter().map(String::as_str).collect::<Vec<_>>();
            let selected_protocol = match policy.validate(&origins, &protocol_refs) {
                Ok(protocol) => protocol,
                Err(zirk_runtime_core::http_websocket::WebSocketPolicyError::OriginDenied) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::FORBIDDEN));
                }
                Err(_) => {
                    if let Some(sender) = sender {
                        let _ = sender.send(None);
                    }
                    return Ok(reject(StatusCode::BAD_REQUEST));
                }
            };
            let on_upgrade = hyper::upgrade::on(&mut request);
            let mut response = Response::builder().status(StatusCode::OK);
            if let Some(protocol) = selected_protocol.as_ref() {
                response = response.header(SEC_WEBSOCKET_PROTOCOL, protocol);
            }
            let response = response
                .body(Full::new(Bytes::new()))
                .expect("validated WebSocket response headers are valid");
            if let Some(sender) = sender {
                let _ = sender.send(Some((on_upgrade, selected_protocol)));
            } else {
                return Ok(reject(StatusCode::SERVICE_UNAVAILABLE));
            }
            Ok(response)
        }
    });
    let connection = hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
        .enable_connect_protocol()
        .serve_connection(TokioIo::new(stream), service);
    let connection_task = crate::network_future::spawn_on_reactor(async move {
        let mut connection = Box::pin(connection);
        tokio::select! {
            result = &mut connection => match result {
                Ok(()) => Ok(()),
                Err(error) if is_peer_disconnect(&error) => Ok(()),
                Err(_) => Err(HttpRuntimeError::Protocol),
            },
            shutdown = shutdown_rx => {
                if shutdown.is_ok() {
                    connection.as_mut().graceful_shutdown();
                }
                match connection.await {
                    Ok(()) => Ok(()),
                    Err(error) if shutdown.is_ok() && is_peer_disconnect(&error) => Ok(()),
                    Err(_) => Err(HttpRuntimeError::Protocol),
                }
            }
        }
    });
    let (on_upgrade, selected_protocol) =
        match crate::network_future::block_on_current(upgrade_rx).map_err(map_network_future)? {
            Ok(Some(upgrade)) => upgrade,
            Ok(None) | Err(_) => {
                crate::network_future::block_on_current(connection_task)
                    .map_err(map_network_future)?
                    .map_err(|_| HttpRuntimeError::Transport)??;
                return Ok(());
            }
        };
    let upgraded = crate::network_future::block_on_current(on_upgrade)
        .map_err(map_network_future)?
        .map_err(|_| HttpRuntimeError::Protocol)?;
    let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    config.max_frame_size = Some(HTTP_WEBSOCKET_MESSAGE_LIMIT);
    let websocket_stream = crate::network_future::block_on_current(
        tokio_tungstenite::WebSocketStream::from_raw_socket(
            HttpIo::Upgraded(Box::new(TokioIo::new(upgraded))),
            Role::Server,
            Some(config),
        ),
    )
    .map_err(map_network_future)?;
    let websocket = HttpWebSocket {
        stream: websocket_stream,
        h2_driver: None,
        owner: crate::executor::current_task(),
        idle_timeout: DEFAULT_HTTP_SERVER_TIMEOUTS.response_idle,
        negotiated_subprotocol: selected_protocol,
        closed: false,
        receive_limit: max_messages,
        received_messages: 0,
    };
    handler(websocket)?;
    let _ = shutdown_tx.send(());
    crate::network_future::block_on_current(connection_task)
        .map_err(map_network_future)?
        .map_err(|_| HttpRuntimeError::Transport)??;
    Ok(())
}

#[allow(dead_code)] // Used by the internal WebSocket server transport.
fn map_network_future(error: crate::network_future::NetworkFutureError) -> HttpRuntimeError {
    match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    }
}

/// Runs HTTP/2 framing and flow control on the native reactor while keeping
/// the generated Zirk handler on its owning task stack. One bounded channel
/// admits one request for this public one-shot server operation.
fn serve_http2_offloaded(
    stream: HttpServerIo,
    secure: bool,
    handler: impl FnOnce(HttpRequestValues) -> HttpResponseValues,
) -> Result<(), HttpRuntimeError> {
    let owner_executor = crate::executor::current_executor_id();
    let owner_task = crate::executor::current_task();
    let (request_tx, mut request_rx) = tokio::sync::mpsc::channel(1);
    let server = crate::network_future::spawn_on_reactor(async move {
        let dispatched = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
            let request_tx = request_tx.clone();
            let dispatched = Arc::clone(&dispatched);
            async move {
                if dispatched.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return Ok::<_, std::convert::Infallible>(to_hyper_response(simple_response(
                        400,
                        b"one request per accepted stream",
                    )));
                }
                let request = match parse_incoming_request(
                    request,
                    secure,
                    DEFAULT_HTTP_SERVER_TIMEOUTS.response_idle,
                )
                .await
                {
                    Ok(request) => request,
                    Err(HttpRuntimeError::BodyTooLarge) => {
                        return Ok(to_hyper_response(simple_response(
                            413,
                            b"request body too large",
                        )));
                    }
                    Err(HttpRuntimeError::Timeout) => {
                        return Ok(to_hyper_response(simple_response(
                            408,
                            b"request body timed out",
                        )));
                    }
                    Err(_) => {
                        return Ok(to_hyper_response(simple_response(400, b"invalid request")));
                    }
                };
                let accepted = match AcceptEncoding::parse(&request.headers) {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        return Ok(to_hyper_response(simple_response(
                            400,
                            b"invalid Accept-Encoding",
                        )));
                    }
                };
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                if request_tx.send((request, response_tx)).await.is_err() {
                    return Ok(to_hyper_response(simple_response(
                        503,
                        b"request handler unavailable",
                    )));
                }
                let mut cancellation = RequestCancellationGuard::new(owner_executor, owner_task);
                let response = response_rx.await;
                cancellation.disarm();
                match response {
                    Ok(response) => Ok(to_hyper_response(
                        prepare_server_response(response, &accepted)
                            .await
                            .unwrap_or_else(|_| simple_response(500, b"response encoding failed")),
                    )),
                    Err(_) => Ok(to_hyper_response(simple_response(
                        500,
                        b"request handler cancelled",
                    ))),
                }
            }
        });
        hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
            .serve_connection(TokioIo::new(stream), service)
            .await
            .map_err(|_| HttpRuntimeError::Protocol)
    });

    let Some((request, response_tx)) = crate::network_future::block_on_current(request_rx.recv())
        .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })?
    else {
        return crate::network_future::block_on_current(server)
            .map_err(|error| match error {
                crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
                crate::network_future::NetworkFutureError::CapacityExceeded => {
                    HttpRuntimeError::Transport
                }
            })?
            .map_err(|_| HttpRuntimeError::Transport)?;
    };
    let response =
        run_server_handler_with_deadline(request, handler, DEFAULT_HTTP_SERVER_HANDLER_TIMEOUT)?;
    let (response, producer) = attach_http_response_producer(response)?;
    response_tx
        .send(response)
        .map_err(|_| HttpRuntimeError::Cancelled)?;
    if let Some((producer, sender)) = producer {
        pump_http_response_producer(producer, sender)?;
    }
    crate::network_future::block_on_current(server)
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?
        .map_err(|_| HttpRuntimeError::Protocol)?
}

/// Serves a fixed number of HTTP/2 streams over one TLS connection. Hyper owns
/// framing and flow control on the reactor; completed requests move over a
/// bounded channel and run in independently cancelable Zirk tasks on the
/// owning executor.
pub(crate) fn serve_http2_requests(
    stream: impl Into<HttpServerIo>,
    secure: bool,
    max_requests: u64,
    handler: impl Fn(HttpRequestValues) -> HttpResponseValues + Clone + Send + 'static,
) -> Result<(), HttpRuntimeError> {
    let stream = stream.into();
    if !secure || !stream.negotiated_http2() || max_requests == 0 {
        return Err(HttpRuntimeError::Protocol);
    }
    if max_requests == 1 {
        return serve_http2_offloaded(stream, secure, handler);
    }
    serve_http2_many_offloaded(stream, secure, max_requests, handler)
}

type Http2ShutdownSignal = Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>;

fn signal_http2_shutdown_after_final_request(
    shutdown: &Http2ShutdownSignal,
    sequence: u64,
    max_requests: u64,
) {
    if sequence.checked_add(1) == Some(max_requests)
        && let Some(signal) = shutdown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    {
        let _ = signal.send(());
    }
}

fn serve_http2_many_offloaded(
    stream: HttpServerIo,
    secure: bool,
    max_requests: u64,
    handler: impl Fn(HttpRequestValues) -> HttpResponseValues + Clone + Send + 'static,
) -> Result<(), HttpRuntimeError> {
    const MAX_CONCURRENT_SERVER_STREAMS: u32 = 8;

    let owner_executor = crate::executor::current_executor_id();
    let (request_tx, mut request_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let shutdown_tx: Http2ShutdownSignal = Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));
    let admitted = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let server = crate::network_future::spawn_on_reactor(async move {
        let service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
            let request_tx = request_tx.clone();
            let shutdown_tx = Arc::clone(&shutdown_tx);
            let admitted = Arc::clone(&admitted);
            async move {
                let sequence = admitted.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                if sequence >= max_requests {
                    return Ok::<_, std::convert::Infallible>(to_hyper_response(simple_response(
                        503,
                        b"HTTP/2 request limit reached",
                    )));
                }
                let request = match parse_incoming_request(
                    request,
                    secure,
                    DEFAULT_HTTP_SERVER_TIMEOUTS.response_idle,
                )
                .await
                {
                    Ok(request) => request,
                    Err(HttpRuntimeError::BodyTooLarge) => {
                        signal_http2_shutdown_after_final_request(
                            &shutdown_tx,
                            sequence,
                            max_requests,
                        );
                        return Ok(to_hyper_response(simple_response(
                            413,
                            b"request body too large",
                        )));
                    }
                    Err(HttpRuntimeError::Timeout) => {
                        signal_http2_shutdown_after_final_request(
                            &shutdown_tx,
                            sequence,
                            max_requests,
                        );
                        return Ok(to_hyper_response(simple_response(
                            408,
                            b"request body timed out",
                        )));
                    }
                    Err(_) => {
                        signal_http2_shutdown_after_final_request(
                            &shutdown_tx,
                            sequence,
                            max_requests,
                        );
                        return Ok(to_hyper_response(simple_response(400, b"invalid request")));
                    }
                };
                let accepted = match AcceptEncoding::parse(&request.headers) {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        signal_http2_shutdown_after_final_request(
                            &shutdown_tx,
                            sequence,
                            max_requests,
                        );
                        return Ok(to_hyper_response(simple_response(
                            400,
                            b"invalid Accept-Encoding",
                        )));
                    }
                };
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                let request_cancellation = Arc::new(RequestTaskCancellation::default());
                if request_tx
                    .send((request, response_tx, Arc::clone(&request_cancellation)))
                    .await
                    .is_err()
                {
                    signal_http2_shutdown_after_final_request(&shutdown_tx, sequence, max_requests);
                    return Ok(to_hyper_response(simple_response(
                        503,
                        b"request handler unavailable",
                    )));
                }
                let mut cancellation =
                    RequestCancellationGuard::for_request(owner_executor, request_cancellation);
                let response = response_rx.await;
                cancellation.disarm();
                let response = match response {
                    Ok(response) => response,
                    Err(_) => simple_response(500, b"request handler cancelled"),
                };
                signal_http2_shutdown_after_final_request(&shutdown_tx, sequence, max_requests);
                Ok(to_hyper_response(
                    prepare_server_response(response, &accepted)
                        .await
                        .unwrap_or_else(|_| simple_response(500, b"response encoding failed")),
                ))
            }
        });
        let mut builder =
            hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new());
        let mut connection = Box::pin(
            builder
                .max_concurrent_streams(Some(MAX_CONCURRENT_SERVER_STREAMS))
                .serve_connection(TokioIo::new(stream), service),
        );
        tokio::select! {
            result = &mut connection => result.map_err(|_| HttpRuntimeError::Protocol),
            signal = &mut shutdown_rx => {
                if signal.is_ok() {
                    connection.as_mut().graceful_shutdown();
                }
                await_http2_graceful_drain(connection, DEFAULT_HTTP_SERVER_DRAIN_TIMEOUT).await
            }
        }
    });

    let mut active_tasks = std::collections::VecDeque::new();
    let handler_failure = Arc::new(std::sync::Mutex::new(None));
    while let Some((request, response_tx, cancellation)) =
        crate::network_future::block_on_current(request_rx.recv()).map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?
    {
        if active_tasks.len() >= MAX_CONCURRENT_SERVER_STREAMS as usize
            && let Some(task) = active_tasks.pop_front()
        {
            await_http2_handler_task(task);
        }
        let handler = handler.clone();
        let task_failure = Arc::clone(&handler_failure);
        let task_cancellation = Arc::clone(&cancellation);
        let task = crate::executor::spawn(move || {
            if task_cancellation.is_cancelled() {
                return 0;
            }
            let response = match run_server_handler_with_deadline(
                request,
                handler,
                DEFAULT_HTTP_SERVER_HANDLER_TIMEOUT,
            ) {
                Ok(response) => response,
                Err(error) => {
                    if error != HttpRuntimeError::Cancelled {
                        task_failure
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .get_or_insert(error);
                    }
                    return 0;
                }
            };
            let (response, producer) = match attach_http_response_producer(response) {
                Ok(response) => response,
                Err(error) => {
                    task_failure
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .get_or_insert(error);
                    return 0;
                }
            };
            if response_tx.send(response).is_err() {
                return 0;
            }
            if let Some((producer, sender)) = producer
                && let Err(error) = pump_http_response_producer(producer, sender)
            {
                task_failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_or_insert(error);
            }
            1
        });
        cancellation.register(owner_executor, task);
        active_tasks.push_back(task);
        // Let the newly admitted handler reach its first cooperative wait
        // before the dispatcher goes back to polling the network future. This
        // keeps the single-threaded Zirk executor fair when the reactor
        // publishes several readiness events at once.
        crate::executor::yield_now();
    }
    while let Some(task) = active_tasks.pop_front() {
        await_http2_handler_task(task);
    }
    drop(request_rx);
    crate::network_future::block_on_current(server)
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?
        .map_err(|_| HttpRuntimeError::Cancelled)??;
    if let Some(error) = handler_failure
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
    {
        return Err(error);
    }
    Ok(())
}

/// A peer reset cancels only the Zirk task serving that HTTP/2 stream. Joining
/// such a child normally rethrows its `Cancelled` payload into the dispatcher,
/// which would cancel the whole connection and unwind across the server ABI.
/// Consume that child outcome while preserving cancellation of the dispatcher
/// itself and propagating every non-cancellation failure.
fn await_http2_handler_task(task: crate::task::TaskId) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::executor::await_task(task)
    }));
    match outcome {
        Ok(_) => {}
        Err(payload)
            if payload.is::<crate::executor::Cancelled>()
                && !crate::executor::current_task_cancel_requested() =>
        {
            // The peer disconnected this stream; sibling handlers keep running.
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn is_peer_disconnect(error: &hyper::Error) -> bool {
    let mut source: &(dyn std::error::Error + 'static) = error;
    loop {
        if let Some(io_error) = source.downcast_ref::<io::Error>() {
            return matches!(
                io_error.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
            );
        }
        let Some(next) = source.source() else {
            return false;
        };
        source = next;
    }
}

/// Serves one request through a bounded method-aware route table. The selected
/// handler executes in the caller's Zirk task; this is the dispatch primitive
/// the public long-lived HTTPServer adapter will build on.
#[allow(dead_code)] // Internal until compiler-owned server bindings land.
pub(crate) fn serve_once_routed(
    stream: impl Into<HttpServerIo>,
    secure: bool,
    routes: zirk_runtime_core::http_router::HttpRouteTable,
    handler: impl FnOnce(
        zirk_runtime_core::http_router::HttpRouteMatch,
        HttpRequestValues,
    ) -> HttpResponseValues,
) -> Result<(), HttpRuntimeError> {
    serve_once(stream, secure, move |request| {
        match routes.resolve(request.method, request.url.path()) {
            Ok(Some(route)) => handler(route, request),
            Ok(None) => {
                let allowed = routes
                    .allowed_methods(request.url.path())
                    .unwrap_or_default();
                if allowed.is_empty() {
                    simple_response(404, b"not found")
                } else {
                    let mut response = simple_response(405, b"method not allowed");
                    let value = allowed
                        .iter()
                        .map(|method| method.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let _ = response.headers.append("allow", value.as_bytes());
                    response
                }
            }
            Err(_) => simple_response(400, b"invalid request path"),
        }
    })
}

#[allow(dead_code)] // Test seam until compiler-owned server bindings land.
fn serve_once_with_timeouts(
    stream: impl Into<HttpServerIo>,
    secure: bool,
    timeouts: HttpTimeouts,
    handler: impl FnOnce(HttpRequestValues) -> HttpResponseValues,
) -> Result<(), HttpRuntimeError> {
    let owner_executor = crate::executor::current_executor_id();
    let owner_task = crate::executor::current_task();
    let (request_tx, mut request_rx) = tokio::sync::mpsc::channel(1);
    let service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
        let request_tx = request_tx.clone();
        async move {
            let parsed = parse_incoming_request(request, secure, timeouts.response_idle).await;
            let (request, accepted) = match parsed {
                Ok(request) => match AcceptEncoding::parse(&request.headers) {
                    Ok(accepted) => (request, accepted),
                    Err(_) => {
                        return Ok::<_, std::convert::Infallible>(to_hyper_response(
                            simple_response(400, b"invalid Accept-Encoding"),
                        ));
                    }
                },
                Err(HttpRuntimeError::BodyTooLarge) => {
                    return Ok(to_hyper_response(simple_response(
                        413,
                        b"request body too large",
                    )));
                }
                Err(HttpRuntimeError::Timeout) => {
                    return Ok(to_hyper_response(simple_response(
                        408,
                        b"request body timed out",
                    )));
                }
                Err(_) => {
                    return Ok(to_hyper_response(simple_response(400, b"invalid request")));
                }
            };
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            if request_tx
                .send((request, accepted, response_tx))
                .await
                .is_err()
            {
                return Ok(to_hyper_response(simple_response(
                    503,
                    b"request handler unavailable",
                )));
            }
            let mut cancellation = RequestCancellationGuard::new(owner_executor, owner_task);
            let response = response_rx.await;
            cancellation.disarm();
            let response =
                response.unwrap_or_else(|_| simple_response(500, b"request handler cancelled"));
            Ok(to_hyper_response(response))
        }
    });
    let rejected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(Some(timeouts.response_headers))
        .keep_alive(false)
        .max_headers(256);
    let connection = builder.serve_connection(
        TokioIo::new(FramingGuardIo::new(stream.into(), Arc::clone(&rejected))),
        service,
    );
    let connection_task = crate::network_future::spawn_on_reactor(async move {
        connection.await.map_err(|_| HttpRuntimeError::Protocol)
    });
    let Some((request, accepted, response_tx)) = crate::network_future::block_on_current(
        request_rx.recv(),
    )
    .map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => HttpRuntimeError::Transport,
    })?
    else {
        let result = crate::network_future::block_on_current(connection_task)
            .map_err(|error| match error {
                crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
                crate::network_future::NetworkFutureError::CapacityExceeded => {
                    HttpRuntimeError::Transport
                }
            })?
            .map_err(|_| HttpRuntimeError::Protocol);
        if rejected.load(std::sync::atomic::Ordering::Acquire) {
            return Err(HttpRuntimeError::Protocol);
        }
        return result?;
    };

    let response =
        run_server_handler_with_deadline(request, handler, DEFAULT_HTTP_SERVER_HANDLER_TIMEOUT)?;
    let response =
        crate::network_future::block_on_current(prepare_server_response(response, &accepted))
            .map_err(|error| match error {
                crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
                crate::network_future::NetworkFutureError::CapacityExceeded => {
                    HttpRuntimeError::Transport
                }
            })?
            .unwrap_or_else(|_| simple_response(500, b"response encoding failed"));
    let (response, producer) = attach_http_response_producer(response)?;
    response_tx
        .send(response)
        .map_err(|_| HttpRuntimeError::Cancelled)?;
    if let Some((producer, sender)) = producer {
        pump_http_response_producer(producer, sender)?;
    }

    let result = crate::network_future::block_on_current(connection_task)
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?
        .map_err(|_| HttpRuntimeError::Protocol);
    if rejected.load(std::sync::atomic::Ordering::Acquire) {
        return Err(HttpRuntimeError::Protocol);
    }
    result?
}

/// Runs an HTTP callback under a cooperative deadline. The watchdog is an
/// executor task, so it fires while the owner is suspended at an I/O or timer
/// safe point. It requests cancellation of the owner task instead of trying
/// to preempt generated code on its stack.
fn run_server_handler_with_deadline(
    request: HttpRequestValues,
    handler: impl FnOnce(HttpRequestValues) -> HttpResponseValues,
    timeout: Duration,
) -> Result<HttpResponseValues, HttpRuntimeError> {
    if timeout.is_zero() || !crate::executor::in_executor_task() {
        return Err(HttpRuntimeError::InvalidRequest);
    }
    let owner = crate::executor::current_task();
    let timeout_nanos = i64::try_from(timeout.as_nanos()).unwrap_or(i64::MAX);
    let deadline_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watchdog_fired = Arc::clone(&deadline_fired);
    let watchdog = crate::executor::spawn(move || {
        crate::executor::sleep(timeout_nanos);
        if crate::executor::current_task_cancel_requested() {
            return 0;
        }
        watchdog_fired.store(true, std::sync::atomic::Ordering::Release);
        crate::executor::cancel_current(owner);
        0
    });
    let _watchdog_guard = ServerHandlerWatchdog(watchdog);

    let response = handler(request);
    if crate::executor::current_task_cancel_requested() {
        return Err(
            if deadline_fired.load(std::sync::atomic::Ordering::Acquire) {
                HttpRuntimeError::Timeout
            } else {
                HttpRuntimeError::Cancelled
            },
        );
    }
    Ok(response)
}

struct ServerHandlerWatchdog(crate::task::TaskId);

impl Drop for ServerHandlerWatchdog {
    fn drop(&mut self) {
        crate::executor::cancel_current(self.0);
    }
}

/// Cancels the owner task if Hyper drops a request future while its Zirk
/// handler is still running. Normal response delivery disarms the guard.
struct RequestCancellationGuard {
    executor: u64,
    task: Option<crate::task::TaskId>,
    request: Option<Arc<RequestTaskCancellation>>,
    armed: bool,
}

impl RequestCancellationGuard {
    fn new(executor: u64, task: crate::task::TaskId) -> Self {
        Self {
            executor,
            task: Some(task),
            request: None,
            armed: true,
        }
    }

    fn for_request(executor: u64, request: Arc<RequestTaskCancellation>) -> Self {
        Self {
            executor,
            task: None,
            request: Some(request),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RequestCancellationGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Some(task) = self.task {
                let _ = crate::executor::request_external_cancel(self.executor, task);
            }
            if let Some(request) = &self.request {
                request.cancel(self.executor);
            }
        }
    }
}

#[derive(Default)]
struct RequestTaskCancellation {
    state: std::sync::Mutex<RequestTaskCancellationState>,
}

#[derive(Default)]
struct RequestTaskCancellationState {
    task: Option<crate::task::TaskId>,
    cancelled: bool,
}

impl RequestTaskCancellation {
    fn register(&self, executor: u64, task: crate::task::TaskId) {
        let cancel_now = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.task = Some(task);
            state.cancelled
        };
        if cancel_now {
            let _ = crate::executor::request_external_cancel(executor, task);
        }
    }

    fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cancelled
    }

    fn cancel(&self, executor: u64) {
        let task = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.cancelled = true;
            state.task
        };
        if let Some(task) = task {
            let _ = crate::executor::request_external_cancel(executor, task);
        }
    }
}

async fn parse_incoming_request(
    request: Request<hyper::body::Incoming>,
    secure: bool,
    body_idle_timeout: Duration,
) -> Result<HttpRequestValues, HttpRuntimeError> {
    let method = match request.method().as_str() {
        "GET" => zirk_runtime_core::http_values::HttpMethod::Get,
        "HEAD" => zirk_runtime_core::http_values::HttpMethod::Head,
        "POST" => zirk_runtime_core::http_values::HttpMethod::Post,
        "PUT" => zirk_runtime_core::http_values::HttpMethod::Put,
        "PATCH" => zirk_runtime_core::http_values::HttpMethod::Patch,
        "DELETE" => zirk_runtime_core::http_values::HttpMethod::Delete,
        "OPTIONS" => zirk_runtime_core::http_values::HttpMethod::Options,
        "CONNECT" => zirk_runtime_core::http_values::HttpMethod::Connect,
        "TRACE" => zirk_runtime_core::http_values::HttpMethod::Trace,
        "QUERY" => zirk_runtime_core::http_values::HttpMethod::Query,
        _ => return Err(HttpRuntimeError::InvalidRequest),
    };
    let authority = request
        .uri()
        .authority()
        .map(|authority| authority.as_str())
        .or_else(|| {
            request
                .headers()
                .get(hyper::header::HOST)
                .and_then(|host| host.to_str().ok())
        })
        .ok_or(HttpRuntimeError::InvalidRequest)?;
    let scheme = if secure { "https" } else { "http" };
    let uri = request.uri();
    let target = uri.path_and_query().map_or("/", |value| value.as_str());
    let url =
        zirk_runtime_core::http_values::Url::parse(&format!("{scheme}://{authority}{target}"))
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    let (parts, mut body) = request.into_parts();
    let mut headers = Headers::new();
    for (name, value) in parts.headers.iter() {
        headers
            .append(name.as_str(), value.as_bytes())
            .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    }
    headers
        .validate_message_framing()
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;

    let mut parsed = HttpRequestValues {
        method,
        method_token: method.as_str().to_owned(),
        url,
        headers,
        body: Vec::new(),
    };
    parsed
        .validate_query()
        .map_err(|_| HttpRuntimeError::InvalidRequest)?;
    loop {
        let frame = tokio::time::timeout(body_idle_timeout, body.frame())
            .await
            .map_err(|_| HttpRuntimeError::Timeout)?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| HttpRuntimeError::Protocol)?;
        if let Ok(data) = frame.into_data() {
            append_bounded_body(&mut parsed.body, &data, MAX_HTTP_BUFFERED_BODY_BYTES)?;
        }
    }
    decode_request_content(&mut parsed).await?;
    Ok(parsed)
}

async fn decode_request_content(request: &mut HttpRequestValues) -> Result<(), HttpRuntimeError> {
    if request.headers.get("content-encoding").is_none() {
        return Ok(());
    }
    let accepted =
        AcceptEncoding::parse(&Headers::new()).map_err(|_| HttpRuntimeError::Protocol)?;
    let headers = std::mem::take(&mut request.headers);
    let encoded = std::mem::take(&mut request.body);
    let (headers, body, worker) = decode_response_body(
        headers,
        HttpStreamingBody::Buffered {
            bytes: Bytes::from(encoded),
            offset: 0,
        },
        &accepted,
    )?;
    match body {
        HttpStreamingBody::Chunks(mut chunks) => {
            while let Some(chunk) = chunks.recv().await {
                append_bounded_body(&mut request.body, &chunk?, MAX_HTTP_BUFFERED_BODY_BYTES)?;
            }
        }
        HttpStreamingBody::Buffered { bytes, .. } => {
            append_bounded_body(&mut request.body, &bytes, MAX_HTTP_BUFFERED_BODY_BYTES)?;
        }
        HttpStreamingBody::Http1(_) => return Err(HttpRuntimeError::Protocol),
    }
    drop(worker);
    request.headers = headers;
    Ok(())
}

fn simple_response(status: u16, body: &[u8]) -> HttpResponseValues {
    HttpResponseValues {
        status: HttpStatusCode::new(status).expect("fixed response status is valid"),
        headers: Headers::new(),
        body: body.to_vec(),
        body_stream: None,
        body_producer: None,
    }
}

fn append_bounded_body(
    current: &mut Vec<u8>,
    chunk: &[u8],
    limit: usize,
) -> Result<(), HttpRuntimeError> {
    let next_len = current
        .len()
        .checked_add(chunk.len())
        .ok_or(HttpRuntimeError::BodyTooLarge)?;
    if next_len > limit {
        return Err(HttpRuntimeError::BodyTooLarge);
    }
    current.extend_from_slice(chunk);
    Ok(())
}

fn decode_text(body: Vec<u8>) -> Result<String, HttpRuntimeError> {
    String::from_utf8(body).map_err(|_| HttpRuntimeError::InvalidText)
}

fn decode_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, HttpRuntimeError> {
    serde_json::from_slice(body).map_err(|_| HttpRuntimeError::InvalidJson)
}

fn to_hyper_response(
    response: HttpResponseValues,
) -> Response<UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>> {
    let response = if response.body_stream.is_none()
        && response.body_producer.is_none()
        && response.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES
    {
        simple_response(500, b"response body exceeds the configured limit")
    } else {
        response
    };
    let mut builder = Response::builder().status(response.status.as_u16());
    for (name, value) in response.headers.iter() {
        if name != hyper::header::CONTENT_LENGTH
            && name != hyper::header::TRANSFER_ENCODING
            && name != hyper::header::CONNECTION
        {
            builder = builder.header(name, value);
        }
    }
    let producer_coding = response
        .body_producer
        .as_ref()
        .and(response.headers.get("content-encoding"))
        .and_then(|value| value.to_str().ok())
        .and_then(|coding| match coding.to_ascii_lowercase().as_str() {
            "br" => Some(ContentCoding::Brotli),
            "gzip" => Some(ContentCoding::Gzip),
            "deflate" => Some(ContentCoding::Deflate),
            _ => None,
        });
    let body = match (
        response.body_producer,
        response.body_stream,
        producer_coding,
    ) {
        (Some(producer), _, Some(coding)) => HttpZirkCompressedProducerBody::new(producer, coding)
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })
            .boxed_unsync(),
        (Some(producer), _, _) => HttpZirkProducerBody {
            producer: Some(producer),
        }
        .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })
        .boxed_unsync(),
        (None, Some(receiver), _) => HttpChannelBody { receiver }
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })
            .boxed_unsync(),
        (None, None, _) => Full::new(Bytes::from(response.body))
            .map_err(|never| -> Box<dyn std::error::Error + Send + Sync> { match never {} })
            .boxed_unsync(),
    };
    builder
        .header(hyper::header::CONNECTION, "close")
        .body(body)
        .expect("validated HTTP response values produce valid Hyper response")
}

type HttpResponseProducerSender = tokio::sync::mpsc::Sender<Result<Bytes, io::Error>>;
type AttachedHttpResponseProducer = (
    HttpResponseValues,
    Option<(
        crate::http_api::HttpServerBodyProducer,
        HttpResponseProducerSender,
    )>,
);

fn attach_http_response_producer(
    mut response: HttpResponseValues,
) -> Result<AttachedHttpResponseProducer, HttpRuntimeError> {
    let Some(producer) = response.body_producer.take() else {
        return Ok((response, None));
    };
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    response.body.clear();
    let coding = response
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .and_then(|coding| match coding.trim().to_ascii_lowercase().as_str() {
            "br" => Some(ContentCoding::Brotli),
            "gzip" => Some(ContentCoding::Gzip),
            "deflate" => Some(ContentCoding::Deflate),
            _ => None,
        });
    response.body_stream = Some(match coding {
        Some(coding) => crate::network_future::block_on_current(async move {
            encode_response_stream(receiver, coding)
        })
        .map_err(|error| match error {
            crate::network_future::NetworkFutureError::Cancelled => HttpRuntimeError::Cancelled,
            crate::network_future::NetworkFutureError::CapacityExceeded => {
                HttpRuntimeError::Transport
            }
        })?,
        None => receiver,
    });
    Ok((response, Some((producer, sender))))
}

/// Drives a generated callback only on the owner task, copying every bounded
/// Zirk array into native bytes before waiting for channel capacity. A dropped
/// receiver is peer disconnect and immediately drops the producer root.
fn pump_http_response_producer(
    mut producer: crate::http_api::HttpServerBodyProducer,
    sender: tokio::sync::mpsc::Sender<Result<Bytes, io::Error>>,
) -> Result<(), HttpRuntimeError> {
    let heartbeat = producer.sse_heartbeat_interval().map(|interval| {
        let (activity_sender, mut activity_receiver) = tokio::sync::watch::channel(());
        let heartbeat_sender = sender.clone();
        let task = crate::network_future::spawn_on_reactor(async move {
            let mut deadline = Box::pin(tokio::time::sleep(interval));
            loop {
                tokio::select! {
                    _ = heartbeat_sender.closed() => return,
                    changed = activity_receiver.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        deadline.as_mut().reset(tokio::time::Instant::now() + interval);
                    }
                    _ = &mut deadline => {
                        if heartbeat_sender
                            .send(Ok(Bytes::from_static(b": keep-alive\n\n")))
                            .await
                            .is_err()
                        {
                            return;
                        }
                        deadline.as_mut().reset(tokio::time::Instant::now() + interval);
                    }
                }
            }
        });
        (task, activity_sender)
    });
    loop {
        match producer.next_chunk() {
            Ok(Some(chunk)) => {
                let sent =
                    crate::network_future::block_on_current(sender.send(Ok(Bytes::from(chunk))))
                        .map_err(|error| match error {
                            crate::network_future::NetworkFutureError::Cancelled => {
                                HttpRuntimeError::Cancelled
                            }
                            crate::network_future::NetworkFutureError::CapacityExceeded => {
                                HttpRuntimeError::Transport
                            }
                        })?;
                if sent.is_err() {
                    return Ok(());
                }
                if let Some((_, activity)) = &heartbeat {
                    activity.send_replace(());
                }
            }
            Ok(None) => return Ok(()),
            Err(_) => {
                let _ = crate::network_future::block_on_current(sender.send(Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Zirk HTTP response producer failed",
                ))));
                return Err(HttpRuntimeError::Transport);
            }
        }
    }
}

fn request_target(url: &zirk_runtime_core::http_values::Url) -> String {
    let mut target = url.path().to_owned();
    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    target
}

fn validate_client_request(request: &HttpRequestValues) -> Result<(), HttpRuntimeError> {
    request
        .validate_client_request()
        .map_err(|error| match error {
            zirk_runtime_core::http_values::HttpValueError::UnsupportedMethod => {
                HttpRuntimeError::UnsupportedMethod
            }
            _ => HttpRuntimeError::InvalidRequest,
        })
}

fn authority(url: &zirk_runtime_core::http_values::Url) -> String {
    let host = url.host();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let default_port = if url.scheme() == "https" { 443 } else { 80 };
    if url.port() == default_port {
        host
    } else {
        format!("{host}:{}", url.port())
    }
}

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, await_task, spawn};
    use crate::network_future::block_on_current;
    use zirk_runtime_core::http_values::{HttpMethod, HttpStatusCode, Url};

    #[test]
    fn http2_graceful_drain_deadline_drops_pending_connection_future() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct PendingConnection(Arc<AtomicBool>);

        impl std::future::Future for PendingConnection {
            type Output = Result<(), hyper::Error>;

            fn poll(
                self: std::pin::Pin<&mut Self>,
                _context: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                std::task::Poll::Pending
            }
        }

        impl Drop for PendingConnection {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let result = runtime.block_on(await_http2_graceful_drain(
            PendingConnection(Arc::clone(&dropped)),
            Duration::from_millis(10),
        ));

        assert_eq!(result, Err(HttpRuntimeError::Timeout));
        assert!(dropped.load(Ordering::Acquire));
    }

    #[test]
    fn final_http2_request_signals_shutdown_even_when_request_is_rejected() {
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let shutdown = Arc::new(std::sync::Mutex::new(Some(sender)));

        signal_http2_shutdown_after_final_request(&shutdown, 0, 2);
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));

        signal_http2_shutdown_after_final_request(&shutdown, 1, 2);
        assert_eq!(receiver.try_recv(), Ok(()));
        signal_http2_shutdown_after_final_request(&shutdown, 1, 2);
    }

    #[test]
    fn server_handler_deadline_cooperatively_cancels_a_suspended_handler() {
        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let deadline_observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_completed = Arc::clone(&completed);
        let child_deadline_observed = Arc::clone(&deadline_observed);
        let outcome = Executor::new().run_with_root(move || {
            let _handler = spawn(move || {
                let request = HttpRequestValues::new(
                    HttpMethod::Get,
                    Url::parse("http://127.0.0.1/deadline").unwrap(),
                );
                let result = run_server_handler_with_deadline(
                    request,
                    move |_| {
                        crate::executor::sleep(10_000_000_000);
                        if crate::executor::current_task_cancel_requested() {
                            return simple_response(504, b"handler deadline exceeded");
                        }
                        child_completed.store(true, std::sync::atomic::Ordering::Release);
                        simple_response(200, b"late")
                    },
                    Duration::from_millis(10),
                );
                let expired = matches!(result, Err(HttpRuntimeError::Timeout));
                child_deadline_observed.store(expired, std::sync::atomic::Ordering::Release);
                usize::from(expired)
            });
            crate::executor::sleep(50_000_000);
            0
        });

        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(deadline_observed.load(std::sync::atomic::Ordering::Acquire));
        assert!(!completed.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn peer_disconnect_cancels_a_cooperatively_suspended_http1_handler() {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        reserved.set_nonblocking(true).unwrap();
        let address = reserved.local_addr().unwrap();
        let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handler_entered = Arc::clone(&entered);
        let cancellation_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handler_cancellation_seen = Arc::clone(&cancellation_seen);

        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let (stream, _) = block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(reserved).unwrap();
                    listener.accept().await.unwrap()
                })
                .unwrap();
                let result = serve_once_with_timeouts(
                    stream,
                    false,
                    DEFAULT_HTTP_SERVER_TIMEOUTS,
                    move |_| {
                        handler_entered.store(true, std::sync::atomic::Ordering::Release);
                        crate::executor::sleep(10_000_000_000);
                        if crate::executor::current_task_cancel_requested() {
                            handler_cancellation_seen
                                .store(true, std::sync::atomic::Ordering::Release);
                        }
                        simple_response(200, b"late response")
                    },
                );
                assert_eq!(result, Err(HttpRuntimeError::Cancelled));
                1
            });

            let client_entered = Arc::clone(&entered);
            let client = std::thread::spawn(move || {
                use std::io::Write;
                let mut socket = std::net::TcpStream::connect(address).unwrap();
                socket
                    .write_all(
                        b"GET /disconnect HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !client_entered.load(std::sync::atomic::Ordering::Acquire) {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "handler did not start"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                drop(socket);
            });

            crate::executor::sleep(100_000_000);
            let disconnected_handler_woke =
                cancellation_seen.load(std::sync::atomic::Ordering::Acquire);
            if !disconnected_handler_woke {
                crate::executor::cancel_current(server);
            }
            client.join().unwrap();
            assert!(
                disconnected_handler_woke,
                "peer disconnect must cancel the suspended handler before its deadline"
            );
            0
        });

        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tls_http2_peer_disconnect_cancels_a_cooperatively_suspended_handler() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let server_config = crate::network_transport::TlsServerConfig::from_der(
            vec![include_bytes!("../testdata/tls-loopback-cert.der").to_vec()],
            include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
            false,
        )
        .unwrap()
        .with_alpn_protocols(vec![b"h2".to_vec()]);
        let entered = Arc::new(AtomicBool::new(false));
        let handler_entered = Arc::clone(&entered);
        let cancellation_seen = Arc::new(AtomicBool::new(false));
        let handler_cancellation_seen = Arc::clone(&cancellation_seen);

        let outcome = Executor::new().run_with_root(move || {
            let server_grant = Arc::clone(&grant);
            let _server = spawn(move || {
                let listener = crate::network_transport::TlsListener::bind(
                    address,
                    &server_grant,
                    server_config,
                )
                .unwrap();
                let (stream, _) = listener.accept().unwrap();
                let stream: HttpServerIo = stream.into();
                assert!(stream.negotiated_http2());
                let result = serve_http2_many_offloaded(stream, true, 2, move |_| {
                    handler_entered.store(true, Ordering::Release);
                    crate::executor::sleep(10_000_000_000);
                    if crate::executor::current_task_cancel_requested() {
                        handler_cancellation_seen.store(true, Ordering::Release);
                    }
                    simple_response(200, b"late response")
                });
                assert_eq!(result, Err(HttpRuntimeError::Cancelled));
                1
            });

            let client_origin = origin.clone();
            let client_entered = Arc::clone(&entered);
            let client = std::thread::spawn(move || {
                use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};

                let mut roots = tokio_rustls::rustls::RootCertStore::empty();
                roots
                    .add(CertificateDer::from(
                        include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
                    ))
                    .unwrap();
                let mut config = tokio_rustls::rustls::ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth();
                config.alpn_protocols = vec![b"h2".to_vec()];
                let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
                    let tls = connector
                        .connect(
                            ServerName::IpAddress(
                                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST).into(),
                            ),
                            socket,
                        )
                        .await
                        .unwrap();
                    let (mut sender, connection) = hyper::client::conn::http2::handshake(
                        hyper_util::rt::TokioExecutor::new(),
                        TokioIo::new(tls),
                    )
                    .await
                    .unwrap();
                    let driver = tokio::spawn(connection);
                    let request = Request::builder()
                        .method(hyper::Method::GET)
                        .uri(format!("{client_origin}/disconnect"))
                        .body(Full::new(Bytes::new()))
                        .unwrap();
                    let response_task =
                        tokio::spawn(async move { sender.send_request(request).await });
                    let deadline = std::time::Instant::now() + Duration::from_secs(2);
                    while !client_entered.load(Ordering::Acquire) {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "handler did not start"
                        );
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                    driver.abort();
                    response_task.abort();
                });
            });

            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !cancellation_seen.load(Ordering::Acquire) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "HTTP/2 peer disconnect did not cancel the handler"
                );
                crate::executor::sleep(1_000_000);
            }
            client.join().unwrap();
            crate::executor::sleep(10_000_000);
            assert!(
                cancellation_seen.load(Ordering::Acquire),
                "HTTP/2 peer disconnect must cancel the suspended handler"
            );
            0
        });

        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn websocket_client_validates_handshake_preserves_message_types_and_closes() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        use tokio_tungstenite::tungstenite::protocol::frame::{
            Frame,
            coding::{Data, OpCode},
        };

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
                    assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Text(text) if text == "client"));
                    websocket
                        .send(Message::Frame(Frame::message(
                            Bytes::from_static(b"ser"),
                            OpCode::Data(Data::Text),
                            false,
                        )))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Frame(Frame::message(
                            Bytes::from_static(b"ver"),
                            OpCode::Data(Data::Continue),
                            true,
                        )))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Ping(Bytes::from_static(b"heartbeat")))
                        .await
                        .unwrap();
                    assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Pong(payload) if payload == "heartbeat"));
                    assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Text(text) if text == "client-reply"));
                    websocket
                        .send(Message::Binary(Bytes::from_static(b"binary")))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Text("server".into()))
                        .await
                        .unwrap();
                    assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Close(_)));
                    websocket.flush().await.unwrap();
                });
        });

        let endpoint = format!("http://127.0.0.1:{}/socket", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&endpoint).unwrap();
            let mut websocket =
                HttpWebSocket::connect(&url, grant, HttpTimeouts::default(), None).unwrap();
            websocket
                .send(HttpWebSocketMessage::Text("client".to_owned()))
                .unwrap();
            assert_eq!(
                websocket.receive(Duration::from_secs(2)).unwrap(),
                Some(HttpWebSocketMessage::Text("server".to_owned()))
            );
            assert_eq!(
                websocket.receive(Duration::from_secs(2)).unwrap(),
                Some(HttpWebSocketMessage::Ping(b"heartbeat".to_vec()))
            );
            // Tungstenite queues and flushes the protocol-mandated Pong while
            // reading Ping; applications observe Ping but must not send a
            // duplicate Pong manually.
            websocket
                .send(HttpWebSocketMessage::Text("client-reply".to_owned()))
                .unwrap();
            assert_eq!(
                websocket.receive(Duration::from_secs(2)).unwrap(),
                Some(HttpWebSocketMessage::Binary(b"binary".to_vec()))
            );
            assert_eq!(
                websocket.receive(Duration::from_secs(2)).unwrap(),
                Some(HttpWebSocketMessage::Text("server".to_owned()))
            );
            websocket.close().unwrap();
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn websocket_client_rejects_fragmented_messages_over_the_configured_limit() {
        use futures_util::SinkExt;
        use tokio_tungstenite::tungstenite::protocol::frame::coding::{Data, OpCode};
        use tokio_tungstenite::tungstenite::{Message, protocol::frame::Frame};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
                    let first_fragment = Bytes::from(vec![b'x'; HTTP_WEBSOCKET_MESSAGE_LIMIT]);
                    websocket
                        .send(Message::Frame(Frame::message(
                            first_fragment,
                            OpCode::Data(Data::Binary),
                            false,
                        )))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Frame(Frame::message(
                            Bytes::from_static(b"y"),
                            OpCode::Data(Data::Continue),
                            true,
                        )))
                        .await
                        .unwrap();
                });
        });

        let endpoint = format!("http://127.0.0.1:{}/socket", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&endpoint).unwrap();
            let mut websocket =
                HttpWebSocket::connect(&url, grant, HttpTimeouts::default(), None).unwrap();
            assert!(matches!(
                websocket.receive(Duration::from_secs(10)),
                Err(HttpRuntimeError::BodyTooLarge)
            ));
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn websocket_server_enforces_origin_selects_protocol_and_dispatches_messages() {
        use futures_util::{SinkExt, StreamExt};
        use http::header::SEC_WEBSOCKET_PROTOCOL;
        use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let endpoint = format!("ws://{address}/socket");
                    let mut denied = endpoint.as_str().into_client_request().unwrap();
                    denied.headers_mut().insert(
                        http::header::ORIGIN,
                        http::HeaderValue::from_static("https://evil.example"),
                    );
                    assert!(matches!(
                        tokio_tungstenite::connect_async(denied).await,
                        Err(tokio_tungstenite::tungstenite::Error::Http(response))
                            if response.status() == http::StatusCode::FORBIDDEN
                    ));

                    let mut request = endpoint.as_str().into_client_request().unwrap();
                    request.headers_mut().insert(
                        http::header::ORIGIN,
                        http::HeaderValue::from_static("https://app.example"),
                    );
                    request.headers_mut().insert(
                        SEC_WEBSOCKET_PROTOCOL,
                        http::HeaderValue::from_static("zirk.v1, zirk.v2"),
                    );
                    let (mut websocket, response) =
                        tokio_tungstenite::connect_async(request).await.unwrap();
                    assert_eq!(
                        response
                            .headers()
                            .get(SEC_WEBSOCKET_PROTOCOL)
                            .unwrap()
                            .to_str()
                            .unwrap(),
                        "zirk.v2"
                    );
                    websocket
                        .send(Message::Ping(Bytes::from_static(b"probe")))
                        .await
                        .unwrap();
                    assert!(matches!(
                        websocket.next().await.unwrap().unwrap(),
                        Message::Pong(payload) if payload == "probe"
                    ));
                    websocket.send(Message::Text("hello".into())).await.unwrap();
                    assert!(matches!(
                        websocket.next().await.unwrap().unwrap(),
                        Message::Text(text) if text == "hello"
                    ));
                    websocket.close(None).await.unwrap();
                    assert!(matches!(
                        websocket.next().await.unwrap().unwrap(),
                        Message::Close(_)
                    ));
                });
        });

        let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
            &["HTTPS://App.Example:443"],
            false,
            &["zirk.v2", "zirk.v1"],
        )
        .unwrap();
        let outcome = Executor::new().run_with_root(move || {
            let listener =
                block_on_current(
                    async move { tokio::net::TcpListener::from_std(listener).unwrap() },
                )
                .unwrap();
            for _ in 0..2 {
                let (stream, _) = block_on_current(listener.accept()).unwrap().unwrap();
                let result = serve_http1_websocket_once(
                    stream,
                    "/socket",
                    policy.clone(),
                    8,
                    |mut websocket| {
                        while let Some(message) = websocket.receive(Duration::from_secs(2))? {
                            match message {
                                HttpWebSocketMessage::Text(value) => {
                                    websocket.send(HttpWebSocketMessage::Text(value))?;
                                }
                                HttpWebSocketMessage::Close { .. } => break,
                                _ => {}
                            }
                        }
                        Ok(())
                    },
                );
                result.expect("HTTP/1.1 WebSocket server loop should complete cleanly");
            }
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        peer.join().unwrap();
    }

    #[test]
    fn websocket_server_rejects_fragmented_messages_over_the_configured_limit() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::{
            Message,
            client::IntoClientRequest,
            protocol::frame::{
                Frame,
                coding::{Data, OpCode},
            },
        };

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let endpoint = format!("ws://{address}/socket");
                    let mut request = endpoint.as_str().into_client_request().unwrap();
                    request.headers_mut().insert(
                        http::header::ORIGIN,
                        http::HeaderValue::from_static("https://app.example"),
                    );
                    let (mut websocket, _) =
                        tokio_tungstenite::connect_async(request).await.unwrap();
                    websocket
                        .send(Message::Frame(Frame::message(
                            Bytes::from(vec![b'x'; HTTP_WEBSOCKET_MESSAGE_LIMIT]),
                            OpCode::Data(Data::Binary),
                            false,
                        )))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Frame(Frame::message(
                            Bytes::from_static(b"y"),
                            OpCode::Data(Data::Continue),
                            true,
                        )))
                        .await
                        .unwrap();
                    assert!(matches!(
                        tokio::time::timeout(Duration::from_secs(10), websocket.next())
                            .await
                            .unwrap(),
                        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
                    ));
                });
        });

        let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
            &["https://app.example"],
            false,
            &[],
        )
        .unwrap();
        let outcome = Executor::new().run_with_root(move || {
            let listener =
                block_on_current(
                    async move { tokio::net::TcpListener::from_std(listener).unwrap() },
                )
                .unwrap();
            let (stream, _) = block_on_current(listener.accept()).unwrap().unwrap();
            serve_http1_websocket_once(stream, "/socket", policy, 8, |mut websocket| {
                assert!(matches!(
                    websocket.receive(Duration::from_secs(10)),
                    Err(HttpRuntimeError::BodyTooLarge)
                ));
                Ok(())
            })
            .expect("bounded WebSocket server should finish after rejecting oversized input");
            0
        });
        peer.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn raw_websocket_upgrade_guard_requires_http11_get_and_connection_token() {
        let valid = b"GET /socket HTTP/1.1\r\nHost: example.test\r\nConnection: keep-alive, Upgrade\r\nUpgrade: websocket\r\n\r\n";
        assert!(raw_has_websocket_upgrade(valid));
        assert!(!raw_has_websocket_upgrade(
            b"GET /socket HTTP/1.1\r\nHost: example.test\r\nUpgrade: websocket\r\n\r\n"
        ));
        assert!(!raw_has_websocket_upgrade(
            b"POST /socket HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n"
        ));
        assert!(!raw_has_websocket_upgrade(
            b"GET /socket HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nUpgrade: websocket\r\n\r\n"
        ));
    }

    #[test]
    fn websocket_client_checks_origin_grant_before_opening_a_socket() {
        let endpoint = "http://127.0.0.1:9/socket";
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                std::iter::empty::<String>(),
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(endpoint).unwrap();
            assert!(matches!(
                HttpWebSocket::connect(&url, grant, HttpTimeouts::default(), None),
                Err(HttpRuntimeError::PermissionDenied)
            ));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http2_websocket_extended_connect_requires_every_rfc8441_component() {
        fn request(
            method: http::Method,
            version: http::Version,
            path: &str,
            protocol: Option<&'static str>,
        ) -> Request<()> {
            let mut request = Request::builder()
                .method(method)
                .version(version)
                .uri(path)
                .body(())
                .unwrap();
            if let Some(protocol) = protocol {
                request
                    .extensions_mut()
                    .insert(hyper::ext::Protocol::from_static(protocol));
            }
            request
        }

        assert!(is_http2_websocket_extended_connect(
            &request(
                http::Method::CONNECT,
                http::Version::HTTP_2,
                "/socket",
                Some("websocket"),
            ),
            "/socket"
        ));
        assert!(!is_http2_websocket_extended_connect(
            &request(
                http::Method::CONNECT,
                http::Version::HTTP_2,
                "/socket",
                None,
            ),
            "/socket"
        ));
        assert!(!is_http2_websocket_extended_connect(
            &request(
                http::Method::CONNECT,
                http::Version::HTTP_2,
                "/socket",
                Some("other"),
            ),
            "/socket"
        ));
        assert!(!is_http2_websocket_extended_connect(
            &request(
                http::Method::GET,
                http::Version::HTTP_2,
                "/socket",
                Some("websocket"),
            ),
            "/socket"
        ));
        assert!(!is_http2_websocket_extended_connect(
            &request(
                http::Method::CONNECT,
                http::Version::HTTP_11,
                "/socket",
                Some("websocket"),
            ),
            "/socket"
        ));
        assert!(!is_http2_websocket_extended_connect(
            &request(
                http::Method::CONNECT,
                http::Version::HTTP_2,
                "/other",
                Some("websocket"),
            ),
            "/socket"
        ));
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn websocket_handshake_sends_origin_and_validates_selected_subprotocol() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut websocket = tokio_tungstenite::accept_hdr_async(
                        socket,
                        |request: &http::Request<()>, mut response: http::Response<()>| {
                            assert_eq!(
                                request.headers().get(http::header::ORIGIN).unwrap(),
                                "https://app.example"
                            );
                            assert_eq!(
                                request
                                    .headers()
                                    .get(http::header::SEC_WEBSOCKET_PROTOCOL)
                                    .unwrap(),
                                "zirk.v1, zirk.v2"
                            );
                            response.headers_mut().insert(
                                http::header::SEC_WEBSOCKET_PROTOCOL,
                                "zirk.v2".parse().unwrap(),
                            );
                            Ok(response)
                        },
                    )
                    .await
                    .unwrap();
                    assert!(matches!(
                        websocket.next().await.unwrap().unwrap(),
                        Message::Close(_)
                    ));
                    websocket.flush().await.unwrap();
                });
        });

        let endpoint = format!("http://127.0.0.1:{}/socket", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let protocols = vec!["zirk.v1".to_owned(), "zirk.v2".to_owned()];
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&endpoint).unwrap();
            let mut websocket = HttpWebSocket::connect_with_options(
                &url,
                grant,
                HttpTimeouts::default(),
                None,
                Some("HTTPS://App.Example:443"),
                &protocols,
            )
            .unwrap();
            assert_eq!(websocket.negotiated_subprotocol(), Some("zirk.v2"));
            websocket.close().unwrap();
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tls_http2_websocket_client_obeys_extended_connect_and_close_lifecycle() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let server_config = crate::network_transport::TlsServerConfig::from_der(
            vec![include_bytes!("../testdata/tls-loopback-cert.der").to_vec()],
            include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
            false,
        )
        .unwrap()
        .with_alpn_protocols(vec![b"h2".to_vec()]);
        let client_config = crate::network_transport::loopback_test_client_tls_config();
        let received = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&received);

        let outcome = Executor::new().run_with_root(move || {
            let server_grant = Arc::clone(&grant);
            let server_received = Arc::clone(&observed);
            let server = spawn(move || {
                let listener = crate::network_transport::TlsListener::bind(
                    address,
                    &server_grant,
                    server_config,
                )
                .unwrap();
                let (stream, _) = listener.accept().unwrap();
                let stream: HttpServerIo = stream.into();
                assert!(stream.negotiated_http2());
                let policy = zirk_runtime_core::http_websocket::WebSocketAcceptPolicy::new(
                    &["https://app.example"],
                    false,
                    &["zirk.v2"],
                )
                .unwrap();
                serve_http2_websocket_once(stream, "/socket", policy, 8, move |mut websocket| {
                    while let Some(message) = websocket.receive(Duration::from_secs(3))? {
                        server_received.fetch_add(1, Ordering::SeqCst);
                        match message {
                            HttpWebSocketMessage::Text(text) => {
                                websocket.send(HttpWebSocketMessage::Binary(text.into_bytes()))?;
                            }
                            HttpWebSocketMessage::Close { .. } => break,
                            HttpWebSocketMessage::Ping(_) | HttpWebSocketMessage::Pong(_) => {}
                            HttpWebSocketMessage::Binary(_) => {
                                return Err(HttpRuntimeError::Protocol);
                            }
                        }
                    }
                    Ok(())
                })
                .unwrap();
                1
            });

            let client_origin = origin.clone();
            let client_grant = Arc::clone(&grant);
            let client_config = Arc::clone(&client_config);
            let client = spawn(move || {
                let url = Url::parse(&format!("{client_origin}/socket")).unwrap();
                let mut websocket = HttpWebSocket::connect_with_options(
                    &url,
                    client_grant,
                    HttpTimeouts {
                        connect: Duration::from_secs(3),
                        response_headers: Duration::from_secs(3),
                        response_idle: Duration::from_secs(3),
                    },
                    Some(client_config),
                    Some("https://app.example"),
                    &["zirk.v2".to_owned()],
                )
                .unwrap();
                assert_eq!(websocket.negotiated_subprotocol(), Some("zirk.v2"));
                websocket
                    .send(HttpWebSocketMessage::Text("hello-h2".to_owned()))
                    .unwrap();
                assert_eq!(
                    websocket.receive(Duration::from_secs(3)).unwrap(),
                    Some(HttpWebSocketMessage::Binary(b"hello-h2".to_vec()))
                );
                websocket
                    .send(HttpWebSocketMessage::Ping(b"probe".to_vec()))
                    .unwrap();
                assert_eq!(
                    websocket.receive(Duration::from_secs(3)).unwrap(),
                    Some(HttpWebSocketMessage::Pong(b"probe".to_vec()))
                );
                websocket.close().unwrap();
                assert!(websocket.h2_driver.is_none());
                1
            });

            assert_eq!(await_task(client), 1);
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(received.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn tls_http2_websocket_client_requires_extended_connect_setting() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
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
                .unwrap(),
        );
        let mut server_config = server_config;
        Arc::get_mut(&mut server_config).unwrap().alpn_protocols = vec![b"h2".to_vec()];
        let client_config = crate::network_transport::loopback_test_client_tls_config();
        let request_seen = Arc::new(AtomicBool::new(false));
        let observed_request = Arc::clone(&request_seen);
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let (socket, _) = listener.accept().await.unwrap();
                let tls = tokio_rustls::TlsAcceptor::from(server_config)
                    .accept(socket)
                    .await
                    .unwrap();
                let service =
                    hyper::service::service_fn(move |_request: Request<hyper::body::Incoming>| {
                        observed_request.store(true, Ordering::Release);
                        async {
                            Ok::<_, std::convert::Infallible>(Response::new(
                                Full::new(Bytes::new()),
                            ))
                        }
                    });
                let connection =
                    hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(TokioIo::new(tls), service);
                let _ = tokio::time::timeout(Duration::from_secs(3), connection).await;
            });
        });

        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&format!("{origin}/socket")).unwrap();
            let result = HttpWebSocket::connect_with_options(
                &url,
                Arc::clone(&grant),
                HttpTimeouts {
                    connect: Duration::from_secs(3),
                    response_headers: Duration::from_millis(100),
                    response_idle: Duration::from_secs(3),
                },
                Some(client_config),
                Some("https://app.example"),
                &["zirk.v2".to_owned()],
            );
            assert!(matches!(result, Err(HttpRuntimeError::Timeout)));
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(!request_seen.load(Ordering::Acquire));
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn websocket_handshake_rejects_invalid_options_and_unoffered_protocols() {
        use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;

        let denied = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                std::iter::empty::<String>(),
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse("http://127.0.0.1:9/socket").unwrap();
            assert!(matches!(
                HttpWebSocket::connect_with_options(
                    &url,
                    Arc::clone(&denied),
                    HttpTimeouts::default(),
                    None,
                    Some("https://app.example\r\nX-Injected: true"),
                    &[],
                ),
                Err(HttpRuntimeError::InvalidRequest)
            ));
            let invalid_protocol = vec!["zirk.v1, evil".to_owned()];
            assert!(matches!(
                HttpWebSocket::connect_with_options(
                    &url,
                    denied,
                    HttpTimeouts::default(),
                    None,
                    None,
                    &invalid_protocol,
                ),
                Err(HttpRuntimeError::InvalidRequest)
            ));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let result = tokio_tungstenite::accept_hdr_async(
                        socket,
                        |_request: &http::Request<()>, mut response: http::Response<()>| {
                            response.headers_mut().insert(
                                http::header::SEC_WEBSOCKET_PROTOCOL,
                                "unoffered.v9".parse().unwrap(),
                            );
                            Ok::<_, ErrorResponse>(response)
                        },
                    )
                    .await;
                    assert!(result.is_ok());
                });
        });
        let endpoint = format!("http://127.0.0.1:{}/socket", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let protocols = vec!["zirk.v1".to_owned()];
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&endpoint).unwrap();
            assert!(matches!(
                HttpWebSocket::connect_with_options(
                    &url,
                    grant,
                    HttpTimeouts::default(),
                    None,
                    None,
                    &protocols,
                ),
                Err(HttpRuntimeError::Protocol)
            ));
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn websocket_close_codes_follow_rfc_allowed_ranges() {
        use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

        for code in [
            1000, 1001, 1002, 1003, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 3000, 4999,
        ] {
            assert!(
                is_valid_websocket_close_code(CloseCode::from(code)),
                "code {code} should be allowed"
            );
        }
        for code in [
            0,
            999,
            1004,
            1005,
            1006,
            1015,
            1016,
            2000,
            2999,
            5000,
            u16::MAX,
        ] {
            assert!(
                !is_valid_websocket_close_code(CloseCode::from(code)),
                "code {code} should be rejected"
            );
        }
    }

    #[test]
    fn websocket_client_flushes_reply_to_peer_initiated_close() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
                    websocket.send(Message::Close(None)).await.unwrap();
                    assert!(matches!(
                        websocket.next().await.unwrap().unwrap(),
                        Message::Close(_)
                    ));
                });
        });

        let endpoint = format!("http://127.0.0.1:{}/socket", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&endpoint).unwrap();
            let mut websocket =
                HttpWebSocket::connect(&url, grant, HttpTimeouts::default(), None).unwrap();
            assert_eq!(
                websocket.receive(Duration::from_secs(2)).unwrap(),
                Some(HttpWebSocketMessage::Close {
                    code: 1005,
                    reason: String::new(),
                })
            );
            assert!(websocket.receive(Duration::from_secs(2)).unwrap().is_none());
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn secret_header_sink_rejects_plaintext_and_header_collisions() {
        let plain = crate::http_api::prepare_secret_header_request(
            "GET",
            "http://example.test/",
            Vec::new(),
            &0u32.to_le_bytes(),
            "Authorization",
            "Bearer secret".to_owned(),
        );
        assert!(plain.is_err());

        let name = b"authorization";
        let value = b"Basic ordinary";
        let mut existing = Vec::new();
        existing.extend_from_slice(&1u32.to_le_bytes());
        existing.extend_from_slice(&(name.len() as u16).to_le_bytes());
        existing.extend_from_slice(&(value.len() as u32).to_le_bytes());
        existing.extend_from_slice(name);
        existing.extend_from_slice(value);
        let duplicate = crate::http_api::prepare_secret_header_request(
            "GET",
            "https://example.test/",
            Vec::new(),
            &existing,
            "Authorization",
            "Bearer secret".to_owned(),
        );
        assert!(duplicate.is_err());

        let request = crate::http_api::prepare_secret_header_request(
            "GET",
            "https://example.test/",
            Vec::new(),
            &0u32.to_le_bytes(),
            "X-Api-Key",
            "private-api-token".to_owned(),
        )
        .unwrap();
        assert!(request.headers.get("x-api-key").unwrap().is_sensitive());
        let debug = format!("{request:?}");
        assert!(!debug.contains("private-api-token"));
    }

    #[test]
    fn cancelling_a_task_reading_an_http_body_closes_its_connection() {
        use crate::executor::{cancel_current, yield_now};
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let headers_sent = Arc::new(AtomicBool::new(false));
        let server_headers_sent = Arc::clone(&headers_sent);
        let read_started = Arc::new(AtomicBool::new(false));
        let client_read_started = Arc::clone(&read_started);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() <= 16 * 1024);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\r\n")
                .unwrap();
            server_headers_sent.store(true, Ordering::Release);
            // Keep the response body open until the canceled client drops its
            // HTTP body and aborts the connection driver.
            loop {
                match stream.read(&mut byte) {
                    Ok(0) => return true,
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        return false;
                    }
                    Err(error) => panic!("server failed while waiting for client close: {error}"),
                }
            }
        });

        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let client_grant = Arc::clone(&grant);
            let waiting = spawn(move || {
                let url = Url::parse(&format!("{origin}/stream")).unwrap();
                let request = HttpRequestValues::new(HttpMethod::Get, url);
                let mut response = send_once_streaming_with_timeouts(
                    request,
                    client_grant,
                    HttpTimeouts {
                        connect: Duration::from_secs(2),
                        response_headers: Duration::from_secs(2),
                        response_idle: Duration::from_secs(10),
                    },
                )
                .unwrap();
                assert_eq!(response.status.as_u16(), 200);
                client_read_started.store(true, Ordering::Release);
                assert!(response.read_chunk(1024).unwrap().is_none());
                0
            });
            while !headers_sent.load(Ordering::Acquire) || !read_started.load(Ordering::Acquire) {
                yield_now();
            }
            // The child reaches this point only after yielding inside its
            // pending body read, so cancellation targets a suspended request.
            cancel_current(waiting);
            // Let the canceled owner resume its pending read so it can drop
            // the response body and abort the HTTP connection driver.
            for _ in 0..8 {
                yield_now();
            }
            assert!(
                server.join().unwrap(),
                "HTTP connection remained open after task cancellation"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn cancelling_a_streaming_upload_closes_its_connection() {
        use crate::executor::{cancel_current, yield_now};
        use std::io::Read;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let headers_received = Arc::new(AtomicBool::new(false));
        let server_headers_received = Arc::clone(&headers_received);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() <= 16 * 1024);
            }
            server_headers_received.store(true, Ordering::Release);
            let mut body = [0; 4096];
            loop {
                match stream.read(&mut body) {
                    Ok(0) => return true,
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        return false;
                    }
                    Err(error) => {
                        panic!("server failed while waiting for upload cancellation: {error}")
                    }
                }
            }
        });

        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let client_grant = Arc::clone(&grant);
            let producer_calls = Arc::new(AtomicUsize::new(0));
            let task_calls = Arc::clone(&producer_calls);
            let upload = spawn(move || {
                let request = HttpRequestValues::new(
                    HttpMethod::Post,
                    Url::parse(&format!("{origin}/upload")).unwrap(),
                );
                let result = send_once_streaming_with_producer(request, client_grant, move || {
                    task_calls.fetch_add(1, Ordering::AcqRel);
                    Ok(Some(vec![b'x'; 64 * 1024]))
                });
                assert!(matches!(result, Err(HttpRuntimeError::Cancelled)));
                0
            });
            while !headers_received.load(Ordering::Acquire)
                || producer_calls.load(Ordering::Acquire) < 3
            {
                yield_now();
            }
            cancel_current(upload);
            for _ in 0..8 {
                yield_now();
            }
            assert!(
                server.join().unwrap(),
                "upload connection stayed open after cancellation"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http_sse_response_reads_events_incrementally_and_preserves_reconnect_state() {
        use std::io::{BufRead, BufReader, Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let wire_body = b": heartbeat\r\nid: cursor-9\r\ndata: alpha\r\ndata: beta\r\nretry: 75\r\n\r\nevent: complete\ndata: done\n\n";
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /events HTTP/1.1\r\n"));
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                wire_body.len()
            )
            .unwrap();
            for fragment in wire_body.chunks(5) {
                stream.write_all(fragment).unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut tail = Vec::new();
            let _ = reader.read_to_end(&mut tail);
        });
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let request = HttpRequestValues::new(
                HttpMethod::Get,
                Url::parse(&format!("{origin}/events")).unwrap(),
            );
            let response = send_once_streaming_with_timeouts(
                request,
                grant,
                HttpTimeouts {
                    connect: Duration::from_secs(2),
                    response_headers: Duration::from_secs(2),
                    response_idle: Duration::from_secs(2),
                },
            )
            .unwrap();
            let mut events = HttpSseResponse::from_response(response).unwrap();
            let first = events.next_event().unwrap().unwrap();
            assert_eq!(first.event, "message");
            assert_eq!(first.data, "alpha\nbeta");
            assert_eq!(first.last_event_id, "cursor-9");
            assert_eq!(first.retry_ms, Some(75));
            let second = events.next_event().unwrap().unwrap();
            assert_eq!(second.event, "complete");
            assert_eq!(second.data, "done");
            assert_eq!(second.last_event_id, "cursor-9");
            assert_eq!(second.retry_ms, Some(75));
            assert_eq!(events.heartbeat_count(), 1);
            assert!(events.next_event().unwrap().is_none());
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        server.join().unwrap();
    }

    #[test]
    fn cancelling_an_sse_wait_closes_its_open_http_connection() {
        use crate::executor::{cancel_current, yield_now};
        use std::io::{BufRead, BufReader, Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let partial_sent = Arc::new(AtomicBool::new(false));
        let server_partial_sent = Arc::clone(&partial_sent);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /events HTTP/1.1\r\n"));
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n")
                .unwrap();
            stream.write_all(b"11\r\ndata: unfinished\n\r\n").unwrap();
            server_partial_sent.store(true, Ordering::Release);
            let mut byte = [0; 1];
            loop {
                match reader.read(&mut byte) {
                    Ok(0) => return true,
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        return false;
                    }
                    Err(error) => panic!("SSE peer failed while waiting for disconnect: {error}"),
                }
            }
        });
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let next_event_started = Arc::new(AtomicBool::new(false));
        let outcome = Executor::new().run_with_root(move || {
            let client_grant = Arc::clone(&grant);
            let started = Arc::clone(&next_event_started);
            let waiting = spawn(move || {
                let request = HttpRequestValues::new(
                    HttpMethod::Get,
                    Url::parse(&format!("{origin}/events")).unwrap(),
                );
                let response = send_once_streaming_with_timeouts(
                    request,
                    client_grant,
                    HttpTimeouts {
                        connect: Duration::from_secs(2),
                        response_headers: Duration::from_secs(2),
                        response_idle: Duration::from_secs(10),
                    },
                )
                .unwrap();
                let mut events = HttpSseResponse::from_response(response).unwrap();
                started.store(true, Ordering::Release);
                let _ = events.next_event();
                0
            });
            while !partial_sent.load(Ordering::Acquire)
                || !next_event_started.load(Ordering::Acquire)
            {
                yield_now();
            }
            // Run the reader until it has consumed the incomplete event frame
            // and parks for the blank-line terminator.
            for _ in 0..16 {
                yield_now();
            }
            cancel_current(waiting);
            for _ in 0..8 {
                yield_now();
            }
            assert!(
                server.join().unwrap(),
                "canceled SSE request left its connection open"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn streaming_reads_can_exceed_the_buffered_body_limit() {
        let outcome = Executor::new().run_with_root(move || {
            let response = HttpStreamingResponse::from_buffered(
                HttpStatusCode::new(200).unwrap(),
                Headers::new(),
                vec![b'x'; MAX_HTTP_BUFFERED_BODY_BYTES + 1],
                crate::executor::current_task(),
            );
            let mut stream = response;
            let mut received = 0;
            while let Some(chunk) = stream.read_chunk(64 * 1024).unwrap() {
                assert!(chunk.len() <= 64 * 1024);
                received += chunk.len();
            }
            assert_eq!(received, MAX_HTTP_BUFFERED_BODY_BYTES + 1);
            let mut buffered = HttpStreamingResponse::from_buffered(
                HttpStatusCode::new(200).unwrap(),
                Headers::new(),
                vec![b'y'; MAX_HTTP_BUFFERED_BODY_BYTES + 1],
                crate::executor::current_task(),
            );
            assert!(matches!(
                buffered.read_all(),
                Err(HttpRuntimeError::BodyTooLarge)
            ));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn gzip_response_decoding_is_incremental_and_rewrites_representation_headers() {
        let compressed = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xcb, 0x48, 0xcd, 0xc9,
            0xc9, 0x57, 0x48, 0xce, 0xcf, 0x2d, 0x28, 0x4a, 0x2d, 0x2e, 0x4e, 0x4d, 0x51, 0x28,
            0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00, 0xa1, 0x2d, 0x94, 0x53, 0x16, 0x00, 0x00, 0x00,
        ];
        let outcome = Executor::new().run_with_root(move || {
            let mut headers = Headers::new();
            headers.insert("content-encoding", b"gzip").unwrap();
            headers.insert("content-length", b"42").unwrap();
            let (headers, body, worker) = block_on_current(async {
                decode_response_body(
                    headers,
                    HttpStreamingBody::Buffered {
                        bytes: Bytes::copy_from_slice(&compressed),
                        offset: 0,
                    },
                    &AcceptEncoding::parse(&Headers::new()).unwrap(),
                )
            })
            .unwrap()
            .unwrap();
            assert!(headers.get("content-encoding").is_none());
            assert!(headers.get("content-length").is_none());
            let HttpStreamingBody::Chunks(mut chunks) = body else {
                panic!("compressed response must stream decoded chunks");
            };
            let decoded = block_on_current(async move {
                let mut output = Vec::new();
                while let Some(chunk) = chunks.recv().await {
                    output.extend_from_slice(&chunk.unwrap());
                }
                output
            })
            .unwrap();
            drop(worker);
            assert_eq!(decoded, b"hello compressed world");
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn gzip_decompression_bomb_stops_at_the_expansion_ratio_limit() {
        let outcome = Executor::new().run_with_root(|| {
            let compressed = block_on_current(encode_response_body(
                Arc::new(vec![b'a'; 2 * 1024 * 1024]),
                ContentCoding::Gzip,
            ))
            .unwrap()
            .unwrap();
            assert!(
                (2 * 1024 * 1024) / compressed.len() > HTTP_MAX_DECOMPRESSION_RATIO,
                "fixture must exceed the configured expansion ratio"
            );

            let mut headers = Headers::new();
            headers.insert("content-encoding", b"gzip").unwrap();
            let (_headers, body, worker) = block_on_current(async {
                decode_response_body(
                    headers,
                    HttpStreamingBody::Buffered {
                        bytes: Bytes::from(compressed),
                        offset: 0,
                    },
                    &AcceptEncoding::parse(&Headers::new()).unwrap(),
                )
            })
            .unwrap()
            .unwrap();
            let HttpStreamingBody::Chunks(mut chunks) = body else {
                panic!("compressed response must stream decoded chunks");
            };
            let result = block_on_current(async move {
                while let Some(chunk) = chunks.recv().await {
                    chunk?;
                }
                Ok::<(), HttpRuntimeError>(())
            })
            .unwrap();
            drop(worker);
            assert_eq!(result, Err(HttpRuntimeError::BodyTooLarge));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn truncated_gzip_response_is_a_typed_protocol_error() {
        let compressed = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xcb, 0x48, 0xcd, 0xc9,
            0xc9, 0x57, 0x48, 0xce, 0xcf, 0x2d, 0x28, 0x4a, 0x2d, 0x2e, 0x4e, 0x4d, 0x51, 0x28,
            0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00, 0xa1, 0x2d, 0x94, 0x53, 0x16,
        ];
        let outcome = Executor::new().run_with_root(move || {
            let mut headers = Headers::new();
            headers.insert("content-encoding", b"gzip").unwrap();
            let (_headers, body, worker) = block_on_current(async {
                decode_response_body(
                    headers,
                    HttpStreamingBody::Buffered {
                        bytes: Bytes::copy_from_slice(&compressed),
                        offset: 0,
                    },
                    &AcceptEncoding::parse(&Headers::new()).unwrap(),
                )
            })
            .unwrap()
            .unwrap();
            let HttpStreamingBody::Chunks(mut chunks) = body else {
                panic!("compressed response must stream decoded chunks");
            };
            let result = block_on_current(async move {
                while let Some(chunk) = chunks.recv().await {
                    chunk?;
                }
                Ok::<(), HttpRuntimeError>(())
            })
            .unwrap();
            drop(worker);
            assert_eq!(result, Err(HttpRuntimeError::Protocol));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn synchronous_decoder_polling_stops_at_its_cpu_budget() {
        struct CpuBurner;

        impl AsyncRead for CpuBurner {
            fn poll_read(
                self: Pin<&mut Self>,
                _context: &mut Context<'_>,
                _output: &mut ReadBuf<'_>,
            ) -> Poll<io::Result<()>> {
                let deadline = std::time::Instant::now() + Duration::from_millis(10);
                while std::time::Instant::now() < deadline {
                    std::hint::spin_loop();
                }
                Poll::Ready(Ok(()))
            }
        }

        let outcome = Executor::new().run_with_root(|| {
            let mut reader: Pin<Box<dyn AsyncRead + Send>> = Box::pin(CpuBurner);
            let mut bytes = [0u8; 8];
            let mut cpu_time = Duration::ZERO;
            let result = block_on_current(read_with_cpu_budget(
                &mut reader,
                &mut bytes,
                &mut cpu_time,
                Duration::from_millis(1),
            ))
            .unwrap();
            assert_eq!(
                map_body_reader_error(&result.unwrap_err()),
                HttpRuntimeError::Timeout
            );
            assert!(cpu_time >= Duration::from_millis(1));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn server_decodes_bounded_gzip_request_content_before_dispatch() {
        let compressed = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xcb, 0x48, 0xcd, 0xc9,
            0xc9, 0x57, 0x48, 0xce, 0xcf, 0x2d, 0x28, 0x4a, 0x2d, 0x2e, 0x4e, 0x4d, 0x51, 0x28,
            0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00, 0xa1, 0x2d, 0x94, 0x53, 0x16, 0x00, 0x00, 0x00,
        ];
        let outcome = Executor::new().run_with_root(move || {
            let mut request = HttpRequestValues::new(
                HttpMethod::Post,
                Url::parse("http://127.0.0.1/upload").unwrap(),
            );
            request.headers.insert("content-encoding", b"gzip").unwrap();
            request.headers.insert("content-length", b"42").unwrap();
            request.body = compressed.to_vec();
            block_on_current(decode_request_content(&mut request))
                .unwrap()
                .unwrap();
            assert_eq!(request.body, b"hello compressed world");
            assert!(request.headers.get("content-encoding").is_none());
            assert!(request.headers.get("content-length").is_none());
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn server_negotiates_gzip_and_preserves_vary_metadata() {
        let outcome = Executor::new().run_with_root(|| {
            let mut request_headers = Headers::new();
            request_headers
                .insert("accept-encoding", b"gzip, identity;q=0")
                .unwrap();
            let accepted = AcceptEncoding::parse(&request_headers).unwrap();
            let mut response_headers = Headers::new();
            response_headers.insert("content-length", b"4096").unwrap();
            response_headers.insert("etag", b"\"stable\"").unwrap();
            response_headers.insert("vary", b"Origin").unwrap();
            let response = HttpResponseValues {
                status: HttpStatusCode::new(200).unwrap(),
                headers: response_headers,
                body: vec![b'a'; 4096],
                body_stream: None,
                body_producer: None,
            };
            let result = block_on_current(async move {
                let response = prepare_server_response(response, &accepted).await?;
                let encoded_headers = response.headers.clone();
                assert_eq!(
                    response.headers.get("content-encoding").unwrap().as_bytes(),
                    b"gzip"
                );
                assert!(response.headers.get("content-length").is_none());
                assert!(response.headers.get("etag").is_none());
                let vary = response
                    .headers
                    .get_all("vary")
                    .unwrap()
                    .map(|value| value.to_str().unwrap().to_ascii_lowercase())
                    .collect::<Vec<_>>();
                assert!(vary.iter().any(|value| value.contains("origin")));
                assert!(vary.iter().any(|value| value.contains("accept-encoding")));
                let (headers, body, worker) = decode_response_body(
                    response.headers,
                    HttpStreamingBody::Buffered {
                        bytes: Bytes::from(response.body),
                        offset: 0,
                    },
                    &accepted,
                )?;
                assert!(headers.get("content-encoding").is_none());
                assert!(headers.get("content-length").is_none());
                let HttpStreamingBody::Chunks(mut chunks) = body else {
                    return Err(HttpRuntimeError::Protocol);
                };
                let mut decoded = Vec::new();
                while let Some(chunk) = chunks.recv().await {
                    decoded.extend_from_slice(&chunk?);
                }
                drop(worker);
                Ok::<_, HttpRuntimeError>((encoded_headers, decoded))
            })
            .unwrap()
            .unwrap();
            assert_eq!(result.1, vec![b'a'; 4096]);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn server_rejects_identity_only_stream_when_identity_is_forbidden() {
        let outcome = Executor::new().run_with_root(|| {
            let mut request_headers = Headers::new();
            request_headers
                .insert(
                    "accept-encoding",
                    b"br;q=0, gzip;q=0, deflate;q=0, identity;q=0",
                )
                .unwrap();
            let accepted = AcceptEncoding::parse(&request_headers).unwrap();
            let (_sender, receiver) = tokio::sync::mpsc::channel(1);
            let response = HttpResponseValues {
                status: HttpStatusCode::new(200).unwrap(),
                headers: Headers::new(),
                body: Vec::new(),
                body_stream: Some(receiver),
                body_producer: None,
            };
            let prepared =
                block_on_current(async { prepare_server_response(response, &accepted).await })
                    .unwrap()
                    .unwrap();
            assert_eq!(prepared.status.as_u16(), 406);
            assert!(prepared.body.is_empty());
            assert!(prepared.body_stream.is_none());
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn server_preencoded_response_is_negotiated_and_varies_by_encoding() {
        let outcome = Executor::new().run_with_root(|| {
            let mut request_headers = Headers::new();
            request_headers
                .insert("accept-encoding", b"gzip, identity;q=0")
                .unwrap();
            let accepted = AcceptEncoding::parse(&request_headers).unwrap();
            let mut headers = Headers::new();
            headers.insert("content-encoding", b"gzip").unwrap();
            headers.insert("vary", b"Origin").unwrap();
            let response = HttpResponseValues {
                status: HttpStatusCode::new(200).unwrap(),
                headers,
                body: vec![0x1f, 0x8b],
                body_stream: None,
                body_producer: None,
            };
            let prepared =
                block_on_current(async { prepare_server_response(response, &accepted).await })
                    .unwrap()
                    .unwrap();
            let vary = prepared
                .headers
                .get_all("vary")
                .unwrap()
                .map(|value| value.to_str().unwrap().to_ascii_lowercase())
                .collect::<Vec<_>>();
            assert!(vary.iter().any(|value| value.contains("origin")));
            assert!(vary.iter().any(|value| value.contains("accept-encoding")));

            let mut nested_headers = Headers::new();
            nested_headers
                .insert("content-encoding", b"gzip, br")
                .unwrap();
            let nested = HttpResponseValues {
                status: HttpStatusCode::new(200).unwrap(),
                headers: nested_headers,
                body: vec![0x1f, 0x8b],
                body_stream: None,
                body_producer: None,
            };
            let rejected =
                block_on_current(async { prepare_server_response(nested, &accepted).await })
                    .unwrap()
                    .unwrap();
            assert_eq!(rejected.status.as_u16(), 500);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn server_compresses_stream_channels_with_backpressure() {
        let outcome = Executor::new().run_with_root(|| {
            let mut request_headers = Headers::new();
            request_headers
                .insert("accept-encoding", b"gzip, identity;q=0")
                .unwrap();
            let accepted = AcceptEncoding::parse(&request_headers).unwrap();
            let (source_tx, source_rx) = tokio::sync::mpsc::channel(1);
            let response = HttpResponseValues {
                status: HttpStatusCode::new(200).unwrap(),
                headers: Headers::new(),
                body: Vec::new(),
                body_stream: Some(source_rx),
                body_producer: None,
            };
            block_on_current(async move {
                let mut response = prepare_server_response(response, &accepted).await.unwrap();
                assert_eq!(
                    response.headers.get("content-encoding").unwrap().as_bytes(),
                    b"gzip"
                );
                assert!(response.headers.get("vary").is_some());
                let encoded = response.body_stream.take().unwrap();
                let source = tokio::spawn(async move {
                    for chunk in (0..256 * 1024)
                        .map(|index| (index % 251) as u8)
                        .collect::<Vec<_>>()
                        .chunks(4096)
                    {
                        source_tx
                            .send(Ok(Bytes::copy_from_slice(chunk)))
                            .await
                            .unwrap();
                    }
                });
                let input = HttpResponseChannelReader {
                    receiver: encoded,
                    pending: Bytes::new(),
                };
                let mut decoder =
                    GzipDecoder::new(tokio::io::BufReader::with_capacity(8192, input));
                let mut decoded = Vec::new();
                let mut chunk = vec![0; 8192];
                loop {
                    let count = decoder.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    decoded.extend_from_slice(&chunk[..count]);
                }
                source.await.unwrap();
                assert_eq!(
                    decoded,
                    (0..256 * 1024)
                        .map(|index| (index % 251) as u8)
                        .collect::<Vec<_>>()
                );
            })
            .unwrap();
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http1_server_emits_the_negotiated_compressed_representation() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /gzip HTTP/1.1\r\nHost: localhost\r\nAccept-Encoding: gzip, identity;q=0\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            let headers_end = response
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("HTTP response includes a complete header block")
                + 4;
            String::from_utf8(response[..headers_end].to_vec()).unwrap()
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                serve_once_with_timeouts(
                    HttpServerIo::from(accepted),
                    false,
                    HttpTimeouts::default(),
                    |_| {
                        let mut headers = Headers::new();
                        headers.insert("content-length", b"4096").unwrap();
                        HttpResponseValues {
                            status: HttpStatusCode::new(200).unwrap(),
                            headers,
                            body: vec![b'a'; 4096],
                            body_stream: None,
                            body_producer: None,
                        }
                    },
                )
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let response = client.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("content-encoding: gzip\r\n"));
        assert!(response.contains("vary: Accept-Encoding\r\n"));
        assert!(!response.contains("content-length: 4096\r\n"));
    }

    #[test]
    fn https_alpn_http2_secret_header_sink_sends_only_to_the_approved_tls_origin() {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let server_config = crate::network_transport::TlsServerConfig::from_der(
            vec![include_bytes!("../testdata/tls-loopback-cert.der").to_vec()],
            include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
            false,
        )
        .unwrap()
        .with_alpn_protocols(vec![b"h2".to_vec()]);
        let client_config = crate::network_transport::loopback_test_client_tls_config();
        let served = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let response_body = vec![b'h'; 192 * 1024];

        let outcome = Executor::new().run_with_root(move || {
            let server_grant = Arc::clone(&grant);
            let server_served = Arc::clone(&served);
            let server = spawn(move || {
                let listener = crate::network_transport::TlsListener::bind(
                    address,
                    &server_grant,
                    server_config,
                )
                .unwrap();
                let (stream, _) = listener.accept().unwrap();
                let served_request = Arc::clone(&server_served);
                let service =
                    hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
                        let served_request = Arc::clone(&served_request);
                        async move {
                            assert_eq!(request.version(), hyper::Version::HTTP_2);
                            assert_eq!(request.method(), hyper::Method::GET);
                            assert_eq!(
                                request
                                    .headers()
                                    .get("authorization")
                                    .and_then(|value| value.to_str().ok()),
                                Some("Bearer fixture-secret")
                            );
                            served_request.store(true, std::sync::atomic::Ordering::Release);
                            Ok::<_, std::convert::Infallible>(Response::new(Full::new(
                                Bytes::from(vec![b'h'; 192 * 1024]),
                            )))
                        }
                    });
                let connection =
                    hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(TokioIo::new(stream.into_inner()), service);
                let _ = block_on_current(connection);
                1
            });

            let request = crate::http_api::prepare_secret_header_request(
                "GET",
                &format!("{origin}/status"),
                Vec::new(),
                &0u32.to_le_bytes(),
                "Authorization",
                "Bearer fixture-secret".to_owned(),
            )
            .unwrap();
            let client_grant = Arc::clone(&grant);
            let client_result = Arc::new(std::sync::Mutex::new(None));
            let task_result = Arc::clone(&client_result);
            let client = spawn(move || {
                let mut response = send_once_streaming_with_tls_config(
                    request,
                    client_grant,
                    HttpTimeouts {
                        connect: Duration::from_secs(2),
                        response_headers: Duration::from_secs(2),
                        response_idle: Duration::from_secs(2),
                    },
                    Some(client_config),
                )
                .unwrap();
                let mut received = Vec::new();
                let mut largest_chunk = 0;
                while let Some(chunk) = response.read_chunk(32 * 1024).unwrap() {
                    largest_chunk = largest_chunk.max(chunk.len());
                    received.extend_from_slice(&chunk);
                }
                *task_result.lock().unwrap() =
                    Some((response.status.as_u16(), received, largest_chunk));
                1
            });
            assert_eq!(await_task(client), 1);
            let (status, received, largest_chunk) = client_result.lock().unwrap().take().unwrap();
            assert_eq!(status, 200);
            assert_eq!(received, response_body);
            assert!(largest_chunk > 0 && largest_chunk <= 32 * 1024);
            assert_eq!(await_task(server), 1);
            assert!(served.load(std::sync::atomic::Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http2_client_decodes_gzip_response_chunks_from_a_prior_knowledge_peer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin.clone()], [address.to_string()])
                .unwrap(),
        );
        let served = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let outcome = Executor::new().run_with_root(move || {
            let server_served = Arc::clone(&served);
            let server_grant = Arc::clone(&grant);
            let server = spawn(move || {
                let listener =
                    crate::network_transport::TcpListener::bind(address, &server_grant).unwrap();
                let (stream, _) = listener.accept().unwrap();
                let served_request = Arc::clone(&server_served);
                let service =
                    hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
                        let served_request = Arc::clone(&served_request);
                        async move {
                            assert_eq!(request.version(), hyper::Version::HTTP_2);
                            assert_eq!(request.method(), hyper::Method::GET);
                            served_request.store(true, std::sync::atomic::Ordering::Release);
                            let response = Response::builder()
                                .header(hyper::header::CONTENT_ENCODING, "gzip")
                                .body(Full::new(Bytes::from_static(&[
                                    0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff,
                                    0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x48, 0xce, 0xcf, 0x2d,
                                    0x28, 0x4a, 0x2d, 0x2e, 0x4e, 0x4d, 0x51, 0x28, 0xcf, 0x2f,
                                    0xca, 0x49, 0x01, 0x00, 0xa1, 0x2d, 0x94, 0x53, 0x16, 0x00,
                                    0x00, 0x00,
                                ])))
                                .unwrap();
                            Ok::<_, std::convert::Infallible>(response)
                        }
                    });
                let connection =
                    hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(TokioIo::new(stream.into_inner()), service);
                let _ = block_on_current(connection);
                1
            });

            let client_grant = Arc::clone(&grant);
            let client_origin = origin.clone();
            let client = spawn(move || {
                let stream = TcpStream::connect_with_timeout(
                    &client_origin,
                    client_grant,
                    Duration::from_secs(2),
                )
                .unwrap();
                let request = Request::builder()
                    .method(hyper::Method::GET)
                    .uri(format!("{client_origin}/"))
                    .body(Full::new(Bytes::new()))
                    .unwrap();
                let response = block_on_current(start_http2_on_reactor(
                    HttpIo::Plain(stream.into_inner()),
                    request,
                    Duration::from_secs(2),
                    Duration::from_secs(2),
                ))
                .unwrap()
                .unwrap();
                assert_eq!(response.status, 200);
                let mut headers = Headers::new();
                for (name, value) in response.headers.iter() {
                    headers.append(name.as_str(), value.as_bytes()).unwrap();
                }
                assert_eq!(headers.get("content-encoding").unwrap().as_bytes(), b"gzip");
                let (headers, body, decoder) = block_on_current(async {
                    decode_response_body(
                        headers,
                        HttpStreamingBody::Chunks(response.chunks),
                        &AcceptEncoding::parse(&Headers::new()).unwrap(),
                    )
                })
                .unwrap()
                .unwrap();
                assert!(headers.get("content-encoding").is_none());
                let HttpStreamingBody::Chunks(mut chunks) = body else {
                    panic!("HTTP/2 compressed response must decode incrementally");
                };
                let body = block_on_current(async move {
                    let mut decoded = Vec::new();
                    while let Some(chunk) = chunks.recv().await {
                        decoded.extend_from_slice(&chunk.unwrap());
                    }
                    decoded
                })
                .unwrap();
                assert_eq!(body, b"hello compressed world");
                drop((response.worker, decoder));
                1
            });

            assert_eq!(await_task(client), 1);
            assert_eq!(await_task(server), 1);
            assert!(served.load(std::sync::atomic::Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn one_shot_query_round_trip_is_bounded_and_uses_exact_origin_grant() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            use std::io::{Read, Write};
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 512];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "client closed before sending the full request");
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n")
                    && request.ends_with(b"{\"q\":\"zirk\"}")
                {
                    break;
                }
                assert!(request.len() <= 4096, "request exceeded fixture bound");
            }
            let request = request.as_slice();
            assert!(request.starts_with(b"QUERY /items?q=zirk HTTP/1.1\r\n"));
            let content_type = b"content-type: application/query+json\r\n";
            assert!(
                request
                    .windows(content_type.len())
                    .any(|window| window == content_type)
            );
            assert!(request.ends_with(b"{\"q\":\"zirk\"}"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nAccept-Query: application/sql;charset=\"UTF-8\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let origin = format!("http://127.0.0.1:{}", address.port());
        let outcome = Executor::new().run_with_root(move || {
            let request_task = spawn(move || {
                let mut request = HttpRequestValues::new(
                    HttpMethod::Query,
                    Url::parse(&format!("{origin}/items?q=zirk")).unwrap(),
                );
                request
                    .headers
                    .insert("content-type", b"application/query+json")
                    .unwrap();
                request.body = b"{\"q\":\"zirk\"}".to_vec();
                let mut response = send_once_streaming(request, grant).unwrap();
                assert_eq!(response.status.as_u16(), 200);
                response.headers.validate_accept_query().unwrap();
                assert_eq!(
                    response
                        .headers
                        .get("accept-query")
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    "application/sql;charset=\"UTF-8\""
                );
                assert_eq!(
                    response.read_chunk(1).unwrap().as_deref(),
                    Some(b"o".as_slice())
                );
                assert_eq!(response.read_all(), Err(HttpRuntimeError::BodyConsumed));
                assert_eq!(
                    response.read_chunk(1).unwrap().as_deref(),
                    Some(b"k".as_slice())
                );
                assert_eq!(response.read_chunk(1).unwrap(), None);
                assert_eq!(response.read_chunk(1).unwrap(), None);
                assert_eq!(response.read_all(), Err(HttpRuntimeError::BodyConsumed));
                7
            });
            assert_eq!(crate::executor::await_task(request_task), 7);
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn query_without_content_type_and_unapproved_origin_fail_before_io() {
        let grant = Arc::new(EffectiveNetworkGrant::default());
        let mut request = HttpRequestValues::new(
            HttpMethod::Query,
            Url::parse("http://127.0.0.1:9/search").unwrap(),
        );
        request.body = b"query".to_vec();
        assert!(matches!(
            send_once(request.clone(), Arc::clone(&grant)),
            Err(HttpRuntimeError::InvalidRequest)
        ));
        request.body.clear();
        request
            .headers
            .insert("content-type", b"application/query+json")
            .unwrap();
        let outcome = Executor::new().run_with_root(move || {
            let task = spawn(move || {
                assert!(matches!(
                    send_once(request, grant),
                    Err(HttpRuntimeError::PermissionDenied)
                ));
                0
            });
            assert_eq!(crate::executor::await_task(task), 0);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn ordinary_connect_request_is_rejected_before_transport_setup() {
        let address = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes([origin], [address.to_string()]).unwrap(),
        );
        let request = HttpRequestValues::new(
            HttpMethod::Connect,
            Url::parse(&format!("http://{address}/")).unwrap(),
        );

        assert!(matches!(
            send_once(request, grant),
            Err(HttpRuntimeError::UnsupportedMethod)
        ));
    }

    #[test]
    fn reusable_http1_connection_sends_multiple_requests_on_one_socket() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let server_accepted = Arc::clone(&accepted);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            server_accepted.fetch_add(1, Ordering::SeqCst);
            for body in [b"first".as_slice(), b"second".as_slice()] {
                let mut request = Vec::new();
                let mut chunk = [0; 256];
                loop {
                    let count = stream.read(&mut chunk).unwrap();
                    assert_ne!(count, 0, "client closed before its next request");
                    request.extend_from_slice(&chunk[..count]);
                    let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                    else {
                        continue;
                    };
                    let headers = std::str::from_utf8(&request[..header_end]).unwrap();
                    if body == b"second" {
                        assert!(
                            headers.lines().any(|line| {
                                line.eq_ignore_ascii_case("cookie: session=secret")
                            }),
                            "second pooled request must carry the stored cookie"
                        );
                    }
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() < header_end + 4 + content_length {
                        continue;
                    }
                    break;
                }
                let set_cookie = if body == b"first" {
                    "Set-Cookie: session=secret; Path=/; HttpOnly; SameSite=Strict\r\n"
                } else {
                    ""
                };
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{set_cookie}\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                stream.write_all(body).unwrap();
            }
        });

        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&format!("{origin}/reuse")).unwrap();
            let timeouts = HttpTimeouts {
                connect: Duration::from_secs(2),
                response_headers: Duration::from_secs(2),
                response_idle: Duration::from_secs(2),
            };
            let mut client = HttpClientPool::new(2, timeouts).unwrap();
            client.enable_cookies();
            for expected in [b"first".as_slice(), b"second".as_slice()] {
                let response = client
                    .request(
                        HttpRequestValues::new(HttpMethod::Get, url.clone()),
                        Arc::clone(&grant),
                    )
                    .unwrap();
                assert_eq!(response.status.as_u16(), 200);
                assert_eq!(response.body, expected);
            }
            assert_eq!(client.pooled_origin_count(), 1);
            let denied = EffectiveNetworkGrant::default();
            assert!(matches!(
                client.request(
                    HttpRequestValues::new(HttpMethod::Get, url.clone()),
                    Arc::new(denied),
                ),
                Err(HttpRuntimeError::PermissionDenied)
            ));
            assert_eq!(client.pooled_origin_count(), 0);
            client.close();
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reusable_http_client_uses_explicit_authorized_http_proxy() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let destination_probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let destination_address = destination_probe.local_addr().unwrap();
        drop(destination_probe);
        let proxy_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let accepted_by_proxy = Arc::clone(&accepted);
        let proxy = std::thread::spawn(move || {
            let (mut stream, _) = proxy_listener.accept().unwrap();
            accepted_by_proxy.fetch_add(1, Ordering::SeqCst);
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") {
                assert_eq!(stream.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
                assert!(request.len() <= 64 * 1024);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with(&format!(
                "GET http://127.0.0.1:{}/through-proxy HTTP/1.1\r\n",
                destination_address.port()
            )));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("proxy-authorization: basic ")
            );
            assert!(request.contains("dXNlcjpzZWNyZXQ="));
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nproxy",
                )
                .unwrap();
        });
        let destination_origin = format!("http://127.0.0.1:{}", destination_address.port());
        let proxy_origin = format!("http://127.0.0.1:{}", proxy_address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [destination_origin.clone(), proxy_origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let proxy_config =
                HttpProxy::parse_with_credentials(&proxy_origin, Some("user:secret")).unwrap();
            let mut client =
                HttpClientPool::new_with_proxy(2, HttpTimeouts::default(), proxy_config).unwrap();
            let response = client
                .request(
                    HttpRequestValues::new(
                        HttpMethod::Get,
                        Url::parse(&format!("{destination_origin}/through-proxy")).unwrap(),
                    ),
                    Arc::clone(&grant),
                )
                .unwrap();
            assert_eq!(response.body, b"proxy");
            client.close();

            let destination_only = Arc::new(
                EffectiveNetworkGrant::from_verified_scopes(
                    [destination_origin.clone()],
                    std::iter::empty::<String>(),
                )
                .unwrap(),
            );
            let mut denied_client = HttpClientPool::new_with_proxy(
                1,
                HttpTimeouts::default(),
                HttpProxy::parse(&proxy_origin).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                denied_client.request(
                    HttpRequestValues::new(
                        HttpMethod::Get,
                        Url::parse(&format!("{destination_origin}/denied")).unwrap(),
                    ),
                    destination_only,
                ),
                Err(HttpRuntimeError::PermissionDenied)
            ));
            0
        });
        proxy.join().unwrap();
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn reusable_client_sends_remote_proxy_credentials_inside_verified_tls() {
        use std::io::{BufRead, BufReader, Write};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
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
                .unwrap(),
        );
        let proxy = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let tls = tokio_rustls::rustls::StreamOwned::new(
                tokio_rustls::rustls::ServerConnection::new(server_config).unwrap(),
                socket,
            );
            let mut reader = BufReader::new(tls);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(
                request_line,
                "GET http://example.invalid/resource HTTP/1.1\r\n"
            );
            let mut authorization_seen = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if header.eq_ignore_ascii_case("proxy-authorization: Basic dXNlcjpzZWNyZXQ=\r\n") {
                    authorization_seen = true;
                }
            }
            assert!(authorization_seen, "TLS proxy did not receive Basic auth");
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
                .unwrap();
        });

        let proxy_origin = format!("https://127.0.0.1:{}", proxy_address.port());
        let destination_origin = "http://example.invalid:80".to_owned();
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [proxy_origin.clone(), destination_origin],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let mut client = HttpClientPool::new_with_tls_config_and_proxy(
                1,
                HttpTimeouts::default(),
                crate::network_transport::client_tls_config_with_trust_roots(
                    vec![include_bytes!("../testdata/tls-loopback-root.der").to_vec()],
                    false,
                )
                .unwrap(),
                HttpProxy::parse_with_credentials(&proxy_origin, Some("user:secret")).unwrap(),
            )
            .unwrap();
            let response = client
                .request(
                    HttpRequestValues::new(
                        HttpMethod::Get,
                        Url::parse("http://example.invalid/resource").unwrap(),
                    ),
                    grant,
                )
                .unwrap();
            assert_eq!(response.body, b"OK");
            client.close();
            0
        });
        proxy.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn reusable_https_client_establishes_authenticated_tls_through_http_connect_proxy() {
        use std::io::{BufRead, BufReader, Read, Write};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let destination_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let destination_address = destination_listener.local_addr().unwrap();
        let certificate = include_bytes!("../testdata/tls-loopback-cert.der");
        let private_key = include_bytes!("../testdata/tls-loopback-key.der");
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from(certificate.to_vec())],
                    PrivateKeyDer::try_from(private_key.to_vec()).unwrap(),
                )
                .unwrap(),
        );
        let destination = std::thread::spawn(move || {
            let (socket, _) = destination_listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let tls = tokio_rustls::rustls::StreamOwned::new(
                tokio_rustls::rustls::ServerConnection::new(server_config).unwrap(),
                socket,
            );
            let mut reader = BufReader::new(tls);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(request_line, "GET /tunneled HTTP/1.1\r\n");
            let mut proxy_auth_forwarded_to_destination = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                proxy_auth_forwarded_to_destination |= header
                    .split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case("proxy-authorization"));
            }
            assert!(!proxy_auth_forwarded_to_destination);
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nTLS")
                .unwrap();
        });

        let proxy_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let proxy = std::thread::spawn(move || {
            let (mut client, _) = proxy_listener.accept().unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0; 1];
            while !request.ends_with(b"\r\n\r\n") {
                assert_eq!(client.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
                assert!(request.len() <= 64 * 1024);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with(&format!(
                "CONNECT 127.0.0.1:{} HTTP/1.1\r\n",
                destination_address.port()
            )));
            assert!(request.contains("Proxy-Authorization: Basic dXNlcjpzZWNyZXQ=\r\n"));
            let upstream = std::net::TcpStream::connect(destination_address).unwrap();
            client
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .unwrap();
            let mut client_read = client.try_clone().unwrap();
            let mut upstream_write = upstream.try_clone().unwrap();
            let forward = std::thread::spawn(move || {
                let _ = std::io::copy(&mut client_read, &mut upstream_write);
            });
            let mut upstream_read = upstream;
            let _ = std::io::copy(&mut upstream_read, &mut client);
            let _ = forward.join();
        });

        let destination_origin = format!("https://127.0.0.1:{}", destination_address.port());
        let proxy_origin = format!("http://127.0.0.1:{}", proxy_address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [destination_origin.clone(), proxy_origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let client_config = crate::network_transport::loopback_test_client_tls_config();
            let mut client = HttpClientPool::new_with_tls_config_and_proxy(
                1,
                HttpTimeouts::default(),
                client_config,
                HttpProxy::parse_with_credentials(&proxy_origin, Some("user:secret")).unwrap(),
            )
            .unwrap();
            let response = client
                .request(
                    HttpRequestValues::new(
                        HttpMethod::Get,
                        Url::parse(&format!("{destination_origin}/tunneled")).unwrap(),
                    ),
                    grant,
                )
                .unwrap();
            assert_eq!(response.body, b"TLS");
            client.close();
            0
        });
        proxy.join().unwrap();
        destination.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn reusable_https_client_tunnels_destination_tls_through_authenticated_https_proxy() {
        use std::io::{BufRead, BufReader, Write};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let destination_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let destination_address = destination_listener.local_addr().unwrap();
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
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
                .unwrap(),
        );
        let destination_config = Arc::clone(&server_config);
        let destination = std::thread::spawn(move || {
            let (socket, _) = destination_listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let tls = tokio_rustls::rustls::StreamOwned::new(
                tokio_rustls::rustls::ServerConnection::new(destination_config).unwrap(),
                socket,
            );
            let mut reader = BufReader::new(tls);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(request_line, "GET /nested-tls HTTP/1.1\r\n");
            let mut proxy_auth_forwarded = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                proxy_auth_forwarded |= header
                    .split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case("proxy-authorization"));
            }
            assert!(!proxy_auth_forwarded);
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nTLS")
                .unwrap();
        });

        let proxy_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        proxy_listener.set_nonblocking(true).unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let proxy_config = Arc::clone(&server_config);
        let proxy = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(proxy_listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    let acceptor = tokio_rustls::TlsAcceptor::from(proxy_config);
                    let mut client = acceptor.accept(socket).await.unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") {
                        assert_eq!(client.read(&mut byte).await.unwrap(), 1);
                        request.push(byte[0]);
                        assert!(request.len() <= 64 * 1024);
                    }
                    let request = String::from_utf8(request).unwrap();
                    assert!(request.starts_with(&format!(
                        "CONNECT 127.0.0.1:{} HTTP/1.1\r\n",
                        destination_address.port()
                    )));
                    assert!(request.contains("Proxy-Authorization: Basic dXNlcjpzZWNyZXQ=\r\n"));
                    client
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await
                        .unwrap();
                    let mut destination = tokio::net::TcpStream::connect(destination_address)
                        .await
                        .unwrap();
                    tokio::io::copy_bidirectional(&mut client, &mut destination)
                        .await
                        .unwrap();
                });
        });

        let destination_origin = format!("https://127.0.0.1:{}", destination_address.port());
        let proxy_origin = format!("https://127.0.0.1:{}", proxy_address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [destination_origin.clone(), proxy_origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let mut client = HttpClientPool::new_with_tls_config_and_proxy(
                1,
                HttpTimeouts::default(),
                crate::network_transport::loopback_test_client_tls_config(),
                HttpProxy::parse_with_credentials(&proxy_origin, Some("user:secret")).unwrap(),
            )
            .unwrap();
            let response = client
                .request(
                    HttpRequestValues::new(
                        HttpMethod::Get,
                        Url::parse(&format!("{destination_origin}/nested-tls")).unwrap(),
                    ),
                    grant,
                )
                .unwrap();
            assert_eq!(response.body, b"TLS");
            client.close();
            0
        });
        proxy.join().unwrap();
        destination.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http_connect_proxy_response_parser_rejects_ambiguous_or_malformed_headers() {
        use std::io::Write;

        let invalid_responses: &[&[u8]] = &[
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nMalformed-Header\r\n\r\n",
            b"HTTP/1.1 2000 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nX-Invalid: \xff\r\n\r\n",
        ];

        for response in invalid_responses {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let response = response.to_vec();
            let peer = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.write_all(&response).unwrap();
            });
            let origin = format!("http://127.0.0.1:{}", address.port());
            let outcome = Executor::new().run_with_root(move || {
                let grant = Arc::new(
                    EffectiveNetworkGrant::from_verified_scopes(
                        [origin.clone()],
                        std::iter::empty::<String>(),
                    )
                    .unwrap(),
                );
                let stream = crate::network_transport::TcpStream::connect_with_timeout(
                    &origin,
                    grant,
                    Duration::from_secs(2),
                )
                .unwrap()
                .into_inner();
                establish_http_connect_tunnel(
                    stream,
                    "example.invalid:443",
                    None,
                    Duration::from_secs(2),
                )
                .err()
                .map(|error| match error {
                    HttpRuntimeError::Protocol | HttpRuntimeError::Transport => 1,
                    _ => 0,
                })
                .unwrap_or(0)
            });
            peer.join().unwrap();
            assert!(matches!(outcome, crate::task::TaskOutcome::Value(1)));
        }
    }

    #[test]
    fn reusable_client_decodes_a_gzip_response_from_the_server() {
        use std::io::{BufRead, BufReader, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /gzip HTTP/1.1\r\n"));
            let mut accept_encoding = false;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if line.to_ascii_lowercase().starts_with("accept-encoding:") {
                    accept_encoding = line.to_ascii_lowercase().contains("gzip");
                }
            }
            assert!(accept_encoding, "client did not advertise gzip support");
            let compressed = [
                0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xcb, 0x48, 0xcd, 0xc9,
                0xc9, 0x57, 0x48, 0xce, 0xcf, 0x2d, 0x28, 0x4a, 0x2d, 0x2e, 0x4e, 0x4d, 0x51, 0x28,
                0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00, 0xa1, 0x2d, 0x94, 0x53, 0x16, 0x00, 0x00, 0x00,
            ];
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 42\r\nConnection: close\r\n\r\n")
                .unwrap();
            stream.write_all(&compressed).unwrap();
        });
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let request = HttpRequestValues::new(
                HttpMethod::Get,
                Url::parse(&format!("{origin}/gzip")).unwrap(),
            );
            let mut client = HttpClientPool::new(1, HttpTimeouts::default()).unwrap();
            let response = client.request(request, grant).unwrap();
            assert_eq!(response.body, b"hello compressed world");
            assert!(response.headers.get("content-encoding").is_none());
            assert!(response.headers.get("content-length").is_none());
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn http2_request_dispatch_allows_concurrent_streams() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
            let both_requests = Arc::new(tokio::sync::Barrier::new(2));
            let server_barrier = Arc::clone(&both_requests);
            let service =
                hyper::service::service_fn(move |_request: Request<hyper::body::Incoming>| {
                    let barrier = Arc::clone(&server_barrier);
                    async move {
                        barrier.wait().await;
                        Ok::<_, std::convert::Infallible>(Response::new(Full::new(
                            Bytes::from_static(b"concurrent"),
                        )))
                    }
                });
            let server =
                hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(TokioIo::new(server_io), service);
            let server_task = tokio::spawn(server);

            let (mut sender, connection) = hyper::client::conn::http2::handshake(
                hyper_util::rt::TokioExecutor::new(),
                TokioIo::new(client_io),
            )
            .await
            .unwrap();
            let driver = tokio::spawn(connection);
            sender.ready().await.unwrap();

            let make_command = |path: &str| {
                let request = Request::builder()
                    .method(hyper::Method::GET)
                    .uri(path)
                    .body(buffered_client_request_body(Vec::new()))
                    .unwrap();
                let (response, receiver) = tokio::sync::oneshot::channel();
                (
                    Http2RequestCommand {
                        request,
                        accepted: AcceptEncoding::parse(&Headers::new()).unwrap(),
                        timeouts: HttpTimeouts {
                            connect: Duration::from_secs(1),
                            response_headers: Duration::from_secs(2),
                            response_idle: Duration::from_secs(2),
                        },
                        response,
                    },
                    receiver,
                )
            };
            let (first, first_rx) = make_command("/first");
            let (second, second_rx) = make_command("/second");
            let mut streams = tokio::task::JoinSet::new();
            streams.spawn(run_http2_request(sender.clone(), first));
            streams.spawn(run_http2_request(sender, second));
            tokio::time::timeout(Duration::from_secs(3), async {
                while streams.join_next().await.is_some() {}
                let first = first_rx.await.unwrap().unwrap();
                let second = second_rx.await.unwrap().unwrap();
                assert_eq!(first.body, b"concurrent");
                assert_eq!(second.body, b"concurrent");
            })
            .await
            .expect("HTTP/2 streams did not overlap");
            server_task.abort();
            driver.abort();
        });
    }

    #[test]
    fn server_rejects_duplicate_http2_pseudo_headers_before_handler_dispatch() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};

        fn assert_rejected_before_dispatch(malformed_headers: &[u8]) {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let malformed_headers = malformed_headers.to_vec();
            let peer = std::thread::spawn(move || {
                let mut stream = std::net::TcpStream::connect(address).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
                    .unwrap();
                // Empty client SETTINGS frame.
                stream.write_all(b"\0\0\0\x04\0\0\0\0\0").unwrap();
                let frame_header = [0, 0, malformed_headers.len() as u8, 0x01, 0x05, 0, 0, 0, 1];
                stream.write_all(&frame_header).unwrap();
                stream.write_all(&malformed_headers).unwrap();
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let mut response = [0; 64];
                let _ = stream.read(&mut response);
            });

            let handler_called = Arc::new(AtomicBool::new(false));
            let handler_called_by_server = Arc::clone(&handler_called);
            let outcome = Executor::new().run_with_root(move || {
                let server_task = spawn(move || {
                    let accepted = block_on_current(async move {
                        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                        listener.accept().await.unwrap().0
                    })
                    .unwrap();
                    let _result =
                        serve_http2_offloaded(HttpServerIo::from(accepted), false, move |_| {
                            handler_called_by_server.store(true, Ordering::SeqCst);
                            simple_response(200, b"handler must not run")
                        });
                    assert!(!handler_called.load(Ordering::SeqCst));
                    0
                });
                assert_eq!(await_task(server_task), 0);
                0
            });
            peer.join().unwrap();
            assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        }

        // Duplicate :method fields (GET and POST) are invalid.
        assert_rejected_before_dispatch(&[
            0x82, 0x83, 0x86, 0x84, 0x01, 0x09, b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's',
            b't',
        ]);
        // Pseudo-fields after regular fields are invalid, even when the late
        // pseudo-field is otherwise a recognized :authority field.
        assert_rejected_before_dispatch(&[
            0x82, 0x86, 0x84, 0x00, 0x01, b'x', 0x01, b'y', 0x01, 0x09, b'l', b'o', b'c', b'a',
            b'l', b'h', b'o', b's', b't',
        ]);
        // Unknown pseudo-fields must also fail before application dispatch.
        assert_rejected_before_dispatch(&[
            0x82, 0x86, 0x84, 0x01, 0x09, b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's', b't',
            0x00, 0x08, b':', b'u', b'n', b'k', b'n', b'o', b'w', b'n', 0x01, b'x',
        ]);
    }

    #[test]
    fn cancelling_one_http2_request_resets_only_its_stream() {
        struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyOnDrop {
            fn drop(&mut self) {
                if let Some(notify) = self.0.take() {
                    let _ = notify.send(());
                }
            }
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
            let started_tx = Arc::new(std::sync::Mutex::new(Some(started_tx)));
            let dropped_tx = Arc::new(std::sync::Mutex::new(Some(dropped_tx)));
            let service =
                hyper::service::service_fn(move |_request: Request<hyper::body::Incoming>| {
                    let started = Arc::clone(&started_tx);
                    let dropped = Arc::clone(&dropped_tx);
                    async move {
                        let _drop = NotifyOnDrop(dropped.lock().unwrap().take());
                        if let Some(started) = started.lock().unwrap().take() {
                            let _ = started.send(());
                        }
                        std::future::pending::<()>().await;
                        Ok::<_, std::convert::Infallible>(Response::new(Full::new(Bytes::new())))
                    }
                });
            let server =
                hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(TokioIo::new(server_io), service);
            let server_task = tokio::spawn(server);
            let (sender, connection) = hyper::client::conn::http2::handshake(
                hyper_util::rt::TokioExecutor::new(),
                TokioIo::new(client_io),
            )
            .await
            .unwrap();
            let driver = tokio::spawn(connection);
            let request = Request::builder()
                .method(hyper::Method::GET)
                .uri("/cancel")
                .body(buffered_client_request_body(Vec::new()))
                .unwrap();
            let (response, receiver) = tokio::sync::oneshot::channel();
            let command = Http2RequestCommand {
                request,
                accepted: AcceptEncoding::parse(&Headers::new()).unwrap(),
                timeouts: HttpTimeouts {
                    connect: Duration::from_secs(1),
                    response_headers: Duration::from_secs(5),
                    response_idle: Duration::from_secs(5),
                },
                response,
            };
            let request_task = tokio::spawn(run_http2_request(sender, command));
            tokio::time::timeout(Duration::from_secs(1), started_rx)
                .await
                .expect("the server did not receive the stream")
                .expect("the server start notification closed");
            drop(receiver);
            tokio::time::timeout(Duration::from_secs(1), request_task)
                .await
                .expect("cancelled request worker remained pending")
                .expect("request worker panicked");
            tokio::time::timeout(Duration::from_secs(1), dropped_rx)
                .await
                .expect("server did not observe the cancelled stream reset")
                .expect("server drop notification closed");
            driver.abort();
            server_task.abort();
        });
    }

    #[test]
    fn reusable_http2_pool_reuses_one_authenticated_tls_connection() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
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
                .unwrap(),
        );
        let mut server_config = Arc::unwrap_or_clone(server_config);
        server_config.alpn_protocols = vec![b"h2".to_vec()];
        let accepted = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let server_accepted = Arc::clone(&accepted);
        let server_requests = Arc::clone(&requests);
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (socket, _) = listener.accept().await.unwrap();
                    server_accepted.fetch_add(1, Ordering::SeqCst);
                    let tls = tokio_rustls::TlsAcceptor::from(Arc::new(server_config))
                        .accept(socket)
                        .await
                        .unwrap();
                    assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));
                    let service = hyper::service::service_fn(
                        move |request: Request<hyper::body::Incoming>| {
                            let requests = Arc::clone(&server_requests);
                            async move {
                                assert_eq!(request.version(), hyper::Version::HTTP_2);
                                requests.fetch_add(1, Ordering::SeqCst);
                                Ok::<_, std::convert::Infallible>(Response::new(Full::new(
                                    Bytes::from_static(b"h2-ok"),
                                )))
                            }
                        },
                    );
                    let _ = hyper::server::conn::http2::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(TokioIo::new(tls), service)
                    .await;
                });
        });

        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&format!("{origin}/pool")).unwrap();
            let timeouts = HttpTimeouts {
                connect: Duration::from_secs(3),
                response_headers: Duration::from_secs(3),
                response_idle: Duration::from_secs(3),
            };
            let mut client = HttpClientPool::new_with_tls_config(
                2,
                timeouts,
                crate::network_transport::loopback_test_client_tls_config(),
            )
            .unwrap();
            for _ in 0..2 {
                let response = client
                    .request(
                        HttpRequestValues::new(HttpMethod::Get, url.clone()),
                        Arc::clone(&grant),
                    )
                    .unwrap();
                assert_eq!(response.status.as_u16(), 200);
                assert_eq!(response.body, b"h2-ok");
            }
            assert_eq!(client.pooled_origin_count(), 1);
            client.close();
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn reusable_http1_pool_reuses_one_authenticated_https_connection() {
        use std::io::{BufRead, BufReader, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let certificate = include_bytes!("../testdata/tls-loopback-cert.der");
        let private_key = include_bytes!("../testdata/tls-loopback-key.der");
        let server_config = Arc::new(
            tokio_rustls::rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from(certificate.to_vec())],
                    PrivateKeyDer::try_from(private_key.to_vec()).unwrap(),
                )
                .unwrap(),
        );
        let accepted = Arc::new(AtomicUsize::new(0));
        let server_accepted = Arc::clone(&accepted);
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            server_accepted.fetch_add(1, Ordering::SeqCst);
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let connection = tokio_rustls::rustls::ServerConnection::new(server_config).unwrap();
            let tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
            let mut reader = BufReader::new(tls);
            for expected in [b"first".as_slice(), b"second".as_slice()] {
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                assert!(request_line.starts_with("GET /reuse HTTP/1.1\r\n"));
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" || header.is_empty() {
                        break;
                    }
                }
                write!(
                    reader.get_mut(),
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    expected.len()
                )
                .unwrap();
                reader.get_mut().write_all(expected).unwrap();
                reader.get_mut().flush().unwrap();
            }
        });

        let origin = format!("https://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let url = Url::parse(&format!("{origin}/reuse")).unwrap();
            let timeouts = HttpTimeouts {
                connect: Duration::from_secs(3),
                response_headers: Duration::from_secs(3),
                response_idle: Duration::from_secs(3),
            };
            let mut client = HttpClientPool::new_with_tls_config(
                2,
                timeouts,
                crate::network_transport::loopback_test_client_tls_config(),
            )
            .unwrap();
            for expected in [b"first".as_slice(), b"second".as_slice()] {
                let response = client
                    .request(
                        HttpRequestValues::new(HttpMethod::Get, url.clone()),
                        Arc::clone(&grant),
                    )
                    .unwrap();
                assert_eq!(response.status.as_u16(), 200);
                assert_eq!(response.body, expected);
            }
            assert_eq!(client.pooled_origin_count(), 1);
            client.close();
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn chunked_query_without_content_type_is_rejected_before_body_completion() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nbody\r\n",
                )
                .unwrap();
            // The request deliberately never sends the terminating chunk. A
            // prompt 400 proves the server rejected its media contract as
            // soon as the first content frame arrived.
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });

        let called = Arc::new(AtomicBool::new(false));
        let handler_called = Arc::clone(&called);
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                serve_once(HttpServerIo::from(accepted), false, move |_| {
                    handler_called.store(true, Ordering::Release);
                    simple_response(200, b"unexpected")
                })
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });

        let response = String::from_utf8(client.join().unwrap()).unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(!called.load(Ordering::Acquire));
        assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    }

    #[test]
    fn malformed_accept_query_from_peer_is_a_protocol_error() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 256];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nAccept-Query:\r\nAccept-Query: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [format!("http://127.0.0.1:{}", address.port())],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let request = HttpRequestValues::new(
                HttpMethod::Get,
                Url::parse(&format!("http://127.0.0.1:{}/", address.port())).unwrap(),
            );
            let task = spawn(move || {
                assert!(matches!(
                    send_once(request, grant),
                    Err(HttpRuntimeError::Protocol)
                ));
                0
            });
            assert_eq!(crate::executor::await_task(task), 0);
            0
        });
        peer.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn one_shot_server_dispatches_query_on_the_zirk_task() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"QUERY /items HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/query+json\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"q\":\"zirk\"}",
                )
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                let owner = std::thread::current().id();
                let mut routes = zirk_runtime_core::http_router::HttpRouteTable::new();
                routes.register(HttpMethod::Query, "/items").unwrap();
                serve_once_routed(
                    HttpServerIo::from(accepted),
                    false,
                    routes,
                    move |route, request| {
                        assert_eq!(std::thread::current().id(), owner);
                        assert_eq!(request.method, HttpMethod::Query);
                        assert_eq!(request.url.path(), "/items");
                        assert_eq!(request.body, b"{\"q\":\"zirk\"}");
                        assert_eq!(route.route_index, 0);
                        HttpResponseValues {
                            status: HttpStatusCode::new(200).unwrap(),
                            headers: Headers::new(),
                            body: b"ok".to_vec(),
                            body_stream: None,
                            body_producer: None,
                        }
                    },
                )
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let response = String::from_utf8(client.join().unwrap()).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("ok"));
    }

    #[test]
    fn one_shot_server_emits_bounded_streamed_response_frames() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            let bytes: &[u8] = match chunk_index {
                0 => b"data: first\n\n",
                1 => b"data: second\n\n",
                _ => b"",
            };
            let value = crate::array::alloc_byte_array(bytes);
            if !out.is_null() {
                unsafe { out.write(value) };
            }
            0
        }

        let _gc_guard = crate::collector::test_support::test_guard();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let capture_address = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let capture_for_server = Arc::clone(&capture_address);
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /events HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Encoding: gzip, identity;q=0\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                serve_once(HttpServerIo::from(accepted), false, move |_| {
                    let mut headers = Headers::new();
                    headers
                        .append("content-type", b"text/event-stream; charset=utf-8")
                        .unwrap();
                    let capture = crate::string::alloc_owned("producer capture");
                    capture_for_server
                        .store(capture as usize, std::sync::atomic::Ordering::Release);
                    let context = crate::http_api::HttpServerProducerContext {
                        function: 0,
                        captures: capture as usize,
                        dispatch: 0,
                    };
                    let handle = unsafe {
                        crate::http_api::zirk_http_server_producer_register(
                            Some(producer),
                            &context,
                            capture,
                        )
                    };
                    assert_ne!(handle, 0);
                    crate::collector::collect_for_tests();
                    let body_producer = crate::http_api::take_http_server_producer(handle).unwrap();
                    HttpResponseValues {
                        status: HttpStatusCode::new(200).unwrap(),
                        headers,
                        body: Vec::new(),
                        body_stream: None,
                        body_producer: Some(body_producer),
                    }
                })
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let response = client.join().unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(
            response
                .windows(b"content-encoding: gzip".len())
                .any(|window| window == b"content-encoding: gzip")
        );
        assert!(!response.windows(11).any(|window| window == b"data: first"));
        assert!(!response.windows(12).any(|window| window == b"data: second"));
        let headers_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let mut encoded_body = Vec::new();
        let mut cursor = headers_end;
        loop {
            let line_end = response[cursor..]
                .windows(2)
                .position(|window| window == b"\r\n")
                .map(|offset| cursor + offset)
                .unwrap();
            let size = usize::from_str_radix(
                std::str::from_utf8(&response[cursor..line_end]).unwrap(),
                16,
            )
            .unwrap();
            cursor = line_end + 2;
            if size == 0 {
                break;
            }
            encoded_body.extend_from_slice(&response[cursor..cursor + size]);
            cursor += size + 2;
        }
        let decoded_output = Arc::new(std::sync::Mutex::new(None));
        let decoded_for_task = Arc::clone(&decoded_output);
        let outcome = Executor::new().run_with_root(move || {
            let mut headers = Headers::new();
            headers.insert("content-encoding", b"gzip").unwrap();
            let accepted = AcceptEncoding::parse(&headers).unwrap();
            let (headers, body, worker) = block_on_current(async move {
                decode_response_body(
                    headers,
                    HttpStreamingBody::Buffered {
                        bytes: Bytes::from(encoded_body),
                        offset: 0,
                    },
                    &accepted,
                )
            })
            .unwrap()
            .unwrap();
            assert!(headers.get("content-encoding").is_none());
            let HttpStreamingBody::Chunks(mut chunks) = body else {
                panic!("producer response must decode as a response stream");
            };
            let decoded = block_on_current(async move {
                let mut output = Vec::new();
                while let Some(chunk) = chunks.recv().await {
                    output.extend_from_slice(&chunk.unwrap());
                }
                output
            })
            .unwrap();
            drop(worker);
            *decoded_for_task
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(decoded);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert_eq!(
            decoded_output
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap(),
            b"data: first\n\ndata: second\n\n"
        );
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(
            capture_address.load(std::sync::atomic::Ordering::Acquire) as *mut std::ffi::c_void
        ));
    }

    #[cfg(unix)]
    #[test]
    fn live_http1_slow_consumer_backpressures_the_zirk_sse_producer() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};
        use std::os::fd::AsRawFd;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static PRODUCER_CALLS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            PRODUCER_CALLS.fetch_add(1, Ordering::AcqRel);
            let mut bytes = vec![0; 64 * 1024];
            let mut state = chunk_index.wrapping_add(0x9e37_79b9_7f4a_7c15);
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            let chunk = crate::array::alloc_byte_array(&bytes);
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        let _gc_guard = crate::collector::test_support::test_guard();
        for compressed in [false, true] {
            PRODUCER_CALLS.store(0, Ordering::Release);
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let capture_address = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let capture_for_server = Arc::clone(&capture_address);
            let client = std::thread::spawn(move || {
                let mut stream = std::net::TcpStream::connect(address).unwrap();
                let receive_buffer: libc::c_int = 1024;
                let result = unsafe {
                    libc::setsockopt(
                        stream.as_raw_fd(),
                        libc::SOL_SOCKET,
                        libc::SO_RCVBUF,
                        (&receive_buffer as *const libc::c_int).cast(),
                        std::mem::size_of_val(&receive_buffer) as libc::socklen_t,
                    )
                };
                assert_eq!(result, 0);
                let accept_encoding = if compressed {
                    b"gzip, identity;q=0".as_slice()
                } else {
                    b"identity".as_slice()
                };
                write!(stream, "GET /events HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Encoding: {}\r\nConnection: close\r\n\r\n", std::str::from_utf8(accept_encoding).unwrap()).unwrap();
                let mut header = Vec::new();
                let mut byte = [0u8; 1];
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 16 * 1024, "response headers exceeded limit");
                }
                let header_lower = header.to_ascii_lowercase();
                assert_eq!(
                    header_lower
                        .windows(b"content-encoding: gzip".len())
                        .any(|window| window == b"content-encoding: gzip"),
                    compressed,
                    "response coding must match the slow-consumer case"
                );
                std::thread::sleep(Duration::from_millis(500));
                let stalled_count = PRODUCER_CALLS.load(Ordering::Acquire);
                std::thread::sleep(Duration::from_millis(100));
                let later_count = PRODUCER_CALLS.load(Ordering::Acquire);
                assert!(
                    stalled_count > 0,
                    "the server never started the SSE producer"
                );
                assert_eq!(
                    later_count, stalled_count,
                    "the Zirk producer advanced while the transport peer stopped reading"
                );
                drop(stream);
            });

            let outcome = Executor::new().run_with_root(move || {
                let server = spawn(move || {
                    let accepted = crate::network_future::block_on_current(async move {
                        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                        listener.accept().await.unwrap().0
                    })
                    .unwrap();
                    let _ = serve_once(HttpServerIo::from(accepted), false, move |_| {
                        let mut headers = Headers::new();
                        headers
                            .append("content-type", b"text/event-stream; charset=utf-8")
                            .unwrap();
                        let capture = crate::string::alloc_owned("slow transport capture");
                        capture_for_server.store(capture as usize, Ordering::Release);
                        let context = crate::http_api::HttpServerProducerContext {
                            function: 0,
                            captures: capture as usize,
                            dispatch: 0,
                        };
                        let handle = unsafe {
                            crate::http_api::zirk_http_server_producer_register(
                                Some(producer),
                                &context,
                                capture,
                            )
                        };
                        assert_ne!(handle, 0);
                        crate::collector::collect_for_tests();
                        let body_producer =
                            crate::http_api::take_http_server_producer(handle).unwrap();
                        HttpResponseValues {
                            status: HttpStatusCode::new(200).unwrap(),
                            headers,
                            body: Vec::new(),
                            body_stream: None,
                            body_producer: Some(body_producer),
                        }
                    });
                    1
                });
                assert_eq!(await_task(server), 1);
                crate::collector::collect_for_tests();
                let capture = capture_address.load(Ordering::Acquire) as *mut std::ffi::c_void;
                assert!(!capture.is_null());
                assert!(!crate::collector::is_live_for_tests(capture));
                0
            });
            client.join().unwrap();
            assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        }
    }

    #[test]
    fn live_http2_slow_consumer_backpressures_the_zirk_sse_producer() {
        use crate::executor::{await_task, spawn};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};

        static PRODUCER_CALLS: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            PRODUCER_CALLS.fetch_add(1, Ordering::AcqRel);
            let mut bytes = vec![0; 64 * 1024];
            let mut state = chunk_index.wrapping_add(0x9e37_79b9_7f4a_7c15);
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            let chunk = crate::array::alloc_byte_array(&bytes);
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            for compressed in [false, true] {
                PRODUCER_CALLS.store(0, Ordering::Release);
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
                let server_config = crate::network_transport::TlsServerConfig::from_der(
                    vec![include_bytes!("../testdata/tls-loopback-cert.der").to_vec()],
                    include_bytes!("../testdata/tls-loopback-key.der").to_vec(),
                    false,
                )
                .unwrap()
                .with_alpn_protocols(vec![b"h2".to_vec()]);
                let capture_address = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let capture_for_server = Arc::clone(&capture_address);
                let server_grant = Arc::clone(&grant);
                let server = spawn(move || {
                    let listener = crate::network_transport::TlsListener::bind(
                        address,
                        &server_grant,
                        server_config,
                    )
                    .unwrap();
                    let (stream, _) = listener.accept().unwrap();
                    let stream: HttpServerIo = stream.into();
                    assert!(stream.negotiated_http2());
                    let _ = serve_http2_requests(stream, true, 1, move |_| {
                        let mut headers = Headers::new();
                        headers
                            .append("content-type", b"text/event-stream; charset=utf-8")
                            .unwrap();
                        let capture = crate::string::alloc_owned("slow HTTP/2 capture");
                        capture_for_server.store(capture as usize, Ordering::Release);
                        let context = crate::http_api::HttpServerProducerContext {
                            function: 0,
                            captures: capture as usize,
                            dispatch: 0,
                        };
                        let handle = unsafe {
                            crate::http_api::zirk_http_server_producer_register(
                                Some(producer),
                                &context,
                                capture,
                            )
                        };
                        assert_ne!(handle, 0);
                        crate::collector::collect_for_tests();
                        let body_producer =
                            crate::http_api::take_http_server_producer(handle).unwrap();
                        let mut response = HttpResponseValues {
                            status: HttpStatusCode::new(200).unwrap(),
                            headers,
                            body: Vec::new(),
                            body_stream: None,
                            body_producer: Some(body_producer),
                        };
                        if compressed {
                            response.headers.insert("vary", b"accept-encoding").unwrap();
                        }
                        response
                    });
                    1
                });

                let client_origin = origin.clone();
                let client = std::thread::spawn(move || {
                    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
                    roots
                        .add(CertificateDer::from(
                            include_bytes!("../testdata/tls-loopback-root.der").to_vec(),
                        ))
                        .unwrap();
                    let mut config = tokio_rustls::rustls::ClientConfig::builder()
                        .with_root_certificates(roots)
                        .with_no_client_auth();
                    config.alpn_protocols = vec![b"h2".to_vec()];
                    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    runtime.block_on(async move {
                        let socket = tokio::net::TcpStream::connect(address).await.unwrap();
                        let tls = connector
                            .connect(
                                ServerName::IpAddress(
                                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST).into(),
                                ),
                                socket,
                            )
                            .await
                            .unwrap();
                        let (mut sender, connection) = hyper::client::conn::http2::handshake(
                            hyper_util::rt::TokioExecutor::new(),
                            TokioIo::new(tls),
                        )
                        .await
                        .unwrap();
                        let driver = tokio::spawn(connection);
                        let mut request = Request::builder()
                            .method(hyper::Method::GET)
                            .uri(format!("{client_origin}/events"));
                        request = request.header(
                            hyper::header::ACCEPT_ENCODING,
                            if compressed {
                                "gzip, identity;q=0"
                            } else {
                                "identity"
                            },
                        );
                        let request = request.body(Full::new(Bytes::new())).unwrap();
                        let response = tokio::time::timeout(
                            Duration::from_secs(3),
                            sender.send_request(request),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                        assert_eq!(response.status(), hyper::StatusCode::OK);
                        assert_eq!(
                            response
                                .headers()
                                .contains_key(hyper::header::CONTENT_ENCODING),
                            compressed,
                            "HTTP/2 response coding must match the slow-consumer case"
                        );
                        // HTTP/2 grants a bounded receive window that Hyper may
                        // fill before the application starts reading the body.
                        // Wait for that window to saturate instead of assuming
                        // backpressure must happen within one arbitrary delay.
                        let stalled_count = tokio::time::timeout(
                            Duration::from_secs(5),
                            async {
                                let mut previous = PRODUCER_CALLS.load(Ordering::Acquire);
                                let mut stable_intervals = 0;
                                loop {
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    let current = PRODUCER_CALLS.load(Ordering::Acquire);
                                    if current == previous {
                                        stable_intervals += 1;
                                        if stable_intervals == 3 {
                                            break current;
                                        }
                                    } else {
                                        previous = current;
                                        stable_intervals = 0;
                                    }
                                }
                            },
                        )
                        .await
                        .expect("the unread HTTP/2 response should eventually exhaust its flow-control window");
                        assert!(stalled_count > 0, "the server never started the producer");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        assert_eq!(
                            PRODUCER_CALLS.load(Ordering::Acquire),
                            stalled_count,
                            "the Zirk producer advanced after the HTTP/2 flow-control window filled"
                        );
                        drop(response);
                        driver.abort();
                    });
                });

                assert_eq!(await_task(server), 1);
                client.join().unwrap();
                crate::collector::collect_for_tests();
                let capture = capture_address.load(Ordering::Acquire) as *mut std::ffi::c_void;
                assert!(!capture.is_null());
                assert!(!crate::collector::is_live_for_tests(capture));
            }
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn disconnected_streaming_client_releases_the_producer_capture_root() {
        for _ in 0..8 {
            disconnected_streaming_client_releases_the_producer_capture_root_once();
        }
    }

    fn disconnected_streaming_client_releases_the_producer_capture_root_once() {
        use crate::executor::{await_task, spawn};
        use std::io::{BufRead, Write};

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            _chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            let chunk = crate::array::alloc_byte_array(&vec![b'x'; 64 * 1024]);
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        let _gc_guard = crate::collector::test_support::test_guard();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let capture_address = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let capture_for_server = Arc::clone(&capture_address);
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut reader = std::io::BufReader::new(&mut stream);
            let mut line = Vec::new();
            loop {
                line.clear();
                reader.read_until(b'\n', &mut line).unwrap();
                if line == b"\r\n" || line.is_empty() {
                    break;
                }
            }
            drop(reader);
            stream.shutdown(std::net::Shutdown::Both).unwrap();
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                let result = serve_once(HttpServerIo::from(accepted), false, move |_| {
                    let capture = crate::string::alloc_owned("disconnect capture");
                    capture_for_server
                        .store(capture as usize, std::sync::atomic::Ordering::Release);
                    let context = crate::http_api::HttpServerProducerContext {
                        function: 0,
                        captures: capture as usize,
                        dispatch: 0,
                    };
                    let handle = unsafe {
                        crate::http_api::zirk_http_server_producer_register(
                            Some(producer),
                            &context,
                            capture,
                        )
                    };
                    assert_ne!(handle, 0);
                    crate::collector::collect_for_tests();
                    let body_producer = crate::http_api::take_http_server_producer(handle).unwrap();
                    HttpResponseValues {
                        status: HttpStatusCode::new(200).unwrap(),
                        headers: Headers::new(),
                        body: Vec::new(),
                        body_stream: None,
                        body_producer: Some(body_producer),
                    }
                });
                assert!(
                    result.is_err(),
                    "peer disconnect must end the response write"
                );
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        client.join().unwrap();
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(
            capture_address.load(std::sync::atomic::Ordering::Acquire) as *mut std::ffi::c_void
        ));
    }

    #[test]
    fn cancelling_a_streaming_server_task_drops_its_producer_root() {
        for _ in 0..8 {
            cancelling_a_streaming_server_task_drops_its_producer_root_once();
        }
    }

    fn cancelling_a_streaming_server_task_drops_its_producer_root_once() {
        use crate::executor::{await_task, cancel_current, spawn, yield_now};
        use std::io::{BufRead, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        static PRODUCER_CALLS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            _chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            PRODUCER_CALLS.fetch_add(1, Ordering::AcqRel);
            let chunk = crate::array::alloc_byte_array(&vec![b'y'; 64 * 1024]);
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        let _gc_guard = crate::collector::test_support::test_guard();
        PRODUCER_CALLS.store(0, Ordering::Release);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let capture_address = Arc::new(AtomicUsize::new(0));
        let capture_for_server = Arc::clone(&capture_address);
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut reader = std::io::BufReader::new(&mut stream);
            let mut line = Vec::new();
            loop {
                line.clear();
                reader.read_until(b'\n', &mut line).unwrap();
                if line == b"\r\n" || line.is_empty() {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            drop(reader);
            let _ = stream.shutdown(std::net::Shutdown::Both);
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                let result = serve_once(HttpServerIo::from(accepted), false, move |_| {
                    let capture = crate::string::alloc_owned("cancel capture");
                    capture_for_server.store(capture as usize, Ordering::Release);
                    let context = crate::http_api::HttpServerProducerContext {
                        function: 0,
                        captures: capture as usize,
                        dispatch: 0,
                    };
                    let handle = unsafe {
                        crate::http_api::zirk_http_server_producer_register(
                            Some(producer),
                            &context,
                            capture,
                        )
                    };
                    assert_ne!(handle, 0);
                    crate::collector::collect_for_tests();
                    HttpResponseValues {
                        status: HttpStatusCode::new(200).unwrap(),
                        headers: Headers::new(),
                        body: Vec::new(),
                        body_stream: None,
                        body_producer: Some(
                            crate::http_api::take_http_server_producer(handle).unwrap(),
                        ),
                    }
                });
                assert!(matches!(result, Err(HttpRuntimeError::Cancelled)));
                1
            });
            while PRODUCER_CALLS.load(Ordering::Acquire) == 0 {
                yield_now();
            }
            cancel_current(server);
            let joined =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| await_task(server)));
            assert!(
                joined.is_err(),
                "cancelled response task must not report success"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        client.join().unwrap();
        crate::collector::collect_for_tests();
        assert!(!crate::collector::is_live_for_tests(
            capture_address.load(Ordering::Acquire) as *mut std::ffi::c_void
        ));
    }

    #[test]
    fn routed_server_returns_not_found_and_method_not_allowed_with_allow() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            [("PUT", "/items/7"), ("GET", "/unknown")].map(|(method, path)| {
                let mut stream = std::net::TcpStream::connect(address).unwrap();
                write!(
                    stream,
                    "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                let mut response = Vec::new();
                stream.read_to_end(&mut response).unwrap();
                String::from_utf8(response).unwrap()
            })
        });

        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let listener = block_on_current(async {
                    tokio::net::TcpListener::from_std(listener).unwrap()
                })
                .unwrap();
                let mut routes = zirk_runtime_core::http_router::HttpRouteTable::new();
                routes.register(HttpMethod::Get, "/items/:id").unwrap();
                for _ in 0..2 {
                    let accepted =
                        block_on_current(async { listener.accept().await.unwrap().0 }).unwrap();
                    serve_once_routed(
                        HttpServerIo::from(accepted),
                        false,
                        routes.clone(),
                        |_, _| panic!("neither test request should match a route"),
                    )
                    .unwrap();
                }
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let responses = client.join().unwrap();
        assert!(responses[0].starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
        assert!(responses[0].contains("allow: GET\r\n"));
        assert!(responses[1].starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn server_times_out_a_stalled_request_body() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"POST /upload HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 8\r\nConnection: close\r\n\r\npart")
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                serve_once_with_timeouts(
                    HttpServerIo::from(accepted),
                    false,
                    HttpTimeouts {
                        connect: Duration::from_secs(1),
                        response_headers: Duration::from_secs(1),
                        response_idle: Duration::from_millis(20),
                    },
                    |_| panic!("handler must not run for an incomplete body"),
                )
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let response = String::from_utf8(client.join().unwrap()).unwrap();
        assert!(response.starts_with("HTTP/1.1 408 Request Timeout\r\n"));
    }

    #[test]
    fn server_closes_a_connection_with_stalled_headers() {
        use crate::executor::{await_task, spawn};
        use std::io::Read;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                assert_eq!(
                    serve_once_with_timeouts(
                        HttpServerIo::from(accepted),
                        false,
                        HttpTimeouts {
                            connect: Duration::from_secs(1),
                            response_headers: Duration::from_millis(20),
                            response_idle: Duration::from_secs(1),
                        },
                        |_| panic!("handler must not run before complete headers"),
                    ),
                    Err(HttpRuntimeError::Protocol)
                );
                1
            });
            assert_eq!(await_task(server), 1);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(client.join().unwrap().is_empty());
    }

    #[test]
    fn server_rejects_ambiguous_framing_before_handler_dispatch() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let called_by_handler = Arc::clone(&called);
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 1\r\n")
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
            stream
                .write_all(b"Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n0\r\n\r\n")
                .unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response);
            response
        });
        let outcome = Executor::new().run_with_root(move || {
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                assert_eq!(
                    serve_once(HttpServerIo::from(accepted), false, move |_| {
                        called_by_handler.store(true, Ordering::Release);
                        simple_response(200, b"unexpected")
                    }),
                    Err(HttpRuntimeError::Protocol)
                );
                0
            });
            assert_eq!(await_task(server), 0);
            0
        });
        let response = client.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(!called.load(Ordering::Acquire));
        assert!(response.is_empty(), "ambiguous request was not dispatched");
    }

    #[test]
    fn raw_framing_enforces_content_length_grammar_and_consistency() {
        for header in [
            b"POST / HTTP/1.1\r\nContent-Length: +1\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nContent-Length: 1 0\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nContent-Length: \t\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nContent-Length: 18446744073709551616\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n".as_slice(),
        ] {
            assert_eq!(
                validate_raw_framing(header).unwrap_err().kind(),
                io::ErrorKind::InvalidData,
                "accepted invalid Content-Length: {:?}",
                String::from_utf8_lossy(header)
            );
        }
        assert!(
            validate_raw_framing(b"POST / HTTP/1.1\r\nContent-Length: \t10 \t\r\n\r\n").is_ok()
        );
        assert!(
            validate_raw_framing(
                b"POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 10\r\n\r\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn body_accumulation_stops_at_the_configured_limit() {
        let mut body = vec![1, 2];
        assert_eq!(append_bounded_body(&mut body, &[3, 4], 4), Ok(()));
        assert_eq!(
            append_bounded_body(&mut body, &[5], 4),
            Err(HttpRuntimeError::BodyTooLarge)
        );
        assert_eq!(body, [1, 2, 3, 4]);
    }

    #[test]
    fn text_and_json_decoding_are_strict() {
        assert_eq!(decode_text(b"hello".to_vec()), Ok("hello".into()));
        assert_eq!(decode_text(vec![0xff]), Err(HttpRuntimeError::InvalidText));
        let parsed: serde_json::Value = decode_json(br#"{"ok":true}"#).unwrap();
        assert_eq!(parsed["ok"], true);
        let malformed: Result<serde_json::Value, _> = decode_json(b"{");
        assert_eq!(malformed, Err(HttpRuntimeError::InvalidJson));
    }

    #[test]
    fn response_body_idle_timeout_aborts_the_owned_connection() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 256];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream.read(&mut chunk).unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&chunk[..count]);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
                .unwrap();
            std::thread::sleep(Duration::from_millis(100));
            let _ = stream.write_all(b"ok");
        });
        let origin = format!("http://127.0.0.1:{}", address.port());
        let grant = Arc::new(
            EffectiveNetworkGrant::from_verified_scopes(
                [origin.clone()],
                std::iter::empty::<String>(),
            )
            .unwrap(),
        );
        let outcome = Executor::new().run_with_root(move || {
            let request = HttpRequestValues::new(
                HttpMethod::Get,
                Url::parse(&format!("{origin}/slow-body")).unwrap(),
            );
            let task = spawn(move || {
                let mut response = send_once_streaming_with_timeouts(
                    request,
                    grant,
                    HttpTimeouts {
                        connect: Duration::from_secs(1),
                        response_headers: Duration::from_secs(1),
                        response_idle: Duration::from_millis(20),
                    },
                )
                .unwrap();
                assert_eq!(response.read_chunk(8), Err(HttpRuntimeError::Timeout));
                1
            });
            assert_eq!(crate::executor::await_task(task), 1);
            0
        });
        server.join().unwrap();
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn uncompressed_server_stream_produces_only_on_transport_demand() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static PRODUCER_CALLS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            _chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            PRODUCER_CALLS.fetch_add(1, Ordering::AcqRel);
            let chunk = crate::array::alloc_byte_array(b"event: one\n\n");
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        PRODUCER_CALLS.store(0, Ordering::Release);
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let capture = crate::string::alloc_owned("slow SSE consumer capture");
            let context = crate::http_api::HttpServerProducerContext {
                function: 0,
                captures: capture as usize,
                dispatch: 0,
            };
            let handle = unsafe {
                crate::http_api::zirk_http_server_producer_register(
                    Some(producer),
                    &context,
                    capture,
                )
            };
            assert_ne!(handle, 0);
            crate::collector::collect_for_tests();
            assert!(crate::collector::is_live_for_tests(capture));

            let mut body = HttpZirkProducerBody {
                producer: Some(crate::http_api::take_http_server_producer(handle).unwrap()),
            };
            assert_eq!(PRODUCER_CALLS.load(Ordering::Acquire), 0);
            let waker = futures_util::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            let frame = Pin::new(&mut body).poll_frame(&mut context);
            assert!(matches!(frame, Poll::Ready(Some(Ok(_)))));
            assert_eq!(PRODUCER_CALLS.load(Ordering::Acquire), 1);

            // A stalled consumer does not cause a detached producer to build
            // more chunks while the transport is not polling the body.
            std::thread::sleep(Duration::from_millis(20));
            assert_eq!(PRODUCER_CALLS.load(Ordering::Acquire), 1);
            drop(body);
            crate::collector::collect_for_tests();
            assert!(!crate::collector::is_live_for_tests(capture));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn compressed_server_stream_stops_prefetching_when_output_is_not_consumed() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static PRODUCER_CALLS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn producer(
            context: *mut std::ffi::c_void,
            chunk_index: u64,
            out: *mut *mut std::ffi::c_void,
        ) -> i32 {
            let context =
                unsafe { &*(context as *const crate::http_api::HttpServerProducerContext) };
            assert!(crate::collector::is_live_for_tests(
                context.captures as *mut std::ffi::c_void
            ));
            PRODUCER_CALLS.fetch_add(1, Ordering::AcqRel);
            let mut bytes = Vec::with_capacity(64 * 1024);
            for line in 0..4 {
                bytes.extend_from_slice(b"data:");
                for offset in 0..16_000 {
                    let value = (offset as u64)
                        .wrapping_mul(73)
                        .wrapping_add(chunk_index.wrapping_mul(19))
                        .wrapping_add(line * 7);
                    bytes.push(b'a' + (value % 26) as u8);
                }
                bytes.push(b'\n');
            }
            bytes.push(b'\n');
            let chunk = crate::array::alloc_byte_array(&bytes);
            if !out.is_null() {
                unsafe { out.write(chunk) };
            }
            0
        }

        PRODUCER_CALLS.store(0, Ordering::Release);
        let _gc_guard = crate::collector::test_support::test_guard();
        let outcome = Executor::new().run_with_root(|| {
            let capture = crate::string::alloc_owned("compressed SSE consumer capture");
            let producer_context = crate::http_api::HttpServerProducerContext {
                function: 0,
                captures: capture as usize,
                dispatch: 0,
            };
            let handle = unsafe {
                crate::http_api::zirk_http_server_producer_register(
                    Some(producer),
                    &producer_context,
                    capture,
                )
            };
            assert_ne!(handle, 0);
            crate::collector::collect_for_tests();
            assert!(crate::collector::is_live_for_tests(capture));

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                let mut body = HttpZirkCompressedProducerBody::new(
                    crate::http_api::take_http_server_producer(handle).unwrap(),
                    ContentCoding::Gzip,
                );
                let first_frame =
                    std::future::poll_fn(|context| Pin::new(&mut body).poll_frame(context)).await;
                assert!(matches!(first_frame, Some(Ok(_))));
                let prefetched = PRODUCER_CALLS.load(Ordering::Acquire);
                assert!(prefetched > 0);
                assert!(
                    prefetched <= 8,
                    "producer ran {prefetched} times before backpressure"
                );

                // The encoder may drain its bounded input/output buffers, but
                // it cannot keep invoking the Zirk callback while the consumer
                // stops polling the response body.
                tokio::time::sleep(Duration::from_millis(20)).await;
                assert_eq!(PRODUCER_CALLS.load(Ordering::Acquire), prefetched);
                drop(body);
            });
            crate::collector::collect_for_tests();
            assert!(!crate::collector::is_live_for_tests(capture));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn generated_server_callback_runs_on_owner_stack_and_returns_bounded_values() {
        use crate::executor::{await_task, spawn};
        use std::io::{Read, Write};

        struct CallbackState {
            owner: std::thread::ThreadId,
            observed_method: bool,
            observed_target: bool,
            observed_body: bool,
        }

        unsafe extern "C-unwind" fn handler(
            request: *const crate::http_api::HttpServerRequest,
            response: *mut crate::http_api::HttpServerResponse,
            context: *mut std::ffi::c_void,
        ) -> i32 {
            let state = unsafe { &mut *context.cast::<CallbackState>() };
            assert_eq!(std::thread::current().id(), state.owner);
            let request = unsafe { &*request };
            let method = unsafe { crate::string::borrow(request.method) }.unwrap();
            state.observed_method = unsafe { method.as_str() } == "QUERY";
            let target = unsafe { crate::string::borrow(request.target) }.unwrap();
            state.observed_target = unsafe { target.as_str() } == "/items?q=zirk";
            let body = unsafe { crate::array::copy_byte_array(request.body, 1024) };
            state.observed_body = body.as_deref() == Some(b"payload");

            let body = crate::array::alloc_byte_array(b"created");
            let mut rooted_body = body;
            let mut roots = [(&mut rooted_body as *mut *mut std::ffi::c_void).cast()];
            unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
            let headers = crate::array::alloc_byte_array(&0u32.to_le_bytes());
            unsafe {
                response.write(crate::http_api::HttpServerResponse {
                    status: 201,
                    headers,
                    body: rooted_body,
                    producer_handle: 0,
                    sse_heartbeat_ms: 0,
                });
                crate::zirk_rt_pop_frame();
            }
            0
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"QUERY /items?q=zirk HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/query+json\r\nContent-Length: 7\r\nConnection: close\r\n\r\npayload",
                )
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });

        let mut state = CallbackState {
            owner: std::thread::current().id(),
            observed_method: false,
            observed_target: false,
            observed_body: false,
        };
        let outcome = Executor::new().run_with_root(move || {
            let context = (&mut state as *mut CallbackState).cast::<std::ffi::c_void>();
            let context = context as usize;
            let server = spawn(move || {
                let accepted = crate::network_future::block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap().0
                })
                .unwrap();
                crate::http_api::serve_once_with_callback(
                    HttpServerIo::from(accepted),
                    false,
                    1,
                    false,
                    handler,
                    context as *mut std::ffi::c_void,
                )
                .unwrap();
                1
            });
            assert_eq!(await_task(server), 1);
            assert!(state.observed_method && state.observed_target && state.observed_body);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let response = String::from_utf8(client.join().unwrap()).unwrap();
        assert!(response.starts_with("HTTP/1.1 201"), "{response}");
        assert!(response.ends_with("created"), "{response}");
    }
}
