//! Shared JSON input limits and duplicate-object-key validation.

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_NESTING_DEPTH: usize = 128;
pub const HARD_MAX_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
pub const HARD_MAX_NESTING_DEPTH: usize = 512;
pub const HARD_MAX_STRING_BYTES: usize = 16 * 1024 * 1024;
pub const HARD_MAX_CONTAINER_ITEMS: usize = 1_000_000;
pub const HARD_MAX_NUMBER_BYTES: usize = 4_096;
pub const MAX_JSONPATH_QUERY_BYTES: usize = 64 * 1024;
pub const MAX_JSONPATH_STEPS: usize = 1_000_000;
pub const MAX_JSONPATH_RESULTS: usize = 100_000;
pub const MAX_JSONPATH_RESULT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum amount of data passed to an underlying JSON stream operation at a
/// time. The reader may request one additional byte to distinguish an exact
/// limit from an oversized document.
pub const JSON_STREAM_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonLimits {
    pub max_document_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_string_bytes: usize,
    pub max_container_items: usize,
    pub max_number_bytes: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_document_bytes: MAX_DOCUMENT_BYTES,
            max_nesting_depth: MAX_NESTING_DEPTH,
            max_string_bytes: HARD_MAX_STRING_BYTES,
            max_container_items: HARD_MAX_CONTAINER_ITEMS,
            max_number_bytes: HARD_MAX_NUMBER_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonValidationError {
    DocumentTooLarge,
    NestingTooDeep,
    StringTooLarge,
    TooManyContainerItems,
    NumberTooLarge,
    NumberOutsideCanonicalDomain,
    InvalidLimits,
    InvalidDocument,
}

/// Failure while incrementally reading or writing a bounded JSON document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonStreamError {
    Validation(JsonValidationError),
    ChunkTooLarge,
    Io,
    Cancelled,
    Allocation,
    InvalidState,
}

static NEXT_STREAM_READER_HANDLE: AtomicU64 = AtomicU64::new(1);
static STREAM_READERS: OnceLock<Mutex<HashMap<u64, JsonStreamReader>>> = OnceLock::new();
static NEXT_STREAM_WRITER_HANDLE: AtomicU64 = AtomicU64::new(1);
static STREAM_WRITERS: OnceLock<Mutex<HashMap<u64, JsonStreamWriter>>> = OnceLock::new();

fn stream_readers() -> &'static Mutex<HashMap<u64, JsonStreamReader>> {
    STREAM_READERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stream_writers() -> &'static Mutex<HashMap<u64, JsonStreamWriter>> {
    STREAM_WRITERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Output format supported by the stateful JSON stream writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonStreamFormat {
    Compact,
    Pretty,
    Canonical,
}

/// Creates a pull-based writer handle. Serialization runs against a bounded
/// one-chunk channel, so the complete output document is never materialized.
pub fn stream_writer_create(
    value: serde_json::Value,
    limits: JsonLimits,
    format: JsonStreamFormat,
) -> Result<u64, JsonStreamError> {
    let writer = JsonStreamWriter::new(value, limits, format)?;
    let handle = NEXT_STREAM_WRITER_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 || handle >= u64::MAX - 1 {
        return Err(JsonStreamError::Allocation);
    }
    stream_writers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(handle, writer);
    Ok(handle)
}

/// Returns the next valid UTF-8 text chunk, or `None` after successful end.
pub fn stream_writer_next(
    handle: u64,
    max_chunk_bytes: usize,
) -> Result<Option<String>, JsonStreamError> {
    let mut writers = stream_writers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    writers
        .get_mut(&handle)
        .ok_or(JsonStreamError::InvalidState)?
        .next_chunk(max_chunk_bytes)
}

/// Pulls and stores one chunk for adapters that expose status and payload as
/// separate ABI results. Status values are 0=data, 1=end, and 2=error.
pub fn stream_writer_next_status(handle: u64, max_chunk_bytes: usize) -> u32 {
    let mut writers = stream_writers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(writer) = writers.get_mut(&handle) else {
        return 3;
    };
    writer.pending_chunk = None;
    let status = match writer.next_chunk(max_chunk_bytes) {
        Ok(Some(chunk)) => {
            writer.pending_chunk = Some(chunk);
            0
        }
        Ok(None) => 1,
        Err(error) => match error {
            JsonStreamError::Cancelled => 3,
            JsonStreamError::InvalidState => 4,
            JsonStreamError::Io | JsonStreamError::Allocation => 5,
            JsonStreamError::ChunkTooLarge => 6,
            JsonStreamError::Validation(_) => 2,
        },
    };
    if status != 0 {
        writers.remove(&handle);
    }
    status
}

/// Takes the chunk produced by the immediately preceding status call.
pub fn stream_writer_take(handle: u64) -> Result<String, JsonStreamError> {
    stream_writers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_mut(&handle)
        .and_then(|writer| writer.pending_chunk.take())
        .ok_or(JsonStreamError::InvalidState)
}

/// Cancels and releases a writer. No subsequent call can report successful
/// completion for a document that was only partially serialized.
pub fn stream_writer_cancel(handle: u64) -> Result<(), JsonStreamError> {
    let Some(mut writer) = stream_writers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle)
    else {
        return Err(JsonStreamError::InvalidState);
    };
    writer.cancel();
    Ok(())
}

/// Creates a bounded, stateful JSON reader handle for language/runtime
/// adapters. Input is parsed as it is pushed and the registry owns at most one
/// queued chunk plus the final JSON value tree.
pub fn stream_reader_create(limits: JsonLimits) -> Result<u64, JsonStreamError> {
    let reader = JsonStreamReader::new(limits)?;
    let handle = NEXT_STREAM_READER_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 || handle == u64::MAX {
        return Err(JsonStreamError::Allocation);
    }
    stream_readers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(handle, reader);
    Ok(handle)
}

/// Pushes one source chunk into a registered reader. The call may wait for the
/// one-chunk channel to drain, providing bounded backpressure.
pub fn stream_reader_push(handle: u64, chunk: &[u8]) -> Result<(), JsonStreamError> {
    let mut readers = stream_readers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let reader = readers
        .get_mut(&handle)
        .ok_or(JsonStreamError::InvalidState)?;
    reader.push_chunk(chunk)
}

/// Cancels and releases a reader handle. A cancelled handle cannot publish a
/// partial document and cannot be resumed.
pub fn stream_reader_cancel(handle: u64) -> Result<(), JsonStreamError> {
    let Some(mut reader) = stream_readers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle)
    else {
        return Err(JsonStreamError::InvalidState);
    };
    reader.cancel();
    Ok(())
}

/// Closes a reader handle and publishes its complete parsed JSON value only
/// when the parser has validated the entire document.
pub fn stream_reader_finish(handle: u64) -> Result<serde_json::Value, JsonStreamError> {
    let reader = stream_readers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle)
        .ok_or(JsonStreamError::InvalidState)?;
    reader.finish(|| false)
}

/// Stateful strict-JSON input. Chunks flow through a one-chunk channel into
/// the bounded parser, so the reader does not retain a second full copy of the
/// document while `finish` is the only operation that can publish a value.
pub struct JsonStreamReader {
    sender: Option<SyncSender<Option<Vec<u8>>>>,
    cancelled: Arc<AtomicBool>,
    bytes_pushed: usize,
    parser: Option<JoinHandle<Result<serde_json::Value, JsonStreamError>>>,
    document_limit: usize,
}

impl JsonStreamReader {
    pub fn new(limits: JsonLimits) -> Result<Self, JsonStreamError> {
        validate_limits(limits).map_err(JsonStreamError::Validation)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let parser_cancelled = Arc::clone(&cancelled);
        let parser = std::thread::Builder::new()
            .name("zirk-json-stream-reader".into())
            .spawn(move || {
                let mut reader = ChunkReceiverReader::new(receiver);
                parse_reader_with_limits(&mut reader, limits, || {
                    parser_cancelled.load(Ordering::Acquire)
                })
            })
            .map_err(|_| JsonStreamError::Io)?;
        Ok(Self {
            sender: Some(sender),
            cancelled,
            bytes_pushed: 0,
            parser: Some(parser),
            document_limit: limits.max_document_bytes,
        })
    }

    /// Appends one bounded source chunk. A chunk over 64 KiB or any input
    /// beyond the configured document ceiling is rejected before copying it.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<(), JsonStreamError> {
        if self.cancelled.load(Ordering::Acquire) || self.sender.is_none() {
            return Err(JsonStreamError::Cancelled);
        }
        if chunk.len() > JSON_STREAM_CHUNK_BYTES {
            return Err(JsonStreamError::ChunkTooLarge);
        }
        let Some(total) = self.bytes_pushed.checked_add(chunk.len()) else {
            return Err(JsonStreamError::Validation(
                JsonValidationError::DocumentTooLarge,
            ));
        };
        if total > self.document_limit {
            return Err(JsonStreamError::Validation(
                JsonValidationError::DocumentTooLarge,
            ));
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(chunk.len())
            .map_err(|_| JsonStreamError::Allocation)?;
        owned.extend_from_slice(chunk);
        self.sender
            .as_ref()
            .expect("reader sender checked above")
            .send(Some(owned))
            .map_err(|_| JsonStreamError::Io)?;
        self.bytes_pushed = total;
        Ok(())
    }

    /// Discards buffered input and permanently prevents this reader from
    /// producing a value.
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.sender.take();
    }

    /// Completes the input and parses one entire strict JSON document.
    pub fn finish(
        mut self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<serde_json::Value, JsonStreamError> {
        if self.cancelled.load(Ordering::Acquire) || cancelled() {
            self.cancel();
            let _ = self.parser.take().map(JoinHandle::join);
            return Err(JsonStreamError::Cancelled);
        }
        self.sender.take();
        self.parser
            .take()
            .expect("stream parser is present until finish")
            .join()
            .map_err(|_| JsonStreamError::Io)?
    }
}

impl Drop for JsonStreamReader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.sender.take();
        if let Some(parser) = self.parser.take() {
            let _ = parser.join();
        }
    }
}

enum JsonWriterMessage {
    Chunk(String),
    Finished(Result<(), JsonStreamError>),
}

/// Stateful pull serializer. The worker serializes the owned JSON tree to a
/// one-chunk channel; retained serialized text is bounded by one producer
/// chunk plus one caller-visible chunk.
pub struct JsonStreamWriter {
    receiver: Option<Receiver<JsonWriterMessage>>,
    worker: Option<JoinHandle<()>>,
    pending: VecDeque<u8>,
    pending_chunk: Option<String>,
    finished: bool,
    cancelled: Arc<AtomicBool>,
}

impl JsonStreamWriter {
    pub fn new(
        value: serde_json::Value,
        limits: JsonLimits,
        format: JsonStreamFormat,
    ) -> Result<Self, JsonStreamError> {
        validate_limits(limits).map_err(JsonStreamError::Validation)?;
        validate_value_limits(&value, limits, 0).map_err(JsonStreamError::Validation)?;
        if matches!(format, JsonStreamFormat::Canonical) {
            validate_canonical_numbers(&value).map_err(JsonStreamError::Validation)?;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let worker = std::thread::Builder::new()
            .name("zirk-json-stream-writer".into())
            .spawn(move || {
                let mut sink = JsonChannelWriter::new(sender);
                let result = match format {
                    JsonStreamFormat::Compact => {
                        write_value_with_limits(&mut sink, &value, limits, || {
                            worker_cancelled.load(Ordering::Acquire)
                        })
                    }
                    JsonStreamFormat::Pretty => {
                        write_pretty_value_with_limits(&mut sink, &value, limits, || {
                            worker_cancelled.load(Ordering::Acquire)
                        })
                    }
                    JsonStreamFormat::Canonical => {
                        write_canonical_value_with_limits(&mut sink, &value, limits, || {
                            worker_cancelled.load(Ordering::Acquire)
                        })
                    }
                };
                let _ = sink.sender.send(JsonWriterMessage::Finished(result));
            })
            .map_err(|_| JsonStreamError::Io)?;
        Ok(Self {
            receiver: Some(receiver),
            worker: Some(worker),
            pending: VecDeque::new(),
            pending_chunk: None,
            finished: false,
            cancelled,
        })
    }

    /// Pulls at most `max_chunk_bytes` of valid UTF-8. A scalar wider than
    /// the requested chunk size is rejected instead of split across strings.
    pub fn next_chunk(
        &mut self,
        max_chunk_bytes: usize,
    ) -> Result<Option<String>, JsonStreamError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(JsonStreamError::Cancelled);
        }
        if self.finished {
            return Ok(None);
        }
        if max_chunk_bytes == 0 || max_chunk_bytes > JSON_STREAM_CHUNK_BYTES {
            return Err(JsonStreamError::ChunkTooLarge);
        }
        loop {
            if !self.pending.is_empty() {
                let pending_len = self.pending.len();
                let available = pending_len.min(max_chunk_bytes);
                let bytes = self.pending.make_contiguous();
                match std::str::from_utf8(&bytes[..available]) {
                    Ok(_) => {
                        let chunk = self.pending.drain(..available).collect::<Vec<_>>();
                        return String::from_utf8(chunk)
                            .map(Some)
                            .map_err(|_| JsonStreamError::Io);
                    }
                    Err(error) if error.error_len().is_none() && available < pending_len => {
                        let end = error.valid_up_to();
                        if end > 0 {
                            let chunk = self.pending.drain(..end).collect::<Vec<_>>();
                            return String::from_utf8(chunk)
                                .map(Some)
                                .map_err(|_| JsonStreamError::Io);
                        }
                        return Err(JsonStreamError::ChunkTooLarge);
                    }
                    Err(error) if error.error_len().is_none() => {}
                    Err(_) => return Err(JsonStreamError::Io),
                }
            }
            match self
                .receiver
                .as_ref()
                .ok_or(JsonStreamError::Cancelled)?
                .recv()
                .map_err(|_| JsonStreamError::Io)?
            {
                JsonWriterMessage::Chunk(chunk) => self.pending.extend(chunk.into_bytes()),
                JsonWriterMessage::Finished(Ok(())) => {
                    self.finished = true;
                    self.join_worker();
                    if self.pending.is_empty() {
                        return Ok(None);
                    }
                }
                JsonWriterMessage::Finished(Err(error)) => {
                    self.finished = true;
                    self.pending.clear();
                    self.join_worker();
                    return Err(error);
                }
            }
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.pending.clear();
        self.finished = true;
        self.receiver.take();
        self.join_worker();
    }

    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for JsonStreamWriter {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.receiver.take();
        self.join_worker();
    }
}

