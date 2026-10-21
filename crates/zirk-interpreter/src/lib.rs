//! # zirk-interpreter
//!
//! **Responsibility:** execute verified typed Zirk IR with an explicit value
//! model, resource budgets and a narrow host-effect boundary.
//!
//! **Boundary:** this crate consumes `zirk-ir` and platform-neutral
//! `zirk-runtime-core`; it does not parse source, invoke LLVM, link binaries,
//! access the browser, or depend on the native runtime.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use zirk_diagnostics::{FileId, Span};
use zirk_ir::{
    BinaryOp, BlockId, FloatWidth, InstKind, IntWidth, IrType, Module, Nullable, Terminator,
    UnaryOp, ValueId, verify,
};

/// Browser compatibility classification used by preflight and maintained as
/// exhaustive matches so new IR variants cannot silently acquire support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSupport {
    Direct,
    RuntimeCore,
    BoundedHost,
    StructuredFailure,
    Excluded,
}

/// Resource limits accepted by one interpreter execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterpreterLimits {
    pub instructions: u64,
    pub call_depth: u32,
    pub heap_bytes: usize,
    pub output_bytes: usize,
    pub output_events: u32,
    pub stack_frames: u32,
}

impl InterpreterLimits {
    pub const MAX_INSTRUCTIONS: u64 = 20_000_000;
    pub const MAX_CALL_DEPTH: u32 = 1_024;
    pub const MAX_HEAP_BYTES: usize = 64 * 1024 * 1024;
    pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
    pub const MAX_OUTPUT_EVENTS: u32 = 8_192;
    pub const MAX_STACK_FRAMES: u32 = 256;

    pub const fn validate(self) -> Result<Self, LimitConfigurationError> {
        if self.instructions == 0 || self.instructions > Self::MAX_INSTRUCTIONS {
            return Err(LimitConfigurationError::Instructions);
        }
        if self.call_depth == 0 || self.call_depth > Self::MAX_CALL_DEPTH {
            return Err(LimitConfigurationError::CallDepth);
        }
        if self.heap_bytes == 0 || self.heap_bytes > Self::MAX_HEAP_BYTES {
            return Err(LimitConfigurationError::Heap);
        }
        if self.output_bytes == 0 || self.output_bytes > Self::MAX_OUTPUT_BYTES {
            return Err(LimitConfigurationError::OutputBytes);
        }
        if self.output_events == 0 || self.output_events > Self::MAX_OUTPUT_EVENTS {
            return Err(LimitConfigurationError::OutputEvents);
        }
        if self.stack_frames == 0 || self.stack_frames > Self::MAX_STACK_FRAMES {
            return Err(LimitConfigurationError::StackFrames);
        }
        Ok(self)
    }
}

impl Default for InterpreterLimits {
    fn default() -> Self {
        Self {
            instructions: 2_000_000,
            call_depth: 128,
            heap_bytes: 16 * 1024 * 1024,
            output_bytes: 256 * 1024,
            output_events: 2_048,
            stack_frames: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitConfigurationError {
    Instructions,
    CallDepth,
    Heap,
    OutputBytes,
    OutputEvents,
    StackFrames,
}

/// Opaque, stable identity of a value owned by one interpreter execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeapId(u32);

/// Closed value representation. No value borrows from the host or exposes an
/// address; identity-bearing values use `HeapId`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Void,
    Never,
    Int(IntegerValue),
    Float { width: FloatWidth, text: String },
    Decimal(String),
    Boolean(bool),
    Nullable(Option<Box<Value>>),
    Heap(HeapId),
    Undefined,
}

/// Integer bits together with their declared Zirk width. Keeping the width
/// prevents `UInt128` values from being narrowed through Rust's `i128` and
/// lets arithmetic reject overflow rather than silently wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerValue {
    bits: u128,
    width: IntWidth,
}

impl IntegerValue {
    fn from_literal(value: i128, width: IntWidth) -> Result<Self, String> {
        if width.signed() {
            let minimum = if width.bits() == 128 {
                i128::MIN
            } else {
                -(1_i128 << (width.bits() - 1))
            };
            let maximum = if width.bits() == 128 {
                i128::MAX
            } else {
                (1_i128 << (width.bits() - 1)) - 1
            };
            if !(minimum..=maximum).contains(&value) {
                return Err(format!("integer literal {value} does not fit {width:?}"));
            }
            Ok(Self::from_bits(value as u128, width))
        } else {
            if value < 0 {
                return Err(format!("integer literal {value} does not fit {width:?}"));
            }
            Ok(Self::from_bits(value as u128, width))
        }
    }

    const fn from_bits(bits: u128, width: IntWidth) -> Self {
        let masked = if width.bits() == 128 {
            bits
        } else {
            bits & ((1_u128 << width.bits()) - 1)
        };
        Self {
            bits: masked,
            width,
        }
    }

    const fn unsigned(self) -> u128 {
        self.bits
    }

    const fn signed(self) -> i128 {
        if self.width.bits() == 128 {
            self.bits as i128
        } else {
            let sign_bit = 1_u128 << (self.width.bits() - 1);
            if self.bits & sign_bit == 0 {
                self.bits as i128
            } else {
                (self.bits | (!0_u128 << self.width.bits())) as i128
            }
        }
    }

    fn checked_binary(self, op: BinaryOp, right: Self) -> Result<Value, String> {
        if self.width != right.width {
            return Err("integer binary operands have different widths".into());
        }
        use BinaryOp::*;
        let comparison = |result| Ok(Value::Boolean(result));
        match op {
            Eq => comparison(self.bits == right.bits),
            NotEq => comparison(self.bits != right.bits),
            Lt => comparison(if self.width.signed() {
                self.signed() < right.signed()
            } else {
                self.unsigned() < right.unsigned()
            }),
            LtEq => comparison(if self.width.signed() {
                self.signed() <= right.signed()
            } else {
                self.unsigned() <= right.unsigned()
            }),
            Gt => comparison(if self.width.signed() {
                self.signed() > right.signed()
            } else {
                self.unsigned() > right.unsigned()
            }),
            GtEq => comparison(if self.width.signed() {
                self.signed() >= right.signed()
            } else {
                self.unsigned() >= right.unsigned()
            }),
            Add | Sub | Mul | Div | Rem => self.checked_arithmetic(op, right).map(Value::Int),
            BitAnd => Ok(Value::Int(Self::from_bits(
                self.bits & right.bits,
                self.width,
            ))),
            BitOr => Ok(Value::Int(Self::from_bits(
                self.bits | right.bits,
                self.width,
            ))),
            BitXor => Ok(Value::Int(Self::from_bits(
                self.bits ^ right.bits,
                self.width,
            ))),
            Shl | Shr => self.checked_shift(op, right).map(Value::Int),
            And | Or | Identical => Err(format!("operator {op:?} is not defined for integers")),
        }
    }

    fn checked_arithmetic(self, op: BinaryOp, right: Self) -> Result<Self, String> {
        use BinaryOp::*;
        if self.width.signed() {
            let result = match op {
                Add => self.signed().checked_add(right.signed()),
                Sub => self.signed().checked_sub(right.signed()),
                Mul => self.signed().checked_mul(right.signed()),
                Div => self.signed().checked_div(right.signed()),
                Rem => self.signed().checked_rem(right.signed()),
                _ => unreachable!("arithmetic operator already filtered"),
            }
            .ok_or_else(|| format!("integer {op:?} overflow or invalid domain"))?;
            let minimum = if self.width.bits() == 128 {
                i128::MIN
            } else {
                -(1_i128 << (self.width.bits() - 1))
            };
            let maximum = if self.width.bits() == 128 {
                i128::MAX
            } else {
                (1_i128 << (self.width.bits() - 1)) - 1
            };
            if !(minimum..=maximum).contains(&result) {
                return Err(format!("integer {op:?} overflow"));
            }
            Ok(Self::from_bits(result as u128, self.width))
        } else {
            let result = match op {
                Add => self.unsigned().checked_add(right.unsigned()),
                Sub => self.unsigned().checked_sub(right.unsigned()),
                Mul => self.unsigned().checked_mul(right.unsigned()),
                Div => self.unsigned().checked_div(right.unsigned()),
                Rem => self.unsigned().checked_rem(right.unsigned()),
                _ => unreachable!("arithmetic operator already filtered"),
            }
            .ok_or_else(|| format!("integer {op:?} overflow or invalid domain"))?;
            if self.width.bits() < 128 && result >= (1_u128 << self.width.bits()) {
                return Err(format!("integer {op:?} overflow"));
            }
            Ok(Self::from_bits(result, self.width))
        }
    }

    fn checked_shift(self, op: BinaryOp, right: Self) -> Result<Self, String> {
        let amount = if right.width.signed() {
            u32::try_from(right.signed()).map_err(|_| "invalid negative or oversized shift")?
        } else {
            u32::try_from(right.unsigned()).map_err(|_| "oversized shift")?
        };
        if amount >= self.width.bits() {
            return Err("shift count exceeds integer width".into());
        }
        let bits = match op {
            BinaryOp::Shl => self.bits.checked_shl(amount).ok_or("shift overflow")?,
            BinaryOp::Shr if self.width.signed() => (self.signed() >> amount) as u128,
            BinaryOp::Shr => self.bits >> amount,
            _ => unreachable!("shift operator already filtered"),
        };
        if op == BinaryOp::Shl && self.width.bits() < 128 && bits >= (1_u128 << self.width.bits()) {
            return Err("integer Shl overflow".into());
        }
        Ok(Self::from_bits(bits, self.width))
    }

    /// Mirrors the IR `IntCast` instruction: narrow by truncation and widen
    /// by sign- or zero-extension from the source width.
    const fn cast(self, target: IntWidth) -> Self {
        if target.bits() <= self.width.bits() {
            return Self::from_bits(self.bits, target);
        }
        let bits = if self.width.signed()
            && self.width.bits() < 128
            && self.bits & (1_u128 << (self.width.bits() - 1)) != 0
        {
            self.bits | (!0_u128 << self.width.bits())
        } else {
            self.bits
        };
        Self::from_bits(bits, target)
    }
}

fn float_to_integer_bits(value: f64, width: IntWidth) -> Option<u128> {
    let integer = value.trunc();
    let (minimum, upper_exclusive) = if width.signed() {
        let upper = 2.0_f64.powi((width.bits() - 1) as i32);
        (-upper, upper)
    } else {
        (0.0, 2.0_f64.powi(width.bits() as i32))
    };
    if !integer.is_finite() || integer < minimum || integer >= upper_exclusive {
        return None;
    }
    Some(if width.signed() {
        integer as i128 as u128
    } else {
        integer as u128
    })
}

/// Heap-owned values that can carry browser-local identity.
#[derive(Debug, Clone, PartialEq)]
pub enum HeapEntry {
    String(String),
    Char(String),
    Regex(String),
    Range(zirk_runtime_core::range::Range),
    Closure {
        target: String,
        captures: Vec<Value>,
    },
    Object {
        layout: u32,
        fields: Vec<Value>,
    },
    Record {
        layout: u32,
        fields: Vec<Value>,
    },
    Enum {
        layout: u32,
        variant: u32,
        fields: Vec<Value>,
    },
    Array(Vec<Value>),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Set(Vec<Value>),
    RuntimeError {
        category: String,
        message: String,
    },
}

fn decode_interpreter_http_headers(encoded: &[u8]) -> Result<Vec<(String, Vec<u8>)>, i32> {
    use zirk_runtime_core::http_values::{Headers, MAX_HTTP_HEADER_BYTES, MAX_HTTP_HEADER_COUNT};

    let Some(count_bytes) = encoded.get(..4) else {
        return Err(3);
    };
    let count = u32::from_le_bytes(count_bytes.try_into().map_err(|_| 3)?) as usize;
    if count > MAX_HTTP_HEADER_COUNT {
        return Err(12);
    }
    let mut cursor = 4usize;
    let mut total_bytes = 0usize;
    let mut validated = Headers::new();
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        let name_length = u16::from_le_bytes(
            encoded
                .get(cursor..cursor + 2)
                .ok_or(3)?
                .try_into()
                .map_err(|_| 3)?,
        );
        cursor += 2;
        let sensitive = name_length & 0x8000 != 0;
        let name_length = usize::from(name_length & 0x7fff);
        let value_length = u32::from_le_bytes(
            encoded
                .get(cursor..cursor + 4)
                .ok_or(3)?
                .try_into()
                .map_err(|_| 3)?,
        ) as usize;
        cursor += 4;
        let name = encoded.get(cursor..cursor + name_length).ok_or(3)?;
        cursor += name_length;
        let value = encoded.get(cursor..cursor + value_length).ok_or(3)?;
        cursor += value_length;
        total_bytes = total_bytes
            .checked_add(name_length)
            .and_then(|size| size.checked_add(value_length))
            .ok_or(13)?;
        if name.is_empty() || total_bytes > MAX_HTTP_HEADER_BYTES {
            return Err(if name.is_empty() { 3 } else { 13 });
        }
        let name = std::str::from_utf8(name).map_err(|_| 3)?;
        if if sensitive {
            validated.append_sensitive(name, value)
        } else {
            validated.append(name, value)
        }
        .is_err()
        {
            return Err(3);
        }
        fields.push((name.to_owned(), value.to_vec()));
    }
    if cursor != encoded.len() {
        return Err(3);
    }
    Ok(fields)
}

/// Bounds for the opt-in execution trace. Exhausting any of them stops
/// recording and marks the trace truncated; it never fails the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceLimits {
    pub max_events: usize,
    pub max_bytes: usize,
    pub max_value_bytes: usize,
}

impl TraceLimits {
    pub const MAX_EVENTS: usize = 16_384;
    pub const MAX_BYTES: usize = 1024 * 1024;
    pub const MAX_VALUE_BYTES: usize = 256;
    pub const DEFAULT: Self = Self {
        max_events: 2_048,
        max_bytes: 128 * 1024,
        max_value_bytes: 64,
    };

    /// Clamps every bound to the package maxima.
    pub const fn clamped(self) -> Self {
        Self {
            max_events: if self.max_events < Self::MAX_EVENTS {
                self.max_events
            } else {
                Self::MAX_EVENTS
            },
            max_bytes: if self.max_bytes < Self::MAX_BYTES {
                self.max_bytes
            } else {
                Self::MAX_BYTES
            },
            max_value_bytes: if self.max_value_bytes < Self::MAX_VALUE_BYTES {
                self.max_value_bytes
            } else {
                Self::MAX_VALUE_BYTES
            },
        }
    }
}

/// One read-only observation of interpreter behavior. Object ids are assigned
/// per run in first-seen order and never derive from heap identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEventKind {
    Alloc {
        object: u32,
        class: String,
    },
    FieldWrite {
        object: u32,
        field: String,
        ty: String,
        value: String,
        truncated: bool,
    },
    Call {
        callee: String,
        receiver: Option<u32>,
    },
    Return {
        callee: String,
    },
    Throw {
        object: Option<u32>,
    },
    /// Marks where the output event with this index was accepted, so
    /// consumers can interleave output with object events.
    Output {
        index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub seq: u64,
    pub kind: TraceEventKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    pub events: Vec<TraceEvent>,
    pub truncated: bool,
}

#[derive(Debug)]
struct TraceState {
    limits: TraceLimits,
    trace: Trace,
    bytes: usize,
    next_seq: u64,
    object_ids: HashMap<HeapId, u32>,
}

/// Bounded arena used for the lifetime of a single execution.
#[derive(Debug, Default)]
pub struct Heap {
    entries: Vec<HeapEntry>,
    used_bytes: usize,
}

impl Heap {
    pub fn allocate(
        &mut self,
        entry: HeapEntry,
        bytes: usize,
        limits: InterpreterLimits,
    ) -> Result<HeapId, LimitFailure> {
        let Some(next) = self.used_bytes.checked_add(bytes) else {
            return Err(LimitFailure::Heap);
        };
        if next > limits.heap_bytes {
            return Err(LimitFailure::Heap);
        }
        let id = HeapId(self.entries.len() as u32);
        self.entries.push(entry);
        self.used_bytes = next;
        Ok(id)
    }

    pub fn get(&self, id: HeapId) -> Option<&HeapEntry> {
        self.entries.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: HeapId) -> Option<&mut HeapEntry> {
        self.entries.get_mut(id.0 as usize)
    }

    pub const fn used_bytes(&self) -> usize {
        self.used_bytes
    }
}

/// One explicit interpreter call frame. Values never cross basic blocks; slots
/// carry data across control-flow edges, matching the IR contract.
#[derive(Debug, Clone)]
pub struct Frame {
    pub function: usize,
    pub block: BlockId,
    pub instruction: usize,
    pub slots: Vec<Option<Value>>,
    pub values: HashMap<ValueId, Value>,
    pub closure_environment: Option<HeapId>,
    pub return_to: Option<ReturnDestination>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnDestination {
    pub caller_value: Option<ValueId>,
}

/// A `Void`-typed instruction never produces a value, even if the lowering
/// attached a result id to it. Only non-void results become return
/// destinations, so a `Void` callee returning `None` is never awaited.
fn result_value_id(instruction: &zirk_ir::Instruction) -> Option<ValueId> {
    if instruction.ty == IrType::Void {
        None
    } else {
        instruction.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    pub function: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitFailure {
    Instructions,
    CallDepth,
    Heap,
    Output,
    Cancelled,
}

/// One ordered text emission accepted by an interpreter host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputEvent {
    pub text: String,
    /// Logical standard-stream destination. This is observable to portable
    /// hosts and deliberately does not expose a native handle.
    pub destination: OutputDestination,
    /// True only for the control event emitted when bounded program output was
    /// cut short. Its empty text is not program output and does not consume an
    /// output-event or output-byte allowance.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputDestination {
    Stdout,
    Stderr,
}

/// One bounded HTTP request handed from interpreted Zirk to an explicit
/// embedding host. The interpreter never performs ambient browser I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpHostRequest {
    pub id: u64,
    pub method: String,
    pub url: String,
    pub cached: bool,
    pub validate_json: bool,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

/// Buffered response returned by an embedding HTTP host when it resumes a
/// suspended interpreter request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpHostResponse {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

/// The entire authority surface granted to interpreted Zirk code.
///
/// Hosts may reject an event when their own bounded sink is full. They expose
/// neither native stdout nor clock, storage, network, process, or ambient
/// browser APIs.
pub trait InterpreterHost {
    fn emit(&mut self, event: OutputEvent) -> Result<(), HostOutputError>;
    fn is_cancelled(&self) -> bool;

    /// Authorizes and accepts one HTTP effect for asynchronous completion.
    /// Hosts without an explicitly scoped HTTP capability keep the default
    /// denial. Returning `true` suspends execution and exposes the request in
    /// `ExecutionResult::HttpSuspended`.
    fn begin_http_request(&mut self, _request: &HttpHostRequest) -> bool {
        false
    }

    /// Reads more bytes from the host's bounded standard-input fixture into
    /// `buf`, returning the number of bytes read (`0` at end-of-stream), the
    /// same contract as `std::io::Read::read`
    /// (`add-stdin-stdout-standard-library`).
    ///
    /// Every browser host reaches `preflight` rejecting a reachable
    /// `stdin.read_*` instruction before execution ever starts (design D5),
    /// so this default — behaving as an immediately exhausted stream — is
    /// never actually exercised there; it exists only so hosts that never
    /// grant standard input (every `InterpreterHost` but a direct interpreter
    /// unit test's fixture) do not have to implement it.
    fn read_stdin(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOutputError {
    Rejected,
}

/// Adapts an `InterpreterHost`'s bounded `read_stdin` into `std::io::Read`,
/// so `zirk_runtime_core::stdin`'s platform-neutral line/char/byte/exact
/// logic — the exact same code the native runtime calls against real
/// process stdin — runs identically against the interpreter's injected
/// fixture (`add-stdin-stdout-standard-library`, design D2/D3).
struct HostReader<'a>(&'a mut dyn InterpreterHost);

impl std::io::Read for HostReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read_stdin(buf)
    }
}

/// In-memory host used by deterministic interpreter and browser-boundary tests.
#[derive(Debug, Default)]
pub struct CapturedHost {
    events: Vec<OutputEvent>,
    cancelled: bool,
    http_enabled: bool,
    http_requests: Vec<HttpHostRequest>,
    /// A fixed byte sequence `read_stdin` serves from, for deterministic
    /// `stdin.read_*` interpreter unit tests
    /// (`add-stdin-stdout-standard-library`). Empty by default — browser
    /// hosts never reach `read_stdin` at all, since `preflight` rejects a
    /// reachable stdin read before execution starts.
    stdin: std::io::Cursor<Vec<u8>>,
}

impl CapturedHost {
    pub fn events(&self) -> &[OutputEvent] {
        &self.events
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Enables deterministic capture of browser-compatible HTTP requests for
    /// interpreter tests. It performs no network I/O.
    pub fn with_http_capability() -> Self {
        Self {
            http_enabled: true,
            ..Self::default()
        }
    }

    pub fn http_requests(&self) -> &[HttpHostRequest] {
        &self.http_requests
    }

    /// Injects a fixed byte sequence for subsequent `read_stdin` calls to
    /// serve from, in order.
    pub fn with_stdin(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            stdin: std::io::Cursor::new(bytes.into()),
            ..Self::default()
        }
    }
}

impl InterpreterHost for CapturedHost {
    fn emit(&mut self, event: OutputEvent) -> Result<(), HostOutputError> {
        // Pieces of one line (`println("a", 2)` writes `a`, ` `, `2`, then the
        // line break) are delivered as a single event, so consumers see one
        // event per line regardless of how the standard library composes it.
        if let Some(last) = self.events.last_mut()
            && !event.truncated
            && !last.truncated
            && last.destination == event.destination
            && !last.text.ends_with('\n')
        {
            last.text.push_str(&event.text);
            return Ok(());
        }
        self.events.push(event);
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn begin_http_request(&mut self, request: &HttpHostRequest) -> bool {
        if !self.http_enabled {
            return false;
        }
        self.http_requests.push(request.clone());
        true
    }

    fn read_stdin(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stdin, buf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFailure {
    pub category: String,
    pub message: String,
    pub span: Option<Span>,
    pub frames: Vec<StackFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFailure {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalFailure {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Execution paused at an instruction boundary after consuming the
    /// caller's cooperative slice. The same interpreter instance can resume
    /// with `run_main_with_host_slice`; frames, heap, budgets and tracing stay
    /// attached to that instance.
    Yielded,
    /// Execution is paused on an accepted asynchronous HTTP host effect.
    /// Resume it with the matching request id; stale and duplicate responses
    /// are rejected without changing the retained interpreter state.
    HttpSuspended {
        request: HttpHostRequest,
    },
    Completed {
        value: Option<Value>,
    },
    RuntimeFailure(RuntimeFailure),
    LimitFailure(LimitFailure),
    CapabilityFailure(CapabilityFailure),
    InternalFailure(InternalFailure),
}

/// Reviews all executable declarations before an interpreter instance can be
/// run in a browser. It is intentionally non-mutating: callers can render all
/// diagnostics, then decide whether to construct an execution.
pub fn preflight(module: &Module) -> Result<(), Vec<CapabilityFailure>> {
    preflight_with_standard_sources(module, &HashSet::new())
}

/// Like [`preflight`], while allowing the embedding host to identify trusted
/// bundled standard-library source files. Unreachable functions from those
/// files are omitted from the compatibility scan; reachable effects remain
/// subject to the same fail-closed checks. User sources are always scanned.
pub fn preflight_with_standard_sources(
    module: &Module,
    standard_sources: &HashSet<FileId>,
) -> Result<(), Vec<CapabilityFailure>> {
    let mut failures = Vec::new();
    // Standard-library implementation bodies are bundled into every module,
    // including host leaves for targets such as the browser. Follow ordinary
    // direct calls from `main` so an unused bundled leaf is not treated as
    // ambient authority. Other declarations retain the conservative
    // whole-module review below, preserving existing diagnostics for user code.
    let functions_by_name: std::collections::HashMap<&str, &zirk_ir::Function> = module
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    let mut reachable = std::collections::HashSet::from(["main"]);
    let mut pending = vec!["main"];
    while let Some(name) = pending.pop() {
        let Some(function) = functions_by_name.get(name) else {
            continue;
        };
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let InstKind::Call { callee, .. } = &instruction.kind
                    && functions_by_name.contains_key(callee.as_str())
                    && reachable.insert(callee.as_str())
                {
                    pending.push(callee.as_str());
                }
            }
        }
    }
    // Lowering currently registers a conservative catalog of native helper
    // declarations, including helpers a given module never calls. Browser
    // compatibility is about an actual requested effect, not unused metadata.
    let called_externs: std::collections::HashSet<&str> = module
        .functions
        .iter()
        .filter(|function| {
            !standard_sources.contains(&function.span.file)
                || reachable.contains(function.name.as_str())
        })
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            InstKind::Call { callee, .. } => Some(callee.as_str()),
            _ => None,
        })
        .collect();
    for external in &module.externs {
        if !called_externs.contains(external.name.as_str()) || is_browser_intrinsic(&external.name)
        {
            continue;
        }
        failures.push(CapabilityFailure {
            code: "ZB001".into(),
            message: browser_extern_capability_message(&external.name),
            span: None,
        });
        for ty in external
            .params
            .iter()
            .copied()
            .chain([external.return_type])
        {
            preflight_type(ty, None, &mut failures);
        }
    }
    for function in &module.functions {
        if standard_sources.contains(&function.span.file)
            && !reachable.contains(function.name.as_str())
        {
            continue;
        }
        preflight_type(function.return_type, Some(function.span), &mut failures);
        for slot in &function.slots {
            preflight_type(slot.ty, Some(slot.span), &mut failures);
        }
        for block in &function.blocks {
            if let Some(terminator) = &block.terminator
                && matches!(terminator_support(terminator), BrowserSupport::Excluded)
            {
                failures.push(CapabilityFailure {
                    code: "ZB003".into(),
                    message: format!("terminator {terminator:?} is unavailable in the browser"),
                    span: Some(function.span),
                });
            }
            for instruction in &block.instructions {
                preflight_type(instruction.ty, Some(instruction.span), &mut failures);
                if let InstKind::ConstFloat(FloatWidth::F128, text) = &instruction.kind {
                    // Float128 wire text only encodes finite values; flag
                    // literal spellings of infinities/NaN directly. The
                    // narrower widths carry any `f64` spelling as text.
                    let non_finite = matches!(
                        text.trim_start_matches(['+', '-'])
                            .to_ascii_lowercase()
                            .as_str(),
                        "inf" | "infinity" | "nan"
                    ) || text
                        .parse::<f64>()
                        .map(|value| !value.is_finite())
                        .unwrap_or(false);
                    if non_finite {
                        failures.push(CapabilityFailure {
                            code: "ZB002".into(),
                            message: format!(
                                "non-finite Float128 literal {text:?} is unavailable in the browser"
                            ),
                            span: Some(instruction.span),
                        });
                    }
                }
                if matches!(
                    instruction_support(&instruction.kind),
                    BrowserSupport::Excluded
                ) {
                    let (feature, variant) = excluded_feature(&instruction.kind);
                    failures.push(CapabilityFailure {
                        code: "ZB002".into(),
                        message: format!(
                            "{feature} is unavailable in the browser (instruction {variant})"
                        ),
                        span: Some(instruction.span),
                    });
                }
            }
        }
    }
    failures.sort_by_key(|failure| {
        (
            failure.span.map(|span| span.file.0).unwrap_or(u32::MAX),
            failure.span.map(|span| span.start).unwrap_or(u32::MAX),
            failure.span.map(|span| span.end).unwrap_or(u32::MAX),
            failure.code.clone(),
            failure.message.clone(),
        )
    });
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

/// Explains which host capability a bundled native binding needs. Browser
/// execution stays fail-closed, but diagnostics should identify the actual
/// boundary instead of exposing an implementation-only extern symbol.
fn browser_extern_capability_message(name: &str) -> String {
    if name.starts_with("zirk_environment_") {
        return "process environment access is unavailable in the browser".into();
    }
    if name.starts_with("zirk_http_") {
        return "native std.http transport is unavailable in the browser; this run has no injected browser HTTP capability".into();
    }
    if name.starts_with("zirk_network_udp_") {
        return "native UDP sockets are unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_local_") {
        return "native local sockets are unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_tls_") {
        return "native TLS sockets are unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_tcp_") {
        return "native TCP sockets are unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_dns_") {
        return "native DNS access is unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_interfaces_") {
        return "network interface inspection is unavailable in the browser".into();
    }
    if name.starts_with("zirk_network_") {
        return "native network host access is unavailable in the browser".into();
    }
    format!("extern `{name}` is unavailable in the browser")
}

/// Externs that need a host clock the browser boundary deliberately does not
/// grant: `Date.today`, `Time.now_*` and `DateTime.now_*` are platform
/// authority, not portable value semantics.
fn is_host_clock_extern(name: &str) -> bool {
    matches!(
        name,
        "zirk_rt_date_today"
            | "zirk_rt_time_now_utc"
            | "zirk_rt_time_now_local"
            | "zirk_rt_datetime_now_utc"
            | "zirk_rt_datetime_now_local"
    )
}

fn is_browser_intrinsic(name: &str) -> bool {
    if is_host_clock_extern(name) {
        return false;
    }
    if name.starts_with("zirk_regex_")
        || name.starts_with("zirk_char_")
        || name.starts_with("zirk_str_")
        || name.starts_with("zirk_duration_")
        || name.starts_with("zirk_rt_duration_")
        || name.starts_with("zirk_range_")
        || name.starts_with("zirk_rt_decimal_")
        || name.starts_with("zirk_int_")
        || name.starts_with("zirk_float_")
        || name.starts_with("zirk_rt_date_")
        || name.starts_with("zirk_rt_time_")
        || name.starts_with("zirk_rt_datetime_")
    {
        return true;
    }
    matches!(
        name,
        "zirk_rt_array_from_list" | "zirk_rt_map_sorted_keys" | "zirk_rt_set_sorted_elements"
    )
}

/// Names the Zirk feature behind a browser-excluded instruction so a learner
/// reads "deep `clone()` is unavailable" instead of an IR debug dump. Returns
/// the feature label and the IR variant name; unknown variants fall back to
/// the variant name alone, so a new excluded instruction never loses its
/// diagnostic.
fn excluded_feature(kind: &InstKind) -> (String, String) {
    let debug = format!("{kind:?}");
    let variant = debug
        .split(['(', ' ', '{'])
        .next()
        .unwrap_or_default()
        .to_string();
    let feature = match variant.as_str() {
        "ScopeEnter"
        | "ScopeExit"
        | "ScopeEnterCollectAll"
        | "ScopeExitCollectAll"
        | "ScopeCancel"
        | "BranchStart" => "structured concurrency (`concurrent`, `spawn`)",
        "BackgroundStart" | "AppSetShutdownGrace" | "AppSetBackgroundError" => {
            "managed background work"
        }
        "ThreadRun" => "managed native threads",
        "ParallelForStart" => "`parallel` regions",
        "JobWait"
        | "JobWaitAny"
        | "JobWaitAnyWithTimer"
        | "JobDone"
        | "JobCancel"
        | "JobCancelWithCleanupFailure" => "job handles",
        "JobOutcomeWaitTake" | "ScopeOutcomeWaitTake" | "JobOutcomeTake" | "JobResultLoad" => {
            "managed completion outcomes"
        }
        "EnvironmentGet" | "EnvironmentContains" | "EnvironmentListNames" => {
            "process environment access"
        }
        "NetworkDnsResolve" => "native DNS access",
        "NetworkInterfacesSnapshot" => "network interface inspection",
        "NetworkIpAddressParse" => "IP address parsing",
        "NetworkTcpOriginNormalize" => "TCP endpoint normalization",
        "HttpServerControlCreate" | "HttpServerControlRequestShutdown" => {
            "native HTTP server lifecycle control"
        }
        name if name.starts_with("NetworkUdp") => "native UDP sockets",
        name if name.starts_with("NetworkLocal") => "native local sockets",
        name if name.starts_with("NetworkTls") => "native TLS sockets",
        name if name.starts_with("NetworkTcp") => "native TCP sockets",
        name if name.starts_with("Http") => "native HTTP transport",
        "TimerSleep" => "timers",
        "ResourceTransfer" => "resource transfer",
        "DependentFrom" => "dependent references",
        name if name.starts_with("Atomic") => "managed `Atomic<T>` operations",
        "PinObject" | "UnpinObject" => "pinning",
        "MutexAcquire" | "MutexRelease" => "synchronization locks",
        name if name.starts_with("Channel") => "channels",
        name if name.starts_with("Pointer") => "`Pointer<T>` operations",
        name if name.starts_with("Weak") => "`Weak<T>`",
        name if name.starts_with("Journal") => "`unsafe` journal blocks",
        name if name.starts_with("NativeSlice") => "native slices",
        name if name.starts_with("Stdin") => "standard input",
        other => return (format!("instruction {other}"), variant),
    };
    (feature.to_string(), variant)
}

fn preflight_type(ty: IrType, span: Option<Span>, failures: &mut Vec<CapabilityFailure>) {
    if matches!(type_support(ty), BrowserSupport::Excluded) {
        failures.push(CapabilityFailure {
            code: "ZB004".into(),
            message: format!("type {} is unavailable in the browser", ty.as_str()),
            span,
        });
    }
}

/// Internal control flow for instruction execution. It is deliberately kept
/// private so callers only observe the stable `ExecutionResult` contract.
#[derive(Debug, PartialEq, Eq)]
enum ExecutionFault {
    Runtime { category: String, message: String },
    Internal(String),
    Limit(LimitFailure),
}

#[derive(Debug, Clone)]
struct ThrowableInfo {
    value: Value,
    span: Span,
    frames: Vec<StackFrame>,
    suppressed: Vec<Value>,
}

#[derive(Debug, Clone, Copy)]
struct PendingHttpContinuation {
    request_id: u64,
    frame_index: usize,
    result: ValueId,
    enum_id: u32,
    response_class_id: u32,
    span: Span,
}

impl From<String> for ExecutionFault {
    fn from(message: String) -> Self {
        Self::Internal(message)
    }
}

/// Execution instance for one verified module. Dropping it drops the complete
/// arena and all frames, so no state can leak into another browser run.
pub struct Interpreter<'module> {
    module: &'module Module,
    limits: InterpreterLimits,
    heap: Heap,
    frames: Vec<Frame>,
    executed_steps: u64,
    output_bytes: usize,
    output_events: u32,
    /// Stream of an output line that has been started but not ended by a
    /// newline. Pieces written to it continue the same logical event.
    open_output_line: Option<OutputDestination>,
    output_truncated: bool,
    cancellation_shield_depth: u32,
    main_started: bool,
    main_paused: bool,
    pending_exception: Option<HeapId>,
    throwables: HashMap<HeapId, ThrowableInfo>,
    trace: Option<TraceState>,
    bundled_standard_sources: HashSet<FileId>,
    next_http_request_id: u64,
    pending_http: Option<PendingHttpContinuation>,
}

impl<'module> Interpreter<'module> {
    pub fn new(
        module: &'module Module,
        limits: InterpreterLimits,
    ) -> Result<Self, InternalFailure> {
        limits.validate().map_err(|error| InternalFailure {
            message: format!("invalid interpreter limits: {error:?}"),
            span: None,
        })?;
        verify(module).map_err(|errors| InternalFailure {
            message: format!("IR verification failed: {}", errors[0]),
            span: None,
        })?;
        Ok(Self {
            module,
            limits,
            heap: Heap::default(),
            frames: Vec::new(),
            executed_steps: 0,
            output_bytes: 0,
            output_events: 0,
            open_output_line: None,
            output_truncated: false,
            cancellation_shield_depth: 0,
            main_started: false,
            main_paused: false,
            pending_exception: None,
            throwables: HashMap::new(),
            trace: None,
            bundled_standard_sources: HashSet::new(),
            next_http_request_id: 1,
            pending_http: None,
        })
    }

    /// Marks compiler-bundled source files as trusted standard-library code
    /// for dead-code compatibility scanning. The host must derive this set
    /// from its immutable compiler bundle, never from user-provided source
    /// labels. Reachable code is still fully preflighted.
    pub fn set_bundled_standard_sources(&mut self, sources: impl IntoIterator<Item = FileId>) {
        self.bundled_standard_sources = sources.into_iter().collect();
    }

    /// Enables the read-only execution trace. Tracing has no effect on
    /// program behavior and is not charged to the instruction budget.
    pub fn enable_trace(&mut self, limits: TraceLimits) {
        self.trace = Some(TraceState {
            limits: limits.clamped(),
            trace: Trace::default(),
            bytes: 0,
            next_seq: 1,
            object_ids: HashMap::new(),
        });
    }

    /// Takes the recorded trace, leaving tracing enabled and empty.
    pub fn take_trace(&mut self) -> Trace {
        self.trace
            .as_mut()
            .map(|state| std::mem::take(&mut state.trace))
            .unwrap_or_default()
    }

    pub fn heap_bytes(&self) -> usize {
        self.heap.used_bytes()
    }

    pub fn capture_stack(&self) -> Vec<StackFrame> {
        self.frames
            .iter()
            .rev()
            .take(self.limits.stack_frames as usize)
            .filter_map(|frame| {
                self.module
                    .functions
                    .get(frame.function)
                    .map(|function| StackFrame {
                        function: function.name.clone(),
                        span: Some(function.span),
                    })
            })
            .collect()
    }

    /// Executes the `main` entrypoint for the browser-compatible scalar
    /// literal subset. Subsequent execution tasks extend this loop without
    /// changing the frame or result boundary.
    pub fn run_main(&mut self) -> ExecutionResult {
        let mut host = CapturedHost::default();
        self.run_main_with_host(&mut host)
    }

    /// Executes `main` using the sole authority granted to interpreted code.
    /// The caller owns the output sink and cooperative cancellation state.
    pub fn run_main_with_host(&mut self, host: &mut dyn InterpreterHost) -> ExecutionResult {
        loop {
            let result = self.run_main_with_host_slice(host, self.limits.instructions);
            match result {
                ExecutionResult::Yielded => continue,
                result => return result,
            }
        }
    }

