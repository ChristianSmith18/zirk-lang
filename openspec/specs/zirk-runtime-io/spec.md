# zirk-runtime-io

## Purpose

Defines the C ABI contract of the runtime for standard output and for the representation of `String`.

The layout of a `String` is private to the runtime, which is what allows adding grapheme indexing later without touching the compiler.

## Requirements

### Requirement: Opaque representation of String

The runtime SHALL expose `String` as an opaque handle whose layout is private,
per `docs/decisions/ADR-005-representacion-string.md`.

The handle SHALL also be the observable identity of the `String`: it is what
`is` compares. This is what allows grapheme indexing, normalization, and any
internal cache to live entirely within the runtime, without the compiler
having to know about them.

#### Scenario: Opacity for the compiler
- **WHEN** codegen manipulates a `String` value
- **THEN** it treats it as an opaque handle
- **AND** it does NOT inspect or assume its internal representation

#### Scenario: Construction from a literal
- **WHEN** the generated code materializes a string literal
- **THEN** it invokes the runtime function that constructs a `String` from UTF-8 bytes and their length

#### Scenario: The handle is the identity
- **WHEN** two `String` values come from the same handle
- **THEN** they are identical for the `is` operator
- **AND** two distinct handles are NOT identical even if their content matches

### Requirement: Standard output

Native runtime execution SHALL expose stable `extern "C"` functions that write a `String` to standard output with the requested newline behavior. Interpreted execution SHALL emit the same logical UTF-8 text and newline sequence through an injected bounded host output sink without requiring a native process stream.

#### Scenario: Printing a string
- **WHEN** a native program invokes the print function with a `String`
- **THEN** the content appears on native standard output with the requested newline behavior

#### Scenario: Print through the interpreter
- **WHEN** an interpreted program invokes `stdout.print` or `stdout.println`
- **THEN** the content is emitted to the host output sink in call order
- **AND** `println` appends exactly one newline

#### Scenario: Non-ASCII content
- **WHEN** either execution engine prints Unicode characters outside ASCII
- **THEN** the host receives them correctly encoded in UTF-8

#### Scenario: Flushed before terminating
- **WHEN** a native program terminates
- **THEN** standard output is flushed before the process ends

#### Scenario: Complete interpreted output before returning
- **WHEN** an interpreted program terminates
- **THEN** all accepted output events are present in the structured execution result before it is returned

#### Scenario: Bound interpreted output
- **WHEN** an interpreted output event would exceed the configured host sink limit
- **THEN** the interpreter reports output truncation and a controlled limit failure
- **AND** it does not allocate or emit bytes beyond that limit

### Requirement: Stability of runtime symbols

Every runtime symbol intended for generated code SHALL be declared `extern "C"` with no mangling and a stable name.

They are compatibility surface: changing them breaks already-compiled binaries.

#### Scenario: Symbols without mangling
- **WHEN** the produced static library is inspected
- **THEN** the symbols intended for generated code appear under their literal name

### Requirement: No dependency on the Zirk stdlib

The runtime SHALL not parse, discover, or load Zirk standard-library source.
The compiler MAY map only a fully-qualified declaration from the compiler-owned
`std.io` registry to a stable runtime operation. A user package declaration
with the same name or signature SHALL remain an ordinary Zirk declaration.

#### Scenario: Standard stream declaration
- **WHEN** a program imports and calls `stdout.println` from `std.io`
- **THEN** the compiler maps that identified declaration to its output runtime operation

#### Scenario: External same-name declaration
- **WHEN** an external package publishes `stdout.println` with the same signature
- **THEN** it does not acquire a standard runtime binding

#### Scenario: Program without imports
- **WHEN** a program calls `stdout.println` without importing `stdout` from `std.io`
- **THEN** checking reports that `stdout` is unavailable in that file

### Requirement: Content equality indifferent to normalization

The runtime's equality function SHALL compare the content of two `String`
values treating two canonically equivalent sequences as equal, even if their
bytes differ.

The comparison SHALL resolve without allocating memory in the common case:
the same handle, or identical bytes, decide the result immediately.

#### Scenario: Different normalization forms
- **WHEN** a string in NFC and another in NFD with the same perceived content are compared
- **THEN** equality returns true

#### Scenario: Different content
- **WHEN** two strings whose perceived content differs are compared
- **THEN** equality returns false

#### Scenario: Fast path
- **WHEN** two handles match, or their bytes are identical
- **THEN** the result is decided without normalizing or allocating memory

### Requirement: Literals normalized at compile time

The compiler SHALL emit string literals in canonical form, so that comparison
between literals resolves by byte comparison.

Normalizing once at compile time is what keeps the runtime equality rule cheap.

#### Scenario: Literal in decomposed form in the source
- **WHEN** a string literal appears in the source in decomposed form
- **THEN** the runtime receives it already in canonical form

#### Scenario: Comparison between literals
- **WHEN** two literals with the same perceived content are compared
- **THEN** the comparison resolves by bytes, without normalizing at runtime

### Requirement: Hash coherent with equality

When the runtime exposes the hash of a `String`, it SHALL be derived from its
canonical form.