struct JsonChannelWriter {
    sender: SyncSender<JsonWriterMessage>,
    buffer: Vec<u8>,
}

impl JsonChannelWriter {
    fn new(sender: SyncSender<JsonWriterMessage>) -> Self {
        Self {
            sender,
            buffer: Vec::with_capacity(JSON_STREAM_CHUNK_BYTES + 4),
        }
    }

    fn send_available(&mut self, flush: bool) -> io::Result<()> {
        loop {
            if self.buffer.is_empty() || (!flush && self.buffer.len() < JSON_STREAM_CHUNK_BYTES) {
                return Ok(());
            }
            let candidate = self.buffer.len().min(JSON_STREAM_CHUNK_BYTES);
            let valid_end = match std::str::from_utf8(&self.buffer[..candidate]) {
                Ok(_) => candidate,
                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                Err(_) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"));
                }
            };
            if valid_end == 0 {
                if flush {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "incomplete UTF-8",
                    ));
                }
                return Ok(());
            }
            let chunk = self.buffer.drain(..valid_end).collect::<Vec<_>>();
            let chunk = String::from_utf8(chunk)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"))?;
            self.sender
                .send(JsonWriterMessage::Chunk(chunk))
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "JSON stream closed"))?;
            if !flush && self.buffer.len() < JSON_STREAM_CHUNK_BYTES {
                return Ok(());
            }
        }
    }
}

impl io::Write for JsonChannelWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        self.send_available(false)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send_available(true)
    }
}

struct ChunkReceiverReader {
    receiver: Receiver<Option<Vec<u8>>>,
    current: Vec<u8>,
    offset: usize,
    finished: bool,
}

impl ChunkReceiverReader {
    fn new(receiver: Receiver<Option<Vec<u8>>>) -> Self {
        Self {
            receiver,
            current: Vec::new(),
            offset: 0,
            finished: false,
        }
    }
}

impl io::Read for ChunkReceiverReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.finished {
            return Ok(0);
        }
        while self.offset >= self.current.len() {
            match self.receiver.recv() {
                Ok(Some(chunk)) => {
                    self.current = chunk;
                    self.offset = 0;
                    if self.current.is_empty() {
                        continue;
                    }
                }
                Ok(None) | Err(_) => {
                    self.finished = true;
                    return Ok(0);
                }
            }
        }
        let count = buffer.len().min(self.current.len() - self.offset);
        buffer[..count].copy_from_slice(&self.current[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

/// Reads a JSON document in bounded chunks and returns a value only after the
/// complete document has passed the shared validation limits and strict JSON
/// parser. Cancellation is checked between reads. The input buffer never grows
/// past the configured document-byte limit; one extra byte is read only to
/// distinguish an exact-limit document from an oversized one.
pub fn parse_reader_with_limits<R: std::io::Read>(
    reader: &mut R,
    limits: JsonLimits,
    mut cancelled: impl FnMut() -> bool,
) -> Result<serde_json::Value, JsonStreamError> {
    validate_limits(limits).map_err(JsonStreamError::Validation)?;
    struct LimitedReader<'a, R, F> {
        inner: &'a mut R,
        limits: JsonLimits,
        bytes_read: usize,
        cancelled: &'a mut F,
        exceeded: bool,
        was_cancelled: bool,
        io_failed: bool,
    }

    impl<R: std::io::Read, F: FnMut() -> bool> std::io::Read for LimitedReader<'_, R, F> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if (self.cancelled)() {
                self.was_cancelled = true;
                return Err(std::io::Error::other("JSON stream cancelled"));
            }
            let remaining = self
                .limits
                .max_document_bytes
                .saturating_sub(self.bytes_read);
            let bounded = buffer
                .len()
                .min(JSON_STREAM_CHUNK_BYTES)
                .min(remaining.saturating_add(1));
            let count = match self.inner.read(&mut buffer[..bounded]) {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Err(error),
                Err(error) => {
                    self.io_failed = true;
                    return Err(error);
                }
            };
            if count > remaining {
                self.exceeded = true;
                return Err(std::io::Error::other(
                    "JSON document exceeds its byte limit",
                ));
            }
            self.bytes_read += count;
            Ok(count)
        }
    }

    struct ValueSeed {
        depth: usize,
        limits: JsonLimits,
    }

    impl<'de> DeserializeSeed<'de> for ValueSeed {
        type Value = serde_json::Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            if self.depth > self.limits.max_nesting_depth {
                return Err(serde::de::Error::custom("ZIRK_JSON_DEPTH_LIMIT"));
            }
            deserializer.deserialize_any(ValueVisitor {
                depth: self.depth,
                limits: self.limits,
            })
        }
    }

    struct ValueVisitor {
        depth: usize,
        limits: JsonLimits,
    }

    impl<'de> Visitor<'de> for ValueVisitor {
        type Value = serde_json::Value;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a JSON value within configured limits")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Bool(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Number(value.into()))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Number(value.into()))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            serde_json::Number::from_f64(value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| E::custom("invalid JSON number"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.limits.max_string_bytes {
                return Err(E::custom("ZIRK_JSON_STRING_LIMIT"));
            }
            Ok(serde_json::Value::String(value.to_owned()))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.limits.max_string_bytes {
                return Err(E::custom("ZIRK_JSON_STRING_LIMIT"));
            }
            Ok(serde_json::Value::String(value))
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Null)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element_seed(ValueSeed {
                depth: self.depth + 1,
                limits: self.limits,
            })? {
                if values.len() >= self.limits.max_container_items {
                    return Err(serde::de::Error::custom("ZIRK_JSON_CONTAINER_LIMIT"));
                }
                values.push(value);
            }
            Ok(serde_json::Value::Array(values))
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let Some(first_key) = map.next_key::<String>()? else {
                return Ok(serde_json::Value::Object(serde_json::Map::new()));
            };
            if first_key == "$serde_json::private::Number" {
                let number_text = map.next_value::<String>()?;
                if number_text.len() > self.limits.max_number_bytes {
                    return Err(serde::de::Error::custom("ZIRK_JSON_NUMBER_LIMIT"));
                }
                if map.next_key::<String>()?.is_some() {
                    return Err(serde::de::Error::custom("invalid JSON number token"));
                }
                let number = serde_json::from_str::<serde_json::Number>(&number_text)
                    .map_err(serde::de::Error::custom)?;
                return Ok(serde_json::Value::Number(number));
            }

            let mut result = serde_json::Map::new();
            let mut seen = HashSet::new();
            let mut key = Some(first_key);
            loop {
                let Some(name) = key.take() else {
                    break;
                };
                if result.len() >= self.limits.max_container_items {
                    return Err(serde::de::Error::custom("ZIRK_JSON_CONTAINER_LIMIT"));
                }
                if !seen.insert(name.clone()) {
                    return Err(serde::de::Error::custom("ZIRK_JSON_DUPLICATE_KEY"));
                }
                let value = map.next_value_seed(ValueSeed {
                    depth: self.depth + 1,
                    limits: self.limits,
                })?;
                result.insert(name, value);
                key = map.next_key::<String>()?;
            }
            Ok(serde_json::Value::Object(result))
        }
    }

    let mut limited = LimitedReader {
        inner: reader,
        limits,
        bytes_read: 0,
        cancelled: &mut cancelled,
        exceeded: false,
        was_cancelled: false,
        io_failed: false,
    };
    let parsed = {
        let mut deserializer = serde_json::Deserializer::from_reader(&mut limited);
        let result = ValueSeed { depth: 0, limits }.deserialize(&mut deserializer);
        match result {
            Ok(value) => match deserializer.end() {
                Ok(()) => Ok(value),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        }
    };
    if limited.was_cancelled {
        return Err(JsonStreamError::Cancelled);
    }
    if limited.exceeded {
        return Err(JsonStreamError::Validation(
            JsonValidationError::DocumentTooLarge,
        ));
    }
    if limited.io_failed {
        return Err(JsonStreamError::Io);
    }
    let value = parsed.map_err(|error| {
        let message = error.to_string();
        let validation = match message.as_str() {
            message if message.contains("ZIRK_JSON_DEPTH_LIMIT") => {
                Some(JsonValidationError::NestingTooDeep)
            }
            message if message.contains("ZIRK_JSON_STRING_LIMIT") => {
                Some(JsonValidationError::StringTooLarge)
            }
            message if message.contains("ZIRK_JSON_NUMBER_LIMIT") => {
                Some(JsonValidationError::NumberTooLarge)
            }
            message if message.contains("ZIRK_JSON_CONTAINER_LIMIT") => {
                Some(JsonValidationError::TooManyContainerItems)
            }
            message if message.contains("ZIRK_JSON_DUPLICATE_KEY") => {
                Some(JsonValidationError::InvalidDocument)
            }
            _ => None,
        };
        JsonStreamError::Validation(validation.unwrap_or(JsonValidationError::InvalidDocument))
    })?;
    validate_value_limits(&value, limits, 0).map_err(JsonStreamError::Validation)?;
    Ok(value)
}

/// Serializes a complete JSON value to a synchronous sink using bounded
/// writes. The sink may contain a prefix if it fails; in that case this
/// function returns an error and never reports the document as successful.
/// Cancellation is checked between at-most-64-KiB writes.
pub fn write_value_with_limits<W: std::io::Write>(
    writer: &mut W,
    value: &serde_json::Value,
    limits: JsonLimits,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), JsonStreamError> {
    write_value_with_limits_and_format(writer, value, limits, JsonFormat::Compact, &mut cancelled)
}

/// Writes deterministic pretty JSON with the same byte, depth, string,
/// container, and number limits as ordinary serialization.
pub fn write_pretty_value_with_limits<W: std::io::Write>(
    writer: &mut W,
    value: &serde_json::Value,
    limits: JsonLimits,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), JsonStreamError> {
    write_value_with_limits_and_format(writer, value, limits, JsonFormat::Pretty, &mut cancelled)
}

/// Writes RFC 8785 canonical JSON for values in the supported exact numeric
/// domain, using bounded sink writes and the shared validation limits.
pub fn write_canonical_value_with_limits<W: std::io::Write>(
    writer: &mut W,
    value: &serde_json::Value,
    limits: JsonLimits,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), JsonStreamError> {
    write_value_with_limits_and_format(writer, value, limits, JsonFormat::Canonical, &mut cancelled)
}

fn write_value_with_limits_and_format<W: std::io::Write>(
    writer: &mut W,
    value: &serde_json::Value,
    limits: JsonLimits,
    format: JsonFormat,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), JsonStreamError> {
    if cancelled() {
        return Err(JsonStreamError::Cancelled);
    }
    validate_limits(limits).map_err(JsonStreamError::Validation)?;
    validate_value_limits(value, limits, 0).map_err(JsonStreamError::Validation)?;
    if matches!(format, JsonFormat::Canonical) {
        validate_canonical_numbers(value).map_err(JsonStreamError::Validation)?;
    }
    if cancelled() {
        return Err(JsonStreamError::Cancelled);
    }

    struct BoundedStreamWriter<'a, W, F> {
        inner: &'a mut W,
        limits: JsonLimits,
        written: usize,
        cancelled: &'a mut F,
        error: Option<JsonStreamError>,
    }

    impl<W: std::io::Write, F: FnMut() -> bool> std::io::Write for BoundedStreamWriter<'_, W, F> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if (self.cancelled)() {
                self.error = Some(JsonStreamError::Cancelled);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "JSON stream cancelled",
                ));
            }
            if self.written.saturating_add(bytes.len()) > self.limits.max_document_bytes {
                self.error = Some(JsonStreamError::Validation(
                    JsonValidationError::DocumentTooLarge,
                ));
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "JSON output exceeds configured limit",
                ));
            }
            let count = bytes.len().min(JSON_STREAM_CHUNK_BYTES);
            if let Err(error) = self.inner.write_all(&bytes[..count]) {
                self.error = Some(JsonStreamError::Io);
                return Err(error);
            }
            self.written += count;
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            if (self.cancelled)() {
                self.error = Some(JsonStreamError::Cancelled);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "JSON stream cancelled",
                ));
            }
            self.inner.flush().map_err(|error| {
                self.error = Some(JsonStreamError::Io);
                error
            })
        }
    }

    let mut bounded = BoundedStreamWriter {
        inner: writer,
        limits,
        written: 0,
        cancelled,
        error: None,
    };
    let result = match format {
        JsonFormat::Compact => serde_json::to_writer(&mut bounded, value),
        JsonFormat::Pretty => serde_json::to_writer_pretty(&mut bounded, value),
        JsonFormat::Canonical => serde_jcs::to_writer(&mut bounded, value),
    };
    if result.is_err() {
        return Err(bounded.error.unwrap_or(JsonStreamError::Io));
    }
    std::io::Write::flush(&mut bounded).map_err(|_| bounded.error.unwrap_or(JsonStreamError::Io))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPointerError {
    Syntax,
    Index,
    NotFound,
    WrongContainer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPathError {
    Syntax,
    Function,
    Limit,
    Input,
}

/// Failure to compile or evaluate a bounded JSON Schema Draft 2020-12
/// document. External references are denied by default; callers must provide
/// an explicit resolver before those can be enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonSchemaError {
    SchemaDocument(JsonValidationError),
    InstanceDocument(JsonValidationError),
    InvalidSchema(String),
    InvalidInstance,
    ResolverDocument {
        uri: String,
        error: JsonValidationError,
    },
    ExternalReferenceRequiresResolver(String),
}