    /// Runs at most `max_steps` instruction or terminator steps before
    /// returning `Yielded`. Call this method again on the same instance to
    /// resume. The slice is a scheduling hint; the overall instruction limit,
    /// heap, cancellation state, output budget, frames and trace remain
    /// continuous. Browser hosts can use this boundary to return to the
    /// JavaScript event loop while retaining the execution for an async host
    /// effect.
    pub fn run_main_with_host_slice(
        &mut self,
        host: &mut dyn InterpreterHost,
        max_steps: u64,
    ) -> ExecutionResult {
        if max_steps == 0 {
            return ExecutionResult::InternalFailure(InternalFailure {
                message: "interpreter slice must allow at least one instruction".into(),
                span: None,
            });
        }
        if self.main_started && !self.main_paused {
            return ExecutionResult::InternalFailure(InternalFailure {
                message: "interpreter execution is not suspended".into(),
                span: None,
            });
        }
        if self.pending_http.is_some() {
            if host.is_cancelled() {
                self.pending_http = None;
                self.main_paused = false;
                return ExecutionResult::LimitFailure(LimitFailure::Cancelled);
            }
            return self.internal("interpreter has an HTTP request awaiting a response", None);
        }
        self.main_paused = false;

        if !self.main_started {
            if let Err(mut failures) =
                preflight_with_standard_sources(self.module, &self.bundled_standard_sources)
            {
                return ExecutionResult::CapabilityFailure(failures.remove(0));
            }
            let Some(function) = self
                .module
                .functions
                .iter()
                .position(|function| function.name == "main")
            else {
                return ExecutionResult::InternalFailure(InternalFailure {
                    message: "verified module has no main entrypoint".into(),
                    span: None,
                });
            };
            if host.is_cancelled() {
                self.pending_http = None;
                return ExecutionResult::LimitFailure(LimitFailure::Cancelled);
            }
            if let Err(failure) = self.push_frame(function, Vec::new(), None) {
                return ExecutionResult::LimitFailure(failure);
            }
            self.main_started = true;
        }

        let mut slice_steps = 0;
        loop {
            if self.executed_steps >= self.limits.instructions {
                return ExecutionResult::LimitFailure(LimitFailure::Instructions);
            }
            if host.is_cancelled() {
                self.pending_http = None;
                self.main_paused = false;
                return ExecutionResult::LimitFailure(LimitFailure::Cancelled);
            }
            if slice_steps >= max_steps {
                self.main_paused = true;
                return ExecutionResult::Yielded;
            }
            self.executed_steps += 1;
            slice_steps += 1;
            let frame_index = self.frames.len() - 1;
            let (instruction, terminator) = {
                let frame = &self.frames[frame_index];
                let function = &self.module.functions[frame.function];
                let Some(block) = function.block(frame.block) else {
                    return self.internal("frame references a missing basic block", None);
                };
                if let Some(instruction) = block.instructions.get(frame.instruction) {
                    (Some(instruction.clone()), None)
                } else {
                    (None, block.terminator.clone())
                }
            };

            if let Some(instruction) = instruction {
                self.frames[frame_index].instruction += 1;
                match self.browser_http_convenience_request(frame_index, &instruction.kind) {
                    Ok(Some((request, enum_id, response_class_id))) => {
                        return self.suspend_http_request(
                            frame_index,
                            &instruction,
                            host,
                            request,
                            enum_id,
                            response_class_id,
                        );
                    }
                    Ok(None) => {}
                    Err(ExecutionFault::Internal(message)) => {
                        return self.internal(message, Some(instruction.span));
                    }
                    Err(ExecutionFault::Limit(failure)) => {
                        return ExecutionResult::LimitFailure(failure);
                    }
                    Err(ExecutionFault::Runtime { category, message }) => {
                        return ExecutionResult::RuntimeFailure(RuntimeFailure {
                            category,
                            message,
                            span: Some(instruction.span),
                            frames: self.capture_stack(),
                        });
                    }
                }
                if let InstKind::HttpGet {
                    url,
                    cached,
                    enum_id,
                    response_class_id,
                    ..
                } = &instruction.kind
                {
                    let url = match self.string_operand(frame_index, *url) {
                        Ok(url) => url,
                        Err(ExecutionFault::Internal(message)) => {
                            return self.internal(message, Some(instruction.span));
                        }
                        Err(ExecutionFault::Limit(failure)) => {
                            return ExecutionResult::LimitFailure(failure);
                        }
                        Err(ExecutionFault::Runtime { category, message }) => {
                            return ExecutionResult::RuntimeFailure(RuntimeFailure {
                                category,
                                message,
                                span: Some(instruction.span),
                                frames: self.capture_stack(),
                            });
                        }
                    };
                    let request_id = self.next_http_request_id;
                    let Some(next_request_id) = request_id.checked_add(1) else {
                        return self
                            .internal("HTTP request id space exhausted", Some(instruction.span));
                    };
                    let Some(result) = instruction.result else {
                        return self.internal(
                            "HTTP GET has no result destination",
                            Some(instruction.span),
                        );
                    };
                    let request = HttpHostRequest {
                        id: request_id,
                        method: "GET".into(),
                        url,
                        cached: *cached,
                        validate_json: false,
                        headers: Vec::new(),
                        body: Vec::new(),
                    };
                    if !host.begin_http_request(&request) {
                        return ExecutionResult::CapabilityFailure(CapabilityFailure {
                            code: "ZB006".into(),
                            message: "HTTP requires an explicit host capability for this execution"
                                .into(),
                            span: Some(instruction.span),
                        });
                    }
                    self.next_http_request_id = next_request_id;
                    self.pending_http = Some(PendingHttpContinuation {
                        request_id,
                        frame_index,
                        result,
                        enum_id: *enum_id,
                        response_class_id: *response_class_id,
                        span: instruction.span,
                    });
                    self.main_paused = true;
                    return ExecutionResult::HttpSuspended { request };
                }
                if let InstKind::HttpRequest {
                    method,
                    url,
                    body,
                    headers,
                    validate_json,
                    cached,
                    enum_id,
                    response_class_id,
                } = &instruction.kind
                {
                    let request_values = (|| {
                        let method = self.string_operand(frame_index, *method)?;
                        let url = self.string_operand(frame_index, *url)?;
                        let body = self.byte_array_operand(frame_index, *body)?;
                        let header_block = self.byte_array_operand(frame_index, *headers)?;
                        let headers =
                            decode_interpreter_http_headers(&header_block).map_err(|_| {
                                ExecutionFault::Runtime {
                                    category: "HttpError".into(),
                                    message: "invalid HTTP request headers".into(),
                                }
                            })?;
                        let validate_json = matches!(
                            self.value(frame_index, *validate_json),
                            Ok(Value::Boolean(true))
                        );
                        Ok((method, url, body, headers, validate_json))
                    })();
                    let (method, url, body, headers, validate_json) = match request_values {
                        Ok(values) => values,
                        Err(ExecutionFault::Runtime { .. }) => {
                            let Some(result) = instruction.result else {
                                return self.internal(
                                    "HTTP request has no result destination",
                                    Some(instruction.span),
                                );
                            };
                            match self.build_tagged_outcome(*enum_id, 1, 3, None, 0, 1) {
                                Ok(value) => {
                                    self.frames[frame_index].values.insert(result, value);
                                    continue;
                                }
                                Err(ExecutionFault::Limit(failure)) => {
                                    return ExecutionResult::LimitFailure(failure);
                                }
                                Err(ExecutionFault::Internal(message)) => {
                                    return self.internal(message, Some(instruction.span));
                                }
                                Err(ExecutionFault::Runtime { category, message }) => {
                                    return ExecutionResult::RuntimeFailure(RuntimeFailure {
                                        category,
                                        message,
                                        span: Some(instruction.span),
                                        frames: self.capture_stack(),
                                    });
                                }
                            }
                        }
                        Err(ExecutionFault::Internal(message)) => {
                            return self.internal(message, Some(instruction.span));
                        }
                        Err(ExecutionFault::Limit(failure)) => {
                            return ExecutionResult::LimitFailure(failure);
                        }
                    };
                    let request_id = self.next_http_request_id;
                    let Some(next_request_id) = request_id.checked_add(1) else {
                        return self
                            .internal("HTTP request id space exhausted", Some(instruction.span));
                    };
                    let Some(result) = instruction.result else {
                        return self.internal(
                            "HTTP request has no result destination",
                            Some(instruction.span),
                        );
                    };
                    let request = HttpHostRequest {
                        id: request_id,
                        method,
                        url,
                        cached: *cached,
                        validate_json,
                        headers,
                        body,
                    };
                    if !host.begin_http_request(&request) {
                        return ExecutionResult::CapabilityFailure(CapabilityFailure {
                            code: "ZB006".into(),
                            message: "HTTP requires an explicit host capability for this execution"
                                .into(),
                            span: Some(instruction.span),
                        });
                    }
                    self.next_http_request_id = next_request_id;
                    self.pending_http = Some(PendingHttpContinuation {
                        request_id,
                        frame_index,
                        result,
                        enum_id: *enum_id,
                        response_class_id: *response_class_id,
                        span: instruction.span,
                    });
                    self.main_paused = true;
                    return ExecutionResult::HttpSuspended { request };
                }
                let value = match self.execute_instruction(frame_index, &instruction, host) {
                    Ok(value) => value,
                    Err(ExecutionFault::Runtime { category, message }) => {
                        return ExecutionResult::RuntimeFailure(RuntimeFailure {
                            category,
                            message,
                            span: Some(instruction.span),
                            frames: self.capture_stack(),
                        });
                    }
                    Err(ExecutionFault::Internal(message)) => {
                        return self.internal(message, Some(instruction.span));
                    }
                    Err(ExecutionFault::Limit(failure)) => {
                        return ExecutionResult::LimitFailure(failure);
                    }
                };
                if let (Some(result), Some(value)) = (instruction.result, value) {
                    self.frames[frame_index].values.insert(result, value);
                    continue;
                }
                if instruction.result.is_some() {
                    return self.internal(
                        "instruction result was not materialized",
                        Some(instruction.span),
                    );
                }
                continue;
            }

            match terminator {
                Some(Terminator::Return(value)) => {
                    if self.frames.len() == 1
                        && let Some(exception) = self.pending_exception
                    {
                        return self.uncaught_exception(exception);
                    }
                    let value = value.and_then(|operand| {
                        self.frames[frame_index].values.get(&operand.0).cloned()
                    });
                    let returned = self.frames.pop().expect("active frame exists");
                    if self.trace.is_some() {
                        let callee = self.module.functions[returned.function].name.clone();
                        self.trace_record(TraceEventKind::Return { callee });
                    }
                    match returned.return_to {
                        None => return ExecutionResult::Completed { value },
                        Some(destination) => {
                            let Some(caller) = self.frames.last_mut() else {
                                return self
                                    .internal("callee returned without a caller frame", None);
                            };
                            if let Some(result) = destination.caller_value {
                                let value = value.ok_or_else(|| {
                                    ExecutionFault::Internal(
                                        "non-void call returned no value".into(),
                                    )
                                });
                                match value {
                                    Ok(value) => {
                                        caller.values.insert(result, value);
                                    }
                                    Err(ExecutionFault::Internal(message)) => {
                                        return self.internal(message, None);
                                    }
                                    Err(ExecutionFault::Runtime { category, message }) => {
                                        return ExecutionResult::RuntimeFailure(RuntimeFailure {
                                            category,
                                            message,
                                            span: None,
                                            frames: self.capture_stack(),
                                        });
                                    }
                                    Err(ExecutionFault::Limit(failure)) => {
                                        return ExecutionResult::LimitFailure(failure);
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Terminator::Cancel) => {
                    return ExecutionResult::LimitFailure(LimitFailure::Cancelled);
                }
                Some(Terminator::Jump(target)) => {
                    self.enter_block(frame_index, target);
                }
                Some(Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                }) => {
                    let condition = self.frames[frame_index].values.get(&condition.0).cloned();
                    match condition {
                        Some(Value::Boolean(true)) => self.enter_block(frame_index, then_block),
                        Some(Value::Boolean(false)) => self.enter_block(frame_index, else_block),
                        Some(_) => return self.internal("branch condition is not Boolean", None),
                        None => return self.internal("branch condition is unavailable", None),
                    }
                }
                Some(Terminator::Unreachable) => {
                    let span = self.module.functions[self.frames[frame_index].function].span;
                    return self.internal("execution reached Unreachable terminator", Some(span));
                }
                None => return self.internal("verified block has no terminator", None),
            }
        }
    }

    /// Completes the one outstanding buffered HTTP request. The request id is
    /// single-use; this keeps late or duplicate browser `fetch` completions
    /// from writing into a different interpreter continuation.
    pub fn resume_http_response(
        &mut self,
        request_id: u64,
        response: HttpHostResponse,
    ) -> Result<(), InternalFailure> {
        let pending = self.pending_http.ok_or_else(|| InternalFailure {
            message: "interpreter has no pending HTTP request".into(),
            span: None,
        })?;
        if pending.request_id != request_id {
            return Err(InternalFailure {
                message: "HTTP response request id does not match the pending request".into(),
                span: Some(pending.span),
            });
        }

        let result = match self.build_http_response_result(pending, response) {
            Ok(value) => value,
            Err(ExecutionFault::Limit(failure)) => {
                return Err(InternalFailure {
                    message: format!(
                        "HTTP response exceeded interpreter resource limits: {failure:?}"
                    ),
                    span: Some(pending.span),
                });
            }
            Err(ExecutionFault::Internal(message)) => {
                return Err(InternalFailure {
                    message,
                    span: Some(pending.span),
                });
            }
            Err(ExecutionFault::Runtime { message, .. }) => {
                return Err(InternalFailure {
                    message,
                    span: Some(pending.span),
                });
            }
        };
        let frame = self
            .frames
            .get_mut(pending.frame_index)
            .ok_or_else(|| InternalFailure {
                message: "HTTP continuation references a missing frame".into(),
                span: Some(pending.span),
            })?;
        frame.values.insert(pending.result, result);
        self.pending_http = None;
        self.main_paused = true;
        Ok(())
    }

    /// Completes the pending request with one of the stable `HTTPError`
    /// discriminants declared in `std.http`. Invalid discriminants are
    /// rejected before changing the continuation.
    pub fn resume_http_error(
        &mut self,
        request_id: u64,
        error_code: i32,
    ) -> Result<(), InternalFailure> {
        if !(0..=16).contains(&error_code) {
            return Err(InternalFailure {
                message: "HTTP host error code is outside the HTTPError enum".into(),
                span: None,
            });
        }
        let pending = self.pending_http.ok_or_else(|| InternalFailure {
            message: "interpreter has no pending HTTP request".into(),
            span: None,
        })?;
        if pending.request_id != request_id {
            return Err(InternalFailure {
                message: "HTTP error request id does not match the pending request".into(),
                span: Some(pending.span),
            });
        }
        let result = self
            .build_tagged_outcome(pending.enum_id, 1, error_code, None, 0, 1)
            .map_err(|fault| InternalFailure {
                message: format!("could not materialize HTTP error: {fault:?}"),
                span: Some(pending.span),
            })?;
        let frame = self
            .frames
            .get_mut(pending.frame_index)
            .ok_or_else(|| InternalFailure {
                message: "HTTP continuation references a missing frame".into(),
                span: Some(pending.span),
            })?;
        frame.values.insert(pending.result, result);
        self.pending_http = None;
        self.main_paused = true;
        Ok(())
    }

    fn build_http_response_result(
        &mut self,
        pending: PendingHttpContinuation,
        response: HttpHostResponse,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::http_values::{Headers, MAX_HTTP_BUFFERED_BODY_BYTES};

        if response.body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
            return self.build_tagged_outcome(pending.enum_id, 1, 4, None, 0, 1);
        }
        if !(100..=999).contains(&response.status) {
            return self.build_tagged_outcome(pending.enum_id, 1, 3, None, 0, 1);
        }
        let mut headers = Headers::new();
        for (name, value) in response.headers {
            if headers.append(&name, &value).is_err() {
                return self.build_tagged_outcome(pending.enum_id, 1, 3, None, 0, 1);
            }
        }
        let count = headers.iter().count();
        let mut encoded_headers = Vec::with_capacity(4);
        encoded_headers.extend_from_slice(&(count as u32).to_le_bytes());
        for (name, value) in headers.iter() {
            let name = name.as_str().as_bytes();
            let value = value.as_bytes();
            let Ok(name_len) = u16::try_from(name.len()) else {
                return self.build_tagged_outcome(pending.enum_id, 1, 3, None, 0, 1);
            };
            let Ok(value_len) = u32::try_from(value.len()) else {
                return self.build_tagged_outcome(pending.enum_id, 1, 3, None, 0, 1);
            };
            encoded_headers.extend_from_slice(&name_len.to_le_bytes());
            encoded_headers.extend_from_slice(&value_len.to_le_bytes());
            encoded_headers.extend_from_slice(name);
            encoded_headers.extend_from_slice(value);
        }

        let response_layout = self
            .module
            .objects
            .get(pending.response_class_id as usize)
            .ok_or_else(|| {
                ExecutionFault::Internal("HTTP response references a missing class layout".into())
            })?
            .clone();
        if response_layout.name != "HTTPResponse" {
            return Err(ExecutionFault::Internal(
                "HTTP response class id does not identify the official HTTPResponse layout".into(),
            ));
        }
        let mut fields = Vec::with_capacity(response_layout.fields.len());
        for field in &response_layout.fields {
            fields.push(self.default_value(field.ty)?);
        }
        let status = IntegerValue::from_literal(i128::from(response.status), IntWidth::I32)
            .map_err(ExecutionFault::Internal)?;
        let status_index = response_layout
            .field_index("status_code_value")
            .ok_or_else(|| {
                ExecutionFault::Internal("HTTPResponse has no status_code_value field".into())
            })?;
        fields[status_index] = Value::Int(status);
        let body_index = response_layout.field_index("body_value").ok_or_else(|| {
            ExecutionFault::Internal("HTTPResponse has no body_value field".into())
        })?;
        fields[body_index] = self.allocate_byte_array(response.body)?;
        let headers_index = response_layout.field_index("header_data").ok_or_else(|| {
            ExecutionFault::Internal("HTTPResponse has no header_data field".into())
        })?;
        fields[headers_index] = self.allocate_byte_array(encoded_headers)?;
        for (field_name, value) in [
            ("body_consumed", Value::Boolean(false)),
            ("server_producer_handle", Self::zero_integer(IntWidth::U64)?),
            (
                "server_sse_heartbeat_ms",
                Self::zero_integer(IntWidth::I32)?,
            ),
            ("stream_handle", Self::zero_integer(IntWidth::U64)?),
            ("protocol_value", Self::zero_integer(IntWidth::I32)?),
        ] {
            let index = response_layout.field_index(field_name).ok_or_else(|| {
                ExecutionFault::Internal(format!("HTTPResponse has no {field_name} field"))
            })?;
            fields[index] = value;
        }
        let bytes = std::mem::size_of::<HeapEntry>() + fields.len() * std::mem::size_of::<Value>();
        let response_object = self
            .heap
            .allocate(
                HeapEntry::Object {
                    layout: pending.response_class_id,
                    fields,
                },
                bytes,
                self.limits,
            )
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)?;
        self.build_tagged_outcome(pending.enum_id, 0, 0, Some(response_object), 0, 1)
    }

    fn zero_integer(width: IntWidth) -> Result<Value, ExecutionFault> {
        IntegerValue::from_literal(0, width)
            .map(Value::Int)
            .map_err(ExecutionFault::Internal)
    }

    fn enter_block(&mut self, frame_index: usize, block: BlockId) {
        let frame = &mut self.frames[frame_index];
        frame.block = block;
        frame.instruction = 0;
        frame.values.clear();
    }

    fn push_frame(
        &mut self,
        function: usize,
        arguments: Vec<Value>,
        return_to: Option<ReturnDestination>,
    ) -> Result<(), LimitFailure> {
        if self.frames.len() >= self.limits.call_depth as usize {
            return Err(LimitFailure::CallDepth);
        }
        let entry = &self.module.functions[function];
        if entry.params.len() != arguments.len() {
            // The verifier proves direct-call arity, so a mismatch is a
            // compiler fault rather than user-triggerable runtime behavior.
            return Err(LimitFailure::CallDepth);
        }
        if self.trace.is_some() {
            let callee = entry.name.clone();
            let receiver = match arguments.first() {
                Some(Value::Heap(id))
                    if matches!(self.heap.get(*id), Some(HeapEntry::Object { .. })) =>
                {
                    self.trace_object_id(*id)
                }
                _ => None,
            };
            self.trace_record(TraceEventKind::Call { callee, receiver });
        }
        let entry = &self.module.functions[function];
        let mut slots = vec![None; entry.slots.len()];
        for (slot, value) in entry.params.iter().zip(arguments) {
            slots[slot.0 as usize] = Some(value);
        }
        self.frames.push(Frame {
            function,
            block: entry.entry,
            instruction: 0,
            slots,
            values: HashMap::new(),
            closure_environment: None,
            return_to,
        });
        Ok(())
    }

    fn push_dispatch_frame(
        &mut self,
        callee: &str,
        receiver: Value,
        args: &[zirk_ir::Operand],
        frame_index: usize,
        result: Option<ValueId>,
    ) -> Result<(), ExecutionFault> {
        let function = self
            .module
            .functions
            .iter()
            .position(|function| function.name == callee)
            .ok_or_else(|| {
                ExecutionFault::Internal(format!(
                    "dispatch table references unavailable interpreted function {callee:?}"
                ))
            })?;
        let mut arguments = Vec::with_capacity(args.len() + 1);
        arguments.push(receiver);
        arguments.extend(
            args.iter()
                .map(|operand| self.value(frame_index, *operand))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ExecutionFault::Internal)?,
        );
        self.push_frame(
            function,
            arguments,
            Some(ReturnDestination {
                caller_value: result,
            }),
        )
        .map_err(ExecutionFault::Limit)
    }

    fn exception_id(&self, value: &Value) -> Result<HeapId, ExecutionFault> {
        match value {
            Value::Heap(id) if matches!(self.heap.get(*id), Some(HeapEntry::Object { .. })) => {
                Ok(*id)
            }
            _ => Err(ExecutionFault::Internal(
                "throwable value is not an object".into(),
            )),
        }
    }

    fn record_throwable(&mut self, value: Value, span: Span) -> Result<HeapId, ExecutionFault> {
        let id = self.exception_id(&value)?;
        let frames = self.capture_stack();
        self.throwables.entry(id).or_insert_with(|| ThrowableInfo {
            value,
            span,
            frames,
            suppressed: Vec::new(),
        });
        Ok(id)
    }

    fn uncaught_exception(&self, id: HeapId) -> ExecutionResult {
        let Some(info) = self.throwables.get(&id) else {
            return self.internal("pending exception metadata is unavailable", None);
        };
        let (category, message) = match self.heap.get(id) {
            Some(HeapEntry::Object { layout, fields }) => {
                let name = self
                    .module
                    .objects
                    .get(*layout as usize)
                    .map(|layout| layout.name.clone())
                    .unwrap_or_else(|| "Throwable".into());
                // Every throwable carries its human-readable text in the
                // `reason` field the constructor stores first — the same
                // field `message()` reads. Falling back to the class name
                // keeps a hand-built or unusual throwable still printable
                // without leaking `HeapId` internals.
                let reason = self
                    .module
                    .objects
                    .get(*layout as usize)
                    .and_then(|layout| layout.field_index("reason"))
                    .and_then(|index| fields.get(index))
                    .and_then(|value| match value {
                        Value::Heap(id) => match self.heap.get(*id) {
                            Some(HeapEntry::String(text)) => Some(text.clone()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or_else(|| name.clone());
                (name, reason)
            }
            Some(HeapEntry::RuntimeError { category, message }) => {
                (category.clone(), message.clone())
            }
            _ => ("Throwable".into(), "uncaught throwable".into()),
        };
        ExecutionResult::RuntimeFailure(RuntimeFailure {
            category,
            message,
            span: Some(info.span),
            frames: info.frames.clone(),
        })
    }

    fn materialize_constant(&self, instruction: &zirk_ir::Instruction) -> Result<Value, String> {
        match &instruction.kind {
            InstKind::ConstInt(value) => match instruction.ty {
                IrType::Int(width) => IntegerValue::from_literal(*value, width).map(Value::Int),
                ty => Err(format!("ConstInt has non-integer type {ty:?}")),
            },
            InstKind::ConstBool(value) => Ok(Value::Boolean(*value)),
            InstKind::Undefined => Ok(Value::Undefined),
            InstKind::ConstFloat(width, text) => Ok(Value::Float {
                width: *width,
                text: if *width == FloatWidth::F128 {
                    zirk_runtime_core::binary_float::Float128::from_decimal(text)
                        .map_err(|error| format!("invalid Float128 literal {text:?}: {error:?}"))?
                        .wire_text()
                } else {
                    let value = Self::parse_binary_float(text)
                        .map_err(|_| format!("invalid {width:?} literal {text:?}"))?;
                    Self::format_binary_float(Self::round_binary_float(value, *width), *width)
                },
            }),
            InstKind::ConstDecimal(text) => Ok(Value::Decimal(text.clone())),
            unsupported => Err(format!(
                "instruction {unsupported:?} is not implemented yet"
            )),
        }
    }

    fn execute_instruction(
        &mut self,
        frame_index: usize,
        instruction: &zirk_ir::Instruction,
        host: &mut dyn InterpreterHost,
    ) -> Result<Option<Value>, ExecutionFault> {
        match &instruction.kind {
            // The interpreter owns its heap and does not run generated code on
            // concurrent managed mutators. Polls are therefore semantic
            // no-ops here; native codegen maps the same IR instruction to the
            // runtime collector ABI.
            InstKind::SafepointPoll => Ok(None),
            // Job values and the completion-order scheduler are not modeled by
            // this interpreter. Browser preflight rejects this operation; keep
            // a defensive failure here so future execution paths cannot invent
            // a completion index or confuse cancellation with a completed job.
            InstKind::JobWaitAny { .. } => Err(ExecutionFault::Internal(
                "JobWaitAny cannot execute without interpreter job handles and a scheduler".into(),
            )),
            InstKind::JobWaitAnyWithTimer { .. } => Err(ExecutionFault::Internal(
                "JobWaitAnyWithTimer cannot execute without interpreter job handles and a scheduler"
                    .into(),
            )),
            InstKind::ThreadRun { .. } => Err(ExecutionFault::Internal(
                "ThreadRun requires the native managed-thread runtime and is unavailable in the interpreter".into(),
            )),
            InstKind::FatalError(message) => Err(ExecutionFault::Runtime {
                category: "FatalError".into(),
                message: self.render_value(frame_index, *message)?,
            }),
            InstKind::Throw(exception) => {
                let value = self.value(frame_index, *exception)?;
                let id = self.record_throwable(value, instruction.span)?;
                self.pending_exception = Some(id);
                if self.trace.is_some() {
                    let object = self.trace_object_id(id);
                    self.trace_record(TraceEventKind::Throw { object });
                }
                Ok(None)
            }
            InstKind::SetSuppressed {
                exception,
                suppressed,
            } => {
                let exception = self.value(frame_index, *exception)?;
                let suppressed = self.value(frame_index, *suppressed)?;
                let id = self.record_throwable(exception, instruction.span)?;
                self.record_throwable(suppressed.clone(), instruction.span)?;
                self.throwables
                    .get_mut(&id)
                    .expect("recorded throwable exists")
                    .suppressed
                    .push(suppressed);
                Ok(None)
            }
            InstKind::HasPendingException => {
                Ok(Some(Value::Boolean(self.pending_exception.is_some())))
            }
            InstKind::TakePendingException => {
                let id = self.pending_exception.take().ok_or_else(|| {
                    ExecutionFault::Internal("TakePendingException has no pending throwable".into())
                })?;
                self.throwables
                    .get(&id)
                    .map(|info| Some(info.value.clone()))
                    .ok_or_else(|| {
                        ExecutionFault::Internal("pending exception metadata is unavailable".into())
                    })
            }
            InstKind::Suppressed(exception) => {
                let value = self.value(frame_index, *exception)?;
                let id = self.exception_id(&value)?;
                let suppressed = self
                    .throwables
                    .get(&id)
                    .and_then(|info| info.suppressed.first())
                    .cloned();
                Ok(Some(Value::Nullable(suppressed.map(Box::new))))
            }
            InstKind::StackTrace(exception) => {
                let value = self.value(frame_index, *exception)?;
                let id = self.exception_id(&value)?;
                let frames = self.throwables.get(&id).ok_or_else(|| {
                    ExecutionFault::Internal("throwable stack trace metadata is unavailable".into())
                })?;
                let text = frames
                    .frames
                    .iter()
                    .map(|frame| frame.function.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.allocate_string(text).map(|id| Some(Value::Heap(id)))
            }
            InstKind::ConstString(id) => {
                let text = self
                    .module
                    .strings
                    .get(id.0 as usize)
                    .cloned()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "ConstString references missing string {id:?}"
                        ))
                    })?;
                self.allocate_string(text).map(|id| Some(Value::Heap(id)))
            }
            InstKind::HttpUrlNormalize { value, enum_id } => {
                let value = self.string_operand(frame_index, *value)?;
                match zirk_runtime_core::http_values::Url::parse(&value) {
                    Ok(url) => {
                        let normalized = self.allocate_string(url.as_str().to_owned())?;
                        self.build_tagged_outcome(
                            *enum_id,
                            0,
                            0,
                            Some(Value::Heap(normalized)),
                            0,
                            1,
                        )
                        .map(Some)
                    }
                    Err(_) => self
                        .build_tagged_outcome(*enum_id, 1, 0, None, 0, 1)
                        .map(Some),
                }
            }
            InstKind::HttpResponseReadAll { handle, enum_id } => {
                let handle = self.uint_operand(frame_index, *handle)?;
                let error = if handle == 0 { 8 } else { 2 };
                self.build_tagged_outcome(*enum_id, 1, error, None, 0, 1)
                    .map(Some)
            }
            InstKind::HttpResponseText { body, enum_id } => {
                let body = self.byte_array_operand(frame_index, *body)?;
                match std::str::from_utf8(&body) {
                    Ok(text) => {
                        let text = self.allocate_string(text.to_owned())?;
                        self.build_tagged_outcome(
                            *enum_id,
                            0,
                            0,
                            Some(Value::Heap(text)),
                            0,
                            1,
                        )
                        .map(Some)
                    }
                    Err(_) => self
                        .build_tagged_outcome(*enum_id, 1, 5, None, 0, 1)
                        .map(Some),
                }
            }
            InstKind::HttpUrlResolve {
                base,
                reference,
                enum_id,
            } => {
                let base = self.string_operand(frame_index, *base)?;
                let reference = self.string_operand(frame_index, *reference)?;
                let resolved = zirk_runtime_core::http_values::Url::parse(&base)
                    .and_then(|base| base.resolve(&reference));
                match resolved {
                    Ok(url) => {
                        let normalized = self.allocate_string(url.as_str().to_owned())?;
                        self.build_tagged_outcome(
                            *enum_id,
                            0,
                            0,
                            Some(Value::Heap(normalized)),
                            0,
                            1,
                        )
                        .map(Some)
                    }
                    Err(_) => self
                        .build_tagged_outcome(*enum_id, 1, 0, None, 0, 1)
                        .map(Some),
                }
            }
            InstKind::ConstChar(id) => {
                let text = self
                    .module
                    .strings
                    .get(id.0 as usize)
                    .cloned()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "ConstChar references missing string {id:?}"
                        ))
                    })?;
                if zirk_runtime_core::text::grapheme_count(&text) != 1 {
                    return Err(ExecutionFault::Internal(
                        "ConstChar does not contain exactly one grapheme".into(),
                    ));
                }
                self.allocate_char(text).map(|id| Some(Value::Heap(id)))
            }
            InstKind::StringGraphemeOffset { string, index } => {
                let text = self.string_operand(frame_index, *string)?;
                let index = self.integer_operand(frame_index, *index)?;
                let IrType::Int(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "StringGraphemeOffset has a non-integer result".into(),
                    ));
                };
                let offset = zirk_runtime_core::text::grapheme_offset(&text, index)
                    .map(|offset| offset as i128)
                    .unwrap_or(-1);
                IntegerValue::from_literal(offset, width)
                    .map(Value::Int)
                    .map(Some)
                    .map_err(ExecutionFault::Internal)
            }
            InstKind::GraphemeLenAt { string, offset } => {
                let text = self.string_operand(frame_index, *string)?;
                let offset = self.integer_operand(frame_index, *offset)?;
                let IrType::Int(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "GraphemeLenAt has a non-integer result".into(),
                    ));
                };
                let length = usize::try_from(offset)
                    .ok()
                    .and_then(|offset| zirk_runtime_core::text::grapheme_len_at(&text, offset))
                    .map(|length| length as i128)
                    .unwrap_or(-1);
                IntegerValue::from_literal(length, width)
                    .map(Value::Int)
                    .map(Some)
                    .map_err(ExecutionFault::Internal)
            }
            InstKind::GraphemeSlice {
                string,
                offset,
                len,
            } => {
                let text = self.string_operand(frame_index, *string)?;
                let offset = self.integer_operand(frame_index, *offset)?;
                let len = self.integer_operand(frame_index, *len)?;
                let Some(end) = offset.checked_add(len) else {
                    return Err(ExecutionFault::Runtime {
                        category: "BoundsError".into(),
                        message: "grapheme range overflows".into(),
                    });
                };
                let value = text
                    .get(
                        usize::try_from(offset).unwrap_or(usize::MAX)
                            ..usize::try_from(end).unwrap_or(usize::MAX),
                    )
                    .filter(|value| zirk_runtime_core::text::grapheme_count(value) == 1)
                    .ok_or_else(|| ExecutionFault::Runtime {
                        category: "BoundsError".into(),
                        message: "grapheme range is invalid".into(),
                    })?
                    .to_owned();
                self.allocate_char(value).map(|id| Some(Value::Heap(id)))
            }
            InstKind::Concat { left, right } => {
                let left = self.string_operand(frame_index, *left)?;
                let right = self.string_operand(frame_index, *right)?;
                let length = left
                    .len()
                    .checked_add(right.len())
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.ensure_string_allocation_fits(length)?;
                self.allocate_string(zirk_runtime_core::text::concat(&left, &right))
                    .map(|id| Some(Value::Heap(id)))
            }
            InstKind::Repeat { string, count } => {
                let string = self.string_operand(frame_index, *string)?;
                let count = self.value(frame_index, *count)?;
                let Value::Int(count) = count else {
                    return Err(ExecutionFault::Internal(
                        "Repeat count is not an integer".into(),
                    ));
                };
                let count = usize::try_from(count.signed()).map_err(|_| {
                    ExecutionFault::Internal("Repeat count is negative or too large".into())
                })?;
                let length = string
                    .len()
                    .checked_mul(count)
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.ensure_string_allocation_fits(length)?;
                let repeated = zirk_runtime_core::text::repeat(&string, count)
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.allocate_string(repeated)
                    .map(|id| Some(Value::Heap(id)))
            }
            InstKind::ToString(operand) => {
                let text = self.render_value(frame_index, *operand)?;
                self.allocate_string(text).map(|id| Some(Value::Heap(id)))
            }
            InstKind::Load(slot) => self.frames[frame_index]
                .slots
                .get(slot.0 as usize)
                .cloned()
                .flatten()
                .map(Some)
                .ok_or_else(|| {
                    ExecutionFault::Internal(format!("load from uninitialized slot {slot:?}"))
                }),
            InstKind::Store(slot, operand) => {
                let value = self.frames[frame_index]
                    .values
                    .get(&operand.0)
                    .cloned()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "store reads unavailable operand {operand:?}"
                        ))
                    })?;
                let destination = self.frames[frame_index]
                    .slots
                    .get_mut(slot.0 as usize)
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!("store references missing slot {slot:?}"))
                    })?;
                *destination = Some(value);
                Ok(None)
            }
            InstKind::Unary { op, operand } => {
                let value = self.value(frame_index, *operand)?;
                self.execute_unary(*op, value).map(Some).map_err(|message| {
                    ExecutionFault::Runtime {
                        category: "ArithmeticError".into(),
                        message,
                    }
                })
            }
            InstKind::Binary { op, left, right } => {
                let left = self.value(frame_index, *left)?;
                let right = self.value(frame_index, *right)?;
                self.execute_binary(*op, left, right).map(Some)
            }
            InstKind::CheckedArithmetic {
                op,
                left,
                right,
                signed,
            } => {
                let Value::Int(left) = self.value(frame_index, *left)? else {
                    return Err(ExecutionFault::Internal(
                        "CheckedArithmetic reads a non-integer left operand".into(),
                    ));
                };
                let Value::Int(right) = self.value(frame_index, *right)? else {
                    return Err(ExecutionFault::Internal(
                        "CheckedArithmetic reads a non-integer right operand".into(),
                    ));
                };
                if left.width != right.width || left.width.signed() != *signed {
                    return Err(ExecutionFault::Internal(
                        "CheckedArithmetic operands disagree with declared signedness".into(),
                    ));
                }
                let overflow = if *signed {
                    let value = match op {
                        BinaryOp::Add => left.signed().checked_add(right.signed()),
                        BinaryOp::Sub => left.signed().checked_sub(right.signed()),
                        BinaryOp::Mul => left.signed().checked_mul(right.signed()),
                        _ => {
                            return Err(ExecutionFault::Internal(
                                "unsupported CheckedArithmetic operation".into(),
                            ));
                        }
                    };
                    let maximum = if left.width.bits() == 128 {
                        i128::MAX
                    } else {
                        (1_i128 << (left.width.bits() - 1)) - 1
                    };
                    let minimum = if left.width.bits() == 128 {
                        i128::MIN
                    } else {
                        -(1_i128 << (left.width.bits() - 1))
                    };
                    value.is_none_or(|value| value < minimum || value > maximum)
                } else {
                    let value = match op {
                        BinaryOp::Add => left.unsigned().checked_add(right.unsigned()),
                        BinaryOp::Sub => left.unsigned().checked_sub(right.unsigned()),
                        BinaryOp::Mul => left.unsigned().checked_mul(right.unsigned()),
                        _ => {
                            return Err(ExecutionFault::Internal(
                                "unsupported CheckedArithmetic operation".into(),
                            ));
                        }
                    };
                    let maximum = if left.width.bits() == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << left.width.bits()) - 1
                    };
                    value.is_none_or(|value| value > maximum)
                };
                Ok(Some(Value::Boolean(overflow)))
            }
            // `Retype` is a compile-time-directed coercion: the runtime type
            // check that justifies a narrowing was already emitted as its own
            // instruction (`IsInstance`/`CheckedCast`), and an upcast needs no
            // work at all — the heap value already carries its concrete
            // layout. The instruction therefore evaluates to its operand.
            InstKind::Retype(operand) => {
                let value = self.value(frame_index, *operand)?;
                Ok(Some(value))
            }
            // `WordCast` exists because native `Map`/`Set` store keys as
            // `i64` words; interpreted collections hold `Value`s directly,
            // so the "word" read back is already the typed value.
            InstKind::WordCast(operand) => {
                let value = self.value(frame_index, *operand)?;
                Ok(Some(value))
            }
            InstKind::IntCast(operand) => {
                let value = self.value(frame_index, *operand)?;
                let IrType::Int(target) = instruction.ty else {
                    return Err(ExecutionFault::Internal(format!(
                        "IntCast has non-integer target {:?}",
                        instruction.ty
                    )));
                };
                match value {
                    Value::Int(value) => Ok(Some(Value::Int(value.cast(target)))),
                    value => Err(ExecutionFault::Internal(format!(
                        "IntCast reads non-integer value {value:?}"
                    ))),
                }
            }
            InstKind::FloatCast(operand) => {
                let Value::Float {
                    width: source_width,
                    text,
                } = self.value(frame_index, *operand)?
                else {
                    return Err(ExecutionFault::Internal(
                        "FloatCast reads a non-float value".into(),
                    ));
                };
                let IrType::Float(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "FloatCast has a non-float target".into(),
                    ));
                };
                if width == FloatWidth::F128 {
                    let value = if source_width == FloatWidth::F128 {
                        zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                            .map_err(|error| {
                                ExecutionFault::Internal(format!("float cast failed: {error:?}"))
                            })?
                    } else {
                        zirk_runtime_core::binary_float::Float128::from_bits(
                            zirk_runtime_core::scalar::f64_to_float128_bits(
                                Self::parse_binary_float(&text)?,
                            ),
                        )
                    };
                    return Ok(Some(Value::Float {
                        width,
                        text: value.wire_text(),
                    }));
                }
                let value = if source_width == FloatWidth::F128 {
                    zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                        .map_err(|error| {
                            ExecutionFault::Internal(format!("float cast failed: {error:?}"))
                        })?
                        .display_f64()
                } else {
                    Self::parse_binary_float(&text)?
                };
                Self::binary_float_value(value, width).map(Some)
            }
            InstKind::IntToFloat(operand) => {
                let Value::Int(value) = self.value(frame_index, *operand)? else {
                    return Err(ExecutionFault::Internal(
                        "IntToFloat reads a non-integer value".into(),
                    ));
                };
                let IrType::Float(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "IntToFloat has a non-float target".into(),
                    ));
                };
                if width == FloatWidth::F128 {
                    let text = if value.width.signed() {
                        zirk_runtime_core::binary_float::Float128::from_i128(value.signed())
                            .wire_text()
                    } else {
                        zirk_runtime_core::binary_float::Float128::from_decimal(
                            &value.unsigned().to_string(),
                        )
                        .map_err(|error| {
                            ExecutionFault::Internal(format!(
                                "integer-to-Float128 conversion failed: {error:?}"
                            ))
                        })?
                        .wire_text()
                    };
                    return Ok(Some(Value::Float { width, text }));
                }
                let number = if value.width.signed() {
                    value.signed() as f64
                } else {
                    value.unsigned() as f64
                };
                Self::binary_float_value(number, width).map(Some)
            }
            InstKind::FloatToInt(operand) => {
                let Value::Float {
                    width: source_width,
                    text,
                } = self.value(frame_index, *operand)?
                else {
                    return Err(ExecutionFault::Internal(
                        "FloatToInt reads a non-float value".into(),
                    ));
                };
                let IrType::Int(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "FloatToInt has a non-integer target".into(),
                    ));
                };
                if source_width == FloatWidth::F128 {
                    let literal =
                        zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                            .and_then(|value| {
                                value.trunc_to_integer_bits(width.bits(), width.signed())
                            })
                            .map_err(|error| {
                                let message = match error {
                                    zirk_runtime_core::binary_float::Float128Error::NonFinite => {
                                        "non-finite float cannot be converted to an integer"
                                    }
                                    _ => "float-to-integer conversion is outside the target range",
                                };
                                ExecutionFault::Runtime {
                                    category: "CastError".into(),
                                    message: message.into(),
                                }
                            })?;
                    return Ok(Some(Value::Int(IntegerValue::from_bits(literal, width))));
                }
                let value = Self::parse_binary_float(&text)?;
                let bits =
                    float_to_integer_bits(value, width).ok_or_else(|| ExecutionFault::Runtime {
                        category: "CastError".into(),
                        message: "float-to-integer conversion is outside the target range".into(),
                    })?;
                Ok(Some(Value::Int(IntegerValue::from_bits(bits, width))))
            }
            InstKind::IntToDecimal(operand) => {
                let Value::Int(value) = self.value(frame_index, *operand)? else {
                    return Err(ExecutionFault::Internal(
                        "IntToDecimal reads a non-integer value".into(),
                    ));
                };
                let integer = if value.width.signed() {
                    value.signed()
                } else {
                    i128::try_from(value.unsigned()).map_err(|_| {
                        ExecutionFault::Internal(
                            "unsigned integer cannot fit the decimal representation".into(),
                        )
                    })?
                };
                Ok(Some(Value::Decimal(zirk_runtime_core::decimal::format(
                    zirk_runtime_core::decimal::Decimal::new(integer, 0),
                ))))
            }
            InstKind::DecimalToInt(operand) => {
                let Value::Decimal(text) = self.value(frame_index, *operand)? else {
                    return Err(ExecutionFault::Internal(
                        "DecimalToInt reads a non-decimal value".into(),
                    ));
                };
                let IrType::Int(target) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "DecimalToInt has a non-integer target".into(),
                    ));
                };
                let decimal = self.parse_decimal(&text)?;
                let integer = zirk_runtime_core::decimal::to_i128_checked(
                    decimal,
                    target.bits(),
                    target.signed(),
                )
                .map_err(|error| {
                    ExecutionFault::Internal(format!(
                        "decimal-to-integer conversion failed: {error:?}"
                    ))
                })?;
                Ok(Some(Value::Int(
                    IntegerValue::from_literal(integer, target)
                        .map_err(ExecutionFault::Internal)?,
                )))
            }
            InstKind::DecimalToFloat(operand) => {
                let Value::Decimal(text) = self.value(frame_index, *operand)? else {
                    return Err(ExecutionFault::Internal(
                        "DecimalToFloat reads a non-decimal value".into(),
                    ));
                };
                let IrType::Float(width) = instruction.ty else {
                    return Err(ExecutionFault::Internal(
                        "DecimalToFloat has a non-float target".into(),
                    ));
                };
                if width == FloatWidth::F128 {
                    let text = zirk_runtime_core::binary_float::Float128::from_decimal(&text)
                        .map_err(|error| {
                            ExecutionFault::Internal(format!(
                                "decimal-to-Float128 conversion failed: {error:?}"
                            ))
                        })?
                        .wire_text();
                    return Ok(Some(Value::Float { width, text }));
                }
                let value = Self::parse_binary_float(&text)?;
                Self::binary_float_value(value, width).map(Some)
            }
            InstKind::FloatToDecimal(operand) => {
                let Value::Float { width, text } = self.value(frame_index, *operand)? else {
                    return Err(ExecutionFault::Internal(
                        "FloatToDecimal reads a non-float value".into(),
                    ));
                };
                let text = if width == FloatWidth::F128 {
                    zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                        .map_err(|error| {
                            ExecutionFault::Internal(format!(
                                "Float128-to-decimal conversion failed: {error:?}"
                            ))
                        })?
                        .display_f64()
                        .to_string()
                } else {
                    text
                };
                self.parse_decimal(&text)?;
                Ok(Some(Value::Decimal(text)))
            }
            InstKind::Call { callee, args } => {
                let arguments = args
                    .iter()
                    .map(|operand| self.value(frame_index, *operand))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ExecutionFault::Internal)?;
                if is_browser_intrinsic(callee) {
                    return self
                        .execute_browser_intrinsic(callee, &arguments, instruction.ty)
                        .map(Some);
                }
                let function = self
                    .module
                    .functions
                    .iter()
                    .position(|function| function.name == *callee)
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "Call references unavailable interpreted function {callee:?}"
                        ))
                    })?;
                self.push_frame(
                    function,
                    arguments,
                    Some(ReturnDestination {
                        caller_value: result_value_id(instruction),
                    }),
                )
                .map_err(ExecutionFault::Limit)?;
                // The caller is suspended until its child returns. A result
                // placeholder avoids treating this deferred value as an IR
                // fault; it is replaced atomically at return propagation.
                Ok(instruction.result.map(|_| Value::Undefined))
            }
            InstKind::CallVirtual {
                object,
                index,
                args,
            } => {
                let receiver = self.value(frame_index, *object)?;
                let layout = self.object_layout(frame_index, *object)?;
                let callee = self
                    .module
                    .objects
                    .get(layout as usize)
                    .and_then(|layout| layout.methods.get(*index as usize))
                    .cloned()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "CallVirtual references missing method {index} on object layout {layout}"
                        ))
                    })?;
                self.push_dispatch_frame(
                    &callee,
                    receiver,
                    args,
                    frame_index,
                    result_value_id(instruction),
                )?;
                Ok(instruction.result.map(|_| Value::Undefined))
            }
            InstKind::CallContract {
                object,
                contract,
                index,
                args,
            } => {
                let receiver = self.value(frame_index, *object)?;
                let layout = self.object_layout(frame_index, *object)?;
                let callee = self
                    .module
                    .objects
                    .get(layout as usize)
                    .and_then(|layout| layout.contracts.iter().find(|table| table.contract == *contract))
                    .and_then(|table| table.methods.get(*index as usize))
                    .cloned()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "CallContract references missing contract {contract} method {index} on object layout {layout}"
                        ))
                    })?;
                self.push_dispatch_frame(
                    &callee,
                    receiver,
                    args,
                    frame_index,
                    result_value_id(instruction),
                )?;
                Ok(instruction.result.map(|_| Value::Undefined))
            }
            InstKind::MakeClosure {
                target, captures, ..
            }
            | InstKind::MakeCallable { target, captures } => {
                let captures = captures
                    .iter()
                    .map(|operand| self.value(frame_index, *operand))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ExecutionFault::Internal)?;
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(captures.len().saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.heap
                    .allocate(
                        HeapEntry::Closure {
                            target: target.clone(),
                            captures,
                        },
                        bytes,
                        self.limits,
                    )
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::CallClosure { callee, args, .. }
            | InstKind::CallCallable {
                callable: callee,
                args,
            } => {
                let Value::Heap(id) = self.value(frame_index, *callee)? else {
                    return Err(ExecutionFault::Internal(
                        "indirect call reads a non-callable value".into(),
                    ));
                };
                let Some(HeapEntry::Closure { target, captures }) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "indirect call reads a heap value that is not callable".into(),
                    ));
                };
                let function = self
                    .module
                    .functions
                    .iter()
                    .position(|function| function.name == *target)
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "callable references unavailable interpreted function {target:?}"
                        ))
                    })?;
                let mut arguments = captures.clone();
                arguments.extend(
                    args.iter()
                        .map(|operand| self.value(frame_index, *operand))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(ExecutionFault::Internal)?,
                );
                self.push_frame(
                    function,
                    arguments,
                    Some(ReturnDestination {
                        caller_value: result_value_id(instruction),
                    }),
                )
                .map_err(ExecutionFault::Limit)?;
                Ok(instruction.result.map(|_| Value::Undefined))
            }
            InstKind::ArrayNew {
                element_id,
                capacity,
            } => {
                let capacity = self.integer_operand(frame_index, *capacity)?;
                let capacity = usize::try_from(capacity).map_err(|_| ExecutionFault::Runtime {
                    category: "BoundsError".into(),
                    message: "array capacity cannot be negative".into(),
                })?;
                let element = self
                    .module
                    .array_types
                    .get(*element_id as usize)
                    .copied()
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "ArrayNew references missing element type {element_id}"
                        ))
                    })?;
                let default = self.default_value(element)?;
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(capacity.saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.heap
                    .allocate(
                        HeapEntry::Array(vec![default; capacity]),
                        bytes,
                        self.limits,
                    )
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::ListNew { .. } => self
                .heap
                .allocate(
                    HeapEntry::List(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(|id| Some(Value::Heap(id)))
                .map_err(ExecutionFault::Limit),
            InstKind::ArrayLength(receiver) | InstKind::ListLength(receiver) => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let length = match self.heap.get(id) {
                    Some(HeapEntry::Array(values)) | Some(HeapEntry::List(values)) => values.len(),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "length receiver is not an array or list".into(),
                        ));
                    }
                };
                self.collection_length_value(length, instruction.ty)
                    .map(Some)
            }
            InstKind::ArrayListLoad { receiver, index } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let index = self.integer_operand(frame_index, *index)?;
                let values = match self.heap.get(id) {
                    Some(HeapEntry::Array(values)) | Some(HeapEntry::List(values)) => values,
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "load receiver is not an array or list".into(),
                        ));
                    }
                };
                let index = zirk_runtime_core::collections::element_index(values.len(), index)
                    .ok_or_else(Self::collection_bounds_error)?;
                Ok(Some(values[index].clone()))
            }
            InstKind::ArrayListStore {
                receiver,
                index,
                value,
            } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let index = self.integer_operand(frame_index, *index)?;
                let value = self.value(frame_index, *value)?;
                let values = match self.heap.get_mut(id) {
                    Some(HeapEntry::Array(values)) | Some(HeapEntry::List(values)) => values,
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "store receiver is not an array or list".into(),
                        ));
                    }
                };
                let index = zirk_runtime_core::collections::element_index(values.len(), index)
                    .ok_or_else(Self::collection_bounds_error)?;
                values[index] = value;
                Ok(None)
            }
            InstKind::ListAdd { receiver, value } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let value = self.value(frame_index, *value)?;
                let Some(HeapEntry::List(values)) = self.heap.get_mut(id) else {
                    return Err(ExecutionFault::Internal(
                        "ListAdd receiver is not a list".into(),
                    ));
                };
                values.push(value);
                Ok(None)
            }
            InstKind::ListInsert {
                receiver,
                index,
                value,
            } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let index = self.integer_operand(frame_index, *index)?;
                let value = self.value(frame_index, *value)?;
                let Some(HeapEntry::List(values)) = self.heap.get_mut(id) else {
                    return Err(ExecutionFault::Internal(
                        "ListInsert receiver is not a list".into(),
                    ));
                };
                let index = zirk_runtime_core::collections::insertion_index(values.len(), index)
                    .ok_or_else(Self::collection_bounds_error)?;
                values.insert(index, value);
                Ok(None)
            }
            InstKind::ListRemove { receiver, index } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let index = self.integer_operand(frame_index, *index)?;
                let Some(HeapEntry::List(values)) = self.heap.get_mut(id) else {
                    return Err(ExecutionFault::Internal(
                        "ListRemove receiver is not a list".into(),
                    ));
                };
                let index = zirk_runtime_core::collections::element_index(values.len(), index)
                    .ok_or_else(Self::collection_bounds_error)?;
                values.remove(index);
                Ok(None)
            }
            InstKind::ListRemoveValue { receiver, value } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let value = self.value(frame_index, *value)?;
                let position = match self.heap.get(id) {
                    Some(HeapEntry::List(values)) => values
                        .iter()
                        .position(|candidate| self.runtime_value_equals(candidate, &value)),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "ListRemoveValue receiver is not a list".into(),
                        ));
                    }
                };
                if let Some(index) = position {
                    let Some(HeapEntry::List(values)) = self.heap.get_mut(id) else {
                        unreachable!("checked above");
                    };
                    values.remove(index);
                }
                Ok(Some(Value::Boolean(position.is_some())))
            }
            InstKind::ArrayClone { receiver } | InstKind::ListClone { receiver } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let (entry, bytes) = match self.heap.get(id) {
                    Some(HeapEntry::Array(values)) => (
                        HeapEntry::Array(values.clone()),
                        values.len() * std::mem::size_of::<Value>(),
                    ),
                    Some(HeapEntry::List(values)) => (
                        HeapEntry::List(values.clone()),
                        values.len() * std::mem::size_of::<Value>(),
                    ),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "clone receiver is not an array or list".into(),
                        ));
                    }
                };
                self.heap
                    .allocate(entry, std::mem::size_of::<HeapEntry>() + bytes, self.limits)
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::Clone(operand) => {
                let value = self
                    .value(frame_index, *operand)
                    .map_err(ExecutionFault::Internal)?;
                self.deep_clone(value).map(Some)
            }
            InstKind::MapNew { .. } => self
                .heap
                .allocate(
                    HeapEntry::Map(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(|id| Some(Value::Heap(id)))
                .map_err(ExecutionFault::Limit),
            InstKind::SetNew { .. } => self
                .heap
                .allocate(
                    HeapEntry::Set(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(|id| Some(Value::Heap(id)))
                .map_err(ExecutionFault::Limit),
            InstKind::MapLength(receiver) | InstKind::SetLength(receiver) => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let length = match self.heap.get(id) {
                    Some(HeapEntry::Map(entries)) => entries.len(),
                    Some(HeapEntry::Set(values)) => values.len(),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "length receiver is not a map or set".into(),
                        ));
                    }
                };
                self.collection_length_value(length, instruction.ty)
                    .map(Some)
            }
            InstKind::MapIsEmpty(receiver) | InstKind::SetIsEmpty(receiver) => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let empty = match self.heap.get(id) {
                    Some(HeapEntry::Map(entries)) => entries.is_empty(),
                    Some(HeapEntry::Set(values)) => values.is_empty(),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "is_empty receiver is not a map or set".into(),
                        ));
                    }
                };
                Ok(Some(Value::Boolean(empty)))
            }
            InstKind::MapSet {
                receiver,
                key,
                value,
            } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let key = self.value(frame_index, *key)?;
                let value = self.value(frame_index, *value)?;
                let position = match self.heap.get(id) {
                    Some(HeapEntry::Map(entries)) => entries
                        .iter()
                        .position(|(candidate, _)| self.runtime_value_equals(candidate, &key)),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "MapSet receiver is not a map".into(),
                        ));
                    }
                };
                let Some(HeapEntry::Map(entries)) = self.heap.get_mut(id) else {
                    unreachable!("checked above");
                };
                if let Some(index) = position {
                    entries[index].1 = value;
                } else {
                    entries.push((key, value));
                }
                Ok(None)
            }
            InstKind::MapContainsKey { receiver, key } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let key = self.value(frame_index, *key)?;
                let Some(HeapEntry::Map(entries)) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "MapContainsKey receiver is not a map".into(),
                    ));
                };
                Ok(Some(Value::Boolean(entries.iter().any(
                    |(candidate, _)| self.runtime_value_equals(candidate, &key),
                ))))
            }
            InstKind::MapGet { receiver, key } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let key = self.value(frame_index, *key)?;
                let Some(HeapEntry::Map(entries)) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "MapGet receiver is not a map".into(),
                    ));
                };
                Ok(Some(Value::Nullable(
                    entries
                        .iter()
                        .find(|(candidate, _)| self.runtime_value_equals(candidate, &key))
                        .map(|(_, value)| Box::new(value.clone())),
                )))
            }
            InstKind::MapRemove { receiver, key } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let key = self.value(frame_index, *key)?;
                let position = match self.heap.get(id) {
                    Some(HeapEntry::Map(entries)) => entries
                        .iter()
                        .position(|(candidate, _)| self.runtime_value_equals(candidate, &key)),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "MapRemove receiver is not a map".into(),
                        ));
                    }
                };
                if let Some(index) = position {
                    let Some(HeapEntry::Map(entries)) = self.heap.get_mut(id) else {
                        unreachable!("checked above");
                    };
                    entries.remove(index);
                }
                Ok(Some(Value::Boolean(position.is_some())))
            }
            InstKind::SetAdd { receiver, value } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let value = self.value(frame_index, *value)?;
                let present = match self.heap.get(id) {
                    Some(HeapEntry::Set(values)) => values
                        .iter()
                        .any(|candidate| self.runtime_value_equals(candidate, &value)),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "SetAdd receiver is not a set".into(),
                        ));
                    }
                };
                if !present {
                    let Some(HeapEntry::Set(values)) = self.heap.get_mut(id) else {
                        unreachable!("checked above");
                    };
                    values.push(value);
                }
                Ok(None)
            }
            InstKind::SetContains { receiver, value } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let value = self.value(frame_index, *value)?;
                let Some(HeapEntry::Set(values)) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "SetContains receiver is not a set".into(),
                    ));
                };
                Ok(Some(Value::Boolean(values.iter().any(|candidate| {
                    self.runtime_value_equals(candidate, &value)
                }))))
            }
            InstKind::SetRemove { receiver, value } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let value = self.value(frame_index, *value)?;
                let position = match self.heap.get(id) {
                    Some(HeapEntry::Set(values)) => values
                        .iter()
                        .position(|candidate| self.runtime_value_equals(candidate, &value)),
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "SetRemove receiver is not a set".into(),
                        ));
                    }
                };
                if let Some(index) = position {
                    let Some(HeapEntry::Set(values)) = self.heap.get_mut(id) else {
                        unreachable!("checked above");
                    };
                    values.remove(index);
                }
                Ok(Some(Value::Boolean(position.is_some())))
            }
            InstKind::ArraySlice {
                receiver,
                start,
                end,
                step,
            } => {
                let id = self.heap_operand(frame_index, *receiver)?;
                let start = self.integer_operand(frame_index, *start)?;
                let end = self.integer_operand(frame_index, *end)?;
                let step = self.integer_operand(frame_index, *step)?;
                let values = match self.heap.get(id) {
                    Some(HeapEntry::Array(values)) => values,
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "ArraySlice receiver is not an array".into(),
                        ));
                    }
                };
                let indexes =
                    zirk_runtime_core::collections::slice_indices(values.len(), start, end, step)
                        .map_err(|message| ExecutionFault::Runtime {
                        category: "BoundsError".into(),
                        message: message.into(),
                    })?;
                let sliced = indexes
                    .into_iter()
                    .map(|index| values[index].clone())
                    .collect::<Vec<_>>();
                let bytes =
                    std::mem::size_of::<HeapEntry>() + sliced.len() * std::mem::size_of::<Value>();
                self.heap
                    .allocate(HeapEntry::Array(sliced), bytes, self.limits)
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::Alloc(layout) => {
                let fields = self
                    .module
                    .objects
                    .get(*layout as usize)
                    .ok_or_else(|| {
                        ExecutionFault::Internal(format!(
                            "Alloc references missing object layout {layout}"
                        ))
                    })?
                    .fields
                    .len();
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(fields.saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                let id = self
                    .heap
                    .allocate(
                        HeapEntry::Object {
                            layout: *layout,
                            fields: vec![Value::Undefined; fields],
                        },
                        bytes,
                        self.limits,
                    )
                    .map_err(ExecutionFault::Limit)?;
                if self.trace.is_some() {
                    let class = self.module.objects[*layout as usize].name.clone();
                    if let Some(object) = self.trace_object_id(id) {
                        self.trace_record(TraceEventKind::Alloc { object, class });
                    }
                }
                Ok(Some(Value::Heap(id)))
            }
            InstKind::BuildValue { class, fields } => {
                let fields = fields
                    .iter()
                    .map(|operand| self.value(frame_index, *operand))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ExecutionFault::Internal)?;
                let layout = self.module.values.get(*class as usize).ok_or_else(|| {
                    ExecutionFault::Internal(format!(
                        "BuildValue references missing layout {class}"
                    ))
                })?;
                if fields.len() != layout.fields.len() {
                    return Err(ExecutionFault::Internal(
                        "BuildValue field count does not match layout".into(),
                    ));
                }
                let bytes =
                    std::mem::size_of::<HeapEntry>() + fields.len() * std::mem::size_of::<Value>();
                self.heap
                    .allocate(
                        HeapEntry::Record {
                            layout: *class,
                            fields,
                        },
                        bytes,
                        self.limits,
                    )
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::BuildEnum {
                enum_id,
                variant,
                fields,
            } => {
                let fields = fields
                    .iter()
                    .map(|operand| self.value(frame_index, *operand))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ExecutionFault::Internal)?;
                let layout = self.module.enums.get(*enum_id as usize).ok_or_else(|| {
                    ExecutionFault::Internal(format!(
                        "BuildEnum references missing layout {enum_id}"
                    ))
                })?;
                let expected = layout.variants.get(*variant as usize).ok_or_else(|| {
                    ExecutionFault::Internal(format!(
                        "BuildEnum references missing variant {variant}"
                    ))
                })?;
                if fields.len() != expected.len() {
                    return Err(ExecutionFault::Internal(
                        "BuildEnum field count does not match variant".into(),
                    ));
                }
                let bytes =
                    std::mem::size_of::<HeapEntry>() + fields.len() * std::mem::size_of::<Value>();
                self.heap
                    .allocate(
                        HeapEntry::Enum {
                            layout: *enum_id,
                            variant: *variant,
                            fields,
                        },
                        bytes,
                        self.limits,
                    )
                    .map(|id| Some(Value::Heap(id)))
                    .map_err(ExecutionFault::Limit)
            }
            InstKind::Discriminant(value) => {
                let Value::Heap(id) = self.value(frame_index, *value)? else {
                    return Err(ExecutionFault::Internal(
                        "Discriminant reads a non-enum value".into(),
                    ));
                };
                let Some(HeapEntry::Enum { variant, .. }) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "Discriminant reads a heap value that is not an enum".into(),
                    ));
                };
                Ok(Some(Value::Int(
                    IntegerValue::from_literal(*variant as i128, IntWidth::I32)
                        .map_err(ExecutionFault::Internal)?,
                )))
            }
            InstKind::LoadField { object, index } => {
                let Value::Heap(id) = self.value(frame_index, *object)? else {
                    return Err(ExecutionFault::Internal(
                        "LoadField reads a non-aggregate value".into(),
                    ));
                };
                let Some(entry) = self.heap.get(id) else {
                    return Err(ExecutionFault::Internal(
                        "LoadField reads a missing heap value".into(),
                    ));
                };
                let position = match entry {
                    HeapEntry::Object { .. } | HeapEntry::Record { .. } => *index as usize,
                    HeapEntry::Enum {
                        layout, variant, ..
                    } => {
                        // `index` is the flat field index in the enum layout;
                        // the heap entry stores only the active variant's
                        // payload, so translate through the variant's field list.
                        let Some(field_list) = self
                            .module
                            .enums
                            .get(*layout as usize)
                            .and_then(|enum_layout| enum_layout.variants.get(*variant as usize))
                        else {
                            return Err(ExecutionFault::Internal(format!(
                                "LoadField references missing enum variant {variant}"
                            )));
                        };
                        let Some(position) = field_list.iter().position(|field| *field == *index)
                        else {
                            return Err(ExecutionFault::Internal(format!(
                                "LoadField references missing field {index}"
                            )));
                        };
                        position
                    }
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "LoadField reads a heap value that is not an aggregate".into(),
                        ));
                    }
                };
                let fields = match entry {
                    HeapEntry::Object { fields, .. }
                    | HeapEntry::Record { fields, .. }
                    | HeapEntry::Enum { fields, .. } => fields,
                    _ => unreachable!(),
                };
                fields.get(position).cloned().map(Some).ok_or_else(|| {
                    ExecutionFault::Internal(format!("LoadField references missing field {index}"))
                })
            }
            InstKind::StoreField {
                object,
                index,
                value,
            } => {
                let Value::Heap(id) = self.value(frame_index, *object)? else {
                    return Err(ExecutionFault::Internal(
                        "StoreField writes a non-object value".into(),
                    ));
                };
                let value = self.value(frame_index, *value)?;
                let Some(HeapEntry::Object { layout, fields }) = self.heap.get_mut(id) else {
                    return Err(ExecutionFault::Internal(
                        "StoreField writes a heap value that is not an object".into(),
                    ));
                };
                let layout = *layout;
                let field = fields.get_mut(*index as usize).ok_or_else(|| {
                    ExecutionFault::Internal(format!("StoreField references missing field {index}"))
                })?;
                *field = value.clone();
                if self.trace.is_some() {
                    let described = self
                        .module
                        .objects
                        .get(layout as usize)
                        .and_then(|object| object.fields.get(*index as usize))
                        .map(|field| (field.name.clone(), field.ty.as_str().to_string()));
                    if let (Some((field, ty)), Some(object)) = (described, self.trace_object_id(id))
                    {
                        let (value, truncated) = if self.is_secret_trace_field(layout, *index as usize) {
                            ("<redacted>".to_owned(), false)
                        } else {
                            self.trace_render(&value)
                        };
                        self.trace_record(TraceEventKind::FieldWrite {
                            object,
                            field,
                            ty,
                            value,
                            truncated,
                        });
                    }
                }
                Ok(None)
            }
            InstKind::IsInstance {
                object,
                target_class,
            } => {
                let actual = self.object_layout(frame_index, *object)?;
                let matched = self
                    .module
                    .objects
                    .get(actual as usize)
                    .is_some_and(|layout| {
                        actual == *target_class || layout.ancestors.contains(target_class)
                    });
                Ok(Some(Value::Boolean(matched)))
            }
            InstKind::CheckedCast {
                object,
                target_class,
            } => {
                let actual = self.object_layout(frame_index, *object)?;
                let matched = self
                    .module
                    .objects
                    .get(actual as usize)
                    .is_some_and(|layout| {
                        actual == *target_class || layout.ancestors.contains(target_class)
                    });
                if !matched {
                    return Err(ExecutionFault::Runtime {
                        category: "CastError".into(),
                        message: format!(
                            "object layout {actual} is not an instance of {target_class}"
                        ),
                    });
                }
                Ok(Some(self.value(frame_index, *object)?))
            }
            InstKind::NullValue(_) => Ok(Some(Value::Nullable(None))),
            InstKind::Wrap { value, .. } => Ok(Some(Value::Nullable(Some(Box::new(
                self.value(frame_index, *value)?,
            ))))),
            InstKind::IsNull(operand) => match self.value(frame_index, *operand)? {
                Value::Nullable(value) => Ok(Some(Value::Boolean(value.is_none()))),
                value => Err(ExecutionFault::Internal(format!(
                    "IsNull reads non-nullable value {value:?}"
                ))),
            },
            InstKind::Unwrap(operand) => match self.value(frame_index, *operand)? {
                Value::Nullable(Some(value)) => Ok(Some(*value)),
                Value::Nullable(None) => Err(ExecutionFault::Internal(
                    "Unwrap reached an absent nullable value".into(),
                )),
                value => Err(ExecutionFault::Internal(format!(
                    "Unwrap reads non-nullable value {value:?}"
                ))),
            },
            InstKind::Print(operand) => self.emit_output(
                frame_index,
                *operand,
                false,
                OutputDestination::Stdout,
                host,
            ),
            InstKind::Println(operand) => {
                self.emit_output(frame_index, *operand, true, OutputDestination::Stdout, host)
            }
            InstKind::PrintStderr(operand) => self.emit_output(
                frame_index,
                *operand,
                false,
                OutputDestination::Stderr,
                host,
            ),
            InstKind::PrintlnStderr(operand) => {
                self.emit_output(frame_index, *operand, true, OutputDestination::Stderr, host)
            }
            InstKind::IsCancelled => Ok(Some(Value::Boolean(
                self.cancellation_shield_depth == 0 && host.is_cancelled(),
            ))),
            InstKind::CancelShieldEnter => {
                self.cancellation_shield_depth = self.cancellation_shield_depth.saturating_add(1);
                Ok(None)
            }
            InstKind::CancelShieldExit => {
                self.cancellation_shield_depth = self.cancellation_shield_depth.saturating_sub(1);
                Ok(None)
            }
            InstKind::FlushStdout { enum_id } | InstKind::FlushStderr { enum_id } => {
                // The bounded host's own sink is in-memory
                // (`add-stdin-stdout-standard-library`, design D3): flushing
                // it always succeeds.
                self.build_flush_ok(*enum_id).map(Some)
            }
            InstKind::StdinReadLineResult { enum_id } => {
                let outcome = zirk_runtime_core::stdin::read_line(&mut HostReader(host));
                self.build_text_outcome(*enum_id, outcome, false).map(Some)
            }
            InstKind::StdinReadChar { enum_id } => {
                let outcome = zirk_runtime_core::stdin::read_char(&mut HostReader(host));
                self.build_text_outcome(*enum_id, outcome, true).map(Some)
            }
            InstKind::StdinReadBytes { max, enum_id } => {
                let max = self.uint_operand(frame_index, *max)?;
                let outcome = zirk_runtime_core::stdin::read_bytes(&mut HostReader(host), max);
                self.build_bytes_outcome(*enum_id, outcome).map(Some)
            }
            InstKind::StdinReadExact { count, enum_id } => {
                let count = self.uint_operand(frame_index, *count)?;
                let outcome = zirk_runtime_core::stdin::read_exact(&mut HostReader(host), count);
                self.build_exact_outcome(*enum_id, outcome).map(Some)
            }
            _ => self
                .materialize_constant(instruction)
                .map(Some)
                .map_err(ExecutionFault::Internal),
        }
    }

    fn emit_output(
        &mut self,
        frame_index: usize,
        operand: zirk_ir::Operand,
        newline: bool,
        destination: OutputDestination,
        host: &mut dyn InterpreterHost,
    ) -> Result<Option<Value>, ExecutionFault> {
        let Value::Heap(id) = self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        else {
            return Err(ExecutionFault::Internal(
                "print operand is not a heap string".into(),
            ));
        };
        let HeapEntry::String(text) = self.heap.get(id).ok_or_else(|| {
            ExecutionFault::Internal("print operand references a missing heap value".into())
        })?
        else {
            return Err(ExecutionFault::Internal(
                "print operand is not a string heap value".into(),
            ));
        };
        let mut text = text.clone();
        if newline {
            text.push('\n');
        }
        let bytes = text.len();
        let next_bytes = match self
            .output_bytes
            .checked_add(bytes)
            .filter(|bytes| *bytes <= self.limits.output_bytes)
        {
            Some(bytes) => bytes,
            None => return Err(self.truncate_output(host)),
        };
        // `println("a", 2)` is several writes to one line. The output-event
        // allowance counts logical lines, so a write that continues an open
        // line on the same stream costs no extra event.
        let continues_line = self.open_output_line == Some(destination);
        let ends_line = text.ends_with('\n');
        let next_events = if continues_line {
            self.output_events
        } else {
            match self
                .output_events
                .checked_add(1)
                .filter(|events| *events <= self.limits.output_events)
            {
                Some(events) => events,
                None => return Err(self.truncate_output(host)),
            }
        };
        if host
            .emit(OutputEvent {
                text,
                destination,
                truncated: false,
            })
            .is_err()
        {
            return Err(self.truncate_output(host));
        }
        self.output_bytes = next_bytes;
        self.output_events = next_events;
        self.open_output_line = if ends_line { None } else { Some(destination) };
        self.trace_record(TraceEventKind::Output {
            index: next_events - 1,
        });
        Ok(None)
    }

    fn truncate_output(&mut self, host: &mut dyn InterpreterHost) -> ExecutionFault {
        if !self.output_truncated {
            self.output_truncated = true;
            let _ = host.emit(OutputEvent {
                text: String::new(),
                destination: OutputDestination::Stdout,
                truncated: true,
            });
        }
        ExecutionFault::Limit(LimitFailure::Output)
    }

    fn string_operand(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<String, ExecutionFault> {
        let Value::Heap(id) = self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        else {
            return Err(ExecutionFault::Internal(
                "string operation operand is not a heap string".into(),
            ));
        };
        match self.heap.get(id) {
            Some(HeapEntry::String(text)) => Ok(text.clone()),
            Some(_) => Err(ExecutionFault::Internal(
                "string operation operand is not a string heap value".into(),
            )),
            None => Err(ExecutionFault::Internal(
                "string operation operand references a missing heap value".into(),
            )),
        }
    }

    fn byte_array_operand(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<Vec<u8>, ExecutionFault> {
        let id = self.heap_operand(frame_index, operand)?;
        let Some(HeapEntry::Array(values)) = self.heap.get(id) else {
            return Err(ExecutionFault::Internal(
                "HTTP byte operand is not an Array<Byte>".into(),
            ));
        };
        values
            .iter()
            .map(|value| match value {
                Value::Int(integer) if integer.width == IntWidth::U8 => Ok(integer.bits as u8),
                _ => Err(ExecutionFault::Internal(
                    "HTTP byte array contains a non-Byte value".into(),
                )),
            })
            .collect()
    }

    fn browser_http_convenience_request(
        &self,
        frame_index: usize,
        kind: &zirk_ir::InstKind,
    ) -> Result<Option<(HttpHostRequest, u32, u32)>, ExecutionFault> {
        use zirk_ir::InstKind;

        let mut request = HttpHostRequest {
            id: 0,
            method: String::new(),
            url: String::new(),
            cached: false,
            validate_json: false,
            headers: Vec::new(),
            body: Vec::new(),
        };
        let (enum_id, response_class_id) = match kind {
            InstKind::HttpHead {
                url,
                enum_id,
                response_class_id,
            } => {
                request.method = "HEAD".into();
                request.url = self.string_operand(frame_index, *url)?;
                (*enum_id, *response_class_id)
            }
            InstKind::HttpQuery {
                url,
                query,
                enum_id,
                response_class_id,
            } => {
                request.method = "QUERY".into();
                request.url = self.string_operand(frame_index, *url)?;
                request.body = self.string_operand(frame_index, *query)?.into_bytes();
                request.validate_json = true;
                request
                    .headers
                    .push(("content-type".into(), b"application/json".to_vec()));
                (*enum_id, *response_class_id)
            }
            InstKind::HttpQueryWithContentType {
                url,
                query,
                content_type,
                enum_id,
                response_class_id,
            } => {
                request.method = "QUERY".into();
                request.url = self.string_operand(frame_index, *url)?;
                request.body = self.string_operand(frame_index, *query)?.into_bytes();
                request.headers.push((
                    "content-type".into(),
                    self.string_operand(frame_index, *content_type)?
                        .into_bytes(),
                ));
                (*enum_id, *response_class_id)
            }
            InstKind::HttpPost {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                request.method = "POST".into();
                request.url = self.string_operand(frame_index, *url)?;
                request.body = self.string_operand(frame_index, *body)?.into_bytes();
                request
                    .headers
                    .push(("content-type".into(), b"text/plain; charset=utf-8".to_vec()));
                (*enum_id, *response_class_id)
            }
            InstKind::HttpPostJson {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                request.method = "POST".into();
                request.url = self.string_operand(frame_index, *url)?;
                request.body = self.string_operand(frame_index, *body)?.into_bytes();
                request.validate_json = true;
                request
                    .headers
                    .push(("content-type".into(), b"application/json".to_vec()));
                (*enum_id, *response_class_id)
            }
            InstKind::HttpPostBytes {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                request.method = "POST".into();
                request.url = self.string_operand(frame_index, *url)?;
                request.body = self.byte_array_operand(frame_index, *body)?;
                request
                    .headers
                    .push(("content-type".into(), b"application/octet-stream".to_vec()));
                (*enum_id, *response_class_id)
            }
            _ => return Ok(None),
        };
        Ok(Some((request, enum_id, response_class_id)))
    }

    fn suspend_http_request(
        &mut self,
        frame_index: usize,
        instruction: &zirk_ir::Instruction,
        host: &mut dyn InterpreterHost,
        mut request: HttpHostRequest,
        enum_id: u32,
        response_class_id: u32,
    ) -> ExecutionResult {
        let request_id = self.next_http_request_id;
        let Some(next_request_id) = request_id.checked_add(1) else {
            return self.internal("HTTP request id space exhausted", Some(instruction.span));
        };
        let Some(result) = instruction.result else {
            return self.internal(
                "HTTP request has no result destination",
                Some(instruction.span),
            );
        };
        request.id = request_id;
        if !host.begin_http_request(&request) {
            return ExecutionResult::CapabilityFailure(CapabilityFailure {
                code: "ZB006".into(),
                message: "HTTP requires an explicit host capability for this execution".into(),
                span: Some(instruction.span),
            });
        }
        self.next_http_request_id = next_request_id;
        self.pending_http = Some(PendingHttpContinuation {
            request_id,
            frame_index,
            result,
            enum_id,
            response_class_id,
            span: instruction.span,
        });
        self.main_paused = true;
        ExecutionResult::HttpSuspended { request }
    }

    fn integer_operand(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<i64, ExecutionFault> {
        let Value::Int(value) = self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        else {
            return Err(ExecutionFault::Internal(
                "integer operation operand is not an integer".into(),
            ));
        };
        if value.width.signed() {
            i64::try_from(value.signed()).map_err(|_| ExecutionFault::Runtime {
                category: "BoundsError".into(),
                message: "integer operand is outside the supported index range".into(),
            })
        } else {
            i64::try_from(value.unsigned()).map_err(|_| ExecutionFault::Runtime {
                category: "BoundsError".into(),
                message: "integer operand is outside the supported index range".into(),
            })
        }
    }

    fn integer_from_value(&self, value: Option<&Value>) -> Result<i64, ExecutionFault> {
        let Some(Value::Int(value)) = value else {
            return Err(ExecutionFault::Internal(
                "integer intrinsic argument is missing or not an integer".into(),
            ));
        };
        if value.width.signed() {
            i64::try_from(value.signed()).map_err(|_| ExecutionFault::Runtime {
                category: "BoundsError".into(),
                message: "integer operand is outside the supported index range".into(),
            })
        } else {
            i64::try_from(value.unsigned()).map_err(|_| ExecutionFault::Runtime {
                category: "BoundsError".into(),
                message: "integer operand is outside the supported index range".into(),
            })
        }
    }

    fn char_value(&self, value: &Value) -> Result<String, ExecutionFault> {
        let Value::Heap(id) = value else {
            return Err(ExecutionFault::Internal(
                "Char intrinsic received a non-Char value".into(),
            ));
        };
        match self.heap.get(*id) {
            Some(HeapEntry::Char(value)) => Ok(value.clone()),
            _ => Err(ExecutionFault::Internal(
                "Char intrinsic received a non-Char heap value".into(),
            )),
        }
    }

    fn execute_browser_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        if name.starts_with("zirk_rt_decimal_") || name == "zirk_str_from_decimal" {
            return self.execute_decimal_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_int_") {
            return self.execute_int_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_float_") {
            return self.execute_float_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_rt_date_")
            || name.starts_with("zirk_rt_time_")
            || name.starts_with("zirk_rt_datetime_")
        {
            return self.execute_temporal_intrinsic(name, arguments, ty);
        }
        if matches!(
            name,
            "zirk_rt_array_from_list" | "zirk_rt_map_sorted_keys" | "zirk_rt_set_sorted_elements"
        ) {
            return self.execute_collection_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_regex_") {
            return self.execute_regex_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_str_") {
            return self.execute_string_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_duration_") || name.starts_with("zirk_rt_duration_") {
            return self.execute_duration_intrinsic(name, arguments, ty);
        }
        if name.starts_with("zirk_range_") {
            return self.execute_range_intrinsic(name, arguments, ty);
        }
        let receiver = arguments
            .first()
            .ok_or_else(|| ExecutionFault::Internal(format!("{name} has invalid arity")))?;
        let value = self.char_value(receiver)?;
        let boolean = |value| Ok(Value::Boolean(value));
        match name {
            "zirk_char_is_uppercase" => boolean(zirk_runtime_core::char::is_uppercase(&value)),
            "zirk_char_is_lowercase" => boolean(zirk_runtime_core::char::is_lowercase(&value)),
            "zirk_char_is_digit" => boolean(zirk_runtime_core::char::is_digit(&value)),
            "zirk_char_is_letter" => boolean(zirk_runtime_core::char::is_letter(&value)),
            "zirk_char_is_whitespace" => boolean(zirk_runtime_core::char::is_whitespace(&value)),
            "zirk_char_is_ascii" => boolean(value.is_ascii()),
            "zirk_char_is_alphabetic" => boolean(value.chars().all(char::is_alphabetic)),
            "zirk_char_is_numeric" => boolean(value.chars().all(char::is_numeric)),
            "zirk_char_is_alphanumeric" => boolean(value.chars().all(char::is_alphanumeric)),
            "zirk_char_byte_length" => {
                let IrType::Int(width) = ty else {
                    return Err(ExecutionFault::Internal(
                        "Char byte_length has non-integer result".into(),
                    ));
                };
                IntegerValue::from_literal(value.len() as i128, width)
                    .map(Value::Int)
                    .map_err(ExecutionFault::Internal)
            }
            "zirk_char_codepoint_count" => {
                let IrType::Int(width) = ty else {
                    return Err(ExecutionFault::Internal(
                        "Char codepoint_count has non-integer result".into(),
                    ));
                };
                IntegerValue::from_literal(value.chars().count() as i128, width)
                    .map(Value::Int)
                    .map_err(ExecutionFault::Internal)
            }
            "zirk_char_ascii_code" => {
                let IrType::Int(width) = ty else {
                    return Err(ExecutionFault::Internal(
                        "Char ascii_code has non-integer result".into(),
                    ));
                };
                IntegerValue::from_literal(
                    i128::from(zirk_runtime_core::char::ascii_code(&value)),
                    width,
                )
                .map(Value::Int)
                .map_err(ExecutionFault::Internal)
            }
            "zirk_char_to_uppercase" => self
                .allocate_string(zirk_runtime_core::char::uppercase(&value))
                .map(Value::Heap),
            "zirk_char_to_lowercase" => self
                .allocate_string(zirk_runtime_core::char::lowercase(&value))
                .map(Value::Heap),
            "zirk_char_normalize" => self
                .allocate_string(zirk_runtime_core::text::normalize(
                    &value,
                    &self.text_argument(arguments.get(1))?,
                ))
                .map(Value::Heap),
            "zirk_char_bytes" => {
                let values = value
                    .bytes()
                    .map(|byte| Value::Int(IntegerValue::from_bits(u128::from(byte), IntWidth::U8)))
                    .collect();
                self.allocate_value_list(values)
            }
            "zirk_char_codepoints" => {
                let values = value
                    .chars()
                    .map(|point| {
                        Value::Int(IntegerValue::from_bits(
                            u128::from(point as u32),
                            IntWidth::U32,
                        ))
                    })
                    .collect();
                self.allocate_value_list(values)
            }
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    fn execute_duration_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        const WEEK: i64 = 604_800_000_000_000;
        const DAY: i64 = 86_400_000_000_000;
        const HOUR: i64 = 3_600_000_000_000;
        const MINUTE: i64 = 60_000_000_000;
        const SECOND: i64 = 1_000_000_000;
        const MILLI: i64 = 1_000_000;
        const MICRO: i64 = 1_000;
        let nanos = self.integer_from_value(arguments.first())?;
        let int = |value: i64| match ty {
            IrType::Int(width) => IntegerValue::from_literal(i128::from(value), width)
                .map(Value::Int)
                .map_err(ExecutionFault::Internal),
            _ => Err(ExecutionFault::Internal(format!(
                "{name} has non-integer result"
            ))),
        };
        let float = |value: f64| match ty {
            IrType::Float(width) => Self::binary_float_value(value, width),
            _ => Err(ExecutionFault::Internal(format!(
                "{name} has non-float result"
            ))),
        };
        let magnitude = (nanos as i128).abs();
        let component = |unit: i128, modulus: Option<i128>| {
            let value = modulus.map_or(magnitude, |modulus| magnitude % modulus);
            (value / unit) as i64
        };
        match name {
            "zirk_rt_duration_to_string" => self
                .allocate_string(zirk_runtime_core::duration::format(nanos))
                .map(Value::Heap),
            "zirk_duration_abs" => nanos.checked_abs().map_or_else(
                || {
                    Err(ExecutionFault::Runtime {
                        category: "ArithmeticError".into(),
                        message: "Duration.abs overflows".into(),
                    })
                },
                int,
            ),
            "zirk_duration_sign" => int(nanos.signum()),
            "zirk_duration_days" => int(component(i128::from(DAY), None)),
            "zirk_duration_hours" => int(component(i128::from(HOUR), Some(i128::from(DAY)))),
            "zirk_duration_minutes" => int(component(i128::from(MINUTE), Some(i128::from(HOUR)))),
            "zirk_duration_seconds" => int(component(i128::from(SECOND), Some(i128::from(MINUTE)))),
            "zirk_duration_milliseconds" => {
                int(component(i128::from(MILLI), Some(i128::from(SECOND))))
            }
            "zirk_duration_microseconds" => {
                int(component(i128::from(MICRO), Some(i128::from(SECOND))) % 1_000)
            }
            "zirk_duration_nanoseconds" => int((magnitude % i128::from(SECOND) % 1_000) as i64),
            "zirk_duration_total_weeks" => float(nanos as f64 / WEEK as f64),
            "zirk_duration_total_days" => float(nanos as f64 / DAY as f64),
            "zirk_duration_total_hours" => float(nanos as f64 / HOUR as f64),
            "zirk_duration_total_minutes" => float(nanos as f64 / MINUTE as f64),
            "zirk_duration_total_seconds" => float(nanos as f64 / SECOND as f64),
            "zirk_duration_total_milliseconds" => float(nanos as f64 / MILLI as f64),
            "zirk_duration_total_microseconds" => float(nanos as f64 / MICRO as f64),
            "zirk_duration_total_nanoseconds" => float(nanos as f64),
            "zirk_duration_whole_weeks" => int(nanos / WEEK),
            "zirk_duration_whole_days" => int(nanos / DAY),
            "zirk_duration_whole_hours" => int(nanos / HOUR),
            "zirk_duration_whole_minutes" => int(nanos / MINUTE),
            "zirk_duration_whole_seconds" => int(nanos / SECOND),
            "zirk_duration_whole_milliseconds" => int(nanos / MILLI),
            "zirk_duration_whole_microseconds" => int(nanos / MICRO),
            "zirk_duration_whole_nanoseconds" => int(nanos),
            "zirk_duration_to_iso_string" => self
                .allocate_string(zirk_runtime_core::duration::format_iso(nanos))
                .map(Value::Heap),
            "zirk_duration_format" => {
                let template = self.text_argument(arguments.get(1))?;
                self.allocate_string(zirk_runtime_core::duration::format_template(
                    nanos, &template,
                ))
                .map(Value::Heap)
            }
            "zirk_duration_humanize" => {
                let locale = self.text_argument(arguments.get(1))?;
                let max_units = self.i32_argument(arguments.get(2))?;
                self.allocate_string(zirk_runtime_core::duration::humanize(
                    nanos, &locale, max_units,
                ))
                .map(Value::Heap)
            }
            "zirk_duration_round" => int(zirk_runtime_core::duration::round(
                nanos,
                self.integer_from_value(arguments.get(1))?,
            )),
            "zirk_duration_floor" => int(zirk_runtime_core::duration::floor(
                nanos,
                self.integer_from_value(arguments.get(1))?,
            )),
            "zirk_duration_ceil" => int(zirk_runtime_core::duration::ceil(
                nanos,
                self.integer_from_value(arguments.get(1))?,
            )),
            "zirk_duration_truncate" => int(zirk_runtime_core::duration::truncate(
                nanos,
                self.integer_from_value(arguments.get(1))?,
            )),
            "zirk_rt_duration_mul_f64" => int(zirk_runtime_core::duration::multiply(
                nanos,
                self.float64_argument(arguments.get(1))?,
            )
            .map_err(Self::duration_scalar_fault)?),
            "zirk_rt_duration_div_f64" => int(zirk_runtime_core::duration::divide(
                nanos,
                self.float64_argument(arguments.get(1))?,
            )
            .map_err(Self::duration_scalar_fault)?),
            "zirk_rt_duration_div_duration" => float(zirk_runtime_core::duration::ratio(
                nanos,
                self.integer_from_value(arguments.get(1))?,
            )),
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    fn execute_range_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        let int = |value: i64| match ty {
            IrType::Int(width) => IntegerValue::from_literal(i128::from(value), width)
                .map(Value::Int)
                .map_err(ExecutionFault::Internal),
            _ => Err(ExecutionFault::Internal(format!(
                "{name} has non-integer result"
            ))),
        };
        let range = |value: &Value, heap: &Heap| match value {
            Value::Heap(id) => match heap.get(*id) {
                Some(HeapEntry::Range(value)) => Ok(*value),
                _ => Err(ExecutionFault::Internal(
                    "Range intrinsic received a non-Range value".into(),
                )),
            },
            _ => Err(ExecutionFault::Internal(
                "Range intrinsic received a non-heap value".into(),
            )),
        };
        match name {
            "zirk_range_new" => {
                let [start, end, step, inclusive] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_range_new has invalid arity".into(),
                    ));
                };
                let value = zirk_runtime_core::range::Range::new(
                    self.integer_from_value(Some(start))?,
                    self.integer_from_value(Some(end))?,
                    self.integer_from_value(Some(step))?,
                    self.integer_from_value(Some(inclusive))? != 0,
                );
                self.heap
                    .allocate(
                        HeapEntry::Range(value),
                        std::mem::size_of::<HeapEntry>(),
                        self.limits,
                    )
                    .map(Value::Heap)
                    .map_err(ExecutionFault::Limit)
            }
            "zirk_range_start" => int(range(
                arguments.first().ok_or_else(|| {
                    ExecutionFault::Internal("zirk_range_start has invalid arity".into())
                })?,
                &self.heap,
            )?
            .start),
            "zirk_range_end" => int(range(
                arguments.first().ok_or_else(|| {
                    ExecutionFault::Internal("zirk_range_end has invalid arity".into())
                })?,
                &self.heap,
            )?
            .end),
            "zirk_range_step" => int(range(
                arguments.first().ok_or_else(|| {
                    ExecutionFault::Internal("zirk_range_step has invalid arity".into())
                })?,
                &self.heap,
            )?
            .step),
            "zirk_range_inclusive" => int(i64::from(
                range(
                    arguments.first().ok_or_else(|| {
                        ExecutionFault::Internal("zirk_range_inclusive has invalid arity".into())
                    })?,
                    &self.heap,
                )?
                .inclusive,
            )),
            "zirk_range_length" => int(range(
                arguments.first().ok_or_else(|| {
                    ExecutionFault::Internal("zirk_range_length has invalid arity".into())
                })?,
                &self.heap,
            )?
            .len()
            .unwrap_or(-1)),
            "zirk_range_element_at" => {
                let [receiver, index] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_range_element_at has invalid arity".into(),
                    ));
                };
                int(range(receiver, &self.heap)?.element_at(self.integer_from_value(Some(index))?))
            }
            "zirk_range_slice" => {
                let [receiver, start, end, step] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_range_slice has invalid arity".into(),
                    ));
                };
                let value = range(receiver, &self.heap)?.slice(
                    self.integer_from_value(Some(start))?,
                    self.integer_from_value(Some(end))?,
                    self.integer_from_value(Some(step))?,
                );
                self.heap
                    .allocate(
                        HeapEntry::Range(value),
                        std::mem::size_of::<HeapEntry>(),
                        self.limits,
                    )
                    .map(Value::Heap)
                    .map_err(ExecutionFault::Limit)
            }
            "zirk_range_to_string" => {
                let [receiver, duration_parts] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_range_to_string has invalid arity".into(),
                    ));
                };
                let text = range(receiver, &self.heap)?
                    .format(self.integer_from_value(Some(duration_parts))? != 0);
                self.allocate_string(text).map(Value::Heap)
            }
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// The raw `i128` payload of an `Int` argument. The `zirk_int_*` and
    /// `DateTime` externs receive their value already widened to `Int128`.
    fn int128_argument(&self, value: Option<&Value>) -> Result<i128, ExecutionFault> {
        match value {
            Some(Value::Int(value)) => Ok(value.signed()),
            _ => Err(ExecutionFault::Internal(
                "integer intrinsic argument is missing or not an integer".into(),
            )),
        }
    }

    /// `i32` metadata arguments (`bits`, `signed`, radix, rounding modes).
    fn i32_argument(&self, value: Option<&Value>) -> Result<i32, ExecutionFault> {
        Ok(self.integer_from_value(value)? as i32)
    }

    /// `f64` arguments of the `zirk_float_*` and scalar `zirk_rt_*` externs.
    fn float64_argument(&self, value: Option<&Value>) -> Result<f64, ExecutionFault> {
        match value {
            Some(Value::Float {
                width: FloatWidth::F128,
                text,
            }) => zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(text)
                .map(|value| value.display_f64())
                .map_err(|error| {
                    ExecutionFault::Internal(format!("invalid Float128 value: {error:?}"))
                }),
            Some(Value::Float { text, .. }) => Self::parse_binary_float(text),
            _ => Err(ExecutionFault::Internal(
                "float intrinsic argument is missing or not a float".into(),
            )),
        }
    }

    fn decimal_argument(
        &self,
        value: Option<&Value>,
    ) -> Result<zirk_runtime_core::decimal::Decimal, ExecutionFault> {
        match value {
            Some(Value::Decimal(text)) => self.parse_decimal(text),
            _ => Err(ExecutionFault::Internal(
                "decimal intrinsic argument is missing or not a Decimal".into(),
            )),
        }
    }

    /// A `String`/`Char` heap argument — they share representation (ADR-014).
    fn text_argument(&self, value: Option<&Value>) -> Result<String, ExecutionFault> {
        match value {
            Some(Value::Heap(id)) => match self.heap.get(*id) {
                Some(HeapEntry::String(text)) | Some(HeapEntry::Char(text)) => Ok(text.clone()),
                _ => Err(ExecutionFault::Internal(
                    "intrinsic argument is not a String or Char".into(),
                )),
            },
            _ => Err(ExecutionFault::Internal(
                "intrinsic argument is missing or not a heap value".into(),
            )),
        }
    }

    fn int_result(&self, value: i128, ty: IrType, name: &str) -> Result<Value, ExecutionFault> {
        let IrType::Int(width) = ty else {
            return Err(ExecutionFault::Internal(format!(
                "{name} has a non-integer result"
            )));
        };
        IntegerValue::from_literal(value, width)
            .map(Value::Int)
            .map_err(ExecutionFault::Internal)
    }

    fn float_result(&self, value: f64, ty: IrType, name: &str) -> Result<Value, ExecutionFault> {
        let IrType::Float(width) = ty else {
            return Err(ExecutionFault::Internal(format!(
                "{name} has a non-float result"
            )));
        };
        Self::binary_float_value(value, width)
    }

    /// Maps a decimal operation's controlled failure to the failure category
    /// the native runtime reports for the same condition.
    fn decimal_fault(error: zirk_runtime_core::decimal::DecimalError) -> ExecutionFault {
        use zirk_runtime_core::decimal::DecimalError;
        let (category, message) = match error {
            DecimalError::Overflow => ("ArithmeticOverflowError", "decimal arithmetic overflow"),
            DecimalError::DivisionByZero => ("DivisionByZeroError", "division by zero"),
            DecimalError::Domain => ("DomainError", "decimal operation is outside its domain"),
            DecimalError::InvalidCast => (
                "InvalidCastError",
                "decimal value does not fit the target type",
            ),
        };
        ExecutionFault::Runtime {
            category: category.into(),
            message: message.into(),
        }
    }

    fn duration_scalar_fault(error: zirk_runtime_core::duration::ScalarError) -> ExecutionFault {
        use zirk_runtime_core::duration::ScalarError;
        let (category, message) = match error {
            ScalarError::DivisionByZero => ("DivisionByZeroError", "division by zero"),
            ScalarError::NonFiniteResult => ("FloatNanError", "Float operation produced NaN"),
            ScalarError::Overflow => ("ArithmeticOverflowError", "duration scalar overflow"),
        };
        ExecutionFault::Runtime {
            category: category.into(),
            message: message.into(),
        }
    }

    /// Allocates `values` as a `List` heap entry.
    fn allocate_value_list(&mut self, values: Vec<Value>) -> Result<Value, ExecutionFault> {
        let bytes = std::mem::size_of::<HeapEntry>()
            .checked_add(values.len().saturating_mul(std::mem::size_of::<Value>()))
            .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
        self.heap
            .allocate(HeapEntry::List(values), bytes, self.limits)
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// Allocates each part as a `String` and wraps them into a `List`.
    fn string_list_result(&mut self, parts: Vec<String>) -> Result<Value, ExecutionFault> {
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            values.push(Value::Heap(self.allocate_string(part)?));
        }
        self.allocate_value_list(values)
    }

    /// The `i64` word a `Map`/`Set` sorted snapshot compares, mirroring the
    /// native runtime's word order: integers contribute their bit pattern
    /// sign- or zero-extended per declared width, heap values their identity
    /// (the analogue of a native pointer).
    fn value_sort_word(&self, value: &Value) -> i64 {
        match value {
            Value::Int(value) if value.width.signed() => value.signed() as i64,
            Value::Int(value) => value.unsigned() as i64,
            Value::Boolean(value) => i64::from(*value),
            Value::Heap(id) => i64::from(id.0),
            _ => 0,
        }
    }

    /// The contents a string-content `Map`/`Set` sort compares; non-text
    /// values sort as empty, matching the native null-word rule.
    fn value_sort_text(&self, value: &Value) -> String {
        match value {
            Value::Heap(id) => match self.heap.get(*id) {
                Some(HeapEntry::String(text)) | Some(HeapEntry::Char(text)) => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }

    /// The `Regex.Match` class layout behind a `Nullable<Object>` result
    /// (`find`) or the element type of a `List` result (`find_all`).
    fn regex_match_layout(&self, ty: IrType) -> Result<u32, ExecutionFault> {
        let layout = match ty {
            IrType::Nullable(Nullable::Object(layout)) => Some(layout),
            IrType::List(list) => match self.module.list_types.get(list as usize) {
                Some(&IrType::Object(layout)) => Some(layout),
                _ => None,
            },
            _ => None,
        };
        layout.ok_or_else(|| {
            ExecutionFault::Internal("regex match intrinsic has a non-match result".into())
        })
    }

    /// Allocates a `Regex.Match` object. The field order is `text`,
    /// `haystack`, `start`, `end`, `regex`, matching the class the checker
    /// declares; `start`/`end` are byte offsets into `haystack`.
    fn allocate_regex_match(
        &mut self,
        layout: u32,
        matched: &str,
        haystack: &str,
        start: usize,
        end: usize,
        regex: Value,
    ) -> Result<Value, ExecutionFault> {
        let text = Value::Heap(self.allocate_string(matched.to_owned())?);
        let haystack = Value::Heap(self.allocate_string(haystack.to_owned())?);
        let offset =
            |value: usize| Value::Int(IntegerValue::from_bits(value as u128, IntWidth::I64));
        let fields = vec![text, haystack, offset(start), offset(end), regex];
        let bytes = std::mem::size_of::<HeapEntry>()
            .checked_add(fields.len().saturating_mul(std::mem::size_of::<Value>()))
            .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
        self.heap
            .allocate(HeapEntry::Object { layout, fields }, bytes, self.limits)
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// The exact decimal surface: every `zirk_rt_decimal_*` extern plus
    /// `zirk_str_from_decimal`, dispatched to the same `runtime-core`
    /// arithmetic the native adapters call.
    fn execute_decimal_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::decimal as dec;
        let value = |result: Result<dec::Decimal, dec::DecimalError>| {
            result
                .map(|value| Value::Decimal(dec::format(value)))
                .map_err(Self::decimal_fault)
        };
        let dec_arg = |index: usize| self.decimal_argument(arguments.get(index));
        let int_arg = |index: usize| self.i32_argument(arguments.get(index));
        let ordered = |left: dec::Decimal,
                       right: dec::Decimal,
                       pick_less: bool|
         -> Result<bool, ExecutionFault> {
            let order = dec::compare(left, right).map_err(Self::decimal_fault)?;
            Ok(if pick_less { order <= 0 } else { order >= 0 })
        };
        match name {
            "zirk_rt_decimal_add" => value(dec::add(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_sub" => value(dec::sub(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_mul" => value(dec::mul(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_div" => value(dec::div(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_rem" => value(dec::rem(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_pow" => value(dec::pow(dec_arg(0)?, dec_arg(1)?)),
            "zirk_rt_decimal_pow_i" => value(dec::pow_int(
                dec_arg(0)?,
                i128::from(self.integer_from_value(arguments.get(1))?),
            )),
            "zirk_rt_decimal_div_ex" => value(dec::div_to_scale(
                dec_arg(0)?,
                dec_arg(1)?,
                int_arg(3)?.max(0) as u32,
                dec::RoundingMode::from_abi(int_arg(2)?),
            )),
            "zirk_rt_decimal_round" => value(dec::round_to_places(
                dec_arg(0)?,
                int_arg(1)?,
                dec::RoundingMode::from_abi(int_arg(2)?),
            )),
            "zirk_rt_decimal_min" => {
                let (a, b) = (dec_arg(0)?, dec_arg(1)?);
                value(Ok(if ordered(a, b, true)? { a } else { b }))
            }
            "zirk_rt_decimal_max" => {
                let (a, b) = (dec_arg(0)?, dec_arg(1)?);
                value(Ok(if ordered(a, b, false)? { a } else { b }))
            }
            "zirk_rt_decimal_clamp" => {
                let (v, lo, hi) = (dec_arg(0)?, dec_arg(1)?, dec_arg(2)?);
                let picked = if dec::compare(v, lo).map_err(Self::decimal_fault)? < 0 {
                    lo
                } else if dec::compare(v, hi).map_err(Self::decimal_fault)? > 0 {
                    hi
                } else {
                    v
                };
                value(Ok(picked))
            }
            "zirk_rt_decimal_neg" => value(dec::negate(dec_arg(0)?)),
            "zirk_rt_decimal_abs" => value(dec::abs(dec_arg(0)?)),
            "zirk_rt_decimal_sqrt" => value(dec::sqrt(dec_arg(0)?)),
            "zirk_rt_decimal_floor" => value(dec::round_to_places(
                dec_arg(0)?,
                0,
                dec::RoundingMode::Floor,
            )),
            "zirk_rt_decimal_ceil" => value(dec::round_to_places(
                dec_arg(0)?,
                0,
                dec::RoundingMode::Ceil,
            )),
            "zirk_rt_decimal_truncate" => value(dec::round_to_places(
                dec_arg(0)?,
                0,
                dec::RoundingMode::Down,
            )),
            "zirk_rt_decimal_fraction" => {
                let v = dec_arg(0)?;
                let whole = dec::round_to_places(v, 0, dec::RoundingMode::Down)
                    .map_err(Self::decimal_fault)?;
                value(dec::sub(v, whole))
            }
            "zirk_rt_decimal_cmp" => self.int_result(
                i128::from(dec::compare(dec_arg(0)?, dec_arg(1)?).map_err(Self::decimal_fault)?),
                ty,
                name,
            ),
            "zirk_rt_decimal_sign" => self.int_result(i128::from(dec_arg(0)?.sign()), ty, name),
            "zirk_rt_decimal_scale" => self.int_result(i128::from(dec_arg(0)?.scale), ty, name),
            "zirk_rt_decimal_is_zero" => Ok(Value::Boolean(dec_arg(0)?.is_zero())),
            "zirk_rt_decimal_is_negative" => Ok(Value::Boolean(dec_arg(0)?.is_negative())),
            "zirk_rt_decimal_is_integer" => Ok(Value::Boolean(dec_arg(0)?.is_integer())),
            "zirk_str_from_decimal" => self
                .allocate_string(dec::format(dec_arg(0)?))
                .map(Value::Heap),
            "zirk_rt_decimal_to_f64" => self.float_result(dec::to_f64(dec_arg(0)?), ty, name),
            "zirk_rt_decimal_from_i128" => Ok(Value::Decimal(dec::format(dec::Decimal::new(
                self.int128_argument(arguments.first())?,
                0,
            )))),
            "zirk_rt_decimal_to_i128_checked" => {
                let signed = matches!(arguments.get(2), Some(Value::Boolean(true)));
                let value = dec::to_i128_checked(dec_arg(0)?, int_arg(1)? as u32, signed)
                    .map_err(Self::decimal_fault)?;
                self.int_result(value, ty, name)
            }
            "zirk_rt_decimal_from_f64" => value(
                dec::from_f64_checked(self.float64_argument(arguments.first())?).map_err(|error| {
                    // A non-finite source is an invalid cast, matching the
                    // native boundary's `zirk_rt_invalid_cast`.
                    if error == dec::DecimalError::Domain {
                        dec::DecimalError::InvalidCast
                    } else {
                        error
                    }
                }),
            ),
            "zirk_rt_decimal_from_literal" => {
                let text = self.text_argument(arguments.first())?;
                // A literal the lexer accepted always parses; a miss is a
                // compiler bug — the native boundary traps `zirk_rt_overflow`.
                dec::parse(&text)
                    .map(|parsed| Value::Decimal(dec::format(parsed)))
                    .ok_or_else(|| Self::decimal_fault(dec::DecimalError::Overflow))
            }
            "zirk_rt_decimal_parse_ok" => Ok(Value::Boolean(
                dec::parse(&self.text_argument(arguments.first())?).is_some(),
            )),
            "zirk_rt_decimal_parse_value" => Ok(Value::Decimal(dec::format(
                dec::parse(&self.text_argument(arguments.first())?).unwrap_or(dec::Decimal::ZERO),
            ))),
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// The `IntN` member surface: `zirk_int_*` externs dispatched to the
    /// shared `runtime-core::scalar` implementations. The receiver arrives
    /// widened to `Int128`; `bits`/`signed` describe the original width.
    fn execute_int_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::scalar as sc;
        let wide = |index: usize| self.int128_argument(arguments.get(index));
        let meta = |index: usize| self.i32_argument(arguments.get(index));
        let int = |value: i128| self.int_result(value, ty, name);
        let binary =
            |f: fn(i128, i128, i32, i32) -> i128| int(f(wide(0)?, wide(1)?, meta(2)?, meta(3)?));
        let checked_ok = |f: fn(i128, i128, i32, i32) -> bool| {
            Ok(Value::Boolean(f(wide(0)?, wide(1)?, meta(2)?, meta(3)?)))
        };
        let checked_value =
            |f: fn(i128, i128, i32, i32) -> i128| int(f(wide(0)?, wide(1)?, meta(2)?, meta(3)?));
        match name {
            "zirk_int_abs" => int(sc::int_abs(wide(0)?, meta(1)?, meta(2)?)),
            "zirk_int_sign" => int(i128::from(sc::int_sign(wide(0)?, meta(1)?, meta(2)?))),
            "zirk_int_min" => int(sc::int_min(wide(0)?, wide(1)?, meta(2)?, meta(3)?)),
            "zirk_int_max" => int(sc::int_max(wide(0)?, wide(1)?, meta(2)?, meta(3)?)),
            "zirk_int_clamp" => int(sc::int_clamp(
                wide(0)?,
                wide(1)?,
                wide(2)?,
                meta(3)?,
                meta(4)?,
            )),
            "zirk_int_is_zero" => Ok(Value::Boolean(wide(0)? == 0)),
            "zirk_int_is_even" => Ok(Value::Boolean(wide(0)? % 2 == 0)),
            "zirk_int_is_odd" => Ok(Value::Boolean(wide(0)? % 2 != 0)),
            "zirk_int_bit_count" => int(i128::from(sc::int_bit_count(wide(0)?, meta(1)?))),
            "zirk_int_leading_zeros" => int(i128::from(sc::int_leading_zeros(wide(0)?, meta(1)?))),
            "zirk_int_trailing_zeros" => {
                int(i128::from(sc::int_trailing_zeros(wide(0)?, meta(1)?)))
            }
            "zirk_int_rotate_left" => int(sc::int_rotate_left(
                wide(0)?,
                self.integer_from_value(arguments.get(1))?,
                meta(2)?,
            )),
            "zirk_int_rotate_right" => int(sc::int_rotate_right(
                wide(0)?,
                self.integer_from_value(arguments.get(1))?,
                meta(2)?,
            )),
            "zirk_int_wrapping_add" => binary(sc::int_wrapping_add),
            "zirk_int_wrapping_sub" => binary(sc::int_wrapping_sub),
            "zirk_int_wrapping_mul" => binary(sc::int_wrapping_mul),
            "zirk_int_saturating_add" => binary(sc::int_saturating_add),
            "zirk_int_saturating_sub" => binary(sc::int_saturating_sub),
            "zirk_int_saturating_mul" => binary(sc::int_saturating_mul),
            "zirk_int_to_string_radix" => self
                .allocate_string(sc::format_integer_radix(wide(0)?, meta(1)?))
                .map(Value::Heap),
            "zirk_int_checked_add_ok" => checked_ok(sc::int_checked_add_ok),
            // The `value` half of checked add/sub/mul is the wrapped result —
            // only meaningful when the `ok` probe already passed.
            "zirk_int_checked_add_value" => checked_value(sc::int_wrapping_add),
            "zirk_int_checked_sub_ok" => checked_ok(sc::int_checked_sub_ok),
            "zirk_int_checked_sub_value" => checked_value(sc::int_wrapping_sub),
            "zirk_int_checked_mul_ok" => checked_ok(sc::int_checked_mul_ok),
            "zirk_int_checked_mul_value" => checked_value(sc::int_wrapping_mul),
            "zirk_int_checked_div_ok" => checked_ok(sc::int_checked_div_ok),
            "zirk_int_checked_div_value" => checked_value(sc::int_checked_div_value),
            "zirk_int_checked_rem_ok" => checked_ok(sc::int_checked_rem_ok),
            "zirk_int_checked_rem_value" => checked_value(sc::int_checked_rem_value),
            "zirk_int_checked_pow_ok" => checked_ok(sc::int_checked_pow_ok),
            "zirk_int_checked_pow_value" => checked_value(sc::int_checked_pow_value),
            "zirk_int_parse_ok" => Ok(Value::Boolean(sc::int_parse_ok(
                &self.text_argument(arguments.first())?,
                meta(1)?,
                meta(2)?,
            ))),
            "zirk_int_parse_value" => int(sc::int_parse_value(
                &self.text_argument(arguments.first())?,
                meta(1)?,
                meta(2)?,
            )),
            "zirk_int_parse_radix_ok" => Ok(Value::Boolean(sc::int_parse_radix_ok(
                &self.text_argument(arguments.first())?,
                meta(1)?,
                meta(2)?,
                meta(3)?,
            ))),
            "zirk_int_parse_radix_value" => int(sc::int_parse_radix_value(
                &self.text_argument(arguments.first())?,
                meta(1)?,
                meta(2)?,
                meta(3)?,
            )),
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// The binary-float member surface: `zirk_float_*` externs over the
    /// `f64` view the ABI declares for them.
    fn execute_float_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::scalar as sc;
        let float = |value: f64| self.float_result(value, ty, name);
        match name {
            "zirk_float_parse_ok" => Ok(Value::Boolean(
                sc::parse_float(&self.text_argument(arguments.first())?).is_some(),
            )),
            "zirk_float_parse_value" => {
                float(sc::parse_float(&self.text_argument(arguments.first())?).unwrap_or(0.0))
            }
            _ => {
                let value = self.float64_argument(arguments.first())?;
                match name {
                    "zirk_float_abs" => float(value.abs()),
                    "zirk_float_sign" => self.int_result(
                        i128::from(if value > 0.0 {
                            1_i32
                        } else if value < 0.0 {
                            -1
                        } else {
                            0
                        }),
                        ty,
                        name,
                    ),
                    "zirk_float_min" => float(value.min(self.float64_argument(arguments.get(1))?)),
                    "zirk_float_max" => float(value.max(self.float64_argument(arguments.get(1))?)),
                    "zirk_float_clamp" => {
                        let lo = self.float64_argument(arguments.get(1))?;
                        let hi = self.float64_argument(arguments.get(2))?;
                        // `f64::clamp` panics when `lo > hi`; the manual form
                        // keeps the interpreter deterministic where the
                        // native ABI would trap.
                        float(if value < lo {
                            lo
                        } else if value > hi {
                            hi
                        } else {
                            value
                        })
                    }
                    "zirk_float_is_zero" => Ok(Value::Boolean(value == 0.0)),
                    "zirk_float_floor" => float(value.floor()),
                    "zirk_float_ceil" => float(value.ceil()),
                    "zirk_float_round" => float(value.round()),
                    "zirk_float_truncate" => float(value.trunc()),
                    "zirk_float_fraction" => float(value.fract()),
                    "zirk_float_is_finite" => Ok(Value::Boolean(value.is_finite())),
                    "zirk_float_is_infinite" => Ok(Value::Boolean(value.is_infinite())),
                    "zirk_float_is_negative" => Ok(Value::Boolean(value.is_sign_negative())),
                    "zirk_float_pow" => {
                        float(value.powi(self.integer_from_value(arguments.get(1))? as i32))
                    }
                    "zirk_float_powf" => {
                        float(value.powf(self.float64_argument(arguments.get(1))?))
                    }
                    "zirk_float_sqrt" => float(value.sqrt()),
                    "zirk_float_format" => self
                        .allocate_string(sc::format_float(
                            value,
                            &self.text_argument(arguments.get(1))?,
                        ))
                        .map(Value::Heap),
                    _ => Err(ExecutionFault::Internal(format!(
                        "unsupported browser intrinsic {name}"
                    ))),
                }
            }
        }
    }

    /// The civil `Date`/`Time`/`DateTime` surface — pure value operations
    /// shared with the native runtime through `runtime-core::temporal`. The
    /// host-clock externs (`today`, `now_*`) are excluded upstream by
    /// `is_host_clock_extern` and never reach this dispatcher.
    fn execute_temporal_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::temporal as tmp;
        let int64 = |index: usize| self.integer_from_value(arguments.get(index));
        let int32 = |index: usize| self.i32_argument(arguments.get(index));
        let wide = |index: usize| self.int128_argument(arguments.get(index));
        let text = |index: usize| self.text_argument(arguments.get(index));
        let int = |value: i64| self.int_result(i128::from(value), ty, name);
        let wide_result = |value: i128| self.int_result(value, ty, name);
        let check_days = |index: usize| -> Result<i64, ExecutionFault> {
            let days = int64(index)?;
            tmp::civil_from_days_checked(days).ok_or_else(|| ExecutionFault::Runtime {
                category: "ArithmeticOverflowError".into(),
                message: "date year is outside the public Int32 range".into(),
            })?;
            Ok(days)
        };
        let check_datetime = |index: usize| -> Result<i128, ExecutionFault> {
            let value = wide(index)?;
            let days =
                tmp::datetime_days_checked(value).ok_or_else(|| ExecutionFault::Runtime {
                    category: "ArithmeticOverflowError".into(),
                    message: "datetime day count is outside the public Int64 range".into(),
                })?;
            tmp::civil_from_days_checked(days).ok_or_else(|| ExecutionFault::Runtime {
                category: "ArithmeticOverflowError".into(),
                message: "date year is outside the public Int32 range".into(),
            })?;
            Ok(value)
        };
        match name {
            "zirk_rt_date_is_valid" => Ok(Value::Boolean(tmp::date_is_valid(
                int32(0)?,
                int32(1)?,
                int32(2)?,
            ))),
            "zirk_rt_date_days" => int(tmp::days_from_civil(int32(0)?, int32(1)?, int32(2)?)),
            "zirk_rt_date_year" => int(i64::from(tmp::civil_from_days(check_days(0)?).0)),
            "zirk_rt_date_month" => int(i64::from(tmp::civil_from_days(check_days(0)?).1)),
            "zirk_rt_date_day" => int(i64::from(tmp::civil_from_days(check_days(0)?).2)),
            "zirk_rt_date_to_string" => self
                .allocate_string(tmp::date_to_string(check_days(0)?))
                .map(Value::Heap),
            "zirk_rt_date_day_of_week" => int(i64::from(tmp::date_day_of_week(check_days(0)?))),
            "zirk_rt_date_day_of_year" => int(i64::from(tmp::date_day_of_year(check_days(0)?))),
            "zirk_rt_date_week_of_year" => int(i64::from(tmp::date_week_of_year(check_days(0)?))),
            "zirk_rt_date_quarter" => int(i64::from(tmp::date_quarter(check_days(0)?))),
            "zirk_rt_date_days_in_month" => int(i64::from(tmp::date_days_in_month(check_days(0)?))),
            "zirk_rt_date_days_in_year" => int(i64::from(tmp::date_days_in_year(check_days(0)?))),
            "zirk_rt_date_is_leap" => Ok(Value::Boolean(tmp::date_is_leap(check_days(0)?))),
            "zirk_rt_date_start_of_ok" | "zirk_rt_date_end_of_ok" => {
                Ok(Value::Boolean(tmp::date_unit_ok(&text(0)?)))
            }
            "zirk_rt_date_start_of_value" => int(tmp::date_start_of(check_days(0)?, &text(1)?)),
            "zirk_rt_date_end_of_value" => int(tmp::date_end_of(check_days(0)?, &text(1)?)),
            "zirk_rt_date_parse_ok" => {
                Ok(Value::Boolean(tmp::parse_date_text(&text(0)?).is_some()))
            }
            "zirk_rt_date_parse_value" => int(tmp::parse_date_text(&text(0)?).unwrap_or(0)),
            "zirk_rt_date_format" => self
                .allocate_string(tmp::date_format(check_days(0)?, &text(1)?))
                .map(Value::Heap),
            "zirk_rt_time_is_valid" => Ok(Value::Boolean(tmp::time_is_valid(
                int32(0)?,
                int32(1)?,
                int32(2)?,
                int64(3)?,
            ))),
            "zirk_rt_time_nanos" => {
                int(tmp::time_nanos(int32(0)?, int32(1)?, int32(2)?, int64(3)?))
            }
            "zirk_rt_time_hour" => int(i64::from(tmp::time_hour(int64(0)?))),
            "zirk_rt_time_minute" => int(i64::from(tmp::time_minute(int64(0)?))),
            "zirk_rt_time_second" => int(i64::from(tmp::time_second(int64(0)?))),
            "zirk_rt_time_nanosecond" => int(tmp::time_nanosecond(int64(0)?)),
            "zirk_rt_time_add_nanos" => int(tmp::time_add_nanos(int64(0)?, int64(1)?)),
            "zirk_rt_time_diff" => int(tmp::time_diff(int64(0)?, int64(1)?)),
            "zirk_rt_time_to_string" => self
                .allocate_string(tmp::format_time(int64(0)?))
                .map(Value::Heap),
            "zirk_rt_time_start_of_ok" | "zirk_rt_time_end_of_ok" => {
                Ok(Value::Boolean(tmp::time_unit_ok(&text(0)?)))
            }
            "zirk_rt_time_start_of_value" => int(tmp::time_start_of(int64(0)?, &text(1)?)),
            "zirk_rt_time_end_of_value" => int(tmp::time_end_of(int64(0)?, &text(1)?)),
            "zirk_rt_time_parse_ok" => {
                Ok(Value::Boolean(tmp::parse_time_text(&text(0)?).is_some()))
            }
            "zirk_rt_time_parse_value" => int(tmp::parse_time_text(&text(0)?).unwrap_or(0)),
            "zirk_rt_time_format" => self
                .allocate_string(tmp::time_format(int64(0)?, &text(1)?))
                .map(Value::Heap),
            "zirk_rt_datetime_new" => wide_result(tmp::datetime_pack(check_days(0)?, int64(1)?)),
            "zirk_rt_datetime_days" => int(tmp::datetime_days(check_datetime(0)?)),
            "zirk_rt_datetime_nanos" => int(tmp::datetime_nanos(check_datetime(0)?)),
            "zirk_rt_datetime_to_string" => self
                .allocate_string(tmp::datetime_to_string(check_datetime(0)?))
                .map(Value::Heap),
            "zirk_rt_datetime_start_of_ok" | "zirk_rt_datetime_end_of_ok" => {
                Ok(Value::Boolean(tmp::datetime_unit_ok(&text(0)?)))
            }
            "zirk_rt_datetime_start_of_value" => {
                wide_result(tmp::datetime_start_of(check_datetime(0)?, &text(1)?))
            }
            "zirk_rt_datetime_end_of_value" => {
                wide_result(tmp::datetime_end_of(check_datetime(0)?, &text(1)?))
            }
            "zirk_rt_datetime_parse_ok" => Ok(Value::Boolean(
                tmp::parse_datetime_text(&text(0)?).is_some(),
            )),
            "zirk_rt_datetime_parse_value" => {
                wide_result(tmp::parse_datetime_text(&text(0)?).unwrap_or(0))
            }
            "zirk_rt_datetime_format" => self
                .allocate_string(tmp::datetime_format(check_datetime(0)?, &text(1)?))
                .map(Value::Heap),
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// The three collection-shape externs: `List` to `Array` conversion and
    /// the sorted key/element snapshots `Map.to_string`/`Set.to_string` use.
    fn execute_collection_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        _ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        let elements = |value: Option<&Value>, heap: &Heap, name: &str| match value {
            Some(Value::Heap(id)) => match heap.get(*id) {
                Some(HeapEntry::List(values)) | Some(HeapEntry::Array(values)) => {
                    Ok(values.clone())
                }
                Some(HeapEntry::Set(values)) => Ok(values.clone()),
                Some(HeapEntry::Map(entries)) => {
                    Ok(entries.iter().map(|(key, _)| key.clone()).collect())
                }
                _ => Err(ExecutionFault::Internal(format!(
                    "{name} received a non-collection value"
                ))),
            },
            _ => Err(ExecutionFault::Internal(format!(
                "{name} received a non-heap value"
            ))),
        };
        match name {
            "zirk_rt_array_from_list" => {
                let values = elements(arguments.first(), &self.heap, name)?;
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(values.len().saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.heap
                    .allocate(HeapEntry::Array(values), bytes, self.limits)
                    .map(Value::Heap)
                    .map_err(ExecutionFault::Limit)
            }
            "zirk_rt_map_sorted_keys" | "zirk_rt_set_sorted_elements" => {
                let mut values = elements(arguments.first(), &self.heap, name)?;
                if self.integer_from_value(arguments.get(1))? != 0 {
                    values.sort_by_key(|value| self.value_sort_text(value));
                } else {
                    values.sort_by_key(|value| self.value_sort_word(value));
                }
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(values.len().saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                self.heap
                    .allocate(HeapEntry::Array(values), bytes, self.limits)
                    .map(Value::Heap)
                    .map_err(ExecutionFault::Limit)
            }
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// Browser-owned regex calls are deliberately bounded before invoking the
    /// regex engine.  The native ABI remains the authority for native runs;
    /// this path only replaces the four pure operations in the browser
    /// compatibility baseline.
    fn execute_regex_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        const MAX_REGEX_INPUT_BYTES: usize = 65_536;
        let string = |value: &Value, heap: &Heap| match value {
            Value::Heap(id) => match heap.get(*id) {
                Some(HeapEntry::String(text)) => Ok(text.clone()),
                _ => Err(ExecutionFault::Internal(
                    "Regex intrinsic received a non-String value".into(),
                )),
            },
            _ => Err(ExecutionFault::Internal(
                "Regex intrinsic received a non-heap value".into(),
            )),
        };
        let pattern = |value: &Value, heap: &Heap| match value {
            Value::Heap(id) => match heap.get(*id) {
                Some(HeapEntry::Regex(pattern)) => Ok(pattern.clone()),
                _ => Err(ExecutionFault::Internal(
                    "Regex intrinsic received a non-Regex value".into(),
                )),
            },
            _ => Err(ExecutionFault::Internal(
                "Regex intrinsic received a non-heap value".into(),
            )),
        };
        let bounded = |text: &str| {
            (text.len() <= MAX_REGEX_INPUT_BYTES)
                .then_some(())
                .ok_or_else(|| ExecutionFault::Runtime {
                    category: "LimitError".into(),
                    message: "regex input exceeds the browser limit".into(),
                })
        };
        match name {
            "zirk_regex_from_pattern" => {
                let [source] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_from_pattern has invalid arity".into(),
                    ));
                };
                let source = string(source, &self.heap)?;
                bounded(&source)?;
                zirk_runtime_core::regex::compile(&source).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                self.heap
                    .allocate(
                        HeapEntry::Regex(source),
                        std::mem::size_of::<HeapEntry>(),
                        self.limits,
                    )
                    .map(Value::Heap)
                    .map_err(ExecutionFault::Limit)
            }
            "zirk_regex_is_match" => {
                let [receiver, text] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_is_match has invalid arity".into(),
                    ));
                };
                let pattern = pattern(receiver, &self.heap)?;
                let text = string(text, &self.heap)?;
                bounded(&text)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                Ok(Value::Boolean(zirk_runtime_core::regex::is_match(
                    &regex, &text,
                )))
            }
            "zirk_regex_to_string" => {
                let [receiver] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_to_string has invalid arity".into(),
                    ));
                };
                self.allocate_string(pattern(receiver, &self.heap)?)
                    .map(Value::Heap)
            }
            "zirk_regex_replace" => {
                let [receiver, text, replacement] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_replace has invalid arity".into(),
                    ));
                };
                let pattern = pattern(receiver, &self.heap)?;
                let text = string(text, &self.heap)?;
                let replacement = string(replacement, &self.heap)?;
                bounded(&text)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                self.allocate_string(zirk_runtime_core::regex::replace(
                    &regex,
                    &text,
                    &replacement,
                ))
                .map(Value::Heap)
            }
            "zirk_regex_is_valid_pattern" => {
                let [source] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_is_valid_pattern has invalid arity".into(),
                    ));
                };
                Ok(Value::Boolean(zirk_runtime_core::regex::is_valid(&string(
                    source, &self.heap,
                )?)))
            }
            "zirk_regex_split" => {
                let [receiver, text] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_split has invalid arity".into(),
                    ));
                };
                let pattern = pattern(receiver, &self.heap)?;
                let text = string(text, &self.heap)?;
                bounded(&text)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                self.string_list_result(zirk_runtime_core::regex::split(&regex, &text))
            }
            "zirk_regex_find" => {
                let [receiver, text] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_find has invalid arity".into(),
                    ));
                };
                let pattern = pattern(receiver, &self.heap)?;
                let text = string(text, &self.heap)?;
                bounded(&text)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                match zirk_runtime_core::regex::find(&regex, &text) {
                    Some((start, end)) => self
                        .allocate_regex_match(
                            self.regex_match_layout(ty)?,
                            &text[start..end],
                            &text,
                            start,
                            end,
                            receiver.clone(),
                        )
                        .map(|value| Value::Nullable(Some(Box::new(value)))),
                    None => Ok(Value::Nullable(None)),
                }
            }
            "zirk_regex_find_all" => {
                let [receiver, text] = arguments else {
                    return Err(ExecutionFault::Internal(
                        "zirk_regex_find_all has invalid arity".into(),
                    ));
                };
                let layout = self.regex_match_layout(ty)?;
                let pattern = pattern(receiver, &self.heap)?;
                let text = string(text, &self.heap)?;
                bounded(&text)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                let mut values = Vec::new();
                for (start, end) in zirk_runtime_core::regex::find_all(&regex, &text) {
                    values.push(self.allocate_regex_match(
                        layout,
                        &text[start..end],
                        &text,
                        start,
                        end,
                        receiver.clone(),
                    )?);
                }
                self.allocate_value_list(values)
            }
            "zirk_regex_match_group_pos" | "zirk_regex_match_group_name" => {
                let [receiver, ..] = arguments else {
                    return Err(ExecutionFault::Internal(format!(
                        "{name} has invalid arity"
                    )));
                };
                let (haystack, start, pattern) = self.regex_match_parts(receiver, name)?;
                bounded(&haystack)?;
                let regex = zirk_runtime_core::regex::compile(&pattern).map_err(|error| {
                    ExecutionFault::Runtime {
                        category: "RegexError".into(),
                        message: error.to_string(),
                    }
                })?;
                let group = if name == "zirk_regex_match_group_pos" {
                    let index = self.integer_from_value(arguments.get(1))?;
                    let index = usize::try_from(index).map_err(|_| ExecutionFault::Runtime {
                        category: "BoundsError".into(),
                        message: "regex group index is out of range".into(),
                    })?;
                    zirk_runtime_core::regex::match_group_pos(&regex, &haystack, start, index)
                } else {
                    let group = string(
                        arguments.get(1).ok_or_else(|| {
                            ExecutionFault::Internal(format!("{name} has invalid arity"))
                        })?,
                        &self.heap,
                    )?;
                    zirk_runtime_core::regex::match_group_name(&regex, &haystack, start, &group)
                };
                self.allocate_string(group).map(Value::Heap)
            }
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// `(haystack, match_start, pattern)` of a `Regex.Match` object produced
    /// by `find`/`find_all` — the pieces `group(index)`/`group(name)` need to
    /// resolve captures by re-running the regex, matching the native layout.
    fn regex_match_parts(
        &self,
        receiver: &Value,
        name: &str,
    ) -> Result<(String, usize, String), ExecutionFault> {
        let invalid = || ExecutionFault::Internal(format!("{name} received a non-match value"));
        let Value::Heap(id) = receiver else {
            return Err(invalid());
        };
        let Some(HeapEntry::Object { fields, .. }) = self.heap.get(*id) else {
            return Err(invalid());
        };
        let [_, haystack, start, _, regex] = fields.as_slice() else {
            return Err(invalid());
        };
        let haystack = match haystack {
            Value::Heap(id) => match self.heap.get(*id) {
                Some(HeapEntry::String(text)) => text.clone(),
                _ => return Err(invalid()),
            },
            _ => return Err(invalid()),
        };
        let start = match start {
            Value::Int(value) => usize::try_from(value.signed()).map_err(|_| invalid())?,
            _ => return Err(invalid()),
        };
        let pattern = match regex {
            Value::Heap(id) => match self.heap.get(*id) {
                Some(HeapEntry::Regex(pattern)) => pattern.clone(),
                _ => return Err(invalid()),
            },
            _ => return Err(invalid()),
        };
        Ok((haystack, start, pattern))
    }

    fn execute_string_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Value],
        ty: IrType,
    ) -> Result<Value, ExecutionFault> {
        let receiver = arguments
            .first()
            .ok_or_else(|| ExecutionFault::Internal(format!("{name} has invalid arity")))?;
        let text = match receiver {
            Value::Heap(id) => match self.heap.get(*id) {
                Some(HeapEntry::String(text)) => text.clone(),
                _ => {
                    return Err(ExecutionFault::Internal(
                        "String intrinsic received a non-String value".into(),
                    ));
                }
            },
            _ => {
                return Err(ExecutionFault::Internal(
                    "String intrinsic received a non-heap value".into(),
                ));
            }
        };
        let argument_string = |index: usize| -> Result<String, ExecutionFault> {
            match arguments.get(index) {
                Some(Value::Heap(id)) => match self.heap.get(*id) {
                    Some(HeapEntry::String(value)) => Ok(value.clone()),
                    _ => Err(ExecutionFault::Internal(
                        "String intrinsic argument is not a String".into(),
                    )),
                },
                _ => Err(ExecutionFault::Internal(
                    "String intrinsic argument is missing".into(),
                )),
            }
        };
        let integer = |value: i128| -> Result<Value, ExecutionFault> {
            let IrType::Int(width) = ty else {
                return Err(ExecutionFault::Internal(
                    "String intrinsic has non-integer result".into(),
                ));
            };
            IntegerValue::from_literal(value, width)
                .map(Value::Int)
                .map_err(ExecutionFault::Internal)
        };
        match name {
            "zirk_str_length" => integer(zirk_runtime_core::text::grapheme_count(&text) as i128),
            "zirk_str_byte_length" => integer(text.len() as i128),
            "zirk_str_is_empty" => Ok(Value::Boolean(text.is_empty())),
            "zirk_str_find" => integer(
                zirk_runtime_core::text::grapheme_find(&text, &argument_string(1)?)
                    .map(|index| index as i128)
                    .unwrap_or(-1),
            ),
            "zirk_str_contains" => Ok(Value::Boolean(zirk_runtime_core::text::contains(
                &text,
                &argument_string(1)?,
            ))),
            "zirk_str_starts_with" => Ok(Value::Boolean(zirk_runtime_core::text::starts_with(
                &text,
                &argument_string(1)?,
            ))),
            "zirk_str_ends_with" => Ok(Value::Boolean(zirk_runtime_core::text::ends_with(
                &text,
                &argument_string(1)?,
            ))),
            "zirk_str_replace" => self
                .allocate_string(zirk_runtime_core::text::replace(
                    &text,
                    &argument_string(1)?,
                    &argument_string(2)?,
                ))
                .map(Value::Heap),
            "zirk_str_trim" => self.allocate_string(text.trim().into()).map(Value::Heap),
            "zirk_str_trim_start" => self
                .allocate_string(text.trim_start().into())
                .map(Value::Heap),
            "zirk_str_trim_end" => self
                .allocate_string(text.trim_end().into())
                .map(Value::Heap),
            "zirk_str_to_lowercase" => self.allocate_string(text.to_lowercase()).map(Value::Heap),
            "zirk_str_to_uppercase" => self.allocate_string(text.to_uppercase()).map(Value::Heap),
            "zirk_str_normalize" => self
                .allocate_string(zirk_runtime_core::text::normalize(
                    &text,
                    &argument_string(1)?,
                ))
                .map(Value::Heap),
            "zirk_str_clone" => self.allocate_string(text).map(Value::Heap),
            "zirk_str_substring" => {
                let start = self.integer_from_value(arguments.get(1))?;
                let end = self.integer_from_value(arguments.get(2))?;
                self.allocate_string(zirk_runtime_core::text::substring(&text, start, end).into())
                    .map(Value::Heap)
            }
            "zirk_str_search" => integer(
                text.find(&argument_string(1)?)
                    .map(|index| index as i128)
                    .unwrap_or(-1),
            ),
            "zirk_str_set" => {
                let offset = self.integer_from_value(arguments.get(1))?;
                let length = self.integer_from_value(arguments.get(2))?;
                let replacement = match arguments.get(3) {
                    Some(Value::Heap(id)) => match self.heap.get(*id) {
                        Some(HeapEntry::String(value)) | Some(HeapEntry::Char(value)) => {
                            value.clone()
                        }
                        _ => {
                            return Err(ExecutionFault::Internal(
                                "zirk_str_set replacement is not a String or Char".into(),
                            ));
                        }
                    },
                    _ => {
                        return Err(ExecutionFault::Internal(
                            "zirk_str_set replacement is missing".into(),
                        ));
                    }
                };
                // An out-of-range write is a no-op, matching the native
                // `zirk_str_set` boundary.
                if let Some(out) =
                    zirk_runtime_core::text::replace_range(&text, offset, length, &replacement)
                {
                    let Value::Heap(id) = receiver else {
                        return Err(ExecutionFault::Internal(
                            "zirk_str_set receiver is not a heap value".into(),
                        ));
                    };
                    match self.heap.get_mut(*id) {
                        Some(HeapEntry::String(slot)) => *slot = out,
                        _ => {
                            return Err(ExecutionFault::Internal(
                                "zirk_str_set receiver is not a String".into(),
                            ));
                        }
                    }
                }
                Ok(Value::Void)
            }
            "zirk_str_slice" => {
                let start = self.integer_from_value(arguments.get(1))?;
                let end = self.integer_from_value(arguments.get(2))?;
                let step = self.integer_from_value(arguments.get(3))?;
                self.allocate_string(zirk_runtime_core::text::grapheme_slice(
                    &text, start, end, step,
                ))
                .map(Value::Heap)
            }
            "zirk_str_split" => self.string_list_result(
                zirk_runtime_core::text::split(&text, &argument_string(1)?)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            ),
            "zirk_str_split_whitespace" => {
                self.string_list_result(text.split_whitespace().map(str::to_owned).collect())
            }
            "zirk_str_lines" => self.string_list_result(text.lines().map(str::to_owned).collect()),
            "zirk_str_chars" => {
                let mut values = Vec::new();
                for grapheme in zirk_runtime_core::text::graphemes(&text) {
                    values.push(Value::Heap(self.allocate_char(grapheme.to_owned())?));
                }
                self.allocate_value_list(values)
            }
            "zirk_str_bytes" => {
                let values = text
                    .bytes()
                    .map(|byte| Value::Int(IntegerValue::from_bits(u128::from(byte), IntWidth::U8)))
                    .collect();
                self.allocate_value_list(values)
            }
            "zirk_str_codepoints" => {
                let values = text
                    .chars()
                    .map(|point| {
                        Value::Int(IntegerValue::from_bits(
                            u128::from(point as u32),
                            IntWidth::U32,
                        ))
                    })
                    .collect();
                self.allocate_value_list(values)
            }
            _ => Err(ExecutionFault::Internal(format!(
                "unsupported browser intrinsic {name}"
            ))),
        }
    }

    /// Deep `clone()` over the interpreter heap (`zirk-memory-safety`, "Deep
    /// clone graph semantics").
    ///
    /// Every reference object reachable from `root` receives a new identity;
    /// a child referenced twice is cloned once, so sharing inside the graph is
    /// preserved, and a field that reaches back to an ancestor already being
    /// cloned resolves through the same map, so cycles terminate and mirror
    /// the source. Strings, chars, regexes, ranges and runtime errors are
    /// immutable and have no reference fields, so they are shared exactly as
    /// the native runtime shares string handles. The traversal is an explicit
    /// worklist rather than recursion, so a long chain cannot overflow the
    /// host stack inside a worker.
    ///
    /// Work is charged before each allocation: every cloned node costs one
    /// step plus one per field against the instruction budget, and each
    /// allocation goes through the weighted heap accounting. Either limit ends
    /// execution with the usual limit failure; the source graph is never
    /// written, and an abandoned partial clone is unreachable.
    fn deep_clone(&mut self, root: Value) -> Result<Value, ExecutionFault> {
        let mut cloned: HashMap<HeapId, HeapId> = HashMap::new();
        let mut pending: Vec<HeapId> = Vec::new();
        let result = self.clone_value(root, &mut cloned, &mut pending)?;
        while let Some(destination) = pending.pop() {
            let patched = match self.heap.get(destination).cloned() {
                Some(HeapEntry::Object { layout, fields }) => HeapEntry::Object {
                    layout,
                    fields: self.clone_values(fields, &mut cloned, &mut pending)?,
                },
                Some(HeapEntry::Record { layout, fields }) => HeapEntry::Record {
                    layout,
                    fields: self.clone_values(fields, &mut cloned, &mut pending)?,
                },
                Some(HeapEntry::Enum {
                    layout,
                    variant,
                    fields,
                }) => HeapEntry::Enum {
                    layout,
                    variant,
                    fields: self.clone_values(fields, &mut cloned, &mut pending)?,
                },
                Some(HeapEntry::Closure { target, captures }) => HeapEntry::Closure {
                    target,
                    captures: self.clone_values(captures, &mut cloned, &mut pending)?,
                },
                Some(HeapEntry::Array(values)) => {
                    HeapEntry::Array(self.clone_values(values, &mut cloned, &mut pending)?)
                }
                Some(HeapEntry::List(values)) => {
                    HeapEntry::List(self.clone_values(values, &mut cloned, &mut pending)?)
                }
                Some(HeapEntry::Set(values)) => {
                    HeapEntry::Set(self.clone_values(values, &mut cloned, &mut pending)?)
                }
                Some(HeapEntry::Map(entries)) => {
                    let mut patched = Vec::with_capacity(entries.len());
                    for (key, value) in entries {
                        let key = self.clone_value(key, &mut cloned, &mut pending)?;
                        let value = self.clone_value(value, &mut cloned, &mut pending)?;
                        patched.push((key, value));
                    }
                    HeapEntry::Map(patched)
                }
                _ => {
                    return Err(ExecutionFault::Internal(
                        "clone worklist holds a non-cloneable entry".into(),
                    ));
                }
            };
            *self
                .heap
                .get_mut(destination)
                .ok_or_else(|| ExecutionFault::Internal("clone destination vanished".into()))? =
                patched;
        }
        Ok(result)
    }

    fn clone_values(
        &mut self,
        values: Vec<Value>,
        cloned: &mut HashMap<HeapId, HeapId>,
        pending: &mut Vec<HeapId>,
    ) -> Result<Vec<Value>, ExecutionFault> {
        values
            .into_iter()
            .map(|value| self.clone_value(value, cloned, pending))
            .collect()
    }

    /// Maps one value into the clone: scalars copy, shared immutable heap
    /// values keep their identity, and every other heap value is allocated
    /// once (a shallow copy whose references the worklist rewrites later).
    fn clone_value(
        &mut self,
        value: Value,
        cloned: &mut HashMap<HeapId, HeapId>,
        pending: &mut Vec<HeapId>,
    ) -> Result<Value, ExecutionFault> {
        match value {
            Value::Nullable(Some(inner)) => Ok(Value::Nullable(Some(Box::new(
                self.clone_value(*inner, cloned, pending)?,
            )))),
            Value::Heap(source) => {
                if let Some(existing) = cloned.get(&source) {
                    return Ok(Value::Heap(*existing));
                }
                let Some(entry) = self.heap.get(source) else {
                    return Err(ExecutionFault::Internal(
                        "clone source is not a live heap value".into(),
                    ));
                };
                let (shallow, fields) = match entry {
                    HeapEntry::String(_)
                    | HeapEntry::Char(_)
                    | HeapEntry::Regex(_)
                    | HeapEntry::Range(_)
                    | HeapEntry::RuntimeError { .. } => return Ok(Value::Heap(source)),
                    HeapEntry::Object { fields, .. }
                    | HeapEntry::Record { fields, .. }
                    | HeapEntry::Enum { fields, .. } => (entry.clone(), fields.len()),
                    HeapEntry::Closure { captures, .. } => (entry.clone(), captures.len()),
                    HeapEntry::Array(values) | HeapEntry::List(values) | HeapEntry::Set(values) => {
                        (entry.clone(), values.len())
                    }
                    HeapEntry::Map(entries) => (entry.clone(), entries.len().saturating_mul(2)),
                };
                // Charge the node's work before it is allocated.
                let cost = 1_u64.saturating_add(fields as u64);
                self.executed_steps = self.executed_steps.saturating_add(cost);
                if self.executed_steps > self.limits.instructions {
                    return Err(ExecutionFault::Limit(LimitFailure::Instructions));
                }
                let bytes = std::mem::size_of::<HeapEntry>()
                    .checked_add(fields.saturating_mul(std::mem::size_of::<Value>()))
                    .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
                let destination = self
                    .heap
                    .allocate(shallow, bytes, self.limits)
                    .map_err(ExecutionFault::Limit)?;
                cloned.insert(source, destination);
                pending.push(destination);
                Ok(Value::Heap(destination))
            }
            other => Ok(other),
        }
    }

    fn trace_object_id(&mut self, id: HeapId) -> Option<u32> {
        let state = self.trace.as_mut()?;
        let next = state.object_ids.len() as u32 + 1;
        Some(*state.object_ids.entry(id).or_insert(next))
    }

    /// SecretString's private payload must not appear in an execution trace.
    /// Check the ancestor layouts too, because a derived class retains the
    /// base field's index and can otherwise expose it through FieldWrite.
    fn is_secret_trace_field(&self, layout: u32, field_index: usize) -> bool {
        self.module
            .objects
            .get(layout as usize)
            .is_some_and(|object| {
                object.ancestors.iter().any(|ancestor| {
                    self.module
                        .objects
                        .get(*ancestor as usize)
                        .is_some_and(|base| {
                            base.name == "SecretString"
                                && base
                                    .fields
                                    .get(field_index)
                                    .is_some_and(|field| field.name == "value")
                        })
                })
            })
    }

    fn trace_record(&mut self, kind: TraceEventKind) {
        let Some(state) = self.trace.as_mut() else {
            return;
        };
        if state.trace.truncated {
            return;
        }
        let weight = 24
            + match &kind {
                TraceEventKind::Alloc { class, .. } => class.len(),
                TraceEventKind::FieldWrite {
                    field, ty, value, ..
                } => field.len() + ty.len() + value.len(),
                TraceEventKind::Call { callee, .. } | TraceEventKind::Return { callee } => {
                    callee.len()
                }
                _ => 0,
            };
        if state.trace.events.len() >= state.limits.max_events
            || state.bytes + weight > state.limits.max_bytes
        {
            state.trace.truncated = true;
            return;
        }
        state.bytes += weight;
        let seq = state.next_seq;
        state.next_seq += 1;
        state.trace.events.push(TraceEvent { seq, kind });
    }

    /// Renders a value for the trace: scalars verbatim, strings quoted,
    /// objects as a reference to their trace id, never their contents.
    fn trace_render(&mut self, value: &Value) -> (String, bool) {
        let limit = self
            .trace
            .as_ref()
            .map_or(0, |state| state.limits.max_value_bytes);
        let text = match value {
            Value::Int(int) => {
                if int.width.signed() {
                    int.signed().to_string()
                } else {
                    int.unsigned().to_string()
                }
            }
            Value::Float { text, .. } | Value::Decimal(text) => text.clone(),
            Value::Boolean(flag) => flag.to_string(),
            Value::Nullable(None) => "null".into(),
            Value::Nullable(Some(inner)) => return self.trace_render(inner),
            Value::Void | Value::Never | Value::Undefined => "undefined".into(),
            Value::Heap(id) => match self.heap.get(*id) {
                Some(HeapEntry::String(text)) | Some(HeapEntry::Char(text)) => format!("{text:?}"),
                Some(HeapEntry::Object { .. }) => match self.trace_object_id(*id) {
                    Some(object) => format!("#{object}"),
                    None => String::new(),
                },
                Some(HeapEntry::Record { .. }) => "<record>".into(),
                Some(HeapEntry::Enum { .. }) => "<enum>".into(),
                Some(HeapEntry::List(_)) => "<list>".into(),
                Some(HeapEntry::Array(_)) => "<array>".into(),
                Some(HeapEntry::Map(_)) => "<map>".into(),
                Some(HeapEntry::Set(_)) => "<set>".into(),
                Some(HeapEntry::Closure { .. }) => "<closure>".into(),
                _ => "<value>".into(),
            },
        };
        if text.len() <= limit {
            return (text, false);
        }
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        (text[..end].to_string(), true)
    }

    fn heap_operand(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<HeapId, ExecutionFault> {
        match self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        {
            Value::Heap(id) => Ok(id),
            _ => Err(ExecutionFault::Internal(
                "collection receiver is not a heap value".into(),
            )),
        }
    }

    fn collection_length_value(&self, length: usize, ty: IrType) -> Result<Value, ExecutionFault> {
        let IrType::Int(width) = ty else {
            return Err(ExecutionFault::Internal(
                "collection length has a non-integer result".into(),
            ));
        };
        let length = i128::try_from(length).map_err(|_| ExecutionFault::Runtime {
            category: "LimitError".into(),
            message: "collection length exceeds the interpreter range".into(),
        })?;
        IntegerValue::from_literal(length, width)
            .map(Value::Int)
            .map_err(ExecutionFault::Internal)
    }

    fn collection_bounds_error() -> ExecutionFault {
        ExecutionFault::Runtime {
            category: "BoundsError".into(),
            message: "collection index is out of bounds".into(),
        }
    }

    /// The zero-initialized element an `Array<T>(n)` allocation starts with,
    /// matching the native collector's defaults (`T[n]` lowers to it).
    fn default_value(&mut self, ty: IrType) -> Result<Value, ExecutionFault> {
        match ty {
            IrType::Int(width) => IntegerValue::from_literal(0, width)
                .map(Value::Int)
                .map_err(ExecutionFault::Internal),
            IrType::Float(FloatWidth::F128) => Ok(Value::Float {
                width: FloatWidth::F128,
                text: zirk_runtime_core::binary_float::Float128::from_decimal("0")
                    .map_err(|error| {
                        ExecutionFault::Internal(format!("Float128 zero failed: {error:?}"))
                    })?
                    .wire_text(),
            }),
            IrType::Float(width) => Ok(Value::Float {
                width,
                text: "0".to_string(),
            }),
            IrType::Decimal => Ok(Value::Decimal("0".to_string())),
            IrType::Boolean => Ok(Value::Boolean(false)),
            IrType::String => self.allocate_string(String::new()).map(Value::Heap),
            IrType::Nullable(_) => Ok(Value::Nullable(None)),
            // Collections default to empty ("empty where meaningful").
            IrType::Array(_) => self
                .heap
                .allocate(
                    HeapEntry::Array(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(Value::Heap)
                .map_err(ExecutionFault::Limit),
            IrType::List(_) => self
                .heap
                .allocate(
                    HeapEntry::List(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(Value::Heap)
                .map_err(ExecutionFault::Limit),
            IrType::Map(_) => self
                .heap
                .allocate(
                    HeapEntry::Map(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(Value::Heap)
                .map_err(ExecutionFault::Limit),
            IrType::Set(_) => self
                .heap
                .allocate(
                    HeapEntry::Set(Vec::new()),
                    std::mem::size_of::<HeapEntry>(),
                    self.limits,
                )
                .map(Value::Heap)
                .map_err(ExecutionFault::Limit),
            // `Duration` and the civil temporals are plain integers at this
            // level (`Int64` nanos/days), covered by `Int` above. `Char`,
            // aggregates and managed references have no implicit default —
            // the checker requires an explicit initializer for those.
            _ => Err(ExecutionFault::Internal(format!(
                "ArrayNew element type {ty:?} has no default value"
            ))),
        }
    }

    fn render_value(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<String, ExecutionFault> {
        self.render_owned_value(
            self.value(frame_index, operand)
                .map_err(ExecutionFault::Internal)?,
        )
    }

    fn render_owned_value(&self, value: Value) -> Result<String, ExecutionFault> {
        match value {
            Value::Int(value) if value.width.signed() => Ok(value.signed().to_string()),
            Value::Int(value) => Ok(value.unsigned().to_string()),
            Value::Float {
                width: FloatWidth::F128,
                text,
            } => zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                .map(|value| value.display_f64().to_string())
                .map_err(|error| {
                    ExecutionFault::Internal(format!("cannot render Float128: {error:?}"))
                }),
            Value::Float { text, .. } | Value::Decimal(text) => Ok(text),
            Value::Boolean(value) => Ok(value.to_string()),
            Value::Nullable(None) => Ok("null".into()),
            Value::Nullable(Some(value)) => self.render_owned_value(*value),
            Value::Heap(id) => match self.heap.get(id) {
                Some(HeapEntry::String(text)) | Some(HeapEntry::Char(text)) => Ok(text.clone()),
                _ => Err(ExecutionFault::Internal(
                    "ToString has no rendering for this value".into(),
                )),
            },
            value => Err(ExecutionFault::Internal(format!(
                "ToString is not defined for {value:?}"
            ))),
        }
    }

    fn ensure_string_allocation_fits(&self, text_bytes: usize) -> Result<(), ExecutionFault> {
        let bytes = text_bytes
            .checked_add(std::mem::size_of::<String>())
            .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
        self.heap
            .used_bytes()
            .checked_add(bytes)
            .filter(|total| *total <= self.limits.heap_bytes)
            .ok_or(ExecutionFault::Limit(LimitFailure::Heap))?;
        Ok(())
    }

    fn allocate_string(&mut self, text: String) -> Result<HeapId, ExecutionFault> {
        self.ensure_string_allocation_fits(text.len())?;
        let bytes = text.len() + std::mem::size_of::<String>();
        self.heap
            .allocate(HeapEntry::String(text), bytes, self.limits)
            .map_err(ExecutionFault::Limit)
    }

    fn allocate_char(&mut self, text: String) -> Result<HeapId, ExecutionFault> {
        self.ensure_string_allocation_fits(text.len())?;
        let bytes = text.len() + std::mem::size_of::<String>();
        self.heap
            .allocate(HeapEntry::Char(text), bytes, self.limits)
            .map_err(ExecutionFault::Limit)
    }

    /// Allocates `bytes` as an `Array<Byte>` heap entry
    /// (`add-stdin-stdout-standard-library`'s `stdin.read_bytes`/
    /// `read_exact`).
    fn allocate_byte_array(&mut self, bytes: Vec<u8>) -> Result<Value, ExecutionFault> {
        let values: Vec<Value> = bytes
            .into_iter()
            .map(|byte| Value::Int(IntegerValue::from_bits(u128::from(byte), IntWidth::U8)))
            .collect();
        let heap_bytes =
            std::mem::size_of::<HeapEntry>() + values.len() * std::mem::size_of::<Value>();
        self.heap
            .allocate(HeapEntry::Array(values), heap_bytes, self.limits)
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// Reads a `UInt64` operand's value (`stdin.read_bytes(max)`/
    /// `read_exact(count)`'s own argument).
    fn uint_operand(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<u64, ExecutionFault> {
        match self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        {
            Value::Int(value) => Ok(value.unsigned() as u64),
            other => Err(ExecutionFault::Internal(format!(
                "expected a UInt64 operand, found {other:?}"
            ))),
        }
    }

    /// Builds `Ok(Void)` for `stdout.flush`/`stderr.flush`
    /// (`add-stdin-stdout-standard-library`, design D3): the interpreter's
    /// own output sink is in-memory, so flushing it never fails.
    fn build_flush_ok(&mut self, enum_id: u32) -> Result<Value, ExecutionFault> {
        let bytes = std::mem::size_of::<HeapEntry>() + std::mem::size_of::<Value>();
        self.heap
            .allocate(
                HeapEntry::Enum {
                    layout: enum_id,
                    variant: 0,
                    fields: vec![Value::Void],
                },
                bytes,
                self.limits,
            )
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// Builds a no-payload enum value with `discriminant` selecting its variant.
    fn build_empty_variant_enum(
        &mut self,
        io_error_enum_id: u32,
        discriminant: u32,
    ) -> Result<Value, ExecutionFault> {
        let bytes = std::mem::size_of::<HeapEntry>();
        self.heap
            .allocate(
                HeapEntry::Enum {
                    layout: io_error_enum_id,
                    variant: discriminant,
                    fields: Vec::new(),
                },
                bytes,
                self.limits,
            )
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// Builds a two-variant `Result`-shaped value. `kind` is the outer
    /// discriminant, `error` is read only for the error variant, and `payload`
    /// is the already-allocated success value. Fieldless enums are
    /// integer-backed; payload enums retain their tagged layout.
    fn build_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: u32,
        error: i32,
        payload: Option<Value>,
        value_variant: u32,
        error_variant: u32,
    ) -> Result<Value, ExecutionFault> {
        let layout = self
            .module
            .enums
            .get(enum_id as usize)
            .ok_or_else(|| {
                ExecutionFault::Internal(format!(
                    "a stdin read references missing enum layout {enum_id}"
                ))
            })?
            .clone();
        let fields = if kind == value_variant {
            vec![payload.ok_or_else(|| {
                ExecutionFault::Internal("a Value/Ok outcome carries no payload".into())
            })?]
        } else if kind == error_variant {
            let error_field_index = *layout
                .variants
                .get(error_variant as usize)
                .and_then(|fields| fields.first())
                .ok_or_else(|| {
                    ExecutionFault::Internal("the Error variant carries no field".into())
                })?;
            let error_ty = layout.fields[error_field_index as usize].ty;
            let error_value = match error_ty {
                IrType::Enum(error_enum_id) => {
                    self.build_empty_variant_enum(error_enum_id, error as u32)?
                }
                IrType::Int(width) => Value::Int(
                    IntegerValue::from_literal(i128::from(error), width)
                        .map_err(ExecutionFault::Internal)?,
                ),
                other => {
                    return Err(ExecutionFault::Internal(format!(
                        "the Error variant carries unsupported type {other:?}"
                    )));
                }
            };
            vec![error_value]
        } else {
            Vec::new()
        };
        let bytes = std::mem::size_of::<HeapEntry>() + fields.len() * std::mem::size_of::<Value>();
        self.heap
            .allocate(
                HeapEntry::Enum {
                    layout: enum_id,
                    variant: kind,
                    fields,
                },
                bytes,
                self.limits,
            )
            .map(Value::Heap)
            .map_err(ExecutionFault::Limit)
    }

    /// `read_line_result`/`read_char`'s shared outcome shape: `as_char`
    /// selects `Char` vs `String` payload allocation, since both share the
    /// same `zirk_runtime_core::stdin::TextOutcome`.
    fn build_text_outcome(
        &mut self,
        enum_id: u32,
        outcome: zirk_runtime_core::stdin::TextOutcome,
        as_char: bool,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::stdin::TextOutcome;
        match outcome {
            TextOutcome::Value(text) => {
                let id = if as_char {
                    self.allocate_char(text)?
                } else {
                    self.allocate_string(text)?
                };
                self.build_tagged_outcome(enum_id, 0, 0, Some(Value::Heap(id)), 0, 2)
            }
            TextOutcome::End => self.build_tagged_outcome(enum_id, 1, 0, None, 0, 2),
            TextOutcome::Error(code) => self.build_tagged_outcome(enum_id, 2, code, None, 0, 2),
        }
    }

    /// `read_bytes`'s outcome shape.
    fn build_bytes_outcome(
        &mut self,
        enum_id: u32,
        outcome: zirk_runtime_core::stdin::BytesOutcome,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::stdin::BytesOutcome;
        match outcome {
            BytesOutcome::Value(bytes) => {
                let payload = self.allocate_byte_array(bytes)?;
                self.build_tagged_outcome(enum_id, 0, 0, Some(payload), 0, 2)
            }
            BytesOutcome::End => self.build_tagged_outcome(enum_id, 1, 0, None, 0, 2),
            BytesOutcome::Error(code) => self.build_tagged_outcome(enum_id, 2, code, None, 0, 2),
        }
    }

    /// `read_exact`'s outcome shape (`Result<Array<Byte>, IoError>`: `Ok =
    /// 0`, `Error = 1`, per `register_native_result_enum`).
    fn build_exact_outcome(
        &mut self,
        enum_id: u32,
        outcome: zirk_runtime_core::stdin::ExactOutcome,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::stdin::ExactOutcome;
        match outcome {
            ExactOutcome::Ok(bytes) => {
                let payload = self.allocate_byte_array(bytes)?;
                self.build_tagged_outcome(enum_id, 0, 0, Some(payload), 0, 1)
            }
            ExactOutcome::Err(code) => self.build_tagged_outcome(enum_id, 1, code, None, 0, 1),
        }
    }

    fn value(&self, frame_index: usize, operand: zirk_ir::Operand) -> Result<Value, String> {
        self.frames[frame_index]
            .values
            .get(&operand.0)
            .cloned()
            .ok_or_else(|| format!("operand {operand:?} is unavailable"))
    }

    fn object_layout(
        &self,
        frame_index: usize,
        operand: zirk_ir::Operand,
    ) -> Result<u32, ExecutionFault> {
        let Value::Heap(id) = self
            .value(frame_index, operand)
            .map_err(ExecutionFault::Internal)?
        else {
            return Err(ExecutionFault::Internal(
                "type test reads a non-object value".into(),
            ));
        };
        match self.heap.get(id) {
            Some(HeapEntry::Object { layout, .. }) => Ok(*layout),
            _ => Err(ExecutionFault::Internal(
                "type test reads a heap value that is not an object".into(),
            )),
        }
    }

    fn execute_unary(&self, op: UnaryOp, value: Value) -> Result<Value, String> {
        match (op, value) {
            (UnaryOp::Neg, Value::Int(value)) if value.width.signed() => {
                let result = value.signed().checked_neg().ok_or("integer Neg overflow")?;
                let maximum = if value.width.bits() == 128 {
                    i128::MAX
                } else {
                    (1_i128 << (value.width.bits() - 1)) - 1
                };
                if result > maximum {
                    return Err("integer Neg overflow".into());
                }
                Ok(Value::Int(IntegerValue::from_bits(
                    result as u128,
                    value.width,
                )))
            }
            (UnaryOp::Neg, Value::Int(_)) => Err("integer Neg requires a signed width".into()),
            (UnaryOp::Neg, Value::Float { width, text }) => {
                if width == FloatWidth::F128 {
                    return zirk_runtime_core::binary_float::Float128::from_wire_or_decimal(&text)
                        .map(|value| Value::Float {
                            width,
                            text: value.neg().wire_text(),
                        })
                        .map_err(|error| format!("invalid Float128 value: {error:?}"));
                }
                // Float16/128 do not have stable Rust primitives. Negation is
                // nevertheless exact at the IR literal boundary: preserve the
                // original decimal spelling and change only its sign.
                if text.parse::<f64>().is_err() {
                    return Err(format!("invalid {width:?} float literal {text:?}"));
                }
                let text = match text.strip_prefix('-') {
                    Some(magnitude) => magnitude.to_owned(),
                    None => format!("-{text}"),
                };
                Ok(Value::Float { width, text })
            }
            (UnaryOp::Neg, Value::Decimal(text)) => {
                let value = self.parse_decimal(&text).map_err(|error| match error {
                    ExecutionFault::Runtime { message, .. } => message,
                    ExecutionFault::Internal(message) => message,
                    ExecutionFault::Limit(limit) => format!("decimal limit failure: {limit:?}"),
                })?;
                let value = zirk_runtime_core::decimal::negate(value)
                    .map_err(|error| format!("decimal negation failed: {error:?}"))?;
                Ok(Value::Decimal(zirk_runtime_core::decimal::format(value)))
            }
            (UnaryOp::Not, Value::Boolean(value)) => Ok(Value::Boolean(!value)),
            (UnaryOp::BitNot, Value::Int(value)) => Ok(Value::Int(IntegerValue::from_bits(
                !value.bits,
                value.width,
            ))),
            (_, value) => Err(format!("unary {op:?} is not defined for {value:?}")),
        }
    }

    /// Plain `Binary` failures carry `ArithmeticError` — the category every
    /// checked operator reports through. Typed error categories
    /// (`FloatNanError`, `DivisionByZeroError`) are produced by the typed
    /// paths themselves.
    fn arithmetic_error(message: impl Into<String>) -> ExecutionFault {
        ExecutionFault::Runtime {
            category: "ArithmeticError".into(),
            message: message.into(),
        }
    }

    fn execute_binary(
        &self,
        op: BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value, ExecutionFault> {
        match (left, right) {
            (Value::Heap(left), Value::Heap(right)) => match op {
                BinaryOp::Eq => Ok(Value::Boolean(self.value_equals(left, right))),
                BinaryOp::NotEq => Ok(Value::Boolean(!self.value_equals(left, right))),
                BinaryOp::Identical => Ok(Value::Boolean(self.identity_equals(left, right))),
                _ => Err(Self::arithmetic_error(format!(
                    "operator {op:?} is not defined for heap values"
                ))),
            },
            // `is` is the one comparison allowed on nullable references: two
            // absent values are identical, absent vs present never is, two
            // present ones compare by identity like the non-nullable case.
            (Value::Nullable(left), Value::Nullable(right)) if op == BinaryOp::Identical => {
                let identical = match (left.as_deref(), right.as_deref()) {
                    (None, None) => true,
                    (Some(Value::Heap(left)), Some(Value::Heap(right))) => {
                        self.identity_equals(*left, *right)
                    }
                    (Some(left), Some(right)) => left == right,
                    _ => false,
                };
                Ok(Value::Boolean(identical))
            }
            (Value::Nullable(inner), Value::Heap(right)) if op == BinaryOp::Identical => {
                Ok(Value::Boolean(matches!(
                    inner.as_deref(),
                    Some(Value::Heap(left)) if self.identity_equals(*left, right)
                )))
            }
            (Value::Heap(left), Value::Nullable(inner)) if op == BinaryOp::Identical => {
                Ok(Value::Boolean(matches!(
                    inner.as_deref(),
                    Some(Value::Heap(right)) if self.identity_equals(left, *right)
                )))
            }
            (Value::Int(left), Value::Int(right)) => left
                .checked_binary(op, right)
                .map_err(Self::arithmetic_error),
            (
                Value::Float {
                    width: left_width,
                    text: left,
                },
                Value::Float {
                    width: right_width,
                    text: right,
                },
            ) if left_width == right_width => {
                self.execute_float_binary(op, left_width, &left, &right)
            }
            (Value::Decimal(left), Value::Decimal(right)) => {
                self.execute_decimal_binary(op, &left, &right)
            }
            (Value::Boolean(left), Value::Boolean(right)) => match op {
                BinaryOp::Eq => Ok(Value::Boolean(left == right)),
                BinaryOp::NotEq => Ok(Value::Boolean(left != right)),
                BinaryOp::And => Ok(Value::Boolean(left && right)),
                BinaryOp::Or => Ok(Value::Boolean(left || right)),
                BinaryOp::Identical => Ok(Value::Boolean(left == right)),
                _ => Err(Self::arithmetic_error(format!(
                    "operator {op:?} is not defined for Boolean"
                ))),
            },
            (left, right) => Err(Self::arithmetic_error(format!(
                "binary {op:?} has incompatible operands {left:?} and {right:?}"
            ))),
        }
    }

    /// Value records and algebraic enums are compared by their visible
    /// fields. Objects retain reference identity: giving records a heap slot
    /// is only an arena implementation detail, never a language observable.
    fn value_equals(&self, left: HeapId, right: HeapId) -> bool {
        if left == right {
            return true;
        }
        match (self.heap.get(left), self.heap.get(right)) {
            (Some(HeapEntry::String(left)), Some(HeapEntry::String(right))) => {
                zirk_runtime_core::text::equal(left, right)
            }
            (Some(HeapEntry::Char(left)), Some(HeapEntry::Char(right)))
            | (Some(HeapEntry::Regex(left)), Some(HeapEntry::Regex(right))) => left == right,
            (
                Some(HeapEntry::Record {
                    layout: left_layout,
                    fields: left_fields,
                }),
                Some(HeapEntry::Record {
                    layout: right_layout,
                    fields: right_fields,
                }),
            ) => {
                left_layout == right_layout
                    && left_fields.len() == right_fields.len()
                    && left_fields
                        .iter()
                        .zip(right_fields)
                        .all(|(left, right)| self.runtime_value_equals(left, right))
            }
            (
                Some(HeapEntry::Enum {
                    layout: left_layout,
                    variant: left_variant,
                    fields: left_fields,
                }),
                Some(HeapEntry::Enum {
                    layout: right_layout,
                    variant: right_variant,
                    fields: right_fields,
                }),
            ) => {
                left_layout == right_layout
                    && left_variant == right_variant
                    && left_fields.len() == right_fields.len()
                    && left_fields
                        .iter()
                        .zip(right_fields)
                        .all(|(left, right)| self.runtime_value_equals(left, right))
            }
            _ => false,
        }
    }

    /// `is` — reference identity. Two bindings to the same allocation compare
    /// equal; a capture-less callable compares by its function target because
    /// every same-signature non-capturing callable denotes one canonical
    /// function value (`module.closures` D12), not one instance per mention.
    fn identity_equals(&self, left: HeapId, right: HeapId) -> bool {
        if left == right {
            return true;
        }
        match (self.heap.get(left), self.heap.get(right)) {
            (
                Some(HeapEntry::Closure {
                    target: left_target,
                    captures: left_captures,
                }),
                Some(HeapEntry::Closure {
                    target: right_target,
                    captures: right_captures,
                }),
            ) => {
                left_captures.is_empty() && right_captures.is_empty() && left_target == right_target
            }
            _ => false,
        }
    }

    fn runtime_value_equals(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Heap(left), Value::Heap(right)) => self.value_equals(*left, *right),
            _ => left == right,
        }
    }

    /// Parses the decimal spelling a `Value::Float` carries. Infinities and
    /// NaN are valid here — they are produced and consumed like any other
    /// `f64`, and the lowering's own `guard_nan` turns a `NaN` result into a
    /// catchable `FloatNanError`. Only `Float128` is finite-only, and it
    /// never reaches this parser.
    fn parse_binary_float(text: &str) -> Result<f64, ExecutionFault> {
        text.parse::<f64>()
            .map_err(|_| ExecutionFault::Internal(format!("invalid binary float literal {text:?}")))
    }

    fn round_binary_float(value: f64, width: FloatWidth) -> f64 {
        match width {
            FloatWidth::F16 => zirk_runtime_core::scalar::round_to_float16(value),
            FloatWidth::F32 => value as f32 as f64,
            FloatWidth::F64 | FloatWidth::F128 => value,
        }
    }

    fn format_binary_float(value: f64, width: FloatWidth) -> String {
        match width {
            FloatWidth::F16 | FloatWidth::F32 => (value as f32).to_string(),
            FloatWidth::F64 | FloatWidth::F128 => value.to_string(),
        }
    }

    /// Builds a runtime `Value::Float` from an `f64` result. Infinities and
    /// NaN carry through as text exactly like native — the lowering's
    /// `guard_nan` raises `FloatNanError` for NaN, and `inf` is a valid value.
    fn binary_float_value(value: f64, width: FloatWidth) -> Result<Value, ExecutionFault> {
        Ok(Value::Float {
            width,
            text: Self::format_binary_float(Self::round_binary_float(value, width), width),
        })
    }

    fn execute_float_binary(
        &self,
        op: BinaryOp,
        width: FloatWidth,
        left: &str,
        right: &str,
    ) -> Result<Value, ExecutionFault> {
        if width == FloatWidth::F128 {
            use zirk_runtime_core::binary_float::Float128;
            let invalid =
                |error| Self::arithmetic_error(format!("invalid Float128 value: {error:?}"));
            let left = Float128::from_wire_or_decimal(left).map_err(invalid)?;
            let right = Float128::from_wire_or_decimal(right).map_err(invalid)?;
            use BinaryOp::*;
            let comparison = left.compare(right).map_err(|error| {
                Self::arithmetic_error(format!("Float128 comparison failed: {error:?}"))
            });
            match op {
                Eq => {
                    return comparison
                        .map(|value| Value::Boolean(value == std::cmp::Ordering::Equal));
                }
                NotEq => {
                    return comparison
                        .map(|value| Value::Boolean(value != std::cmp::Ordering::Equal));
                }
                Lt => {
                    return comparison
                        .map(|value| Value::Boolean(value == std::cmp::Ordering::Less));
                }
                LtEq => {
                    return comparison
                        .map(|value| Value::Boolean(value != std::cmp::Ordering::Greater));
                }
                Gt => {
                    return comparison
                        .map(|value| Value::Boolean(value == std::cmp::Ordering::Greater));
                }
                GtEq => {
                    return comparison
                        .map(|value| Value::Boolean(value != std::cmp::Ordering::Less));
                }
                _ => {}
            }
            return match op {
                Add => left.add(right),
                Sub => left.sub(right),
                Mul => left.mul(right),
                Div => left.div(right),
                Rem => left.rem(right),
                _ => {
                    return Err(Self::arithmetic_error(format!(
                        "operator {op:?} is not defined for binary floats"
                    )));
                }
            }
            .map(|value| Value::Float {
                width,
                text: value.wire_text(),
            })
            .map_err(|error| {
                Self::arithmetic_error(format!("Float128 operation failed: {error:?}"))
            });
        }
        let left = Self::parse_binary_float(left)?;
        let right = Self::parse_binary_float(right)?;
        use BinaryOp::*;
        let comparison = |value| Ok(Value::Boolean(value));
        match op {
            Eq => comparison(left == right),
            NotEq => comparison(left != right),
            Lt => comparison(left < right),
            LtEq => comparison(left <= right),
            Gt => comparison(left > right),
            GtEq => comparison(left >= right),
            Add | Sub | Mul | Div | Rem => {
                let value = match op {
                    Add => left + right,
                    Sub => left - right,
                    Mul => left * right,
                    Div => left / right,
                    Rem => left % right,
                    _ => unreachable!(),
                };
                // Produce the IEEE 754 result verbatim — `inf` is a valid
                // value and a `NaN` is detected by the lowering's `guard_nan`
                // (`x == x`), which throws a catchable `FloatNanError`.
                let value = Self::round_binary_float(value, width);
                Ok(Value::Float {
                    width,
                    text: Self::format_binary_float(value, width),
                })
            }
            _ => Err(Self::arithmetic_error(format!(
                "operator {op:?} is not defined for binary floats"
            ))),
        }
    }

    fn parse_decimal(
        &self,
        text: &str,
    ) -> Result<zirk_runtime_core::decimal::Decimal, ExecutionFault> {
        zirk_runtime_core::decimal::parse(text).ok_or_else(|| {
            ExecutionFault::Internal(format!("invalid exact decimal literal {text:?}"))
        })
    }

    fn execute_decimal_binary(
        &self,
        op: BinaryOp,
        left: &str,
        right: &str,
    ) -> Result<Value, ExecutionFault> {
        use zirk_runtime_core::decimal;
        let left = self.parse_decimal(left)?;
        let right = self.parse_decimal(right)?;
        let comparison = || decimal::compare(left, right).map_err(Self::decimal_fault);
        let value = match op {
            BinaryOp::Eq => return comparison().map(|value| Value::Boolean(value == 0)),
            BinaryOp::NotEq => return comparison().map(|value| Value::Boolean(value != 0)),
            BinaryOp::Lt => return comparison().map(|value| Value::Boolean(value < 0)),
            BinaryOp::LtEq => return comparison().map(|value| Value::Boolean(value <= 0)),
            BinaryOp::Gt => return comparison().map(|value| Value::Boolean(value > 0)),
            BinaryOp::GtEq => return comparison().map(|value| Value::Boolean(value >= 0)),
            BinaryOp::Add => decimal::add(left, right),
            BinaryOp::Sub => decimal::sub(left, right),
            BinaryOp::Mul => decimal::mul(left, right),
            BinaryOp::Div => decimal::div(left, right),
            BinaryOp::Rem => decimal::rem(left, right),
            _ => {
                return Err(Self::arithmetic_error(format!(
                    "operator {op:?} is not defined for Decimal"
                )));
            }
        }
        .map_err(Self::decimal_fault)?;
        Ok(Value::Decimal(decimal::format(value)))
    }

    fn internal(&self, message: impl Into<String>, span: Option<Span>) -> ExecutionResult {
        ExecutionResult::InternalFailure(InternalFailure {
            message: message.into(),
            span,
        })
    }
}

pub const fn type_support(ty: IrType) -> BrowserSupport {
    match ty {
        IrType::Void
        | IrType::Never
        | IrType::Int(_)
        | IrType::Float(_)
        | IrType::Decimal
        | IrType::Boolean
        | IrType::Nullable(_)
        | IrType::Closure(_)
        | IrType::Callable(_)
        | IrType::Object(_)
        | IrType::Contract(_)
        | IrType::Value(_)
        | IrType::Enum(_) => BrowserSupport::Direct,
        IrType::String
        | IrType::Regex
        | IrType::Char
        | IrType::Array(_)
        | IrType::List(_)
        | IrType::Range
        | IrType::Map(_)
        | IrType::Set(_) => BrowserSupport::RuntimeCore,
        IrType::Job
        | IrType::Channel(_)
        | IrType::Pointer(_)
        | IrType::Weak(_)
        | IrType::JournalHandle
        | IrType::NativeSlice(_)
        | IrType::NativeSliceMut(_)
        | IrType::Dependent(_)
        | IrType::Pin(_)
        | IrType::BorrowedView { .. }
        | IrType::Atomic(_)
        | IrType::Mutex(_) => BrowserSupport::Excluded,
    }
}

pub const fn terminator_support(terminator: &Terminator) -> BrowserSupport {
    match terminator {
        Terminator::Return(_)
        | Terminator::Cancel
        | Terminator::Unreachable
        | Terminator::Jump(_)
        | Terminator::Branch { .. } => BrowserSupport::Direct,
    }
}

pub fn instruction_support(instruction: &InstKind) -> BrowserSupport {
    match instruction {
        InstKind::ConstInt(_)
        | InstKind::ConstBool(_)
        | InstKind::Load(_)
        | InstKind::Store(_, _)
        | InstKind::SafepointPoll
        | InstKind::BuildValue { .. }
        | InstKind::BuildEnum { .. }
        | InstKind::Discriminant(_)
        | InstKind::Undefined
        | InstKind::Retype(_)
        | InstKind::IntCast(_)
        | InstKind::WordCast(_)
        | InstKind::ConstFloat(_, _)
        | InstKind::FloatCast(_)
        | InstKind::IntToFloat(_)
        | InstKind::FloatToInt(_)
        | InstKind::ConstDecimal(_)
        | InstKind::IntToDecimal(_)
        | InstKind::DecimalToInt(_)
        | InstKind::DecimalToFloat(_)
        | InstKind::FloatToDecimal(_)
        | InstKind::Unary { .. }
        | InstKind::Binary { .. }
        | InstKind::CheckedArithmetic { .. }
        | InstKind::NullValue(_)
        | InstKind::Wrap { .. }
        | InstKind::IsNull(_)
        | InstKind::Unwrap(_) => BrowserSupport::Direct,
        InstKind::Call { .. } => BrowserSupport::Direct,
        // `stdin.read_*` carries ambient input authority the browser
        // deliberately has no source for (`add-stdin-stdout-standard-
        // library`, design D5): browser execution must fail its preflight
        // on these instructions directly rather than ever attempting to run
        // them, the same way `browser_rejects_reachable_standard_input_
        // before_execution` (`crates/zirk-browser/src/lib.rs`) verifies.
        InstKind::StdinReadLineResult { .. }
        | InstKind::StdinReadChar { .. }
        | InstKind::StdinReadBytes { .. }
        | InstKind::StdinReadExact { .. } => BrowserSupport::Excluded,
        // Environment access and native sockets require process/OS authority
        // and the native runtime. Browser execution rejects them in preflight
        // instead of attempting to emulate host permissions or networking.
        InstKind::EnvironmentGet { .. }
        | InstKind::EnvironmentContains { .. }
        | InstKind::EnvironmentListNames { .. }
        | InstKind::NetworkDnsResolve { .. }
        | InstKind::NetworkIpAddressParse { .. }
        | InstKind::NetworkTlsConnect { .. }
        | InstKind::NetworkTlsConnectWithRoots { .. }
        | InstKind::NetworkTlsConnectWithAlpn { .. }
        | InstKind::NetworkTlsConnectWithClientIdentity { .. }
        | InstKind::NetworkTlsUpgradeTcp { .. }
        | InstKind::NetworkTlsListen { .. }
        | InstKind::NetworkTlsListenWithClientAuth { .. }
        | InstKind::NetworkTlsAccept { .. }
        | InstKind::NetworkTlsCloseListener { .. }
        | InstKind::NetworkTlsListenerIsClosed { .. }
        | InstKind::NetworkTlsPeerAddress { .. }
        | InstKind::NetworkTlsNegotiatedAlpn { .. }
        | InstKind::NetworkTlsPeerCertificateSha256 { .. }
        | InstKind::NetworkTlsRead { .. }
        | InstKind::NetworkTlsWriteAll { .. }
        | InstKind::NetworkTlsShutdown { .. }
        | InstKind::NetworkTlsClose { .. }
        | InstKind::NetworkTlsIsClosed { .. }
        | InstKind::NetworkTcpConnect { .. }
        | InstKind::NetworkTcpPeerAddress { .. }
        | InstKind::NetworkTcpRead { .. }
        | InstKind::NetworkTcpWrite { .. }
        | InstKind::NetworkTcpWriteAll { .. }
        | InstKind::NetworkTcpShutdown { .. }
        | InstKind::NetworkTcpClose { .. }
        | InstKind::NetworkTcpIsClosed { .. }
        | InstKind::NetworkTcpListen { .. }
        | InstKind::NetworkTcpAccept { .. }
        | InstKind::NetworkTcpCloseListener { .. }
        | InstKind::NetworkTcpListenerIsClosed { .. }
        | InstKind::NetworkTcpOriginNormalize { .. }
        | InstKind::NetworkUdpConnect { .. }
        | InstKind::NetworkUdpBind { .. }
        | InstKind::NetworkUdpSend { .. }
        | InstKind::NetworkUdpSendTo { .. }
        | InstKind::NetworkUdpSendBroadcast { .. }
        | InstKind::NetworkUdpSendMulticast { .. }
        | InstKind::NetworkUdpJoinMulticast { .. }
        | InstKind::NetworkUdpLeaveMulticast { .. }
        | InstKind::NetworkUdpReceive { .. }
        | InstKind::NetworkUdpClose { .. }
        | InstKind::NetworkUdpLocalAddress { .. }
        | InstKind::NetworkUdpIsClosed { .. }
        | InstKind::NetworkInterfacesSnapshot { .. }
        | InstKind::NetworkLocalConnect { .. }
        | InstKind::NetworkLocalBind { .. }
        | InstKind::NetworkLocalAccept { .. }
        | InstKind::NetworkLocalRead { .. }
        | InstKind::NetworkLocalWrite { .. }
        | InstKind::NetworkLocalShutdown { .. }
        | InstKind::NetworkLocalClose { .. }
        | InstKind::NetworkLocalIsClosed { .. }
        | InstKind::HttpClientCreate { .. }
        | InstKind::HttpClientCreateWithOptions { .. }
        | InstKind::HttpClientCreateWithTrustRoots { .. }
        | InstKind::HttpClientCreateWithProxy { .. }
        | InstKind::HttpClientCreateWithProxyAndTrustRoots { .. }
        | InstKind::HttpClientRequest { .. }
        | InstKind::HttpClientRequestStream { .. }
        | InstKind::HttpClientAction { .. }
        | InstKind::HttpWebSocketConnect { .. }
        | InstKind::HttpWebSocketConnectWithTrustRoots { .. }
        | InstKind::HttpWebSocketSend { .. }
        | InstKind::HttpWebSocketReceive { .. }
        | InstKind::HttpWebSocketClose { .. }
        | InstKind::HttpWebSocketCloseWith { .. }
        | InstKind::HttpRequestSecret { .. }
        | InstKind::HttpGetStream { .. }
        | InstKind::HttpResponseToStream { .. }
        | InstKind::HttpRequestStream { .. }
        | InstKind::HttpRegisterServerResponseProducer { .. }
        | InstKind::HttpResponseReadChunk { .. }
        | InstKind::HttpResponseReadSseEvent { .. }
        | InstKind::HttpResponseSseHeartbeatCount { .. }
        | InstKind::HttpResponseStreamClose { .. }
        | InstKind::HttpResponseHeaderValues { .. }
        | InstKind::HttpFormParse { .. }
        | InstKind::HttpMultipartParse { .. }
        | InstKind::HttpUploadRead { .. }
        | InstKind::HttpUploadClose { .. }
        | InstKind::HttpRouteMatch { .. } => BrowserSupport::Excluded,
        InstKind::HttpGet { .. }
        | InstKind::HttpRequest { .. }
        | InstKind::HttpHead { .. }
        | InstKind::HttpQuery { .. }
        | InstKind::HttpQueryWithContentType { .. }
        | InstKind::HttpPost { .. }
        | InstKind::HttpPostJson { .. }
        | InstKind::HttpPostBytes { .. } => BrowserSupport::BoundedHost,
        InstKind::HttpServerServeOnce { .. }
        | InstKind::HttpServerControlCreate
        | InstKind::HttpServerControlRequestShutdown { .. } => BrowserSupport::Excluded,
        InstKind::HttpServerServeHttp3 { .. }
        | InstKind::HttpServerWebSocketServeHttp3Once { .. } => BrowserSupport::Excluded,
        InstKind::HttpServerWebSocketServeOnce { .. } => BrowserSupport::Excluded,
        InstKind::ConstString(_)
        | InstKind::ConstChar(_)
        | InstKind::StringGraphemeOffset { .. }
        | InstKind::GraphemeLenAt { .. }
        | InstKind::GraphemeSlice { .. }
        | InstKind::HttpUrlNormalize { .. }
        | InstKind::HttpUrlResolve { .. }
        | InstKind::HttpResponseReadAll { .. }
        | InstKind::HttpResponseText { .. }
        | InstKind::Alloc(_)
        | InstKind::CheckedCast { .. }
        | InstKind::IsInstance { .. }
        | InstKind::Concat { .. }
        | InstKind::Repeat { .. }
        | InstKind::CallContract { .. }
        | InstKind::CallVirtual { .. }
        | InstKind::LoadField { .. }
        | InstKind::StoreField { .. }
        | InstKind::ToString(_)
        | InstKind::MakeClosure { .. }
        | InstKind::CallClosure { .. }
        | InstKind::MakeCallable { .. }
        | InstKind::CallCallable { .. }
        | InstKind::ArrayLength(_)
        | InstKind::ListLength(_)
        | InstKind::ArrayNew { .. }
        | InstKind::ListNew { .. }
        | InstKind::MapNew { .. }
        | InstKind::SetNew { .. }
        | InstKind::ArrayListLoad { .. }
        | InstKind::ArrayListStore { .. }
        | InstKind::ListAdd { .. }
        | InstKind::ListInsert { .. }
        | InstKind::ListRemove { .. }
        | InstKind::ListRemoveValue { .. }
        | InstKind::ArrayClone { .. }
        | InstKind::ListClone { .. }
        | InstKind::MapLength(_)
        | InstKind::MapIsEmpty(_)
        | InstKind::MapSet { .. }
        | InstKind::MapContainsKey { .. }
        | InstKind::MapGet { .. }
        | InstKind::MapRemove { .. }
        | InstKind::SetLength(_)
        | InstKind::SetIsEmpty(_)
        | InstKind::SetAdd { .. }
        | InstKind::SetContains { .. }
        | InstKind::SetRemove { .. }
        | InstKind::ArraySlice { .. }
        | InstKind::Clone(_) => BrowserSupport::RuntimeCore,
        InstKind::Print(_)
        | InstKind::Println(_)
        | InstKind::PrintStderr(_)
        | InstKind::PrintlnStderr(_)
        | InstKind::FlushStdout { .. }
        | InstKind::FlushStderr { .. }
        | InstKind::IsCancelled
        | InstKind::CancelShieldEnter
        | InstKind::CancelShieldExit => BrowserSupport::BoundedHost,
        InstKind::MutexAcquire { .. } | InstKind::MutexRelease { .. } => BrowserSupport::Excluded,
        InstKind::BorrowViewStart { .. }
        | InstKind::BorrowViewEnd { .. }
        | InstKind::BorrowViewLoad { .. }
        | InstKind::BorrowViewStore { .. }
        | InstKind::BorrowViewFieldLoad { .. }
        | InstKind::BorrowViewFieldStore { .. }
        | InstKind::MutexNew { .. }
        | InstKind::MutexWithCall { .. } => BrowserSupport::Excluded,
        // Long-lived application services are hosted by the native executor
        // and have no browser lifecycle supervisor.
        InstKind::ServiceStart { .. } | InstKind::ServiceReady { .. } => BrowserSupport::Excluded,
        InstKind::FatalError(_)
        | InstKind::Throw(_)
        | InstKind::SetSuppressed { .. }
        | InstKind::StackTrace(_)
        | InstKind::Suppressed(_)
        | InstKind::HasPendingException
        | InstKind::TakePendingException => BrowserSupport::StructuredFailure,
        InstKind::ScopeEnter
        | InstKind::ScopeEnterCollectAll
        | InstKind::ScopeExit { .. }
        | InstKind::ScopeExitCollectAll { .. }
        | InstKind::ScopeCancel { .. }
        | InstKind::BranchStart { .. }
        | InstKind::JobWaitAny { .. }
        | InstKind::JobWaitAnyWithTimer { .. }
        | InstKind::JobOutcomeWaitTake { .. }
        | InstKind::ScopeOutcomeWaitTake { .. }
        | InstKind::JobOutcomeTake { .. }
        | InstKind::JobResultLoad { .. }
        | InstKind::ParallelForStart { .. }
        | InstKind::JobWait { .. }
        | InstKind::ThreadRun { .. }
        | InstKind::AppSetShutdownGrace { .. }
        | InstKind::AppSetBackgroundError { .. }
        | InstKind::JobDone { .. }
        | InstKind::JobCancel { .. }
        | InstKind::JobCancelWithCleanupFailure { .. }
        | InstKind::BackgroundStart { .. }
        | InstKind::AtomicNew { .. }
        | InstKind::AtomicLoad { .. }
        | InstKind::AtomicStore { .. }
        | InstKind::AtomicExchange { .. }
        | InstKind::AtomicFetchAdd { .. }
        | InstKind::AtomicFetchSub { .. }
        | InstKind::AtomicCompareExchange { .. }
        | InstKind::TimerSleep { .. }
        | InstKind::ChannelNew { .. }
        | InstKind::ChannelSend { .. }
        | InstKind::ChannelRecv { .. }
        | InstKind::ChannelTrySend { .. }
        | InstKind::ChannelTryRecv { .. }
        | InstKind::ChannelClose { .. }
        | InstKind::ChannelLength(_)
        | InstKind::ChannelIsClosed(_)
        | InstKind::ChannelCapacity(_)
        | InstKind::ChannelDrain { .. }
        | InstKind::PointerFromSlot(_)
        | InstKind::PointerFromField { .. }
        | InstKind::PointerRead(_)
        | InstKind::PointerWrite { .. }
        | InstKind::PointerOffset { .. }
        | InstKind::PointerOffsetBytes { .. }
        | InstKind::PointerCast(_)
        | InstKind::PointerIsNull(_)
        | InstKind::WeakFrom(_)
        | InstKind::WeakUpgrade(_)
        | InstKind::WeakIsAlive(_)
        | InstKind::JournalBegin
        | InstKind::JournalRecordSlot { .. }
        | InstKind::JournalRecordField { .. }
        | InstKind::JournalCommit(_)
        | InstKind::JournalRollback(_)
        | InstKind::NativeSliceValidate { .. }
        | InstKind::NativeSliceValue { .. }
        | InstKind::NativeSliceLength(_)
        | InstKind::NativeSliceLoad { .. }
        | InstKind::NativeSliceStore { .. }
        | InstKind::ResourceTransfer { .. }
        | InstKind::DependentFrom { .. }
        | InstKind::PinObject { .. }
        | InstKind::UnpinObject { .. } => BrowserSupport::Excluded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirk_ir::{Block, BlockId, Function, Instruction, IntWidth, Operand, Slot, SlotId};

    fn int(width: IntWidth, value: i128) -> Value {
        Value::Int(IntegerValue::from_literal(value, width).unwrap())
    }

    #[test]
    fn execution_trace_redacts_secret_string_payload_fields_in_derived_layouts() {
        use zirk_ir::{ObjectField, ObjectLayout, StringId};
        let span = Span::empty(0);
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: IrType::Void,
            slots: Vec::new(),
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::Alloc(0),
                        ty: IrType::Object(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::ConstString(StringId(0)),
                        ty: IrType::String,
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::StoreField {
                            object: Operand(ValueId(0)),
                            index: 0,
                            value: Operand(ValueId(1)),
                        },
                        ty: IrType::Void,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(None)),
            }],
        };

        let layout = |name: &str, ancestors: Vec<u32>| ObjectLayout {
            name: name.into(),
            fields: vec![ObjectField {
                name: "value".into(),
                ty: IrType::String,
            }],
            contracts: Vec::new(),
            methods: Vec::new(),
            ancestors,
        };
        let module = Module {
            functions: vec![main],
            strings: vec!["do-not-record-this-secret".into()],
            objects: vec![
                layout("SecretString", vec![0]),
                layout("DerivedSecret", vec![1, 0]),
                layout("OrdinaryValue", vec![2]),
            ],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();

        assert!(interpreter.is_secret_trace_field(0, 0));
        assert!(interpreter.is_secret_trace_field(1, 0));
        assert!(!interpreter.is_secret_trace_field(2, 0));

        interpreter.enable_trace(TraceLimits::DEFAULT);
        assert!(matches!(
            interpreter.run_main(),
            ExecutionResult::Completed { .. }
        ));
        let trace = interpreter.take_trace();
        assert!(trace.events.iter().any(|event| matches!(
            &event.kind,
            TraceEventKind::FieldWrite { value, .. } if value == "<redacted>"
        )));
        assert!(!format!("{trace:?}").contains("do-not-record-this-secret"));
    }

    #[test]
    fn float_to_integer_rejects_exclusive_power_of_two_bounds() {
        assert_eq!(
            float_to_integer_bits(-128.0, IntWidth::I8),
            Some((-128_i128) as u128)
        );
        assert_eq!(float_to_integer_bits(127.0, IntWidth::I8), Some(127));
        assert_eq!(float_to_integer_bits(128.0, IntWidth::I8), None);
        assert_eq!(float_to_integer_bits(-129.0, IntWidth::I8), None);
        assert_eq!(float_to_integer_bits(255.0, IntWidth::U8), Some(255));
        assert_eq!(float_to_integer_bits(256.0, IntWidth::U8), None);
        assert_eq!(float_to_integer_bits(-1.0, IntWidth::U8), None);
        assert_eq!(float_to_integer_bits(f64::INFINITY, IntWidth::I64), None);
        assert_eq!(
            float_to_integer_bits(2.0_f64.powi(127), IntWidth::I128),
            None
        );
        assert_eq!(
            float_to_integer_bits(-2.0_f64.powi(127), IntWidth::I128),
            Some(1_u128 << 127)
        );
        assert_eq!(
            float_to_integer_bits(2.0_f64.powi(128), IntWidth::U128),
            None
        );
        assert_eq!(
            float_to_integer_bits(2.0_f64.powi(127), IntWidth::U128),
            Some(1_u128 << 127)
        );
    }

    #[test]
    fn interpreter_slices_resume_with_continuous_output_and_trace_state() {
        let span = Span::empty(0);
        let module = Module {
            strings: vec!["resumed".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: None,
                            kind: InstKind::SafepointPoll,
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::SafepointPoll,
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstString(zirk_ir::StringId(0)),
                            ty: IrType::String,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Println(Operand(ValueId(0))),
                            ty: IrType::Void,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        interpreter.enable_trace(TraceLimits::DEFAULT);
        let mut host = CapturedHost::default();

        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::Yielded
        );
        assert!(host.events().is_empty());
        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 2),
            ExecutionResult::Yielded
        );
        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 2),
            ExecutionResult::Completed { value: None }
        );
        assert_eq!(host.events().len(), 1);
        assert_eq!(host.events()[0].text, "resumed\n");
        assert!(
            interpreter
                .take_trace()
                .events
                .iter()
                .any(|event| { matches!(event.kind, TraceEventKind::Output { index: 0 }) })
        );
        assert!(matches!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::InternalFailure(InternalFailure { ref message, .. })
                if message.contains("not suspended")
        ));
    }

    #[test]
    fn interpreter_slice_preserves_global_instruction_budget_across_resumes() {
        let span = Span::empty(0);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: None,
                            kind: InstKind::SafepointPoll,
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::SafepointPoll,
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::SafepointPoll,
                            ty: IrType::Void,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
            }],
            ..Module::default()
        };
        let limits = InterpreterLimits {
            instructions: 3,
            ..InterpreterLimits::default()
        };
        let mut interpreter = Interpreter::new(&module, limits).unwrap();
        let mut host = CapturedHost::default();

        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::Yielded
        );
        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::Yielded
        );
        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::LimitFailure(LimitFailure::Instructions)
        );
    }

    #[test]
    fn interpreter_slice_cancellation_terminates_the_suspended_execution() {
        let span = Span::empty(0);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: None,
                        kind: InstKind::SafepointPoll,
                        ty: IrType::Void,
                        span,
                    }],
                    terminator: Some(Terminator::Return(None)),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let mut host = CapturedHost::default();

        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::Yielded
        );
        host.cancel();
        assert_eq!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::LimitFailure(LimitFailure::Cancelled)
        );
        assert!(matches!(
            interpreter.run_main_with_host_slice(&mut host, 1),
            ExecutionResult::InternalFailure(InternalFailure { ref message, .. })
                if message.contains("not suspended")
        ));
    }

    // Numeric parity regressions use runtime-core as the reference contract:
    // native runtime adapters call these same pure operations. They exercise
    // the interpreter dispatch too, but are not an end-to-end LLVM execution.
    #[test]
    fn float_to_integer_edges_match_the_defined_native_conversion_domain() {
        let cases = [
            (-128.9, IntWidth::I8, Some((-128_i128) as u128)),
            (127.9, IntWidth::I8, Some(127)),
            (128.0, IntWidth::I8, None),
            (-129.0, IntWidth::I8, None),
            (255.9, IntWidth::U8, Some(255)),
            (256.0, IntWidth::U8, None),
            (-0.9, IntWidth::U8, Some(0)),
            (-1.0, IntWidth::U8, None),
            (f64::NAN, IntWidth::I32, None),
            (f64::INFINITY, IntWidth::I32, None),
            (f64::NEG_INFINITY, IntWidth::I32, None),
            (2.0_f64.powi(127), IntWidth::I128, None),
            (-2.0_f64.powi(127), IntWidth::I128, Some(1_u128 << 127)),
        ];
        for (input, width, expected) in cases {
            assert_eq!(
                float_to_integer_bits(input, width),
                expected,
                "{input} as {width:?}"
            );
        }

        // LLVM fptosi/fptoui has poison/undefined results out of range and for
        // NaN/infinities. The cases above pin interpreter rejection, while
        // native parity is asserted only for the in-range finite cases.
        for (input, width, expected) in [
            (-127.75, IntWidth::I8, Some((-127_i128) as u128)),
            (0.0, IntWidth::U8, Some(0)),
            (255.0, IntWidth::U8, Some(255)),
        ] {
            assert_eq!(float_to_integer_bits(input, width), expected);
        }
    }

    #[test]
    fn decimal_exponents_and_overflow_match_runtime_core_and_native_failure_category() {
        use zirk_runtime_core::decimal as dec;

        let module = Module::default();
        let interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        for literal in ["1e30", "-3.125e-12", "6.25e-2"] {
            let parsed = dec::parse(literal).expect("reference decimal parses");
            let normalized = dec::format(parsed);
            let actual = interpreter.parse_decimal(&normalized).unwrap();
            assert_eq!(actual, parsed, "decimal exponent {literal}");
        }

        let max = dec::Decimal {
            coef: i128::MAX,
            scale: 0,
        };
        let one = dec::Decimal::new(1, 0);
        let native = dec::add(max, one);
        let interpreted = Interpreter::decimal_fault(native.unwrap_err());
        assert!(matches!(
            interpreted,
            ExecutionFault::Runtime { category, .. } if category == "ArithmeticOverflowError"
        ));

        let overflow_exp = dec::parse("1e1000000");
        assert!(overflow_exp.is_none());
    }

    #[test]
    fn float_duration_scaling_matches_runtime_core_values_and_failures() {
        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let invoke = |interpreter: &mut Interpreter, name: &str, nanos: i64, scalar: f64| {
            interpreter.execute_duration_intrinsic(
                name,
                &[
                    int(IntWidth::I64, i128::from(nanos)),
                    Value::Float {
                        width: FloatWidth::F64,
                        text: scalar.to_string(),
                    },
                ],
                IrType::Int(IntWidth::I64),
            )
        };

        for (nanos, scalar) in [(3, 2.5), (-9, 0.5), (123_456_789, 2.0)] {
            let native = zirk_runtime_core::duration::multiply(nanos, scalar).unwrap();
            assert_eq!(
                invoke(&mut interpreter, "zirk_rt_duration_mul_f64", nanos, scalar).unwrap(),
                int(IntWidth::I64, i128::from(native))
            );
        }
        for (nanos, scalar) in [(7, 2.0), (-9, 2.0)] {
            let native = zirk_runtime_core::duration::divide(nanos, scalar).unwrap();
            assert_eq!(
                invoke(&mut interpreter, "zirk_rt_duration_div_f64", nanos, scalar).unwrap(),
                int(IntWidth::I64, i128::from(native))
            );
        }

        for (name, scalar, expected_category) in [
            ("zirk_rt_duration_mul_f64", f64::NAN, "FloatNanError"),
            (
                "zirk_rt_duration_mul_f64",
                f64::INFINITY,
                "ArithmeticOverflowError",
            ),
            ("zirk_rt_duration_div_f64", 0.0, "DivisionByZeroError"),
        ] {
            let err = invoke(&mut interpreter, name, 1, scalar).unwrap_err();
            assert!(
                matches!(&err, ExecutionFault::Runtime { category, .. } if category == expected_category),
                "{name}({scalar}) produced {err:?}"
            );
        }
        let overflow =
            invoke(&mut interpreter, "zirk_rt_duration_mul_f64", i64::MAX, 2.0).unwrap_err();
        assert!(
            matches!(overflow, ExecutionFault::Runtime { category, .. } if category == "ArithmeticOverflowError")
        );
    }

    #[test]
    fn time_duration_wrap_and_date_day_bounds_match_runtime_core() {
        use zirk_runtime_core::temporal as tmp;

        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        for (time, delta) in [(0_i64, -1_i64), (tmp::NANOS_PER_DAY - 1, 2), (123, -456)] {
            let actual = interpreter
                .execute_temporal_intrinsic(
                    "zirk_rt_time_add_nanos",
                    &[
                        int(IntWidth::I64, i128::from(time)),
                        int(IntWidth::I64, i128::from(delta)),
                    ],
                    IrType::Int(IntWidth::I64),
                )
                .unwrap();
            assert_eq!(
                actual,
                int(IntWidth::I64, i128::from(tmp::time_add_nanos(time, delta)))
            );
        }

        let max_days = tmp::days_from_civil(i32::MAX, 12, 31);
        let min_days = tmp::days_from_civil(i32::MIN, 1, 1);
        for days in [min_days, max_days] {
            let expected = tmp::civil_from_days_checked(days).unwrap().0;
            let actual = interpreter
                .execute_temporal_intrinsic(
                    "zirk_rt_date_year",
                    &[int(IntWidth::I64, i128::from(days))],
                    IrType::Int(IntWidth::I64),
                )
                .unwrap();
            assert_eq!(actual, int(IntWidth::I64, i128::from(expected)));
        }
        for days in [min_days - 1, max_days + 1] {
            let expected = tmp::civil_from_days_checked(days);
            assert!(expected.is_none());
            let err = interpreter
                .execute_temporal_intrinsic(
                    "zirk_rt_date_year",
                    &[int(IntWidth::I64, i128::from(days))],
                    IrType::Int(IntWidth::I64),
                )
                .unwrap_err();
            assert!(
                matches!(err, ExecutionFault::Runtime { category, .. } if category == "ArithmeticOverflowError")
            );
        }
    }

    #[test]
    fn scalar_and_scheduler_types_have_explicit_policy() {
        assert_eq!(
            type_support(IrType::Int(IntWidth::I32)),
            BrowserSupport::Direct
        );
        assert_eq!(type_support(IrType::Job), BrowserSupport::Excluded);
        assert_eq!(
            type_support(IrType::Atomic(zirk_ir::AtomicKind::I32)),
            BrowserSupport::Excluded
        );
    }

    #[test]
    fn managed_atomic_instructions_fail_closed_with_a_capability_diagnostic() {
        for instruction in [
            InstKind::AtomicNew {
                kind: zirk_ir::AtomicKind::I32,
                value: Operand(ValueId(0)),
                object: SlotId(0),
            },
            InstKind::AtomicLoad {
                object: Operand(ValueId(0)),
                order: None,
                out: SlotId(0),
            },
        ] {
            assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
            assert_eq!(
                excluded_feature(&instruction).0,
                "managed `Atomic<T>` operations"
            );
        }
    }

    #[test]
    fn thread_run_is_excluded_from_browser_and_interpreter_execution() {
        let instruction = InstKind::ThreadRun {
            callable_slot: SlotId(0),
            result_slot: None,
            exception_slot: SlotId(1),
        };
        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            ("managed native threads".into(), "ThreadRun".into())
        );

        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: vec![
                    Slot {
                        name: "callable".into(),
                        ty: IrType::Callable(0),
                        span: Span::empty(0),
                    },
                    Slot {
                        name: "exception".into(),
                        ty: IrType::Object(0),
                        span: Span::empty(0),
                    },
                ],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(0)),
                        kind: instruction,
                        ty: IrType::Int(IntWidth::U32),
                        span: Span::empty(7),
                    }],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: vec![SlotId(0), SlotId(1)],
            }],
            closures: vec![zirk_ir::ClosureLayout {
                captures: vec![],
                params: vec![],
                returns: IrType::Void,
            }],
            objects: vec![zirk_ir::ObjectLayout {
                name: "Throwable".into(),
                fields: vec![],
                contracts: vec![],
                methods: vec![],
                ancestors: vec![0],
            }],
            ..Module::default()
        };
        let failures =
            preflight(&module).expect_err("managed native threads have no browser executor");
        assert!(failures.iter().any(|failure| {
            failure.code == "ZB002" && failure.message.contains("managed native threads")
        }));
    }

    #[test]
    fn background_start_is_excluded_with_a_managed_background_diagnostic() {
        let instruction = InstKind::BackgroundStart {
            target: "background.test".into(),
            body: Operand(ValueId(0)),
            status_slot: SlotId(0),
        };
        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            ("managed background work".into(), "BackgroundStart".into())
        );
    }

    #[test]
    fn app_shutdown_grace_is_excluded_with_a_managed_background_diagnostic() {
        let instruction = InstKind::AppSetShutdownGrace {
            nanos: Operand(ValueId(0)),
        };
        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            (
                "managed background work".into(),
                "AppSetShutdownGrace".into()
            )
        );
    }

    #[test]
    fn job_wait_any_is_excluded_with_a_job_handles_diagnostic() {
        let instruction = InstKind::JobWaitAny {
            jobs: vec![Operand(ValueId(0)), Operand(ValueId(1))],
        };

        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            ("job handles".into(), "JobWaitAny".into())
        );

        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(2)),
                        kind: instruction,
                        ty: IrType::Int(IntWidth::U64),
                        span: Span::empty(7),
                    }],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };

        let failures = preflight(&module).expect_err("wait-any needs the native scheduler");
        assert!(failures.iter().any(|failure| {
            failure.code == "ZB002"
                && failure.span == Some(Span::empty(7))
                && failure.message.contains("job handles")
                && failure.message.contains("JobWaitAny")
        }));
    }

    #[test]
    fn job_wait_any_with_timer_is_excluded_with_a_job_handles_diagnostic() {
        let instruction = InstKind::JobWaitAnyWithTimer {
            jobs: vec![
                Operand(ValueId(0)),
                Operand(ValueId(1)),
                Operand(ValueId(2)),
            ],
        };
        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            ("job handles".into(), "JobWaitAnyWithTimer".into())
        );

        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Undefined,
                            ty: IrType::Job,
                            span: Span::empty(1),
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::Undefined,
                            ty: IrType::Job,
                            span: Span::empty(2),
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::Undefined,
                            ty: IrType::Job,
                            span: Span::empty(3),
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: instruction.clone(),
                            ty: IrType::Int(IntWidth::U64),
                            span: Span::empty(11),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let failures =
            preflight(&module).expect_err("timer wait-any requires the native scheduler");
        assert!(failures.iter().any(|failure| {
            failure.code == "ZB002"
                && failure.span == Some(Span::empty(11))
                && failure.message.contains("job handles")
                && failure.message.contains("JobWaitAnyWithTimer")
        }));

        // Even if a caller bypasses browser preflight, the interpreter must
        // fail closed instead of inventing a completion index or timeout.
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let mut host = CapturedHost::default();
        let error = interpreter
            .execute_instruction(0, &module.functions[0].blocks[0].instructions[3], &mut host)
            .unwrap_err();
        assert!(
            matches!(error, ExecutionFault::Internal(message) if message.contains("JobWaitAnyWithTimer") && message.contains("scheduler"))
        );
    }

    #[test]
    fn cleanup_failure_cancellation_is_excluded_with_a_job_handles_diagnostic() {
        let instruction = InstKind::JobCancelWithCleanupFailure {
            job: Operand(ValueId(0)),
        };

        assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
        assert_eq!(
            excluded_feature(&instruction),
            ("job handles".into(), "JobCancelWithCleanupFailure".into())
        );
    }

    #[test]
    fn preflight_reports_all_excluded_capabilities_in_source_order() {
        let module = Module {
            externs: vec![zirk_ir::ExternFn {
                name: "native_only".into(),
                params: vec![IrType::Pointer(0)],
                return_type: IrType::Void,
            }],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: vec![Slot {
                    name: "native".into(),
                    ty: IrType::Pointer(0),
                    span: Span::empty(8),
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "native_only".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(15),
                        },
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::PointerFromSlot(SlotId(0)),
                            ty: IrType::Pointer(0),
                            span: Span::empty(16),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };

        let failures = preflight(&module).unwrap_err();
        // The extern declaration is itself unavailable and its pointer
        // parameter is independently unavailable, alongside the slot and
        // instruction in the function body.
        assert_eq!(failures.len(), 5);
        assert_eq!(failures[0].code, "ZB004");
        assert_eq!(failures[0].span, Some(Span::empty(8)));
        assert_eq!(failures[1].code, "ZB002");
        assert_eq!(failures[1].span, Some(Span::empty(16)));
        assert_eq!(failures[2].code, "ZB004");
        assert_eq!(failures[2].span, Some(Span::empty(16)));
        assert_eq!(failures[3].code, "ZB001");
        assert_eq!(failures[3].span, None);
        assert_eq!(failures[4].code, "ZB004");
        assert_eq!(failures[4].span, None);
    }

    #[test]
    fn preflight_names_missing_browser_capabilities_for_standard_modules() {
        let names = [
            "zirk_environment_get",
            "zirk_network_udp_bind",
            "zirk_network_udp_receive",
            "zirk_network_local_connect",
            "zirk_network_local_bind",
            "zirk_network_interfaces_snapshot",
            "zirk_network_tls_connect",
            "zirk_network_tcp_listen",
            "zirk_network_dns_resolve",
            "zirk_http_request",
            "zirk_http_server_serve_once",
            "zirk_http_server_control_create",
            "zirk_http_server_control_request_shutdown",
        ];
        let module = Module {
            externs: names
                .iter()
                .map(|name| zirk_ir::ExternFn {
                    name: (*name).into(),
                    params: Vec::new(),
                    return_type: IrType::Void,
                })
                .collect(),
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: names
                        .iter()
                        .map(|name| Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: (*name).into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(1),
                        })
                        .collect(),
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };

        let failures = preflight(&module).expect_err("native host capabilities are unavailable");
        let messages: Vec<_> = failures
            .iter()
            .filter(|failure| failure.code == "ZB001")
            .map(|failure| failure.message.as_str())
            .collect();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("process environment"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("UDP sockets"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("local sockets"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("interface inspection"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("TLS sockets"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("TCP sockets"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("DNS access"))
        );
        assert!(
            messages
                .iter()
                .any(|message| { message.contains("injected browser HTTP capability") })
        );
    }

    #[test]
    fn preflight_accepts_a_baseline_scalar_module() {
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstBool(true),
                        ty: IrType::Boolean,
                        span: Span::empty(0),
                    }],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        assert_eq!(preflight(&module), Ok(()));
    }

    #[test]
    fn preflight_rejects_every_native_authority_and_pointer_family() {
        // Native effects are represented in current IR as extern declarations:
        // the browser does not infer safety from a friendly symbol name.  This
        // covers the process/shell/filesystem/environment/secrets/network
        // families as well as an arbitrary C ABI call.
        let native_effects = [
            "c_abi",
            "process",
            "shell",
            "filesystem",
            "environment",
            "secrets",
            "network",
        ];
        let externs = native_effects
            .into_iter()
            .map(|name| zirk_ir::ExternFn {
                name: name.into(),
                params: Vec::new(),
                return_type: IrType::Void,
            })
            .collect();
        let module = Module {
            externs,
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: vec![
                    Slot {
                        name: "pointer".into(),
                        ty: IrType::Pointer(0),
                        span: Span::empty(10),
                    },
                    Slot {
                        name: "slice".into(),
                        ty: IrType::NativeSlice(0),
                        span: Span::empty(11),
                    },
                    Slot {
                        name: "dependent".into(),
                        ty: IrType::Dependent(0),
                        span: Span::empty(12),
                    },
                    Slot {
                        name: "pin".into(),
                        ty: IrType::Pin(0),
                        span: Span::empty(13),
                    },
                    Slot {
                        name: "journal".into(),
                        ty: IrType::JournalHandle,
                        span: Span::empty(14),
                    },
                ],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        // Every declared external is called so this checks
                        // rejected *effects*, not harmless unused metadata.
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "c_abi".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(1),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "process".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(2),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "shell".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(3),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "filesystem".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(4),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "environment".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(5),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "secrets".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(6),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "network".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(7),
                        },
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::PointerFromSlot(SlotId(0)),
                            ty: IrType::Pointer(0),
                            span: Span::empty(20),
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::JournalBegin,
                            ty: IrType::JournalHandle,
                            span: Span::empty(21),
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::NativeSliceLength(Operand(ValueId(0))),
                            ty: IrType::Int(IntWidth::I64),
                            span: Span::empty(22),
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::DependentFrom {
                                base: Operand(ValueId(0)),
                                field_ptr: Operand(ValueId(0)),
                            },
                            ty: IrType::Dependent(0),
                            span: Span::empty(23),
                        },
                        Instruction {
                            result: Some(ValueId(4)),
                            kind: InstKind::PinObject {
                                object: Operand(ValueId(0)),
                            },
                            ty: IrType::Pin(0),
                            span: Span::empty(24),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };

        let failures = preflight(&module).unwrap_err();
        assert_eq!(
            failures
                .iter()
                .filter(|failure| failure.code == "ZB001")
                .count(),
            native_effects.len()
        );
        // Each concrete pointer/unsafe instruction and its result type are
        // rejected before execution; no excluded family can slip through as
        // an opaque browser value.
        for start in 20..=24 {
            assert!(failures.iter().any(|failure| {
                failure.code == "ZB002" && failure.span == Some(Span::empty(start))
            }));
        }
        for start in 10..=14 {
            assert!(failures.iter().any(|failure| {
                failure.code == "ZB004" && failure.span == Some(Span::empty(start))
            }));
        }
    }

    #[test]
    fn native_environment_and_http_instructions_are_excluded_from_browser() {
        use zirk_ir::{InstKind, Operand};

        let value = Operand(ValueId(0));
        let instructions = [
            InstKind::EnvironmentGet {
                name: value,
                enum_id: 0,
                secret: false,
            },
            InstKind::EnvironmentContains {
                name: value,
                enum_id: 0,
            },
            InstKind::EnvironmentListNames { enum_id: 0 },
            InstKind::NetworkTcpConnect {
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkDnsResolve {
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkInterfacesSnapshot { enum_id: 0 },
            InstKind::HttpServerControlCreate,
            InstKind::HttpServerControlRequestShutdown { handle: value },
            InstKind::HttpServerWebSocketServeHttp3Once {
                endpoint: value,
                certificate_chain_der: value,
                private_key_der: value,
                path: value,
                allowed_origins: value,
                allow_missing_origin: value,
                protocols: value,
                max_messages: value,
                idle_timeout_ms: value,
                callback: value,
                callback_layout: 0,
                websocket_class_id: 0,
                enum_id: 0,
            },
            InstKind::NetworkIpAddressParse { value, enum_id: 0 },
            InstKind::NetworkTlsConnect {
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsConnectWithRoots {
                endpoint: value,
                roots_der: value,
                allow_tls12: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsListen {
                endpoint: value,
                certificate_chain_der: value,
                private_key_der: value,
                allow_tls12: value,
                http_alpn: false,
                enum_id: 0,
            },
            InstKind::NetworkTlsListenWithClientAuth {
                endpoint: value,
                certificate_chain_der: value,
                private_key_der: value,
                client_roots_der: value,
                allow_tls12: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsAccept {
                handle: value,
                enum_id: 0,
                stream_class_id: 0,
                shutdown_control: None,
            },
            InstKind::NetworkTlsUpgradeTcp {
                tcp_handle: value,
                endpoint: value,
                roots_der: value,
                allow_tls12: value,
                protocols: value,
                enum_id: 0,
                stream_class_id: 0,
            },
            InstKind::NetworkTlsCloseListener {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsListenerIsClosed { handle: value },
            InstKind::NetworkTlsPeerAddress {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsNegotiatedAlpn {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkTlsPeerCertificateSha256 {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpSendTo {
                handle: value,
                bytes: value,
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpConnect {
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpBind {
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpSend {
                handle: value,
                bytes: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpSendBroadcast {
                handle: value,
                bytes: value,
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpSendMulticast {
                handle: value,
                bytes: value,
                endpoint: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpJoinMulticast {
                handle: value,
                group: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpLeaveMulticast {
                handle: value,
                group: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpReceive {
                handle: value,
                max_bytes: value,
                enum_id: 0,
                datagram_class_id: 0,
            },
            InstKind::NetworkUdpClose {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpLocalAddress {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkUdpIsClosed { handle: value },
            InstKind::NetworkLocalConnect {
                endpoint: value,
                enum_id: 0,
                stream_class_id: 0,
            },
            InstKind::NetworkLocalBind {
                endpoint: value,
                remove_stale: value,
                enum_id: 0,
                listener_class_id: 0,
            },
            InstKind::NetworkLocalAccept {
                handle: value,
                enum_id: 0,
                stream_class_id: 0,
            },
            InstKind::NetworkLocalRead {
                handle: value,
                max_bytes: value,
                enum_id: 0,
            },
            InstKind::NetworkLocalWrite {
                handle: value,
                bytes: value,
                enum_id: 0,
                write_all: false,
            },
            InstKind::NetworkLocalShutdown {
                handle: value,
                enum_id: 0,
            },
            InstKind::NetworkLocalClose {
                handle: value,
                enum_id: 0,
                listener: false,
            },
            InstKind::NetworkLocalIsClosed {
                handle: value,
                listener: true,
            },
            InstKind::HttpRequestSecret {
                method: value,
                url: value,
                body: value,
                headers: value,
                header_name: value,
                secret_value: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpFormParse {
                body: value,
                content_type: value,
                enum_id: 0,
            },
            InstKind::HttpMultipartParse {
                body: value,
                content_type: value,
                enum_id: 0,
                data_class_id: 0,
            },
        ];

        for instruction in instructions {
            assert_eq!(instruction_support(&instruction), BrowserSupport::Excluded);
            let feature = excluded_feature(&instruction).0;
            assert!(!feature.starts_with("instruction "), "{feature}");
        }
        assert_eq!(
            instruction_support(&InstKind::HttpRequest {
                method: value,
                url: value,
                body: value,
                headers: value,
                validate_json: value,
                cached: false,
                enum_id: 0,
                response_class_id: 0,
            }),
            BrowserSupport::BoundedHost
        );
        for instruction in [
            InstKind::HttpHead {
                url: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpQuery {
                url: value,
                query: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpQueryWithContentType {
                url: value,
                query: value,
                content_type: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpPost {
                url: value,
                body: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpPostJson {
                url: value,
                body: value,
                enum_id: 0,
                response_class_id: 0,
            },
            InstKind::HttpPostBytes {
                url: value,
                body: value,
                enum_id: 0,
                response_class_id: 0,
            },
        ] {
            assert_eq!(
                instruction_support(&instruction),
                BrowserSupport::BoundedHost
            );
        }
        assert_eq!(
            instruction_support(&InstKind::HttpResponseText {
                body: value,
                enum_id: 0,
            }),
            BrowserSupport::RuntimeCore
        );
    }

    #[test]
    fn excluded_capability_prevents_entrypoint_execution() {
        let module = Module {
            externs: vec![zirk_ir::ExternFn {
                name: "native_only".into(),
                params: Vec::new(),
                return_type: IrType::Void,
            }],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: None,
                            kind: InstKind::Call {
                                callee: "native_only".into(),
                                args: Vec::new(),
                            },
                            ty: IrType::Void,
                            span: Span::empty(1),
                        },
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstBool(true),
                            ty: IrType::Boolean,
                            span: Span::empty(2),
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ConstBool(false),
                            ty: IrType::Boolean,
                            span: Span::empty(3),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let limits = InterpreterLimits {
            instructions: 1,
            ..InterpreterLimits::default()
        };
        let mut interpreter = Interpreter::new(&module, limits).unwrap();
        assert!(matches!(
            interpreter.run_main(),
            ExecutionResult::CapabilityFailure(CapabilityFailure { ref code, .. }) if code == "ZB001"
        ));
    }

    #[test]
    fn unary_operations_cover_integer_and_binary_float_widths() {
        let module = Module::default();
        let interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        for width in [
            IntWidth::I8,
            IntWidth::I16,
            IntWidth::I32,
            IntWidth::I64,
            IntWidth::I128,
        ] {
            assert_eq!(
                interpreter.execute_unary(UnaryOp::Neg, int(width, 1)),
                Ok(int(width, -1)),
            );
        }
        for width in [
            FloatWidth::F16,
            FloatWidth::F32,
            FloatWidth::F64,
            FloatWidth::F128,
        ] {
            assert_eq!(
                interpreter.execute_unary(
                    UnaryOp::Neg,
                    Value::Float {
                        width,
                        text: "1.25".into()
                    }
                ),
                Ok(Value::Float {
                    width,
                    text: if width == FloatWidth::F128 {
                        zirk_runtime_core::binary_float::Float128::from_decimal("-1.25")
                            .unwrap()
                            .wire_text()
                    } else {
                        "-1.25".into()
                    }
                }),
            );
        }
        assert_eq!(
            interpreter.execute_unary(UnaryOp::BitNot, int(IntWidth::U8, 0)),
            Ok(int(IntWidth::U8, 255)),
        );
        assert_eq!(
            interpreter.execute_unary(UnaryOp::Not, Value::Boolean(true)),
            Ok(Value::Boolean(false))
        );
    }

    #[test]
    fn direct_calls_bind_parameters_and_propagate_return_values() {
        let span = Span::empty(0);
        let module = Module {
            functions: vec![
                Function {
                    name: "main".into(),
                    params: Vec::new(),
                    return_type: IrType::Int(IntWidth::I32),
                    slots: Vec::new(),
                    blocks: vec![Block {
                        id: BlockId(0),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(0)),
                                kind: InstKind::ConstInt(41),
                                ty: IrType::Int(IntWidth::I32),
                                span,
                            },
                            Instruction {
                                result: Some(ValueId(1)),
                                kind: InstKind::Call {
                                    callee: "increment".into(),
                                    args: vec![Operand(ValueId(0))],
                                },
                                ty: IrType::Int(IntWidth::I32),
                                span,
                            },
                        ],
                        terminator: Some(Terminator::Return(Some(Operand(ValueId(1))))),
                    }],
                    entry: BlockId(0),
                    span,
                    gc_roots: Vec::new(),
                },
                Function {
                    name: "increment".into(),
                    params: vec![SlotId(0)],
                    return_type: IrType::Int(IntWidth::I32),
                    slots: vec![Slot {
                        name: "value".into(),
                        ty: IrType::Int(IntWidth::I32),
                        span,
                    }],
                    blocks: vec![Block {
                        id: BlockId(0),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(0)),
                                kind: InstKind::Load(SlotId(0)),
                                ty: IrType::Int(IntWidth::I32),
                                span,
                            },
                            Instruction {
                                result: Some(ValueId(1)),
                                kind: InstKind::ConstInt(1),
                                ty: IrType::Int(IntWidth::I32),
                                span,
                            },
                            Instruction {
                                result: Some(ValueId(2)),
                                kind: InstKind::Binary {
                                    op: BinaryOp::Add,
                                    left: Operand(ValueId(0)),
                                    right: Operand(ValueId(1)),
                                },
                                ty: IrType::Int(IntWidth::I32),
                                span,
                            },
                        ],
                        terminator: Some(Terminator::Return(Some(Operand(ValueId(2))))),
                    }],
                    entry: BlockId(0),
                    span,
                    gc_roots: Vec::new(),
                },
            ],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
    }

    #[test]
    fn virtual_and_contract_calls_use_the_runtime_layout_tables() {
        use zirk_ir::{ContractTable, ObjectLayout};
        let span = Span::empty(0);
        let object_type = IrType::Object(0);
        let int_type = IrType::Int(IntWidth::I32);
        let method = |name: &str, result: i128| Function {
            name: name.into(),
            params: vec![SlotId(0)],
            return_type: int_type,
            slots: vec![Slot {
                name: "self".into(),
                ty: object_type,
                span,
            }],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![Instruction {
                    result: Some(ValueId(0)),
                    kind: InstKind::ConstInt(result),
                    ty: int_type,
                    span,
                }],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(0))))),
            }],
            entry: BlockId(0),
            span,
            gc_roots: vec![SlotId(0)],
        };
        let module = Module {
            functions: vec![
                Function {
                    name: "main".into(),
                    params: Vec::new(),
                    return_type: int_type,
                    slots: Vec::new(),
                    blocks: vec![Block {
                        id: BlockId(0),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(0)),
                                kind: InstKind::Alloc(0),
                                ty: object_type,
                                span,
                            },
                            Instruction {
                                result: Some(ValueId(1)),
                                kind: InstKind::CallVirtual {
                                    object: Operand(ValueId(0)),
                                    index: 0,
                                    args: Vec::new(),
                                },
                                ty: int_type,
                                span,
                            },
                            Instruction {
                                result: Some(ValueId(2)),
                                kind: InstKind::CallContract {
                                    object: Operand(ValueId(0)),
                                    contract: 0,
                                    index: 0,
                                    args: Vec::new(),
                                },
                                ty: int_type,
                                span,
                            },
                        ],
                        terminator: Some(Terminator::Return(Some(Operand(ValueId(2))))),
                    }],
                    entry: BlockId(0),
                    span,
                    gc_roots: Vec::new(),
                },
                method("override", 7),
                method("contract_override", 9),
            ],
            objects: vec![ObjectLayout {
                name: "Child".into(),
                fields: Vec::new(),
                methods: vec!["override".into()],
                contracts: vec![ContractTable {
                    contract: 0,
                    methods: vec!["contract_override".into()],
                }],
                ancestors: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 9))
            }
        );
    }

    #[test]
    fn typed_throw_reaches_the_root_as_a_structured_runtime_failure() {
        use zirk_ir::ObjectLayout;
        let span = Span::empty(17);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Alloc(0),
                            ty: IrType::Object(0),
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Throw(Operand(ValueId(0))),
                            ty: IrType::Void,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
            }],
            objects: vec![ObjectLayout {
                name: "ExampleError".into(),
                fields: Vec::new(),
                contracts: Vec::new(),
                methods: Vec::new(),
                ancestors: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert!(matches!(
            interpreter.run_main(),
            ExecutionResult::RuntimeFailure(RuntimeFailure { ref category, span: Some(actual), .. })
                if category == "ExampleError" && actual == span
        ));
    }

    #[test]
    fn pending_throwable_is_taken_by_lowered_catch_control_flow() {
        use zirk_ir::ObjectLayout;
        let span = Span::empty(0);
        let module = Module {
            functions: vec![
                Function {
                    name: "main".into(),
                    params: Vec::new(),
                    return_type: IrType::Void,
                    slots: Vec::new(),
                    blocks: vec![
                        Block {
                            id: BlockId(0),
                            instructions: vec![
                                Instruction {
                                    result: None,
                                    kind: InstKind::Call {
                                        callee: "raise".into(),
                                        args: Vec::new(),
                                    },
                                    ty: IrType::Void,
                                    span,
                                },
                                Instruction {
                                    result: Some(ValueId(0)),
                                    kind: InstKind::HasPendingException,
                                    ty: IrType::Boolean,
                                    span,
                                },
                            ],
                            terminator: Some(Terminator::Branch {
                                condition: Operand(ValueId(0)),
                                then_block: BlockId(1),
                                else_block: BlockId(2),
                            }),
                        },
                        Block {
                            id: BlockId(1),
                            instructions: vec![Instruction {
                                result: Some(ValueId(1)),
                                kind: InstKind::TakePendingException,
                                ty: IrType::Object(0),
                                span,
                            }],
                            terminator: Some(Terminator::Return(None)),
                        },
                        Block {
                            id: BlockId(2),
                            instructions: Vec::new(),
                            terminator: Some(Terminator::Unreachable),
                        },
                    ],
                    entry: BlockId(0),
                    span,
                    gc_roots: Vec::new(),
                },
                Function {
                    name: "raise".into(),
                    params: Vec::new(),
                    return_type: IrType::Void,
                    slots: Vec::new(),
                    blocks: vec![Block {
                        id: BlockId(0),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(0)),
                                kind: InstKind::Alloc(0),
                                ty: IrType::Object(0),
                                span,
                            },
                            Instruction {
                                result: None,
                                kind: InstKind::Throw(Operand(ValueId(0))),
                                ty: IrType::Void,
                                span,
                            },
                        ],
                        terminator: Some(Terminator::Return(None)),
                    }],
                    entry: BlockId(0),
                    span,
                    gc_roots: Vec::new(),
                },
            ],
            objects: vec![ObjectLayout {
                name: "ExampleError".into(),
                fields: Vec::new(),
                contracts: Vec::new(),
                methods: Vec::new(),
                ancestors: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed { value: None }
        );
    }

    #[test]
    fn suppressed_throwables_preserve_the_primary_and_first_suppressed_value() {
        use zirk_ir::ObjectLayout;
        let span = Span::empty(0);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Boolean,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Alloc(0),
                            ty: IrType::Object(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::Alloc(0),
                            ty: IrType::Object(0),
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::SetSuppressed {
                                exception: Operand(ValueId(0)),
                                suppressed: Operand(ValueId(1)),
                            },
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::Suppressed(Operand(ValueId(0))),
                            ty: IrType::Nullable(zirk_ir::Nullable::Object(0)),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::IsNull(Operand(ValueId(2))),
                            ty: IrType::Boolean,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(Some(Operand(ValueId(3))))),
                }],
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
            }],
            objects: vec![ObjectLayout {
                name: "ExampleError".into(),
                fields: Vec::new(),
                contracts: Vec::new(),
                methods: Vec::new(),
                ancestors: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(Value::Boolean(false))
            }
        );
    }

    #[test]
    fn arithmetic_domain_errors_are_runtime_failures_not_interpreter_panics() {
        let span = Span::empty(0);
        let int_type = IrType::Int(IntWidth::I32);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: int_type,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstInt(1),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ConstInt(0),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::Binary {
                                op: BinaryOp::Div,
                                left: Operand(ValueId(0)),
                                right: Operand(ValueId(1)),
                            },
                            ty: int_type,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(Some(Operand(ValueId(2))))),
                }],
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert!(matches!(
            interpreter.run_main(),
            ExecutionResult::RuntimeFailure(RuntimeFailure { ref category, .. })
                if category == "ArithmeticError"
        ));
    }

    #[test]
    fn arrays_and_lists_enforce_indexes_and_preserve_mutation_order() {
        let span = Span::empty(0);
        let int_type = IrType::Int(IntWidth::I32);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: int_type,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstInt(1),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ArrayNew {
                                element_id: 0,
                                capacity: Operand(ValueId(0)),
                            },
                            ty: IrType::Array(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::ConstInt(0),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::ConstInt(42),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::ArrayListStore {
                                receiver: Operand(ValueId(1)),
                                index: Operand(ValueId(2)),
                                value: Operand(ValueId(3)),
                            },
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(4)),
                            kind: InstKind::ArrayListLoad {
                                receiver: Operand(ValueId(1)),
                                index: Operand(ValueId(2)),
                            },
                            ty: int_type,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(Some(Operand(ValueId(4))))),
                }],
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
            }],
            array_types: vec![int_type],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
    }

    #[test]
    fn char_intrinsics_are_browser_safe_runtime_core_operations() {
        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let char = Value::Heap(interpreter.allocate_char("Á".into()).unwrap());
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_char_is_uppercase",
                    std::slice::from_ref(&char),
                    IrType::Boolean,
                )
                .unwrap(),
            Value::Boolean(true)
        );
        let lowercase = interpreter
            .execute_browser_intrinsic("zirk_char_to_lowercase", &[char], IrType::String)
            .unwrap();
        assert_eq!(interpreter.render_owned_value(lowercase).unwrap(), "á");
    }

    #[test]
    fn regex_intrinsics_match_replace_reject_invalid_patterns_and_bound_input() {
        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let pattern = Value::Heap(interpreter.allocate_string("a+".into()).unwrap());
        let regex = interpreter
            .execute_browser_intrinsic("zirk_regex_from_pattern", &[pattern], IrType::Regex)
            .unwrap();
        let text = Value::Heap(interpreter.allocate_string("baacaa".into()).unwrap());
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_regex_is_match",
                    &[regex.clone(), text.clone()],
                    IrType::Boolean,
                )
                .unwrap(),
            Value::Boolean(true)
        );
        let replacement = Value::Heap(interpreter.allocate_string("x".into()).unwrap());
        let replaced = interpreter
            .execute_browser_intrinsic(
                "zirk_regex_replace",
                &[regex.clone(), text, replacement],
                IrType::String,
            )
            .unwrap();
        assert_eq!(interpreter.render_owned_value(replaced).unwrap(), "bxcx");

        let invalid = Value::Heap(interpreter.allocate_string("[".into()).unwrap());
        assert!(matches!(
            interpreter.execute_browser_intrinsic("zirk_regex_from_pattern", &[invalid], IrType::Regex),
            Err(ExecutionFault::Runtime { category, .. }) if category == "RegexError"
        ));
        let oversized = Value::Heap(interpreter.allocate_string("a".repeat(65_537)).unwrap());
        assert!(matches!(
            interpreter.execute_browser_intrinsic("zirk_regex_is_match", &[regex, oversized], IrType::Boolean),
            Err(ExecutionFault::Runtime { category, .. }) if category == "LimitError"
        ));
    }

    #[test]
    fn duration_and_range_intrinsics_use_runtime_core_values() {
        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let nanos = int(IntWidth::I64, 1_500_000_000);
        let rendered = interpreter
            .execute_browser_intrinsic(
                "zirk_rt_duration_to_string",
                std::slice::from_ref(&nanos),
                IrType::String,
            )
            .unwrap();
        assert_eq!(interpreter.render_owned_value(rendered).unwrap(), "1.5s");
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_duration_whole_seconds",
                    &[nanos],
                    IrType::Int(IntWidth::I64)
                )
                .unwrap(),
            int(IntWidth::I64, 1)
        );
        let range = interpreter
            .execute_browser_intrinsic(
                "zirk_range_new",
                &[
                    int(IntWidth::I64, 6),
                    int(IntWidth::I64, 0),
                    int(IntWidth::I64, -2),
                    int(IntWidth::I64, 1),
                ],
                IrType::Range,
            )
            .unwrap();
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_range_length",
                    std::slice::from_ref(&range),
                    IrType::Int(IntWidth::I64)
                )
                .unwrap(),
            int(IntWidth::I64, 4)
        );
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_range_element_at",
                    &[range.clone(), int(IntWidth::I64, 2)],
                    IrType::Int(IntWidth::I64),
                )
                .unwrap(),
            int(IntWidth::I64, 2)
        );
        let text = interpreter
            .execute_browser_intrinsic(
                "zirk_range_to_string",
                &[range, int(IntWidth::I64, 0)],
                IrType::String,
            )
            .unwrap();
        assert_eq!(interpreter.render_owned_value(text).unwrap(), "6..=0:-2");
    }

    #[test]
    fn string_intrinsics_preserve_unicode_search_replacement_and_normalized_equality() {
        let module = Module::default();
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let text = Value::Heap(interpreter.allocate_string("Café café".into()).unwrap());
        let needle = Value::Heap(interpreter.allocate_string("café".into()).unwrap());
        assert_eq!(
            interpreter
                .execute_browser_intrinsic(
                    "zirk_str_find",
                    &[text.clone(), needle.clone()],
                    IrType::Int(IntWidth::I64),
                )
                .unwrap(),
            int(IntWidth::I64, 5)
        );
        let replacement = Value::Heap(interpreter.allocate_string("té".into()).unwrap());
        let replaced = interpreter
            .execute_browser_intrinsic(
                "zirk_str_replace",
                &[text, needle, replacement],
                IrType::String,
            )
            .unwrap();
        assert_eq!(interpreter.render_owned_value(replaced).unwrap(), "Café té");
        let composed = interpreter.allocate_string("é".into()).unwrap();
        let decomposed = interpreter.allocate_string("e\u{301}".into()).unwrap();
        assert!(interpreter.value_equals(composed, decomposed));
    }

    #[test]
    fn nested_void_calls_clean_up_frames_before_root_completion() {
        let span = Span::empty(0);
        let void_call = |callee: &str| Instruction {
            result: None,
            kind: InstKind::Call {
                callee: callee.into(),
                args: Vec::new(),
            },
            ty: IrType::Void,
            span,
        };
        let function = |name: &str, instructions: Vec<Instruction>| Function {
            name: name.into(),
            params: Vec::new(),
            return_type: IrType::Void,
            slots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions,
                terminator: Some(Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
        };
        let module = Module {
            functions: vec![
                function("main", vec![void_call("outer")]),
                function("outer", vec![void_call("inner")]),
                function("inner", Vec::new()),
            ],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert!(matches!(
            interpreter.run_main(),
            ExecutionResult::Completed { value: None }
        ));
        assert!(interpreter.capture_stack().is_empty());
    }

    #[test]
    fn direct_recursion_stops_at_the_exact_call_depth_limit() {
        let span = Span::empty(0);
        let recursive_call = |callee: &str| Instruction {
            result: None,
            kind: InstKind::Call {
                callee: callee.into(),
                args: Vec::new(),
            },
            ty: IrType::Void,
            span,
        };
        let function = |name: &str, instructions: Vec<Instruction>| Function {
            name: name.into(),
            params: Vec::new(),
            return_type: IrType::Void,
            slots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions,
                terminator: Some(Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
        };
        let module = Module {
            functions: vec![
                function("main", vec![recursive_call("loop")]),
                function("loop", vec![recursive_call("loop")]),
            ],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(
            &module,
            InterpreterLimits {
                call_depth: 2,
                ..InterpreterLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::LimitFailure(LimitFailure::CallDepth)
        );
    }

    #[test]
    fn closures_and_callables_copy_captures_then_dispatch_indirectly() {
        use zirk_ir::{ClosureLayout, Operand, Slot};
        let span = Span::empty(0);
        let ty = IrType::Int(IntWidth::I32);
        let lifted = Function {
            name: "lifted".into(),
            params: vec![SlotId(0), SlotId(1)],
            return_type: ty,
            slots: vec![
                Slot {
                    name: "capture".into(),
                    ty,
                    span,
                },
                Slot {
                    name: "argument".into(),
                    ty,
                    span,
                },
            ],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::Load(SlotId(0)),
                        ty,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::Load(SlotId(1)),
                        ty,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::Binary {
                            op: BinaryOp::Add,
                            left: Operand(ValueId(0)),
                            right: Operand(ValueId(1)),
                        },
                        ty,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(2))))),
            }],
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
        };
        let make = |kind| Instruction {
            result: Some(ValueId(1)),
            kind,
            ty: IrType::Closure(0),
            span,
        };
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: ty,
            slots: Vec::new(),
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstInt(40),
                        ty,
                        span,
                    },
                    make(InstKind::MakeClosure {
                        id: 0,
                        captures: vec![Operand(ValueId(0))],
                        target: "lifted".into(),
                    }),
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::ConstInt(2),
                        ty,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: InstKind::CallClosure {
                            id: 0,
                            callee: Operand(ValueId(1)),
                            args: vec![Operand(ValueId(2))],
                        },
                        ty,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(3))))),
            }],
        };
        let module = Module {
            functions: vec![main, lifted],
            closures: vec![ClosureLayout {
                captures: vec![ty],
                params: vec![ty],
                returns: ty,
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
        let mut callable_module = module.clone();
        let instructions = &mut callable_module.functions[0].blocks[0].instructions;
        instructions[1].kind = InstKind::MakeCallable {
            target: "lifted".into(),
            captures: vec![Operand(ValueId(0))],
        };
        instructions[1].ty = IrType::Callable(0);
        instructions[3].kind = InstKind::CallCallable {
            callable: Operand(ValueId(1)),
            args: vec![Operand(ValueId(2))],
        };
        let mut interpreter =
            Interpreter::new(&callable_module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
    }

    #[test]
    fn objects_use_layout_fields_and_checked_runtime_type_tests() {
        use zirk_ir::{ObjectField, ObjectLayout, Operand};
        let span = Span::empty(0);
        let ty = IrType::Int(IntWidth::I32);
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: ty,
            slots: Vec::new(),
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::Alloc(1),
                        ty: IrType::Object(1),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::ConstInt(42),
                        ty,
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::StoreField {
                            object: Operand(ValueId(0)),
                            index: 0,
                            value: Operand(ValueId(1)),
                        },
                        ty: IrType::Void,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::IsInstance {
                            object: Operand(ValueId(0)),
                            target_class: 0,
                        },
                        ty: IrType::Boolean,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: InstKind::CheckedCast {
                            object: Operand(ValueId(0)),
                            target_class: 0,
                        },
                        ty: IrType::Object(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(4)),
                        kind: InstKind::LoadField {
                            object: Operand(ValueId(3)),
                            index: 0,
                        },
                        ty,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(4))))),
            }],
        };
        let layout = |name: &str, ancestors: Vec<u32>| ObjectLayout {
            name: name.into(),
            fields: vec![ObjectField {
                name: "value".into(),
                ty,
            }],
            contracts: Vec::new(),
            methods: Vec::new(),
            ancestors,
        };
        let module = Module {
            functions: vec![main],
            objects: vec![layout("Base", vec![]), layout("Child", vec![0])],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
    }

    #[test]
    fn records_and_enums_preserve_fields_and_discriminants() {
        use zirk_ir::{EnumLayout, ObjectField, Operand, ValueLayout};
        let span = Span::empty(0);
        let ty = IrType::Int(IntWidth::I32);
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: ty,
            slots: Vec::new(),
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstInt(7),
                        ty,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::BuildValue {
                            class: 0,
                            fields: vec![Operand(ValueId(0))],
                        },
                        ty: IrType::Value(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::BuildEnum {
                            enum_id: 0,
                            variant: 1,
                            fields: vec![Operand(ValueId(0))],
                        },
                        ty: IrType::Enum(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: InstKind::Discriminant(Operand(ValueId(2))),
                        ty,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(3))))),
            }],
        };
        let field = ObjectField {
            name: "payload".into(),
            ty,
        };
        let module = Module {
            functions: vec![main],
            values: vec![ValueLayout {
                name: "Point".into(),
                fields: vec![field.clone()],
            }],
            enums: vec![EnumLayout {
                name: "Choice".into(),
                fields: vec![field],
                variants: vec![vec![], vec![0]],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 1))
            }
        );
    }

    #[test]
    fn records_are_structural_values_but_keep_object_identity_separate() {
        use zirk_ir::{ObjectField, Operand, ValueLayout};
        let span = Span::empty(0);
        let int_type = IrType::Int(IntWidth::I32);
        let function = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: IrType::Boolean,
            slots: Vec::new(),
            entry: BlockId(0),
            span,
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstInt(7),
                        ty: int_type,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::BuildValue {
                            class: 0,
                            fields: vec![Operand(ValueId(0))],
                        },
                        ty: IrType::Value(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::BuildValue {
                            class: 0,
                            fields: vec![Operand(ValueId(0))],
                        },
                        ty: IrType::Value(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: InstKind::LoadField {
                            object: Operand(ValueId(1)),
                            index: 0,
                        },
                        ty: int_type,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(4)),
                        kind: InstKind::Binary {
                            op: BinaryOp::Eq,
                            left: Operand(ValueId(1)),
                            right: Operand(ValueId(2)),
                        },
                        ty: IrType::Boolean,
                        span,
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(4))))),
            }],
        };
        let module = Module {
            functions: vec![function],
            values: vec![ValueLayout {
                name: "Point".into(),
                fields: vec![ObjectField {
                    name: "x".into(),
                    ty: int_type,
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(Value::Boolean(true))
            }
        );
    }

    #[test]
    fn exact_decimal_arithmetic_comparison_formatting_and_conversion_use_runtime_core() {
        let module = Module::default();
        let interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Add,
                    Value::Decimal("0.1".into()),
                    Value::Decimal("0.2".into()),
                )
                .unwrap(),
            Value::Decimal("0.3".into())
        );
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Lt,
                    Value::Decimal("1.25".into()),
                    Value::Decimal("2".into()),
                )
                .unwrap(),
            Value::Boolean(true)
        );
        assert_eq!(
            interpreter
                .execute_unary(UnaryOp::Neg, Value::Decimal("2.5".into()))
                .unwrap(),
            Value::Decimal("-2.5".into())
        );
        let decimal = zirk_runtime_core::decimal::parse("42.9").unwrap();
        assert_eq!(
            zirk_runtime_core::decimal::to_i128_checked(decimal, 8, true),
            Ok(42)
        );
        assert!(zirk_runtime_core::decimal::to_i128_checked(decimal, 5, false).is_err());
        assert!(
            interpreter
                .execute_binary(
                    BinaryOp::Div,
                    Value::Decimal("1".into()),
                    Value::Decimal("0".into()),
                )
                .is_err()
        );
    }

    #[test]
    fn binary_float_operations_round_to_the_declared_width_and_reject_invalid_domains() {
        let module = Module::default();
        let interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Add,
                    Value::Float {
                        width: FloatWidth::F32,
                        text: "0.1".into()
                    },
                    Value::Float {
                        width: FloatWidth::F32,
                        text: "0.2".into()
                    },
                )
                .unwrap(),
            Value::Float {
                width: FloatWidth::F32,
                text: "0.3".into()
            }
        );
        let one = zirk_runtime_core::binary_float::Float128::from_decimal("1")
            .unwrap()
            .wire_text();
        let ulp = zirk_runtime_core::binary_float::Float128::from_decimal(
            "1.92592994438723585305597794258492732e-34",
        )
        .unwrap()
        .wire_text();
        let widened = interpreter
            .execute_binary(
                BinaryOp::Add,
                Value::Float {
                    width: FloatWidth::F128,
                    text: one.clone(),
                },
                Value::Float {
                    width: FloatWidth::F128,
                    text: ulp,
                },
            )
            .unwrap();
        assert_eq!(
            interpreter.execute_binary(
                BinaryOp::NotEq,
                widened,
                Value::Float {
                    width: FloatWidth::F128,
                    text: one
                },
            ),
            Ok(Value::Boolean(true))
        );
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Add,
                    Value::Float {
                        width: FloatWidth::F16,
                        text: "0.1".into()
                    },
                    Value::Float {
                        width: FloatWidth::F16,
                        text: "0.2".into()
                    },
                )
                .unwrap(),
            Value::Float {
                width: FloatWidth::F16,
                text: "0.30004883".into()
            }
        );
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Lt,
                    Value::Float {
                        width: FloatWidth::F64,
                        text: "1".into()
                    },
                    Value::Float {
                        width: FloatWidth::F64,
                        text: "2".into()
                    },
                )
                .unwrap(),
            Value::Boolean(true)
        );
        // IEEE 754 results carry through verbatim: `1 / 0` is `inf` and `NaN`
        // is a value the lowering's `guard_nan` turns into `FloatNanError`.
        assert_eq!(
            interpreter
                .execute_binary(
                    BinaryOp::Div,
                    Value::Float {
                        width: FloatWidth::F64,
                        text: "1".into()
                    },
                    Value::Float {
                        width: FloatWidth::F64,
                        text: "0".into()
                    },
                )
                .unwrap(),
            Value::Float {
                width: FloatWidth::F64,
                text: "inf".into()
            }
        );
        assert!(Interpreter::parse_binary_float("NaN").unwrap().is_nan());
    }

    #[test]
    fn boolean_logic_and_nullable_presence_are_value_operations() {
        let module = Module::default();
        let interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.execute_binary(BinaryOp::And, Value::Boolean(true), Value::Boolean(false)),
            Ok(Value::Boolean(false))
        );
        assert_eq!(
            interpreter.execute_unary(UnaryOp::Not, Value::Boolean(false)),
            Ok(Value::Boolean(true))
        );
        assert_eq!(
            interpreter.execute_binary(
                BinaryOp::Identical,
                Value::Boolean(true),
                Value::Boolean(true),
            ),
            Ok(Value::Boolean(true))
        );
    }

    #[test]
    fn nullable_operations_preserve_presence_and_absence() {
        use zirk_ir::{Nullable, Operand};
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: IrType::Int(IntWidth::I32),
            slots: Vec::new(),
            entry: BlockId(0),
            span: Span::empty(0),
            gc_roots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstInt(7),
                        ty: IrType::Int(IntWidth::I32),
                        span: Span::empty(0),
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::Wrap {
                            base: Nullable::Int(IntWidth::I32),
                            value: Operand(ValueId(0)),
                        },
                        ty: IrType::Nullable(Nullable::Int(IntWidth::I32)),
                        span: Span::empty(1),
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::IsNull(Operand(ValueId(1))),
                        ty: IrType::Boolean,
                        span: Span::empty(2),
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: InstKind::Unwrap(Operand(ValueId(1))),
                        ty: IrType::Int(IntWidth::I32),
                        span: Span::empty(3),
                    },
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(3))))),
            }],
        };
        let module = Module {
            functions: vec![main],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 7))
            }
        );
    }

    #[test]
    fn host_exposes_only_ordered_output_and_cancellation() {
        let mut host = CapturedHost::default();
        assert!(!host.is_cancelled());
        host.emit(OutputEvent {
            text: "ñ\n".into(),
            destination: OutputDestination::Stdout,
            truncated: false,
        })
        .unwrap();
        host.cancel();
        assert!(host.is_cancelled());
        assert_eq!(
            host.events(),
            &[OutputEvent {
                text: "ñ\n".into(),
                destination: OutputDestination::Stdout,
                truncated: false,
            }]
        );
    }

    #[test]
    fn default_limits_are_valid_and_hard_maxima_are_enforced() {
        assert_eq!(
            InterpreterLimits::default().validate(),
            Ok(InterpreterLimits::default())
        );
        assert_eq!(
            InterpreterLimits {
                instructions: InterpreterLimits::MAX_INSTRUCTIONS + 1,
                ..InterpreterLimits::default()
            }
            .validate(),
            Err(LimitConfigurationError::Instructions)
        );
        assert_eq!(
            InterpreterLimits {
                instructions: 0,
                ..InterpreterLimits::default()
            }
            .validate(),
            Err(LimitConfigurationError::Instructions)
        );
    }

    #[test]
    fn heap_refuses_an_allocation_beyond_the_execution_budget() {
        let mut heap = Heap::default();
        let limits = InterpreterLimits {
            heap_bytes: 3,
            ..InterpreterLimits::default()
        };
        assert_eq!(
            heap.allocate(HeapEntry::String("ok".into()), 3, limits),
            Ok(HeapId(0))
        );
        assert_eq!(
            heap.allocate(HeapEntry::String("no".into()), 1, limits),
            Err(LimitFailure::Heap)
        );
    }

    #[test]
    fn constructor_rejects_unverified_ir_before_allocating_execution_state() {
        let module = Module {
            functions: vec![Function {
                name: "broken".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: Vec::new(),
                entry: BlockId(99),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        assert!(Interpreter::new(&module, InterpreterLimits::default()).is_err());
    }

    #[test]
    fn each_execution_owns_a_fresh_empty_arena() {
        let module = Module::default();
        let first = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(first.heap_bytes(), 0);
        drop(first);
        assert_eq!(
            Interpreter::new(&module, InterpreterLimits::default())
                .unwrap()
                .heap_bytes(),
            0
        );
    }

    #[test]
    fn executes_scalar_constants_and_returns_the_result() {
        let block = Block {
            id: BlockId(0),
            instructions: vec![Instruction {
                result: Some(ValueId(0)),
                kind: InstKind::ConstInt(42),
                ty: IrType::Int(IntWidth::I32),
                span: Span::empty(0),
            }],
            terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(0))))),
        };
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Int(IntWidth::I32),
                slots: Vec::new(),
                blocks: vec![block],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 42))
            }
        );
    }

    #[test]
    fn materializes_string_constants_in_the_execution_heap() {
        let module = Module {
            strings: vec!["ñandú".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::String,
                slots: Vec::new(),
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstString(zirk_ir::StringId(0)),
                        ty: IrType::String,
                        span: Span::empty(0),
                    }],
                    terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(0))))),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let result = interpreter.run_main();
        let ExecutionResult::Completed {
            value: Some(Value::Heap(id)),
        } = result
        else {
            panic!("expected heap string")
        };
        assert_eq!(
            interpreter.heap.get(id),
            Some(&HeapEntry::String("ñandú".into()))
        );
    }

    #[test]
    fn concatenates_and_repeats_strings_through_runtime_core() {
        let module = Module {
            strings: vec!["ñ".into(), "andú".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::String,
                slots: Vec::new(),
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstString(zirk_ir::StringId(0)),
                            ty: IrType::String,
                            span: Span::empty(0),
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ConstString(zirk_ir::StringId(1)),
                            ty: IrType::String,
                            span: Span::empty(1),
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::Concat {
                                left: zirk_ir::Operand(ValueId(0)),
                                right: zirk_ir::Operand(ValueId(1)),
                            },
                            ty: IrType::String,
                            span: Span::empty(2),
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::ConstInt(2),
                            ty: IrType::Int(IntWidth::I32),
                            span: Span::empty(3),
                        },
                        Instruction {
                            result: Some(ValueId(4)),
                            kind: InstKind::Repeat {
                                string: zirk_ir::Operand(ValueId(2)),
                                count: zirk_ir::Operand(ValueId(3)),
                            },
                            ty: IrType::String,
                            span: Span::empty(4),
                        },
                    ],
                    terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(4))))),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let ExecutionResult::Completed {
            value: Some(Value::Heap(id)),
        } = interpreter.run_main()
        else {
            panic!("expected a heap string result")
        };
        assert_eq!(
            interpreter.heap.get(id),
            Some(&HeapEntry::String("ñandúñandú".into()))
        );
    }

    #[test]
    fn materializes_a_single_unicode_grapheme_as_char() {
        let module = Module {
            strings: vec!["👩‍💻".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Char,
                slots: Vec::new(),
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstChar(zirk_ir::StringId(0)),
                        ty: IrType::Char,
                        span: Span::empty(0),
                    }],
                    terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(0))))),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let ExecutionResult::Completed {
            value: Some(Value::Heap(id)),
        } = interpreter.run_main()
        else {
            panic!("expected a heap char result")
        };
        assert_eq!(
            interpreter.heap.get(id),
            Some(&HeapEntry::Char("👩‍💻".into()))
        );
    }

    #[test]
    fn print_and_println_emit_ordered_unicode_events_without_native_stdout() {
        let module = Module {
            strings: vec!["".into(), "ñ".into(), "fin".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstString(zirk_ir::StringId(0)),
                            ty: IrType::String,
                            span: Span::empty(0),
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ConstString(zirk_ir::StringId(1)),
                            ty: IrType::String,
                            span: Span::empty(1),
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::ConstString(zirk_ir::StringId(2)),
                            ty: IrType::String,
                            span: Span::empty(2),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Print(zirk_ir::Operand(ValueId(0))),
                            ty: IrType::Void,
                            span: Span::empty(3),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Println(zirk_ir::Operand(ValueId(1))),
                            ty: IrType::Void,
                            span: Span::empty(4),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Println(zirk_ir::Operand(ValueId(0))),
                            ty: IrType::Void,
                            span: Span::empty(5),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Print(zirk_ir::Operand(ValueId(2))),
                            ty: IrType::Void,
                            span: Span::empty(6),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        let mut host = CapturedHost::default();

        assert_eq!(
            interpreter.run_main_with_host(&mut host),
            ExecutionResult::Completed { value: None }
        );
        assert_eq!(
            host.events(),
            &[
                // The leading empty `print` continues the line that `println`
                // ends, so the host receives one event per line.
                OutputEvent {
                    text: "ñ\n".into(),
                    destination: OutputDestination::Stdout,
                    truncated: false,
                },
                OutputEvent {
                    text: "\n".into(),
                    destination: OutputDestination::Stdout,
                    truncated: false
                },
                OutputEvent {
                    text: "fin".into(),
                    destination: OutputDestination::Stdout,
                    truncated: false
                },
            ]
        );
    }

    #[test]
    fn output_limits_reject_an_event_before_it_reaches_the_host() {
        let module = Module {
            strings: vec!["ñ".into()],
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstString(zirk_ir::StringId(0)),
                            ty: IrType::String,
                            span: Span::empty(0),
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::Println(zirk_ir::Operand(ValueId(0))),
                            ty: IrType::Void,
                            span: Span::empty(1),
                        },
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
            }],
            ..Module::default()
        };
        let limits = InterpreterLimits {
            output_bytes: 2,
            ..InterpreterLimits::default()
        };
        let mut interpreter = Interpreter::new(&module, limits).unwrap();
        let mut host = CapturedHost::default();

        assert_eq!(
            interpreter.run_main_with_host(&mut host),
            ExecutionResult::LimitFailure(LimitFailure::Output)
        );
        assert_eq!(
            host.events(),
            &[OutputEvent {
                text: String::new(),
                destination: OutputDestination::Stdout,
                truncated: true,
            }]
        );
    }

    #[test]
    fn executes_store_then_load_through_a_local_slot() {
        let block = Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: Some(ValueId(0)),
                    kind: InstKind::ConstBool(true),
                    ty: IrType::Boolean,
                    span: Span::empty(0),
                },
                Instruction {
                    result: None,
                    kind: InstKind::Store(SlotId(0), zirk_ir::Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: Span::empty(1),
                },
                Instruction {
                    result: Some(ValueId(1)),
                    kind: InstKind::Load(SlotId(0)),
                    ty: IrType::Boolean,
                    span: Span::empty(2),
                },
            ],
            terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(1))))),
        };
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Boolean,
                slots: vec![Slot {
                    name: "answer".into(),
                    ty: IrType::Boolean,
                    span: Span::empty(0),
                }],
                blocks: vec![block],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(Value::Boolean(true))
            }
        );
    }

    #[test]
    fn executes_the_selected_branch_only() {
        let main = Function {
            name: "main".into(),
            params: Vec::new(),
            return_type: IrType::Int(IntWidth::I32),
            slots: Vec::new(),
            blocks: vec![
                Block {
                    id: BlockId(0),
                    instructions: vec![Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::ConstBool(true),
                        ty: IrType::Boolean,
                        span: Span::empty(0),
                    }],
                    terminator: Some(Terminator::Branch {
                        condition: zirk_ir::Operand(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    }),
                },
                Block {
                    id: BlockId(1),
                    instructions: vec![Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::ConstInt(7),
                        ty: IrType::Int(IntWidth::I32),
                        span: Span::empty(1),
                    }],
                    terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(1))))),
                },
                Block {
                    id: BlockId(2),
                    instructions: vec![Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::ConstInt(9),
                        ty: IrType::Int(IntWidth::I32),
                        span: Span::empty(2),
                    }],
                    terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(2))))),
                },
            ],
            entry: BlockId(0),
            span: Span::empty(0),
            gc_roots: Vec::new(),
        };
        let module = Module {
            functions: vec![main],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 7))
            }
        );
    }

    #[test]
    fn stops_an_instructionless_cycle_at_the_exact_budget() {
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Void,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: Vec::new(),
                    terminator: Some(Terminator::Jump(BlockId(0))),
                }],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let limits = InterpreterLimits {
            instructions: 3,
            ..InterpreterLimits::default()
        };
        let mut interpreter = Interpreter::new(&module, limits).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::LimitFailure(LimitFailure::Instructions)
        );
        assert_eq!(interpreter.executed_steps, 3);
    }

    #[test]
    fn executes_a_finite_loop_through_slots_and_back_edges() {
        let i32 = IrType::Int(IntWidth::I32);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: i32,
                slots: vec![Slot {
                    name: "counter".into(),
                    ty: i32,
                    span: Span::empty(0),
                }],
                blocks: vec![
                    Block {
                        id: BlockId(0),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(0)),
                                kind: InstKind::ConstInt(0),
                                ty: i32,
                                span: Span::empty(0),
                            },
                            Instruction {
                                result: None,
                                kind: InstKind::Store(SlotId(0), zirk_ir::Operand(ValueId(0))),
                                ty: IrType::Void,
                                span: Span::empty(0),
                            },
                        ],
                        terminator: Some(Terminator::Jump(BlockId(1))),
                    },
                    Block {
                        id: BlockId(1),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(1)),
                                kind: InstKind::Load(SlotId(0)),
                                ty: i32,
                                span: Span::empty(1),
                            },
                            Instruction {
                                result: Some(ValueId(2)),
                                kind: InstKind::ConstInt(3),
                                ty: i32,
                                span: Span::empty(1),
                            },
                            Instruction {
                                result: Some(ValueId(3)),
                                kind: InstKind::Binary {
                                    op: BinaryOp::Lt,
                                    left: zirk_ir::Operand(ValueId(1)),
                                    right: zirk_ir::Operand(ValueId(2)),
                                },
                                ty: IrType::Boolean,
                                span: Span::empty(1),
                            },
                        ],
                        terminator: Some(Terminator::Branch {
                            condition: zirk_ir::Operand(ValueId(3)),
                            then_block: BlockId(2),
                            else_block: BlockId(3),
                        }),
                    },
                    Block {
                        id: BlockId(2),
                        instructions: vec![
                            Instruction {
                                result: Some(ValueId(4)),
                                kind: InstKind::Load(SlotId(0)),
                                ty: i32,
                                span: Span::empty(2),
                            },
                            Instruction {
                                result: Some(ValueId(5)),
                                kind: InstKind::ConstInt(1),
                                ty: i32,
                                span: Span::empty(2),
                            },
                            Instruction {
                                result: Some(ValueId(6)),
                                kind: InstKind::Binary {
                                    op: BinaryOp::Add,
                                    left: zirk_ir::Operand(ValueId(4)),
                                    right: zirk_ir::Operand(ValueId(5)),
                                },
                                ty: i32,
                                span: Span::empty(2),
                            },
                            Instruction {
                                result: None,
                                kind: InstKind::Store(SlotId(0), zirk_ir::Operand(ValueId(6))),
                                ty: IrType::Void,
                                span: Span::empty(2),
                            },
                        ],
                        terminator: Some(Terminator::Jump(BlockId(1))),
                    },
                    Block {
                        id: BlockId(3),
                        instructions: vec![Instruction {
                            result: Some(ValueId(7)),
                            kind: InstKind::Load(SlotId(0)),
                            ty: i32,
                            span: Span::empty(3),
                        }],
                        terminator: Some(Terminator::Return(Some(zirk_ir::Operand(ValueId(7))))),
                    },
                ],
                entry: BlockId(0),
                span: Span::empty(0),
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 3))
            }
        );
    }

    #[test]
    fn integer_operations_reject_overflow_instead_of_wrapping() {
        let maximum = IntegerValue::from_literal(127, IntWidth::I8).unwrap();
        let one = IntegerValue::from_literal(1, IntWidth::I8).unwrap();
        assert_eq!(
            maximum.checked_binary(BinaryOp::Add, one),
            Err("integer Add overflow".into())
        );

        let maximum = IntegerValue::from_bits(u128::MAX, IntWidth::U128);
        assert_eq!(
            maximum.checked_binary(BinaryOp::Add, IntegerValue::from_bits(1, IntWidth::U128)),
            Err("integer Add overflow or invalid domain".into())
        );
    }

    #[test]
    fn integer_binary_operations_preserve_every_declared_width() {
        let widths = [
            IntWidth::I8,
            IntWidth::I16,
            IntWidth::I32,
            IntWidth::I64,
            IntWidth::I128,
            IntWidth::U8,
            IntWidth::U16,
            IntWidth::U32,
            IntWidth::U64,
            IntWidth::U128,
        ];

        for width in widths {
            let one = IntegerValue::from_literal(1, width).unwrap();
            let two = IntegerValue::from_literal(2, width).unwrap();
            let three = IntegerValue::from_literal(3, width).unwrap();
            let four = IntegerValue::from_literal(4, width).unwrap();

            assert_eq!(
                one.checked_binary(BinaryOp::Add, one),
                Ok(Value::Int(two)),
                "{width:?} addition"
            );
            assert_eq!(
                three.checked_binary(BinaryOp::Sub, one),
                Ok(Value::Int(two)),
                "{width:?} subtraction"
            );
            assert_eq!(
                two.checked_binary(BinaryOp::Mul, two),
                Ok(Value::Int(four)),
                "{width:?} multiplication"
            );
            assert_eq!(
                three.checked_binary(BinaryOp::Div, two),
                Ok(Value::Int(one)),
                "{width:?} division"
            );
            assert_eq!(
                three.checked_binary(BinaryOp::Rem, two),
                Ok(Value::Int(one)),
                "{width:?} remainder"
            );
            assert_eq!(
                one.checked_binary(BinaryOp::Shl, one),
                Ok(Value::Int(two)),
                "{width:?} left shift"
            );
            assert_eq!(
                two.checked_binary(BinaryOp::Shr, one),
                Ok(Value::Int(one)),
                "{width:?} right shift"
            );
            assert_eq!(
                one.checked_binary(BinaryOp::Lt, two),
                Ok(Value::Boolean(true)),
                "{width:?} comparison"
            );
            assert_eq!(
                three.checked_binary(BinaryOp::BitAnd, one),
                Ok(Value::Int(one)),
                "{width:?} bitwise operation"
            );
            assert!(
                two.checked_binary(BinaryOp::Div, IntegerValue::from_bits(0, width))
                    .is_err()
            );

            if width.signed() {
                let maximum = IntegerValue::from_bits(
                    if width.bits() == 128 {
                        i128::MAX as u128
                    } else {
                        (1_u128 << (width.bits() - 1)) - 1
                    },
                    width,
                );
                let minimum = IntegerValue::from_bits(1_u128 << (width.bits() - 1), width);
                assert!(
                    maximum.checked_binary(BinaryOp::Add, one).is_err(),
                    "{width:?} maximum"
                );
                assert!(
                    minimum.checked_binary(BinaryOp::Sub, one).is_err(),
                    "{width:?} minimum"
                );
                let minus_one = IntegerValue::from_literal(-1, width).unwrap();
                assert_eq!(
                    minus_one.checked_binary(BinaryOp::Lt, one),
                    Ok(Value::Boolean(true)),
                    "{width:?} signed comparison"
                );
            } else {
                let maximum = IntegerValue::from_bits(
                    if width.bits() == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << width.bits()) - 1
                    },
                    width,
                );
                assert!(
                    maximum.checked_binary(BinaryOp::Add, one).is_err(),
                    "{width:?} maximum"
                );
                assert!(
                    IntegerValue::from_bits(0, width)
                        .checked_binary(BinaryOp::Sub, one)
                        .is_err(),
                    "{width:?} minimum"
                );
            }
        }
    }

    #[test]
    fn integer_casts_use_explicit_truncation_and_source_extension() {
        let signed_minus_one = IntegerValue::from_literal(-1, IntWidth::I8).unwrap();
        assert_eq!(
            signed_minus_one.cast(IntWidth::U16),
            IntegerValue::from_literal(65_535, IntWidth::U16).unwrap()
        );

        let unsigned_max = IntegerValue::from_literal(255, IntWidth::U8).unwrap();
        assert_eq!(
            unsigned_max.cast(IntWidth::I16),
            IntegerValue::from_literal(255, IntWidth::I16).unwrap()
        );

        let wide = IntegerValue::from_literal(511, IntWidth::U16).unwrap();
        assert_eq!(
            wide.cast(IntWidth::I8),
            IntegerValue::from_literal(-1, IntWidth::I8).unwrap()
        );
    }

    #[test]
    fn reports_unreachable_with_the_enclosing_function_span() {
        let function_span = Span::empty(17);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: IrType::Never,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: Vec::new(),
                    terminator: Some(Terminator::Unreachable),
                }],
                entry: BlockId(0),
                span: function_span,
                gc_roots: Vec::new(),
            }],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::InternalFailure(InternalFailure {
                message: "execution reached Unreachable terminator".into(),
                span: Some(function_span),
            })
        );
    }

    #[test]
    fn array_new_initializes_elements_with_the_type_default() {
        // `mut fixed: Int32[6]` lowers to `ArrayNew` with the element type in
        // the module's `array_types` table: every slot starts at the type's
        // default (`0` for integers), not `Undefined`. The program stores 50
        // at index 5 and returns `fixed[0] + fixed[5] + fixed.length` = 56.
        let span = Span::empty(0);
        let int_type = IrType::Int(IntWidth::I32);
        let module = Module {
            functions: vec![Function {
                name: "main".into(),
                params: Vec::new(),
                return_type: int_type,
                slots: Vec::new(),
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::ConstInt(6),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ArrayNew {
                                element_id: 0,
                                capacity: Operand(ValueId(0)),
                            },
                            ty: IrType::Array(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::ConstInt(5),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::ConstInt(50),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::ArrayListStore {
                                receiver: Operand(ValueId(1)),
                                index: Operand(ValueId(2)),
                                value: Operand(ValueId(3)),
                            },
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(4)),
                            kind: InstKind::ConstInt(0),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(5)),
                            kind: InstKind::ArrayListLoad {
                                receiver: Operand(ValueId(1)),
                                index: Operand(ValueId(4)),
                            },
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(6)),
                            kind: InstKind::ArrayListLoad {
                                receiver: Operand(ValueId(1)),
                                index: Operand(ValueId(2)),
                            },
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(7)),
                            kind: InstKind::ArrayLength(Operand(ValueId(1))),
                            ty: IrType::Int(IntWidth::U64),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(8)),
                            kind: InstKind::IntCast(Operand(ValueId(7))),
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(9)),
                            kind: InstKind::Binary {
                                op: BinaryOp::Add,
                                left: Operand(ValueId(5)),
                                right: Operand(ValueId(6)),
                            },
                            ty: int_type,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(10)),
                            kind: InstKind::Binary {
                                op: BinaryOp::Add,
                                left: Operand(ValueId(9)),
                                right: Operand(ValueId(8)),
                            },
                            ty: int_type,
                            span,
                        },
                    ],
                    terminator: Some(Terminator::Return(Some(Operand(ValueId(10))))),
                }],
                entry: BlockId(0),
                span,
                gc_roots: Vec::new(),
            }],
            array_types: vec![IrType::Int(IntWidth::I32)],
            ..Module::default()
        };
        let mut interpreter = Interpreter::new(&module, InterpreterLimits::default()).unwrap();
        assert_eq!(
            interpreter.run_main(),
            ExecutionResult::Completed {
                value: Some(int(IntWidth::I32, 56))
            }
        );
    }
}