Two strings equal according to the equality function SHALL always produce the
same hash.

#### Scenario: Hash of equivalent forms
- **WHEN** the hash of a string in NFC and that of its equivalent in NFD are computed
- **THEN** both hashes match

### Requirement: Runtime preserves typed failure and cleanup
The runtime SHALL represent implicit safety failures as typed `RuntimeError` exceptions, preserve exact rethrows, lazily materialize structured traces, redact secrets, attach suppressed cleanup failures, and execute managed resource close exactly once on every exit path.

#### Scenario: Exception and close both fail
- **WHEN** a throwable is propagating and resource close reports an error
- **THEN** the throwable remains primary and the close failure appears in its suppressed list

### Requirement: Environment and external I/O enforce effective grants
Runtime I/O, environment, secret, network, process, and shell operations SHALL validate the signed effective policy and normalized dynamic target before performing an external effect. Denial SHALL produce the operation's typed permission error and SHALL NOT prompt or modify project files.

#### Scenario: Redirect leaves network scope
- **WHEN** an authorized HTTP request redirects to an unauthorized host
- **THEN** the redirect is denied before connecting to the new host

### Requirement: System shutdown events preserve root ownership
The runtime SHALL translate supported termination signals into bounded,
task-aware shutdown events without executing arbitrary Zirk code inside native
signal handlers. The root supervisor SHALL retain shutdown ownership; a second
termination signal or exhausted shutdown bound SHALL force controlled exit.

#### Scenario: Application observes an interrupt
- **WHEN** the runtime receives the platform equivalent of `SIGINT`
- **THEN** observers may receive `Signal.Interrupt`
- **AND** no observer can prevent the root supervisor from initiating shutdown

### Requirement: Task-aware I/O and blocking isolation
Runtime I/O SHALL suspend tasks without occupying scheduler threads where the platform permits, SHALL support cancellation-safe cleanup, and SHALL route explicitly wrapped legacy blocking work to a separate pool.

#### Scenario: Task waits for file or socket readiness
- **WHEN** an authorized asynchronous I/O operation cannot complete immediately
- **THEN** the task suspends and the scheduler thread remains available for other work

### Requirement: Network transports preserve boundaries and backpressure
The runtime SHALL integrate DNS, TCP, UDP, local sockets, and TLS with the task
reactor. TCP SHALL expose ordered bytes, partial writes, backpressured complete
writes, and half-close; UDP SHALL preserve datagram source, size, and boundaries
without silent truncation. DNS caches, connection attempts, accept queues,
buffers, datagram queues, and TLS handshakes SHALL remain bounded.

#### Scenario: Datagram exceeds configured receive bound
- **WHEN** an incoming UDP datagram does not fit the selected receive policy
- **THEN** the operation reports the required size or a typed limit error
- **AND** it does not return silently truncated application data

### Requirement: TLS defaults remain authenticated
Standard TLS SHALL default to TLS 1.3, trusted certificate-chain and host-name
validation, SNI, and safe algorithms. TLS 1.2 SHALL require explicit
interoperability configuration, older versions SHALL be forbidden, and no
general-purpose option SHALL disable certificate validation.

#### Scenario: Peer certificate does not match the requested host
- **WHEN** a TLS client receives a valid certificate for a different host
- **THEN** the handshake returns a typed `TLSError`
- **AND** no application bytes are exchanged

### Requirement: HTTP preserves strict framing and bounded body flow
The runtime SHALL implement HTTP/1.1 and HTTP/2 through one task-aware,
backpressured request/response contract. It SHALL reject ambiguous or invalid
message framing and field syntax, SHALL bound encoded and decoded bodies,
headers, queues, streams, retries, and redirects, and SHALL consume each
response body at most once. HTTP QUERY SHALL preserve its safe, idempotent,
content-bearing semantics.

#### Scenario: HTTP/1.1 message has conflicting framing
- **WHEN** a message contains framing whose interpretation could differ between recipients
- **THEN** the runtime rejects the message as `HTTPProtocolError`
- **AND** closes the connection when safe reuse cannot be proven

#### Scenario: Response is read twice
- **WHEN** one response body has already been consumed as text, bytes, JSON, or a stream
- **THEN** another body access returns `HTTPBodyConsumedError`

### Requirement: HTTP server work is structurally owned
Each HTTP handler and its body streams SHALL belong to the server request scope.
A peer disconnect SHALL cancel that request scope, and graceful shutdown SHALL
stop acceptance, wait only for its bounded grace period, then cancel remaining
handlers without detaching work or creating one operating-system thread per
request.

#### Scenario: Peer disconnects during streaming response
- **WHEN** a client disconnects while a handler is producing a body
- **THEN** the body producer and request-owned descendant tasks are canceled
- **AND** their resources are closed before the request scope ends

### Requirement: Irreversible effects preserve permission enforcement
External effects issued from an unsafe commit region MUST still satisfy normal project permissions and operating-system validation.

#### Scenario: Unsafe network send lacks permission
- **WHEN** an unsafe commit region attempts a network send outside the approved scope
- **THEN** permission enforcement rejects it before the external effect occurs