/// One validation failure, including its RFC 6901 instance location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSchemaViolation {
    pub instance_pointer: String,
    pub message: String,
}

const MAX_JSON_SCHEMA_VIOLATIONS: usize = 1_000;

/// Validates one JSON instance against a Draft 2020-12 schema. Both documents
/// use the common JSON byte, depth, string, item, and numeric limits. Local
/// fragment references are supported by the schema engine; external `$ref`
/// and `$dynamicRef` values are rejected so validation never performs
/// filesystem or network I/O implicitly.
pub fn validate_json_schema(
    schema_document: &str,
    instance_document: &str,
    limits: JsonLimits,
) -> Result<Vec<JsonSchemaViolation>, JsonSchemaError> {
    validate_json_schema_with_resolver(schema_document, instance_document, limits, &[])
}

/// Validates with caller-supplied external schema resources. Each resolver
/// entry is a canonical URI and a JSON Schema document; references are only
/// resolved from this in-memory set, never from the filesystem or network.
pub fn validate_json_schema_with_resolver(
    schema_document: &str,
    instance_document: &str,
    limits: JsonLimits,
    resolver_documents: &[(String, String)],
) -> Result<Vec<JsonSchemaViolation>, JsonSchemaError> {
    validate_document_with_limits(schema_document, limits)
        .map_err(JsonSchemaError::SchemaDocument)?;
    validate_document_with_limits(instance_document, limits)
        .map_err(JsonSchemaError::InstanceDocument)?;

    let schema: serde_json::Value = serde_json::from_str(schema_document)
        .map(canonicalize_schema_object_order)
        .map_err(|_| JsonSchemaError::InvalidSchema("schema is not valid JSON".into()))?;
    let instance: serde_json::Value = serde_json::from_str(instance_document)
        .map(canonicalize_schema_object_order)
        .map_err(|_| JsonSchemaError::InvalidInstance)?;

    let mut resolver_values = Vec::with_capacity(resolver_documents.len());
    for (uri, document) in resolver_documents {
        validate_document_with_limits(document, limits).map_err(|error| {
            JsonSchemaError::ResolverDocument {
                uri: uri.clone(),
                error,
            }
        })?;
        let value = serde_json::from_str::<serde_json::Value>(document)
            .map(canonicalize_schema_object_order)
            .map_err(|_| {
                JsonSchemaError::InvalidSchema(format!(
                    "resolver resource `{uri}` is not valid JSON"
                ))
            })?;
        if !jsonschema::draft202012::meta::is_valid(&value) {
            return Err(JsonSchemaError::InvalidSchema(format!(
                "resolver resource `{uri}` does not conform to the Draft 2020-12 meta-schema"
            )));
        }
        resolver_values.push((uri.clone(), value));
    }
    resolver_values.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some(pair) = resolver_values
        .windows(2)
        .find(|pair| pair[0].0 == pair[1].0)
    {
        return Err(JsonSchemaError::InvalidSchema(format!(
            "duplicate resolver URI `{}`",
            pair[0].0
        )));
    }

    let mut known_resource_uris: Vec<String> =
        resolver_values.iter().map(|(uri, _)| uri.clone()).collect();
    collect_schema_resource_uris(&schema, None, &mut known_resource_uris);
    for (uri, value) in &resolver_values {
        collect_schema_resource_uris(value, Some(uri), &mut known_resource_uris);
    }
    known_resource_uris.sort();
    known_resource_uris.dedup();
    let resolver_uris: Vec<_> = known_resource_uris.iter().map(String::as_str).collect();
    reject_external_schema_references(&schema, &resolver_uris, None)?;
    for (uri, value) in &resolver_values {
        reject_external_schema_references(value, &resolver_uris, Some(uri))?;
    }
    if !jsonschema::draft202012::meta::is_valid(&schema) {
        return Err(JsonSchemaError::InvalidSchema(
            "schema does not conform to the Draft 2020-12 meta-schema".into(),
        ));
    }

    let registry = jsonschema::Registry::new()
        .extend(resolver_values.iter().map(|(uri, value)| {
            (
                uri.as_str(),
                jsonschema::Resource::from_contents(value.clone()),
            )
        }))
        .and_then(jsonschema::RegistryBuilder::prepare)
        .map_err(|error| JsonSchemaError::InvalidSchema(error.to_string()))?;
    let validate_formats = schema
        .get("$schema")
        .and_then(serde_json::Value::as_str)
        .and_then(|dialect_uri| {
            resolver_values
                .iter()
                .find(|(uri, _)| uri == dialect_uri)
                .and_then(|(_, dialect)| dialect.get("$vocabulary"))
        })
        .and_then(|vocabulary| {
            vocabulary.get("https://json-schema.org/draft/2020-12/vocab/format-assertion")
        })
        .is_some();
    let schema_options = match schema.get("$schema").and_then(serde_json::Value::as_str) {
        Some(dialect_uri) if !is_bundled_json_schema_resource(dialect_uri) => jsonschema::options(),
        _ => jsonschema::draft202012::options(),
    };
    let validator = schema_options
        .with_registry(&registry)
        .should_validate_formats(validate_formats)
        .build(&schema)
        .map_err(|error| JsonSchemaError::InvalidSchema(error.to_string()))?;
    Ok(validator
        .iter_errors(&instance)
        .take(MAX_JSON_SCHEMA_VIOLATIONS)
        .map(|error| JsonSchemaViolation {
            instance_pointer: error.instance_path().to_string(),
            message: error.to_string(),
        })
        .collect())
}

fn collect_schema_resource_uris(
    schema: &serde_json::Value,
    inherited_base: Option<&str>,
    uris: &mut Vec<String>,
) {
    match schema {
        serde_json::Value::Object(properties) => {
            let base = properties
                .get("$id")
                .and_then(serde_json::Value::as_str)
                .and_then(|id| resolve_schema_uri(id, inherited_base))
                .or_else(|| inherited_base.map(str::to_owned));
            if properties.contains_key("$id") {
                if let Some(base) = &base {
                    uris.push(base.clone());
                }
            }
            for value in properties.values() {
                collect_schema_resource_uris(value, base.as_deref(), uris);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_schema_resource_uris(item, inherited_base, uris);
            }
        }
        _ => {}
    }
}

// JSON objects are unordered by definition. Workspace feature unification can
// enable serde_json's insertion-ordered Map, whose Value equality is then
// order-sensitive. Sort recursively before schema compilation so `const`,
// `enum`, and `uniqueItems` keep JSON Schema's order-independent object rules.
fn canonicalize_schema_object_order(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(properties) => {
            let mut entries: Vec<_> = properties.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut normalized = serde_json::Map::new();
            for (key, value) in entries {
                normalized.insert(key, canonicalize_schema_object_order(value));
            }
            serde_json::Value::Object(normalized)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(canonicalize_schema_object_order)
                .collect(),
        ),
        scalar => scalar,
    }
}

fn reject_external_schema_references(
    schema: &serde_json::Value,
    resolver_uris: &[&str],
    inherited_base: Option<&str>,
) -> Result<(), JsonSchemaError> {
    match schema {
        serde_json::Value::Object(properties) => {
            let base = properties
                .get("$id")
                .and_then(serde_json::Value::as_str)
                .and_then(|id| resolve_schema_uri(id, inherited_base))
                .or_else(|| inherited_base.map(str::to_owned));
            for (name, value) in properties {
                if name == "$schema" {
                    if let Some(uri) = value.as_str() {
                        let resource_uri = resolve_schema_uri(uri, base.as_deref());
                        if let Some(resource_uri) = resource_uri.as_deref()
                            && !is_bundled_json_schema_resource(resource_uri)
                            && !resolver_uris.contains(&resource_uri)
                        {
                            return Err(JsonSchemaError::ExternalReferenceRequiresResolver(
                                uri.to_owned(),
                            ));
                        }
                    }
                }
                if matches!(name.as_str(), "$ref" | "$dynamicRef") {
                    let Some(reference) = value.as_str() else {
                        continue;
                    };
                    let resource_uri = if reference.starts_with('#') {
                        None
                    } else {
                        resolve_schema_uri(reference, base.as_deref())
                    };
                    if let Some(resource_uri) = resource_uri.as_deref()
                        && !is_bundled_json_schema_resource(resource_uri)
                        && !resolver_uris.contains(&resource_uri)
                    {
                        return Err(JsonSchemaError::ExternalReferenceRequiresResolver(
                            reference.to_owned(),
                        ));
                    }
                }
                reject_external_schema_references(value, resolver_uris, base.as_deref())?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                reject_external_schema_references(item, resolver_uris, inherited_base)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn resolve_schema_uri(reference: &str, base: Option<&str>) -> Option<String> {
    let fragmentless_reference = reference.split('#').next().unwrap_or_default();
    if has_uri_scheme(fragmentless_reference) {
        return Some(fragmentless_reference.to_owned());
    }
    let base = base?;
    let base = url::Url::parse(base).ok()?;
    base.join(reference).ok().map(|resolved| {
        resolved
            .to_string()
            .split('#')
            .next()
            .unwrap_or_default()
            .to_owned()
    })
}

fn has_uri_scheme(uri: &str) -> bool {
    let Some((scheme, _)) = uri.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn is_bundled_json_schema_resource(uri: &str) -> bool {
    matches!(
        uri,
        "https://json-schema.org/draft/2020-12/schema"
            | "https://json-schema.org/draft/2020-12/meta/applicator"
            | "https://json-schema.org/draft/2020-12/meta/content"
            | "https://json-schema.org/draft/2020-12/meta/core"
            | "https://json-schema.org/draft/2020-12/meta/format-annotation"
            | "https://json-schema.org/draft/2020-12/meta/format-assertion"
            | "https://json-schema.org/draft/2020-12/meta/meta-data"
            | "https://json-schema.org/draft/2020-12/meta/unevaluated"
            | "https://json-schema.org/draft/2020-12/meta/validation"
    )
}

/// JSON5's ordinary value model. Numbers use binary64 because JSON5 defines
/// them as IEEE-754 values and permits infinities and NaN, which
/// `serde_json::Value` cannot represent.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum Json5Value {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Json5Value>),
    Object(BTreeMap<String, Json5Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Json5Error {
    Syntax,
    Limit,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Json5ParsedValue {
    Null(()),
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Json5ParsedValue>),
    Object(BTreeMap<String, Json5ParsedValue>),
}

impl From<Json5ParsedValue> for Json5Value {
    fn from(value: Json5ParsedValue) -> Self {
        match value {
            Json5ParsedValue::Null(()) => Self::Null,
            Json5ParsedValue::Boolean(value) => Self::Boolean(value),
            Json5ParsedValue::Number(value) => Self::Number(value),
            Json5ParsedValue::String(value) => Self::String(value),
            Json5ParsedValue::Array(values) => {
                Self::Array(values.into_iter().map(Self::from).collect())
            }
            Json5ParsedValue::Object(properties) => Self::Object(
                properties
                    .into_iter()
                    .map(|(key, value)| (key, Self::from(value)))
                    .collect(),
            ),
        }
    }
}

/// Parses one JSON5 document under the shared document, depth, string,
/// container, and number ceilings. Duplicate object names use last-value-wins
/// behavior, as JSON5 specifies for object initializers.
pub fn parse_json5_with_limits(
    document: &str,
    limits: JsonLimits,
) -> Result<Json5Value, Json5Error> {
    validate_limits(limits).map_err(|_| Json5Error::Limit)?;
    if document.len() > limits.max_document_bytes {
        return Err(Json5Error::Limit);
    }
    validate_json5_lexical_limits(document, limits)?;
    let parsed = json5::from_str::<Json5ParsedValue>(document).map_err(|_| Json5Error::Syntax)?;
    let value = Json5Value::from(parsed);
    validate_json5_value(&value, limits, 0).map_err(|_| Json5Error::Limit)?;
    Ok(value)
}

/// Reject oversized strings and numeric tokens before the JSON5 deserializer
/// allocates decoded strings or parses numeric values. Syntax validation stays
/// with the pinned JSON5 parser; this pass only enforces resource ceilings.
fn validate_json5_lexical_limits(document: &str, limits: JsonLimits) -> Result<(), Json5Error> {
    #[derive(Default)]
    struct ContainerScan {
        commas: usize,
        has_content: bool,
        trailing_comma: bool,
    }

    let bytes = document.as_bytes();
    let mut i = 0;
    let mut containers: Vec<ContainerScan> = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r') {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if matches!(bytes[i], b'\'' | b'"') {
            if let Some(container) = containers.last_mut() {
                container.has_content = true;
                container.trailing_comma = false;
            }
            let quote = bytes[i];
            i += 1;
            let mut decoded_bytes = 0usize;
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i += 1;
                    if i >= bytes.len() {
                        break;
                    }
                    match bytes[i] {
                        b'\r' => {
                            i += 1;
                            if bytes.get(i) == Some(&b'\n') {
                                i += 1;
                            }
                        }
                        b'\n' => i += 1,
                        b'x' => {
                            decoded_bytes += 1;
                            i = (i + 3).min(bytes.len());
                        }
                        b'u' => {
                            if i + 4 < bytes.len() {
                                let raw = &document[i + 1..i + 5];
                                if let Ok(code) = u32::from_str_radix(raw, 16) {
                                    if (0xD800..=0xDBFF).contains(&code)
                                        && bytes.get(i + 5) == Some(&b'\\')
                                        && bytes.get(i + 6) == Some(&b'u')
                                        && i + 10 < bytes.len()
                                        && let Ok(low) =
                                            u32::from_str_radix(&document[i + 7..i + 11], 16)
                                        && (0xDC00..=0xDFFF).contains(&low)
                                    {
                                        decoded_bytes = decoded_bytes.saturating_add(4);
                                        i += 11;
                                        continue;
                                    }
                                    decoded_bytes = decoded_bytes.saturating_add(
                                        char::from_u32(code).map(char::len_utf8).unwrap_or(3),
                                    );
                                }
                                i += 5;
                            } else {
                                i = bytes.len();
                            }
                        }
                        _ => {
                            decoded_bytes += 1;
                            i += 1;
                        }
                    }
                } else {
                    let char_len = document[i..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1);
                    decoded_bytes = decoded_bytes.saturating_add(char_len);
                    i += char_len;
                }
                if decoded_bytes > limits.max_string_bytes {
                    return Err(Json5Error::Limit);
                }
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        let starts_number = bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
            || (matches!(bytes[i], b'+' | b'-')
                && bytes.get(i + 1).is_some_and(|next| {
                    next.is_ascii_digit() || *next == b'.' || *next == b'I' || *next == b'N'
                }));
        if starts_number {
            if let Some(container) = containers.last_mut() {
                container.has_content = true;
                container.trailing_comma = false;
            }
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'+' | b'-' | b'.'))
            {
                i += 1;
            }
            if i - start > limits.max_number_bytes {
                return Err(Json5Error::Limit);
            }
            continue;
        }
        match bytes[i] {
            b'[' | b'{' => {
                if let Some(parent) = containers.last_mut() {
                    parent.has_content = true;
                    parent.trailing_comma = false;
                }
                containers.push(ContainerScan::default());
                if containers.len() > limits.max_nesting_depth {
                    return Err(Json5Error::Limit);
                }
            }
            b']' | b'}' => {
                if let Some(container) = containers.pop() {
                    let items = container.commas
                        + usize::from(container.has_content && !container.trailing_comma);
                    if items > limits.max_container_items {
                        return Err(Json5Error::Limit);
                    }
                }
            }
            b',' => {
                if let Some(container) = containers.last_mut() {
                    container.commas = container.commas.saturating_add(1);
                    container.trailing_comma = true;
                }
            }
            byte if !byte.is_ascii_whitespace() => {
                if let Some(container) = containers.last_mut() {
                    container.has_content = true;
                    container.trailing_comma = false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// Parses one JSON5 document with the strict JSON API's shared default limits.
pub fn parse_json5(document: &str) -> Result<Json5Value, Json5Error> {
    parse_json5_with_limits(document, JsonLimits::default())
}

fn validate_json5_value(value: &Json5Value, limits: JsonLimits, depth: usize) -> Result<(), ()> {
    if depth > limits.max_nesting_depth {
        return Err(());
    }
    match value {
        Json5Value::String(value) if value.len() > limits.max_string_bytes => Err(()),
        Json5Value::Array(values) => {
            if values.len() > limits.max_container_items {
                return Err(());
            }
            values
                .iter()
                .try_for_each(|value| validate_json5_value(value, limits, depth + 1))
        }
        Json5Value::Object(properties) => {
            if properties.len() > limits.max_container_items
                || properties
                    .keys()
                    .any(|key| key.len() > limits.max_string_bytes)
            {
                return Err(());
            }
            properties
                .values()
                .try_for_each(|value| validate_json5_value(value, limits, depth + 1))
        }
        _ => Ok(()),
    }
}

/// Evaluates an RFC 9535 JSONPath query and materializes its ordered result
/// nodelist. Query bytes, selector/filter steps, and every intermediate result
/// list are bounded before the evaluator grows them beyond the configured limit.
pub fn query_jsonpath(
    document: &serde_json::Value,
    query: &str,
) -> Result<Vec<serde_json::Value>, JsonPathError> {
    if query.len() > MAX_JSONPATH_QUERY_BYTES {
        return Err(JsonPathError::Limit);
    }
    let path = jsonpath_rfc9535::JsonPath::parse(query).map_err(|error| match error {
        jsonpath_rfc9535::Error::UnknownFunction { .. }
        | jsonpath_rfc9535::Error::FunctionArity { .. }
        | jsonpath_rfc9535::Error::IllTyped { .. } => JsonPathError::Function,
        _ => JsonPathError::Syntax,
    })?;
    let nodes = path
        .query_values_bounded(document, MAX_JSONPATH_STEPS, MAX_JSONPATH_RESULTS)
        .map_err(|error| match error {
            jsonpath_rfc9535::Error::UnknownFunction { .. }
            | jsonpath_rfc9535::Error::FunctionArity { .. }
            | jsonpath_rfc9535::Error::IllTyped { .. } => JsonPathError::Function,
            jsonpath_rfc9535::Error::EvaluationLimit | jsonpath_rfc9535::Error::ResultLimit => {
                JsonPathError::Limit
            }
            _ => JsonPathError::Syntax,
        })?;
    let mut remaining_bytes = MAX_JSONPATH_RESULT_BYTES;
    for node in &nodes {
        if !charge_jsonpath_value_bytes(node, &mut remaining_bytes) {
            return Err(JsonPathError::Limit);
        }
    }
    Ok(nodes.into_iter().cloned().collect())
}

fn charge_jsonpath_value_bytes(value: &serde_json::Value, remaining: &mut usize) -> bool {
    fn charge(remaining: &mut usize, bytes: usize) -> bool {
        if bytes > *remaining {
            false
        } else {
            *remaining -= bytes;
            true
        }
    }
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) => charge(remaining, 5),
        serde_json::Value::Number(number) => charge(remaining, number.to_string().len()),
        serde_json::Value::String(text) => charge(remaining, text.len().saturating_add(2)),
        serde_json::Value::Array(values) => {
            if !charge(
                remaining,
                2usize.saturating_add(values.len().saturating_sub(1)),
            ) {
                return false;
            }
            values
                .iter()
                .all(|value| charge_jsonpath_value_bytes(value, remaining))
        }
        serde_json::Value::Object(values) => {
            if !charge(
                remaining,
                2usize.saturating_add(values.len().saturating_sub(1)),
            ) {
                return false;
            }
            values.iter().all(|(key, value)| {
                charge(remaining, key.len().saturating_add(3))
                    && charge_jsonpath_value_bytes(value, remaining)
            })
        }
    }
}

/// Validates caller-configured limits against the shared hard ceilings.
pub fn validate_limits(limits: JsonLimits) -> Result<(), JsonValidationError> {
    if limits.max_document_bytes > HARD_MAX_DOCUMENT_BYTES
        || limits.max_nesting_depth > HARD_MAX_NESTING_DEPTH
        || limits.max_string_bytes > HARD_MAX_STRING_BYTES
        || limits.max_container_items > HARD_MAX_CONTAINER_ITEMS
        || limits.max_number_bytes > HARD_MAX_NUMBER_BYTES
    {
        Err(JsonValidationError::InvalidLimits)
    } else {
        Ok(())
    }
}

/// Serializes a validated JSON value without allowing the output buffer to
/// grow beyond the caller's document-byte limit.
pub fn serialize_value_with_limits(
    value: &serde_json::Value,
    limits: JsonLimits,
) -> Result<String, JsonValidationError> {
    serialize_value_with_limits_and_format(value, limits, JsonFormat::Compact)
}

/// Serializes a validated JSON value with two-space indentation while keeping
/// the same output-byte, depth, string, collection, and number limits as the
/// compact serializer.
pub fn serialize_pretty_value_with_limits(
    value: &serde_json::Value,
    limits: JsonLimits,
) -> Result<String, JsonValidationError> {
    serialize_value_with_limits_and_format(value, limits, JsonFormat::Pretty)
}

/// Serializes a value using RFC 8785 JCS, rejecting numbers when converting
/// them to the I-JSON binary64 domain would change their exact decimal value.
pub fn canonicalize_value_with_limits(
    value: &serde_json::Value,
    limits: JsonLimits,
) -> Result<String, JsonValidationError> {
    serialize_value_with_limits_and_format(value, limits, JsonFormat::Canonical)
}

#[derive(Clone, Copy)]
enum JsonFormat {
    Compact,
    Pretty,
    Canonical,
}

fn serialize_value_with_limits_and_format(
    value: &serde_json::Value,
    limits: JsonLimits,
    format: JsonFormat,
) -> Result<String, JsonValidationError> {
    validate_limits(limits)?;
    validate_value_limits(value, limits, 0)?;
    if matches!(format, JsonFormat::Canonical) {
        validate_canonical_numbers(value)?;
    }

    struct BoundedWriter {
        bytes: Vec<u8>,
        max_bytes: usize,
        exceeded: bool,
    }

    impl std::io::Write for BoundedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().saturating_add(bytes.len()) > self.max_bytes {
                self.exceeded = true;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "JSON output exceeds its configured byte limit",
                ));
            }
            self.bytes
                .try_reserve_exact(bytes.len())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        max_bytes: limits.max_document_bytes,
        exceeded: false,
    };
    let result = match format {
        JsonFormat::Compact => serde_json::to_writer(&mut writer, value),
        JsonFormat::Pretty => serde_json::to_writer_pretty(&mut writer, value),
        JsonFormat::Canonical => serde_jcs::to_writer(&mut writer, value),
    };
    if result.is_err() {
        return Err(if writer.exceeded {
            JsonValidationError::DocumentTooLarge
        } else {
            JsonValidationError::InvalidDocument
        });
    }
    String::from_utf8(writer.bytes).map_err(|_| JsonValidationError::InvalidDocument)
}

fn validate_canonical_numbers(value: &serde_json::Value) -> Result<(), JsonValidationError> {
    match value {
        serde_json::Value::Number(number) => {
            let source = number.to_string();
            let canonical = serde_jcs::to_string(number)
                .map_err(|_| JsonValidationError::NumberOutsideCanonicalDomain)?;
            if decimal_identity(&source) != decimal_identity(&canonical) {
                return Err(JsonValidationError::NumberOutsideCanonicalDomain);
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                validate_canonical_numbers(item)?;
            }
            Ok(())
        }
        serde_json::Value::Object(properties) => {
            for item in properties.values() {
                validate_canonical_numbers(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Returns a normalized exact-decimal identity: sign, significant digits, and
/// base-10 exponent. This lets canonicalization detect binary64 rounding
/// without rejecting equivalent spellings such as `1.0` and `1e0`.
fn decimal_identity(value: &str) -> Option<(bool, String, i64)> {
    let (mantissa, exponent) = match value.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (value, 0),
    };
    let (negative, mantissa) = mantissa
        .strip_prefix('-')
        .map_or((false, mantissa), |unsigned| (true, unsigned));
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut digits = format!("{whole}{fraction}");
    let mut exponent = exponent.checked_sub(i64::try_from(fraction.len()).ok()?)?;
    let first_nonzero = digits.find(|digit| digit != '0');
    let Some(first_nonzero) = first_nonzero else {
        return Some((false, "0".into(), 0));
    };
    digits.drain(..first_nonzero);
    while digits.ends_with('0') {
        digits.pop();
        exponent = exponent.checked_add(1)?;
    }
    Some((negative, digits, exponent))
}

fn validate_value_limits(
    value: &serde_json::Value,
    limits: JsonLimits,
    depth: usize,
) -> Result<(), JsonValidationError> {
    if depth > limits.max_nesting_depth {
        return Err(JsonValidationError::NestingTooDeep);
    }
    match value {
        serde_json::Value::String(value) if value.len() > limits.max_string_bytes => {
            Err(JsonValidationError::StringTooLarge)
        }
        serde_json::Value::Number(value) if value.to_string().len() > limits.max_number_bytes => {
            Err(JsonValidationError::NumberTooLarge)
        }
        serde_json::Value::Array(values) => {
            if values.len() > limits.max_container_items {
                return Err(JsonValidationError::TooManyContainerItems);
            }
            for value in values {
                validate_value_limits(value, limits, depth + 1)?;
            }
            Ok(())
        }
        serde_json::Value::Object(properties) => {
            if properties.len() > limits.max_container_items {
                return Err(JsonValidationError::TooManyContainerItems);
            }
            for (key, value) in properties {
                if key.len() > limits.max_string_bytes {
                    return Err(JsonValidationError::StringTooLarge);
                }
                validate_value_limits(value, limits, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Resolves the Unicode string form of an RFC 6901 JSON Pointer.
///
/// URI-fragment decoding is intentionally left to callers. Object names are
/// compared as exact strings and array indexes must use canonical decimal
/// notation.
pub fn lookup_pointer<'a>(
    document: &'a serde_json::Value,
    pointer: &str,
) -> Result<&'a serde_json::Value, JsonPointerError> {
    if pointer.is_empty() {
        return Ok(document);
    }
    if !pointer.starts_with('/') {
        return Err(JsonPointerError::Syntax);
    }

    let mut current = document;
    for encoded_token in pointer[1..].split('/') {
        let mut token = String::with_capacity(encoded_token.len());
        let mut chars = encoded_token.chars();
        while let Some(ch) = chars.next() {
            if ch != '~' {
                token.push(ch);
                continue;
            }
            match chars.next() {
                Some('0') => token.push('~'),
                Some('1') => token.push('/'),
                _ => return Err(JsonPointerError::Syntax),
            }
        }

        current = match current {
            serde_json::Value::Object(properties) => {
                properties.get(&token).ok_or(JsonPointerError::NotFound)?
            }
            serde_json::Value::Array(items) => {
                if token.is_empty()
                    || (token.len() > 1 && token.starts_with('0'))
                    || !token.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(JsonPointerError::Index);
                }
                let index = token
                    .parse::<usize>()
                    .map_err(|_| JsonPointerError::Index)?;
                items.get(index).ok_or(JsonPointerError::NotFound)?
            }
            _ => return Err(JsonPointerError::WrongContainer),
        };
    }
    Ok(current)
}

/// Rejects oversized documents, excessive nesting, and duplicate object keys.
/// Keys are compared after JSON escape decoding, so `"x"` and `"\\u0078"`
/// are considered the same key.
pub fn validate_document(document: &str) -> Result<(), JsonValidationError> {
    validate_document_with_limits(document, JsonLimits::default())
}

pub fn validate_document_with_limits(
    document: &str,
    limits: JsonLimits,
) -> Result<(), JsonValidationError> {
    validate_limits(limits)?;
    if document.len() > limits.max_document_bytes {
        return Err(JsonValidationError::DocumentTooLarge);
    }

    struct Scanner<'a> {
        bytes: &'a [u8],
        cursor: usize,
    }

    impl Scanner<'_> {
        fn whitespace(&mut self) {
            while self
                .bytes
                .get(self.cursor)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.cursor += 1;
            }
        }

        fn string(&mut self, max_bytes: usize) -> Result<String, JsonValidationError> {
            let start = self.cursor;
            if self.bytes.get(self.cursor) != Some(&b'"') {
                return Err(JsonValidationError::InvalidDocument);
            }
            self.cursor += 1;
            let mut escaped = false;
            while let Some(&byte) = self.bytes.get(self.cursor) {
                self.cursor += 1;
                if escaped {
                    escaped = false;
                    continue;
                }
                match byte {
                    b'\\' => escaped = true,
                    b'"' => {
                        // JSON escapes can use six source bytes for one UTF-8
                        // byte (`\\u00XX`), so this bound rejects strings that
                        // cannot fit without decoding/allocating them first.
                        let encoded_len = self.cursor - start - 2;
                        if encoded_len > max_bytes.saturating_mul(6) {
                            return Err(JsonValidationError::StringTooLarge);
                        }
                        let text = std::str::from_utf8(&self.bytes[start..self.cursor])
                            .map_err(|_| JsonValidationError::InvalidDocument)?;
                        let decoded: String = serde_json::from_str(text)
                            .map_err(|_| JsonValidationError::InvalidDocument)?;
                        if decoded.len() > max_bytes {
                            return Err(JsonValidationError::StringTooLarge);
                        }
                        return Ok(decoded);
                    }
                    _ => {}
                }
            }
            Err(JsonValidationError::InvalidDocument)
        }

        fn value(&mut self, depth: usize, limits: JsonLimits) -> Result<(), JsonValidationError> {
            if depth > limits.max_nesting_depth {
                return Err(JsonValidationError::NestingTooDeep);
            }
            self.whitespace();
            match self.bytes.get(self.cursor).copied() {
                Some(b'{') => {
                    self.cursor += 1;
                    self.whitespace();
                    let mut keys = HashSet::new();
                    if self.bytes.get(self.cursor) == Some(&b'}') {
                        self.cursor += 1;
                        return Ok(());
                    }
                    loop {
                        self.whitespace();
                        if keys.len() >= limits.max_container_items {
                            return Err(JsonValidationError::TooManyContainerItems);
                        }
                        if !keys.insert(self.string(limits.max_string_bytes)?) {
                            return Err(JsonValidationError::InvalidDocument);
                        }
                        self.whitespace();
                        if self.bytes.get(self.cursor) != Some(&b':') {
                            return Err(JsonValidationError::InvalidDocument);
                        }
                        self.cursor += 1;
                        self.value(depth + 1, limits)?;
                        self.whitespace();
                        match self.bytes.get(self.cursor) {
                            Some(b',') => self.cursor += 1,
                            Some(b'}') => {
                                self.cursor += 1;
                                return Ok(());
                            }
                            _ => return Err(JsonValidationError::InvalidDocument),
                        }
                    }
                }
                Some(b'[') => {
                    self.cursor += 1;
                    self.whitespace();
                    if self.bytes.get(self.cursor) == Some(&b']') {
                        self.cursor += 1;
                        return Ok(());
                    }
                    let mut items = 0usize;
                    loop {
                        if items >= limits.max_container_items {
                            return Err(JsonValidationError::TooManyContainerItems);
                        }
                        items += 1;
                        self.value(depth + 1, limits)?;
                        self.whitespace();
                        match self.bytes.get(self.cursor) {
                            Some(b',') => self.cursor += 1,
                            Some(b']') => {
                                self.cursor += 1;
                                return Ok(());
                            }
                            _ => return Err(JsonValidationError::InvalidDocument),
                        }
                    }
                }
                Some(b'"') => {
                    self.string(limits.max_string_bytes)?;
                    Ok(())
                }
                Some(_) => {
                    let start = self.cursor;
                    while self.bytes.get(self.cursor).is_some_and(|byte| {
                        !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}')
                    }) {
                        self.cursor += 1;
                    }
                    let token = &self.bytes[start..self.cursor];
                    if token
                        .first()
                        .is_some_and(|b| b.is_ascii_digit() || *b == b'-')
                        && token.len() > limits.max_number_bytes
                    {
                        return Err(JsonValidationError::NumberTooLarge);
                    }
                    Ok(())
                }
                None => Err(JsonValidationError::InvalidDocument),
            }
        }
    }

    let mut scanner = Scanner {
        bytes: document.as_bytes(),
        cursor: 0,
    };
    scanner.value(0, limits)?;
    scanner.whitespace();
    (scanner.cursor == scanner.bytes.len())
        .then_some(())
        .ok_or(JsonValidationError::InvalidDocument)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FragmentedReader<'a> {
        bytes: &'a [u8],
        cursor: usize,
        max_read: usize,
    }

    impl std::io::Read for FragmentedReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output
                .len()
                .min(self.max_read)
                .min(self.bytes.len().saturating_sub(self.cursor));
            output[..count].copy_from_slice(&self.bytes[self.cursor..self.cursor + count]);
            self.cursor += count;
            Ok(count)
        }
    }

    #[derive(Default)]
    struct RecordingWriter {
        bytes: Vec<u8>,
        largest_write: usize,
        fail_after: Option<usize>,
    }

    impl std::io::Write for RecordingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.largest_write = self.largest_write.max(bytes.len());
            let count = self.fail_after.map_or(bytes.len(), |limit| {
                bytes.len().min(limit.saturating_sub(self.bytes.len()))
            });
            self.bytes.extend_from_slice(&bytes[..count]);
            if count == 0 {
                return Err(std::io::Error::other("sink failed"));
            }
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn streaming_reader_handles_utf8_escapes_numbers_and_delimiters_across_chunks() {
        let document = r#"{"text":"café\u0021","items":[12,3.5,true],"exact":123456789012345678901234567890,"fraction":0.10000000000000001}"#;
        let mut reader = FragmentedReader {
            bytes: document.as_bytes(),
            cursor: 0,
            max_read: 1,
        };
        let value = parse_reader_with_limits(&mut reader, JsonLimits::default(), || false).unwrap();
        assert_eq!(value["text"], "café!");
        assert_eq!(value["items"][0], 12);
        assert_eq!(value["items"][1], 3.5);
        assert_eq!(value["exact"].to_string(), "123456789012345678901234567890");
        assert_eq!(value["fraction"].to_string(), "0.10000000000000001");
        assert_eq!(reader.cursor, document.len());
    }

    #[test]
    fn streaming_reader_rejects_duplicate_decoded_keys_without_buffering_source() {
        let mut reader = FragmentedReader {
            bytes: br#"{"x":1,"\u0078":2}"#,
            cursor: 0,
            max_read: 2,
        };
        assert_eq!(
            parse_reader_with_limits(&mut reader, JsonLimits::default(), || false),
            Err(JsonStreamError::Validation(
                JsonValidationError::InvalidDocument
            ))
        );
        assert!(reader.cursor < br#"{"x":1,"\u0078":2}"#.len());
    }

    #[test]
    fn streaming_reader_stops_one_byte_after_the_configured_document_limit() {
        let mut reader = FragmentedReader {
            bytes: br#"{"a":1}"#,
            cursor: 0,
            max_read: JSON_STREAM_CHUNK_BYTES,
        };
        let limits = JsonLimits {
            max_document_bytes: 6,
            ..JsonLimits::default()
        };
        assert_eq!(
            parse_reader_with_limits(&mut reader, limits, || false),
            Err(JsonStreamError::Validation(
                JsonValidationError::DocumentTooLarge
            ))
        );
        assert_eq!(reader.cursor, 7);
    }

    #[test]
    fn streaming_reader_state_rejects_oversized_chunks_and_cancelled_input() {
        let mut fragmented = JsonStreamReader::new(JsonLimits::default()).unwrap();
        fragmented.push_chunk(br#"{"text":"caf"#).unwrap();
        fragmented.push_chunk("é".as_bytes()).unwrap();
        fragmented.push_chunk(br#"\u0021","items":[1,2]}"#).unwrap();
        let parsed = fragmented.finish(|| false).unwrap();
        assert_eq!(parsed["text"], "café!");
        assert_eq!(parsed["items"][1], 2);

        let mut reader = JsonStreamReader::new(JsonLimits::default()).unwrap();
        assert_eq!(
            reader.push_chunk(&vec![b' '; JSON_STREAM_CHUNK_BYTES + 1]),
            Err(JsonStreamError::ChunkTooLarge)
        );
        reader.push_chunk(br#"{"unfinished":"#).unwrap();
        reader.cancel();
        assert_eq!(reader.push_chunk(b"null}"), Err(JsonStreamError::Cancelled));
        assert_eq!(reader.finish(|| false), Err(JsonStreamError::Cancelled));

        let mut incomplete = JsonStreamReader::new(JsonLimits::default()).unwrap();
        incomplete.push_chunk(br#"{"a": "#).unwrap();
        assert_eq!(
            incomplete.finish(|| false),
            Err(JsonStreamError::Validation(
                JsonValidationError::InvalidDocument
            ))
        );
    }

    #[test]
    fn streaming_reader_handles_parse_and_cancel_through_the_shared_registry() {
        let handle = stream_reader_create(JsonLimits::default()).unwrap();
        stream_reader_push(handle, br#"{"items":["caf"#).unwrap();
        stream_reader_push(handle, "é\"]}".as_bytes()).unwrap();
        let parsed = stream_reader_finish(handle).unwrap();
        assert_eq!(parsed["items"][0], "café");
        assert_eq!(
            stream_reader_finish(handle),
            Err(JsonStreamError::InvalidState)
        );

        let cancelled = stream_reader_create(JsonLimits::default()).unwrap();
        stream_reader_push(cancelled, br#"{"incomplete":"#).unwrap();
        stream_reader_cancel(cancelled).unwrap();
        assert_eq!(
            stream_reader_finish(cancelled),
            Err(JsonStreamError::InvalidState)
        );
    }

    #[test]
    fn streaming_writer_pulls_bounded_utf8_chunks_without_materializing_document() {
        let value = serde_json::json!({
            "text": "café 😀",
            "items": [1, true, null, {"nested": "value"}]
        });
        let mut writer = JsonStreamWriter::new(
            value.clone(),
            JsonLimits::default(),
            JsonStreamFormat::Compact,
        )
        .unwrap();
        let mut output = String::new();
        while let Some(chunk) = writer.next_chunk(7).unwrap() {
            assert!(chunk.len() <= 7);
            output.push_str(&chunk);
        }
        assert_eq!(output, serde_json::to_string(&value).unwrap());
        assert_eq!(writer.next_chunk(7).unwrap(), None);
    }

    #[test]
    fn streaming_writer_preserves_format_and_reports_limit_errors_before_success() {
        let value = serde_json::json!({"a": [1, 2, 3]});
        let mut pretty = JsonStreamWriter::new(
            value.clone(),
            JsonLimits::default(),
            JsonStreamFormat::Pretty,
        )
        .unwrap();
        let mut output = String::new();
        while let Some(chunk) = pretty.next_chunk(13).unwrap() {
            output.push_str(&chunk);
        }
        assert_eq!(output, serde_json::to_string_pretty(&value).unwrap());

        let limits = JsonLimits {
            max_document_bytes: 4,
            ..JsonLimits::default()
        };
        let mut too_large =
            JsonStreamWriter::new(value, limits, JsonStreamFormat::Compact).unwrap();
        assert_eq!(
            too_large.next_chunk(4),
            Err(JsonStreamError::Validation(
                JsonValidationError::DocumentTooLarge
            ))
        );
        assert_eq!(too_large.next_chunk(4), Ok(None));
    }

    #[test]
    fn streaming_writer_cancellation_and_registry_state_are_observable() {
        let value = serde_json::json!({"text": "x".repeat(JSON_STREAM_CHUNK_BYTES * 3)});
        let handle =
            stream_writer_create(value, JsonLimits::default(), JsonStreamFormat::Compact).unwrap();
        assert_eq!(
            stream_writer_next(handle, 0),
            Err(JsonStreamError::ChunkTooLarge)
        );
        stream_writer_cancel(handle).unwrap();
        assert_eq!(
            stream_writer_next(handle, 16),
            Err(JsonStreamError::InvalidState)
        );
        assert_eq!(
            stream_writer_cancel(handle),
            Err(JsonStreamError::InvalidState)
        );
    }

    #[test]
    fn streaming_writer_registry_releases_handles_after_end() {
        let handle = stream_writer_create(
            serde_json::json!({"ok": true}),
            JsonLimits::default(),
            JsonStreamFormat::Compact,
        )
        .unwrap();
        assert_eq!(stream_writer_next_status(handle, 64), 0);
        assert_eq!(stream_writer_take(handle).unwrap(), r#"{"ok":true}"#);
        assert_eq!(stream_writer_next_status(handle, 64), 1);
        assert_eq!(stream_writer_next_status(handle, 64), 3);
    }

    #[test]
    fn streaming_reader_cancellation_and_io_failure_never_publish_a_value() {
        let document = br#"{"a":1}"#;
        let mut reader = FragmentedReader {
            bytes: document,
            cursor: 0,
            max_read: 1,
        };
        let mut checks = 0;
        assert_eq!(
            parse_reader_with_limits(&mut reader, JsonLimits::default(), || {
                checks += 1;
                checks > 2
            }),
            Err(JsonStreamError::Cancelled)
        );
        assert_eq!(reader.cursor, 2);

        struct BrokenReader;
        impl std::io::Read for BrokenReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("source failed"))
            }
        }
        assert_eq!(
            parse_reader_with_limits(&mut BrokenReader, JsonLimits::default(), || false),
            Err(JsonStreamError::Io)
        );
    }

    #[test]
    fn streaming_writer_uses_bounded_writes_and_reports_cancellation_or_io_failure() {
        let value = serde_json::json!({"payload": "x".repeat(150_000)});
        let mut writer = RecordingWriter::default();
        write_value_with_limits(&mut writer, &value, JsonLimits::default(), || false).unwrap();
        assert_eq!(writer.bytes, serde_json::to_vec(&value).unwrap());
        assert!(writer.largest_write <= JSON_STREAM_CHUNK_BYTES);

        let mut cancelled_writer = RecordingWriter::default();
        let mut checks = 0;
        assert_eq!(
            write_value_with_limits(&mut cancelled_writer, &value, JsonLimits::default(), || {
                checks += 1;
                checks > 3
            }),
            Err(JsonStreamError::Cancelled)
        );
        assert!(!cancelled_writer.bytes.is_empty());

        let mut failed_writer = RecordingWriter {
            fail_after: Some(4),
            ..RecordingWriter::default()
        };
        assert_eq!(
            write_value_with_limits(&mut failed_writer, &value, JsonLimits::default(), || false),
            Err(JsonStreamError::Io)
        );
    }

    #[test]
    fn streaming_writer_enforces_output_limit_without_returning_success() {
        let value = serde_json::json!({"value": "too long"});
        let expected = serde_json::to_vec(&value).unwrap();
        let limits = JsonLimits {
            max_document_bytes: expected.len() - 1,
            ..JsonLimits::default()
        };
        let mut writer = RecordingWriter::default();
        assert_eq!(
            write_value_with_limits(&mut writer, &value, limits, || false),
            Err(JsonStreamError::Validation(
                JsonValidationError::DocumentTooLarge
            ))
        );
    }

    #[test]
    fn streaming_pretty_and_canonical_writers_match_the_bounded_serializers() {
        let value = serde_json::json!({"z": 1, "a": ["café", true]});
        let mut pretty = RecordingWriter::default();
        write_pretty_value_with_limits(&mut pretty, &value, JsonLimits::default(), || false)
            .unwrap();
        assert_eq!(
            pretty.bytes,
            serialize_pretty_value_with_limits(&value, JsonLimits::default())
                .unwrap()
                .as_bytes()
        );

        let mut canonical = RecordingWriter::default();
        write_canonical_value_with_limits(&mut canonical, &value, JsonLimits::default(), || false)
            .unwrap();
        assert_eq!(
            canonical.bytes,
            canonicalize_value_with_limits(&value, JsonLimits::default())
                .unwrap()
                .as_bytes()
        );
    }

    #[test]
    fn json_schema_2020_12_validates_local_refs_and_reports_instance_pointers() {
        let schema = r##"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": { "count": { "type": "integer", "minimum": 1 } },
            "type": "object",
            "properties": { "count": { "$ref": "#/$defs/count" } },
            "required": ["count"]
        }"##;
        assert!(
            validate_json_schema(schema, r#"{"count":2}"#, JsonLimits::default())
                .unwrap()
                .is_empty()
        );

        let errors = validate_json_schema(schema, r#"{"count":0}"#, JsonLimits::default()).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].instance_pointer, "/count");
        assert!(errors[0].message.contains("minimum"));
    }

    #[test]
    fn json_schema_without_schema_uri_defaults_to_draft_2020_12() {
        let schema = r##"{"$defs":{"label":{"type":"string"}},"$ref":"#/$defs/label"}"##;
        let violations = validate_json_schema(schema, "42", JsonLimits::default()).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].instance_pointer, "");
        assert!(violations[0].message.contains("string"));
    }

    #[test]
    fn json_schema_rejects_external_refs_without_an_explicit_resolver() {
        let error = validate_json_schema(
            r#"{"$ref":"https://example.invalid/schema.json"}"#,
            "null",
            JsonLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            JsonSchemaError::ExternalReferenceRequiresResolver(
                "https://example.invalid/schema.json".into()
            )
        );
    }

    #[test]
    fn json_schema_requires_explicit_resolver_for_external_meta_schema() {
        let uri = "https://schemas.example.test/draft.json";
        let schema = format!(r#"{{"$schema":"{uri}","type":"string"}}"#);
        assert_eq!(
            validate_json_schema(&schema, r#""ok""#, JsonLimits::default()),
            Err(JsonSchemaError::ExternalReferenceRequiresResolver(
                uri.to_owned()
            ))
        );
    }

    #[test]
    fn json_schema_resolves_only_caller_supplied_external_documents() {
        let uri = "https://schemas.example.test/name.json".to_owned();
        let schema = format!(r#"{{"$ref":"{uri}"}}"#);
        let resolver = [(uri, r#"{"type":"string","minLength":2}"#.to_owned())];
        assert!(
            validate_json_schema_with_resolver(
                &schema,
                r#""ok""#,
                JsonLimits::default(),
                &resolver,
            )
            .unwrap()
            .is_empty()
        );

        let errors =
            validate_json_schema_with_resolver(&schema, r#""x""#, JsonLimits::default(), &resolver)
                .unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].instance_pointer, "");
    }

    #[test]
    fn json_schema_resolver_honors_a_caller_supplied_2020_12_dialect() {
        let uri = "http://localhost:1234/draft2020-12/metaschema-no-validation.json";
        let schema = format!(
            r#"{{"$id":"https://schema/using/no/validation","$schema":"{uri}","properties":{{"numberProperty":{{"minimum":10}}}}}}"#
        );
        let meta_schema = include_str!(
            "../tests/fixtures/json/schema/official/remotes/draft2020-12/metaschema-no-validation.json"
        );
        let resolver = [(uri.to_owned(), meta_schema.to_owned())];
        let violations = validate_json_schema_with_resolver(
            &schema,
            r#"{"numberProperty":1}"#,
            JsonLimits::default(),
            &resolver,
        )
        .expect("the explicit resolver supplies the declared dialect");
        assert!(violations.is_empty());
    }

    #[test]
    fn json_schema_resolver_documents_obey_shared_limits() {
        let uri = "https://schemas.example.test/large.json".to_owned();
        let schema = format!(r#"{{"$ref":"{uri}"}}"#);
        let resolver = [(
            uri,
            r#"{"description":"this resolver schema document is deliberately longer than the configured document byte ceiling"}"#.to_owned(),
        )];
        let limits = JsonLimits {
            max_document_bytes: 80,
            ..JsonLimits::default()
        };
        assert!(matches!(
            validate_json_schema_with_resolver(&schema, "null", limits, &resolver),
            Err(JsonSchemaError::ResolverDocument {
                error: JsonValidationError::DocumentTooLarge,
                ..
            })
        ));
    }

    #[test]
    fn json_schema_enforces_the_shared_document_limits() {
        let limits = JsonLimits {
            max_document_bytes: 32,
            ..JsonLimits::default()
        };
        assert_eq!(
            validate_json_schema(
                r#"{"type":"string"}"#,
                r#"{"larger":"than the configured input ceiling"}"#,
                limits,
            ),
            Err(JsonSchemaError::InstanceDocument(
                JsonValidationError::DocumentTooLarge
            ))
        );
    }

    #[test]
    fn json_schema_limits_recursive_schema_depth_and_violation_count() {
        let depth_limited = JsonLimits {
            max_nesting_depth: 4,
            ..JsonLimits::default()
        };
        assert_eq!(
            validate_json_schema(
                r#"{"properties":{"a":{"properties":{"b":{"type":"string"}}}}}"#,
                "{}",
                depth_limited,
            ),
            Err(JsonSchemaError::SchemaDocument(
                JsonValidationError::NestingTooDeep
            ))
        );

        let mut properties = serde_json::Map::new();
        let mut instance_properties = serde_json::Map::new();
        for index in 0..=MAX_JSON_SCHEMA_VIOLATIONS {
            let name = format!("field_{index}");
            properties.insert(name.clone(), serde_json::json!({"type":"string"}));
            instance_properties.insert(name, serde_json::json!(index));
        }
        let schema = serde_json::json!({"type":"object", "properties":properties});
        let instance = serde_json::Value::Object(instance_properties);
        let violations = validate_json_schema(
            &schema.to_string(),
            &instance.to_string(),
            JsonLimits::default(),
        )
        .unwrap();
        assert_eq!(violations.len(), MAX_JSON_SCHEMA_VIOLATIONS);
    }

    #[test]
    fn json_schema_recursion_through_local_refs_obeys_instance_depth_limit() {
        let schema = r##"{
            "$schema":"https://json-schema.org/draft/2020-12/schema",
            "$id":"https://schemas.example.test/tree",
            "$defs":{"node":{"type":"object","properties":{"children":{"items":{"$ref":"#/$defs/node"}}}}},
            "$ref":"#/$defs/node"
        }"##;
        let limits = JsonLimits {
            max_nesting_depth: 8,
            ..JsonLimits::default()
        };
        let mut instance = "{}".to_owned();
        for _ in 0..10 {
            instance = format!(r#"{{"children":[{instance}]}}"#);
        }
        assert_eq!(
            validate_json_schema(schema, &instance, limits),
            Err(JsonSchemaError::InstanceDocument(
                JsonValidationError::NestingTooDeep
            ))
        );
    }

    #[test]
    fn extension_profile_corpus_is_valid_and_covers_every_frozen_profile() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json/profile-cases.json"))
                .expect("JSON profile corpus must be valid JSON");
        let profiles = [
            "strict",
            "pointer",
            "jsonpath",
            "schema",
            "jcs",
            "json5",
            "streaming",
            "limits",
        ];
        let mut ids = HashSet::new();
        for profile in profiles {
            let cases = corpus
                .get(profile)
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("profile `{profile}` must have a case array"));
            assert!(
                !cases.is_empty(),
                "profile `{profile}` needs conformance cases"
            );
            for case in cases {
                let id = case
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_else(|| panic!("profile `{profile}` has a case without an id"));
                assert!(ids.insert(id), "duplicate JSON profile case id `{id}`");
            }
        }
    }

    #[test]
    fn json5_parser_accepts_relaxed_forms_non_finite_numbers_and_last_duplicate() {
        let value = parse_json5(
            "{ // comment\n unquoted: 'text', list: [1, 2,], value: 1, value: 2, hex: +0x10, fraction: .5, positive: Infinity, negative: -Infinity, nan: NaN }",
        )
        .unwrap();
        let Json5Value::Object(properties) = value else {
            panic!("expected an object");
        };
        assert_eq!(
            properties.get("unquoted"),
            Some(&Json5Value::String("text".into()))
        );
        assert_eq!(properties.get("value"), Some(&Json5Value::Number(2.0)));
        assert_eq!(properties.get("hex"), Some(&Json5Value::Number(16.0)));
        assert_eq!(properties.get("fraction"), Some(&Json5Value::Number(0.5)));
        assert_eq!(
            properties.get("positive"),
            Some(&Json5Value::Number(f64::INFINITY))
        );
        assert_eq!(
            properties.get("negative"),
            Some(&Json5Value::Number(f64::NEG_INFINITY))
        );
        assert!(matches!(properties.get("nan"), Some(Json5Value::Number(value)) if value.is_nan()));
        assert_eq!(
            properties.get("list"),
            Some(&Json5Value::Array(vec![
                Json5Value::Number(1.0),
                Json5Value::Number(2.0)
            ]))
        );
    }

    #[test]
    fn json5_parser_keeps_strict_json_untouched_and_enforces_shared_limits() {
        assert_eq!(
            validate_document("{ value: 'json5-only' }"),
            Err(JsonValidationError::InvalidDocument)
        );
        assert_eq!(parse_json5("{ value: '\\u12' }"), Err(Json5Error::Syntax));
        assert_eq!(
            parse_json5_with_limits(
                "[1, 2]",
                JsonLimits {
                    max_container_items: 1,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
        assert_eq!(
            parse_json5_with_limits(
                "'four'",
                JsonLimits {
                    max_string_bytes: 3,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
        assert_eq!(
            parse_json5_with_limits(
                "'\\u0061\\u0062'",
                JsonLimits {
                    max_string_bytes: 1,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
        assert!(
            parse_json5_with_limits(
                r#"'\uD83D\uDE00'"#,
                JsonLimits {
                    max_string_bytes: 4,
                    ..JsonLimits::default()
                }
            )
            .is_ok()
        );
        assert_eq!(
            parse_json5_with_limits(
                "0x1234",
                JsonLimits {
                    max_number_bytes: 4,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
        assert_eq!(
            parse_json5_with_limits(
                "[[0]]",
                JsonLimits {
                    max_nesting_depth: 1,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
        assert!(
            parse_json5_with_limits(
                "[1,]",
                JsonLimits {
                    max_container_items: 1,
                    ..JsonLimits::default()
                }
            )
            .is_ok()
        );
        assert_eq!(
            parse_json5_with_limits(
                "[0]",
                JsonLimits {
                    max_nesting_depth: 0,
                    ..JsonLimits::default()
                }
            ),
            Err(Json5Error::Limit)
        );
    }

    #[test]
    fn json5_profile_corpus_matches_expected_values_and_error_kinds() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json/profile-cases.json"))
                .unwrap();
        let cases = corpus["json5"].as_array().unwrap();
        for case in cases {
            let input = case["input"].as_str().unwrap();
            let expected = case["outcome"].as_str().unwrap();
            let actual = parse_json5(input);
            if expected == "Json5SyntaxError" {
                assert_eq!(actual, Err(Json5Error::Syntax), "{}", case["id"]);
                continue;
            }
            let value = actual.unwrap_or_else(|error| panic!("{}: {error:?}", case["id"]));
            if expected == "floating-infinity-and-nan" {
                assert!(matches!(value, Json5Value::Array(values)
                    if matches!(values.as_slice(), [Json5Value::Number(a), Json5Value::Number(b)] if a.is_infinite() && b.is_nan())));
            } else {
                let normalized = json5_to_serde_value(value);
                let expected = normalize_json5_numbers(serde_json::from_str(expected).unwrap());
                assert_eq!(normalized, expected, "{}", case["id"]);
            }
        }
    }

    #[test]
    fn json5_parser_matches_the_upstream_conformance_corpus() {
        fn collect(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(&path, files);
                } else if matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("json" | "json5" | "js" | "txt")
                ) {
                    files.push(path);
                }
            }
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/json/json5-upstream");
        let mut files = Vec::new();
        collect(&root, &mut files);
        files.sort();

        let mut valid = 0usize;
        let mut invalid = 0usize;
        for path in files {
            let input = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("could not read upstream case {}: {error}", path.display())
            });
            let result = parse_json5(&input);
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("json" | "json5") => {
                    result.unwrap_or_else(|error| {
                        panic!(
                            "valid upstream JSON5 case {} rejected: {error:?}",
                            path.display()
                        )
                    });
                    valid += 1;
                }
                Some("js" | "txt") => {
                    assert!(
                        result.is_err(),
                        "invalid upstream JSON5 case {} was accepted",
                        path.display()
                    );
                    invalid += 1;
                }
                _ => unreachable!("only selected fixture extensions are collected"),
            }
        }
        assert!(
            valid >= 70,
            "the upstream valid corpus unexpectedly shrank to {valid} cases"
        );
        assert!(
            invalid >= 25,
            "the upstream invalid corpus unexpectedly shrank to {invalid} cases"
        );
    }

    fn json5_to_serde_value(value: Json5Value) -> serde_json::Value {
        match value {
            Json5Value::Null => serde_json::Value::Null,
            Json5Value::Boolean(value) => serde_json::Value::Bool(value),
            Json5Value::Number(value) => serde_json::Number::from_f64(value)
                .map(serde_json::Value::Number)
                .expect("finite JSON5 corpus number"),
            Json5Value::String(value) => serde_json::Value::String(value),
            Json5Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(json5_to_serde_value).collect())
            }
            Json5Value::Object(properties) => serde_json::Value::Object(
                properties
                    .into_iter()
                    .map(|(key, value)| (key, json5_to_serde_value(value)))
                    .collect(),
            ),
        }
    }

    fn normalize_json5_numbers(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Number(value) => serde_json::Number::from_f64(
                value
                    .as_f64()
                    .expect("corpus number is representable as f64"),
            )
            .map(serde_json::Value::Number)
            .unwrap(),
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(normalize_json5_numbers).collect())
            }
            serde_json::Value::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, normalize_json5_numbers(value)))
                    .collect(),
            ),
            value => value,
        }
    }

    #[test]
    fn rejects_duplicate_keys_after_escape_decoding() {
        assert_eq!(
            validate_document(r#"{"x":1,"\u0078":2}"#),
            Err(JsonValidationError::InvalidDocument)
        );
        assert_eq!(
            validate_document(r#"{"outer":{"x":1,"x":2}}"#),
            Err(JsonValidationError::InvalidDocument)
        );
        assert!(validate_document(r#"{"x":1,"y":2}"#).is_ok());
    }

    #[test]
    fn enforces_document_size_and_nesting_limits() {
        assert_eq!(
            validate_document(&" ".repeat(MAX_DOCUMENT_BYTES + 1)),
            Err(JsonValidationError::DocumentTooLarge)
        );
        let nested = format!(
            "{}0{}",
            "[".repeat(MAX_NESTING_DEPTH + 1),
            "]".repeat(MAX_NESTING_DEPTH + 1)
        );
        assert_eq!(
            validate_document(&nested),
            Err(JsonValidationError::NestingTooDeep)
        );
    }

    #[test]
    fn configurable_limits_enforce_decoded_strings_container_items_and_numbers() {
        let base = JsonLimits {
            max_document_bytes: 1024,
            max_nesting_depth: 8,
            max_string_bytes: 3,
            max_container_items: 2,
            max_number_bytes: 3,
        };
        assert_eq!(
            validate_document_with_limits(r#"["four"]"#, base),
            Err(JsonValidationError::StringTooLarge)
        );
        assert_eq!(
            validate_document_with_limits("[1,2,3]", base),
            Err(JsonValidationError::TooManyContainerItems)
        );
        assert_eq!(
            validate_document_with_limits("{\"a\":1,\"b\":2,\"c\":3}", base),
            Err(JsonValidationError::TooManyContainerItems)
        );
        assert_eq!(
            validate_document_with_limits("1234", base),
            Err(JsonValidationError::NumberTooLarge)
        );

        // Limits count decoded UTF-8 bytes, not the longer escaped spelling.
        assert!(validate_document_with_limits(r#"["\u0061"]"#, base).is_ok());

        let deeper_than_default = format!(
            "{}0{}",
            "[".repeat(MAX_NESTING_DEPTH + 1),
            "]".repeat(MAX_NESTING_DEPTH + 1)
        );
        let raised_depth = JsonLimits {
            max_nesting_depth: MAX_NESTING_DEPTH + 1,
            ..JsonLimits::default()
        };
        assert!(validate_document_with_limits(&deeper_than_default, raised_depth).is_ok());
    }

    #[test]
    fn rejects_limits_above_frozen_hard_ceilings() {
        let cases = [
            JsonLimits {
                max_document_bytes: HARD_MAX_DOCUMENT_BYTES + 1,
                ..JsonLimits::default()
            },
            JsonLimits {
                max_nesting_depth: HARD_MAX_NESTING_DEPTH + 1,
                ..JsonLimits::default()
            },
            JsonLimits {
                max_string_bytes: HARD_MAX_STRING_BYTES + 1,
                ..JsonLimits::default()
            },
            JsonLimits {
                max_container_items: HARD_MAX_CONTAINER_ITEMS + 1,
                ..JsonLimits::default()
            },
            JsonLimits {
                max_number_bytes: HARD_MAX_NUMBER_BYTES + 1,
                ..JsonLimits::default()
            },
        ];
        for too_large in cases {
            assert_eq!(
                validate_document_with_limits("not parsed", too_large),
                Err(JsonValidationError::InvalidLimits)
            );
        }
    }

    #[test]
    fn json_pointer_resolves_root_members_indexes_and_escaped_tokens() {
        let document: serde_json::Value = serde_json::from_str(
            r#"{"a/b":{"~1":"matched","~":9},"items":["first","second"],"":true}"#,
        )
        .unwrap();
        assert_eq!(lookup_pointer(&document, "").unwrap(), &document);
        assert_eq!(lookup_pointer(&document, "/a~1b/~01").unwrap(), "matched");
        assert_eq!(lookup_pointer(&document, "/items/1").unwrap(), "second");
        assert_eq!(lookup_pointer(&document, "/").unwrap(), true);
    }

    #[test]
    fn json_pointer_reports_typed_syntax_index_and_lookup_errors() {
        let document: serde_json::Value = serde_json::from_str(r#"{"a":[1]}"#).unwrap();
        assert_eq!(
            lookup_pointer(&document, "a"),
            Err(JsonPointerError::Syntax)
        );
        assert_eq!(
            lookup_pointer(&document, "/~2"),
            Err(JsonPointerError::Syntax)
        );
        assert_eq!(
            lookup_pointer(&document, "/a/01"),
            Err(JsonPointerError::Index)
        );
        assert_eq!(
            lookup_pointer(&document, "/a/-"),
            Err(JsonPointerError::Index)
        );
        assert_eq!(
            lookup_pointer(&document, "/absent"),
            Err(JsonPointerError::NotFound)
        );
        assert_eq!(
            lookup_pointer(&document, "/a/2"),
            Err(JsonPointerError::NotFound)
        );
        assert_eq!(
            lookup_pointer(&document, "/a/0/name"),
            Err(JsonPointerError::WrongContainer)
        );
    }

    #[test]
    fn json_pointer_shared_profile_corpus_executes_with_expected_results() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json/profile-cases.json"))
                .unwrap();
        for case in corpus["pointer"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let document: serde_json::Value =
                serde_json::from_str(case["document"].as_str().unwrap()).unwrap();
            let pointer = case["pointer"].as_str().unwrap();
            let expected = case["outcome"].as_str().unwrap();
            match lookup_pointer(&document, pointer) {
                Ok(actual) => {
                    let expected: serde_json::Value = serde_json::from_str(expected)
                        .unwrap_or_else(|error| panic!("{id}: invalid expected JSON: {error}"));
                    assert_eq!(actual, &expected, "{id}");
                }
                Err(error) => {
                    let expected_error = match error {
                        JsonPointerError::Syntax => "JsonPointerSyntaxError",
                        JsonPointerError::Index => "JsonPointerIndexError",
                        JsonPointerError::NotFound => "JsonPointerNotFound",
                        JsonPointerError::WrongContainer => "JsonPointerContainerError",
                    };
                    assert_eq!(expected_error, expected, "{id}");
                }
            }
        }
    }

    #[test]
    fn jsonpath_shared_profile_vectors_execute_through_rfc_engine() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json/profile-cases.json"))
                .unwrap();
        for case in manifest["jsonpath"].as_array().unwrap() {
            if case["id"] == "jsonpath-step-budget" {
                // Keep the fixture compact while still exercising the public
                // one-million-step ceiling with a generated wide document.
                let document = serde_json::Value::Array(vec![
                    serde_json::Value::Null;
                    MAX_JSONPATH_STEPS / 2 + 1
                ]);
                assert_eq!(
                    query_jsonpath(&document, "$[*]"),
                    Err(JsonPathError::Limit),
                    "{}",
                    case["id"]
                );
                continue;
            }
            let document: serde_json::Value =
                serde_json::from_str(case["document"].as_str().unwrap()).unwrap();
            let query = case["query"].as_str().unwrap();
            let expected = case["outcome"].as_str().unwrap();
            match query_jsonpath(&document, query) {
                Ok(values) => {
                    assert!(!expected.starts_with("Json"), "{}", case["id"]);
                    assert_eq!(
                        serde_json::to_string(&values).unwrap(),
                        expected,
                        "{}",
                        case["id"]
                    );
                }
                Err(JsonPathError::Syntax) => {
                    assert_eq!(expected, "JsonPathSyntaxError", "{}", case["id"]);
                }
                Err(JsonPathError::Function) => {
                    assert_eq!(expected, "JsonPathFunctionError", "{}", case["id"]);
                }
                Err(JsonPathError::Limit) => {
                    assert_eq!(expected, "JsonLimitError", "{}", case["id"]);
                }
                Err(JsonPathError::Input) => {
                    panic!(
                        "{}: query evaluator reported a Dynamic input error",
                        case["id"]
                    );
                }
            }
        }
    }

    #[test]
    fn jsonpath_rejects_queries_over_the_hard_byte_limit() {
        let document = serde_json::Value::Null;
        let query = format!("${}", ".a".repeat(MAX_JSONPATH_QUERY_BYTES / 2));
        assert_eq!(query_jsonpath(&document, &query), Err(JsonPathError::Limit));
    }

    #[test]
    fn jsonpath_bounds_result_materialization_before_cloning() {
        let document =
            serde_json::Value::Array(vec![serde_json::Value::Null; MAX_JSONPATH_RESULTS + 1]);
        assert_eq!(query_jsonpath(&document, "$[*]"), Err(JsonPathError::Limit));
    }

    #[test]
    fn limited_serializer_stops_at_the_document_byte_boundary() {
        let value = serde_json::json!({"name": "Ada"});
        let expected = r#"{"name":"Ada"}"#;
        let exact = JsonLimits {
            max_document_bytes: expected.len(),
            ..JsonLimits::default()
        };
        assert_eq!(
            serialize_value_with_limits(&value, exact).unwrap(),
            expected
        );
        let short = JsonLimits {
            max_document_bytes: expected.len() - 1,
            ..JsonLimits::default()
        };
        assert_eq!(
            serialize_value_with_limits(&value, short),
            Err(JsonValidationError::DocumentTooLarge)
        );
    }

    #[test]
    fn pretty_serializer_is_deterministic_and_obeys_output_limits() {
        let value = serde_json::json!({"items": [1, {"ok": true}]});
        let expected = "{\n  \"items\": [\n    1,\n    {\n      \"ok\": true\n    }\n  ]\n}";
        let limits = JsonLimits {
            max_document_bytes: expected.len(),
            ..JsonLimits::default()
        };
        assert_eq!(
            serialize_pretty_value_with_limits(&value, limits).unwrap(),
            expected
        );
        let too_small = JsonLimits {
            max_document_bytes: expected.len() - 1,
            ..JsonLimits::default()
        };
        assert_eq!(
            serialize_pretty_value_with_limits(&value, too_small),
            Err(JsonValidationError::DocumentTooLarge)
        );
    }

    #[test]
    fn canonical_serializer_matches_rfc8785_primitives_and_sorting() {
        let value = serde_json::json!({
            "numbers": [
                333333333.33333329_f64,
                1e30_f64,
                4.50_f64,
                2e-3_f64,
                1e-27_f64,
            ],
            "string": "€$\u{000f}\nA'B\"\\\\\"/",
            "literals": [null, true, false],
        });
        let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#;
        assert_eq!(
            canonicalize_value_with_limits(&value, JsonLimits::default()).unwrap(),
            expected
        );

        let unicode = serde_json::from_str::<serde_json::Value>(
            r#"{"€":"Euro","\r":"CR","דּ":"Hebrew","1":"One","😀":"Emoji","\u0080":"Control","ö":"Latin"}"#,
        )
        .unwrap();
        assert_eq!(
            canonicalize_value_with_limits(&unicode, JsonLimits::default()).unwrap(),
            r#"{"\r":"CR","1":"One","":"Control","ö":"Latin","€":"Euro","😀":"Emoji","דּ":"Hebrew"}"#
        );
    }

    #[test]
    fn canonical_serializer_matches_the_upstream_rfc8785_appendix_i_corpus() {
        fn ieee754_tree(value: serde_json::Value) -> serde_json::Value {
            match value {
                serde_json::Value::Number(number) => {
                    let number = number
                        .as_f64()
                        .and_then(serde_json::Number::from_f64)
                        .expect("official JCS corpus numbers fit binary64");
                    serde_json::Value::Number(number)
                }
                serde_json::Value::Array(values) => {
                    serde_json::Value::Array(values.into_iter().map(ieee754_tree).collect())
                }
                serde_json::Value::Object(values) => serde_json::Value::Object(
                    values
                        .into_iter()
                        .map(|(key, value)| (key, ieee754_tree(value)))
                        .collect(),
                ),
                value => value,
            }
        }

        for name in [
            "arrays",
            "french",
            "structures",
            "unicode",
            "values",
            "weird",
        ] {
            let input = std::fs::read_to_string(format!(
                "{}/tests/fixtures/json/jcs/input/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let expected = std::fs::read_to_string(format!(
                "{}/tests/fixtures/json/jcs/output/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let value = serde_json::from_str::<serde_json::Value>(&input).unwrap();
            let value = if name == "values" {
                ieee754_tree(value)
            } else {
                value
            };
            assert_eq!(
                canonicalize_value_with_limits(&value, JsonLimits::default()).unwrap(),
                expected.trim_end(),
                "official JCS fixture {name}"
            );
        }
    }

    #[test]
    fn jcs_number_formatter_matches_every_finite_rfc8785_appendix_b_vector() {
        let vectors = [
            (0x0000_0000_0000_0000, "0"),
            (0x8000_0000_0000_0000, "0"),
            (0x0000_0000_0000_0001, "5e-324"),
            (0x8000_0000_0000_0001, "-5e-324"),
            (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
            (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
            (0x4340_0000_0000_0000, "9007199254740992"),
            (0xc340_0000_0000_0000, "-9007199254740992"),
            (0x4430_0000_0000_0000, "295147905179352830000"),
            (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
            (0x44b5_2d02_c7e1_4af6, "1e+23"),
            (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
            (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
            (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
            (0x444b_1ae4_d6e2_ef50, "1e+21"),
            (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
            (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
            (0x41b3_de43_5555_5553, "333333333.3333332"),
            (0x41b3_de43_5555_5554, "333333333.33333325"),
            (0x41b3_de43_5555_5555, "333333333.3333333"),
            (0x41b3_de43_5555_5556, "333333333.3333334"),
            (0x41b3_de43_5555_5557, "333333333.33333343"),
            (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
            (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
        ];

        for (bits, expected) in vectors {
            let number = f64::from_bits(bits);
            let value = serde_json::Number::from_f64(number).unwrap();
            assert_eq!(
                serde_jcs::to_string(&value).unwrap(),
                expected,
                "{bits:016x}"
            );
        }
    }

    #[test]
    fn canonical_serializer_rejects_numbers_that_would_change_exact_value() {
        for source in [
            "9007199254740993",
            "-9007199254740993",
            "0.10000000000000001",
            "1.0000000000000001",
            "1e400",
            "1e-4000",
            "3.141592653589793238462643383279",
        ] {
            let value = serde_json::from_str::<serde_json::Value>(source).unwrap();
            assert_eq!(
                canonicalize_value_with_limits(&value, JsonLimits::default()),
                Err(JsonValidationError::NumberOutsideCanonicalDomain),
                "{source}"
            );
        }

        let exact = serde_json::from_str::<serde_json::Value>("0.1000").unwrap();
        assert_eq!(
            canonicalize_value_with_limits(&exact, JsonLimits::default()).unwrap(),
            "0.1"
        );

        for (source, expected) in [
            ("-0", "0"),
            ("9007199254740991", "9007199254740991"),
            ("9007199254740992", "9007199254740992"),
            ("1e-6", "0.000001"),
            ("5e-324", "5e-324"),
            ("1.7976931348623157e308", "1.7976931348623157e+308"),
        ] {
            let value = serde_json::from_str::<serde_json::Value>(source).unwrap();
            assert_eq!(
                canonicalize_value_with_limits(&value, JsonLimits::default()).unwrap(),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn canonical_serializer_enforces_output_and_structural_limits() {
        let value = serde_json::json!({"a": 1});
        let too_short = JsonLimits {
            max_document_bytes: 5,
            ..JsonLimits::default()
        };
        assert_eq!(
            canonicalize_value_with_limits(&value, too_short),
            Err(JsonValidationError::DocumentTooLarge)
        );
        let no_properties = JsonLimits {
            max_container_items: 0,
            ..JsonLimits::default()
        };
        assert_eq!(
            canonicalize_value_with_limits(&value, no_properties),
            Err(JsonValidationError::TooManyContainerItems)
        );
    }

    #[test]
    fn limited_serializer_enforces_every_value_budget() {
        let cases = [
            (
                serde_json::json!("four"),
                JsonLimits {
                    max_string_bytes: 3,
                    ..JsonLimits::default()
                },
                JsonValidationError::StringTooLarge,
            ),
            (
                serde_json::json!({"four": 1}),
                JsonLimits {
                    max_string_bytes: 3,
                    ..JsonLimits::default()
                },
                JsonValidationError::StringTooLarge,
            ),
            (
                serde_json::json!([1, 2, 3]),
                JsonLimits {
                    max_container_items: 2,
                    ..JsonLimits::default()
                },
                JsonValidationError::TooManyContainerItems,
            ),
            (
                serde_json::json!({"a": 1, "b": 2}),
                JsonLimits {
                    max_container_items: 1,
                    ..JsonLimits::default()
                },
                JsonValidationError::TooManyContainerItems,
            ),
            (
                serde_json::json!(1234),
                JsonLimits {
                    max_number_bytes: 3,
                    ..JsonLimits::default()
                },
                JsonValidationError::NumberTooLarge,
            ),
            (
                serde_json::json!([[0]]),
                JsonLimits {
                    max_nesting_depth: 1,
                    ..JsonLimits::default()
                },
                JsonValidationError::NestingTooDeep,
            ),
        ];
        for (value, limits, expected) in cases {
            assert_eq!(serialize_value_with_limits(&value, limits), Err(expected));
        }
    }

    #[test]
    fn shared_limits_profile_executes_for_parsing_and_serialization() {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json/limits-cases.json"))
                .expect("the shared JSON limits corpus is valid");
        let limits_for = |case: &serde_json::Value| {
            let values = case["limits"].as_array().expect("limits array");
            assert_eq!(values.len(), 5);
            JsonLimits {
                max_document_bytes: values[0].as_u64().unwrap() as usize,
                max_nesting_depth: values[1].as_u64().unwrap() as usize,
                max_string_bytes: values[2].as_u64().unwrap() as usize,
                max_container_items: values[3].as_u64().unwrap() as usize,
                max_number_bytes: values[4].as_u64().unwrap() as usize,
            }
        };
        for case in corpus["parse"].as_array().expect("parse cases") {
            let id = case["id"].as_str().unwrap();
            let input = case["input"].as_str().unwrap();
            let expected = case["outcome"].as_str().unwrap();
            let outcome = validate_document_with_limits(input, limits_for(case)).and_then(|()| {
                serde_json::from_str::<serde_json::Value>(input)
                    .map(|_| ())
                    .map_err(|_| JsonValidationError::InvalidDocument)
            });
            let actual = outcome.map(|_| "ok").unwrap_or_else(|error| match error {
                JsonValidationError::DocumentTooLarge => "DocumentTooLarge",
                JsonValidationError::NestingTooDeep => "NestingTooDeep",
                JsonValidationError::StringTooLarge => "StringTooLarge",
                JsonValidationError::TooManyContainerItems => "TooManyContainerItems",
                JsonValidationError::NumberTooLarge => "NumberTooLarge",
                _ => "other",
            });
            assert_eq!(actual, expected, "{id}");
        }
        for case in corpus["serialize"].as_array().expect("serialization cases") {
            let id = case["id"].as_str().unwrap();
            let input = case["value"].as_str().unwrap();
            let value = serde_json::from_str::<serde_json::Value>(input).unwrap();
            let expected = case["outcome"].as_str().unwrap();
            let outcome = serialize_value_with_limits(&value, limits_for(case));
            match outcome {
                Ok(actual) => assert_eq!(actual, expected, "{id}"),
                Err(error) => assert_eq!(format!("{error:?}"), expected, "{id}"),
            }
        }
    }
}
