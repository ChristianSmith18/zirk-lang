# zirk-networking Specification

## Purpose

Defines the native DNS, TCP, TLS, and task-aware transport foundation required by Zirk's HTTP client and server. This capability is limited to the stream transports and host resolution needed by the full HTTP contract.

## Requirements

### Requirement: HTTP transport APIs are published through std.net

The bundled `std.net` package SHALL publish validated host/address values, DNS resolution, TCP client streams, TCP listeners, and authenticated TLS streams needed by `std.http`. Public declarations SHALL be ordinary Zirk package APIs; compiler-owned host bindings SHALL remain tied to official package provenance.

#### Scenario: Import the HTTP transport foundation
- **WHEN** a program imports the TCP, TLS, and address APIs from `std.net`
- **THEN** the names resolve from the toolchain-bundled package on every supported native target
- **AND** external declarations with the same names do not gain host bindings

### Requirement: DNS and connect authorization are checked at the effect boundary

An outbound connection SHALL require effective `network.connect.origins` authority. The runtime SHALL validate the requested origin, DNS answer, and every candidate address immediately before each connect attempt. DNS caches SHALL be bounded and SHALL NOT grant authority.

The connect boundary SHALL reject IPv6 socket candidates with a nonzero
interface scope ID because network origin grants do not encode interface
authority.
Public IP-address and host-port parsers SHALL reject IPv6 zone identifiers;
the corresponding public endpoint types do not express an interface scope.

#### Scenario: DNS changes to a forbidden address
- **WHEN** an authorized hostname resolves to or is rebound to an address outside the effective grant
- **THEN** the runtime denies the connection before opening the socket
- **AND** it returns a typed network permission error without exposing unrelated host state

#### Scenario: A candidate selects an ungranted IPv6 interface
- **WHEN** a connection candidate contains a nonzero IPv6 scope ID
- **THEN** the runtime rejects it before creating a socket
- **AND** an exact origin grant does not implicitly authorize interface-specific routing

#### Scenario: A public endpoint includes an IPv6 zone identifier
- **WHEN** a caller parses an IPv6 literal or host-port endpoint with a zone suffix
- **THEN** parsing returns the typed invalid-address error before DNS or network access

### Requirement: Listening requires separate inbound authority

Binding a TCP listener SHALL require matching `network.listen.addresses` authority independent of outbound connect grants. Address validation and permission checks SHALL complete before the operating system accepts connections.

#### Scenario: Outbound authority cannot open a server
- **WHEN** a program has connect permission but no grant for its requested bind address
- **THEN** listener creation fails with a typed permission error before the socket listens

### Requirement: Network waits suspend tasks and preserve cancellation

Connect, accept, read, write, and TLS-handshake operations SHALL wait without blocking a Zirk scheduler thread. Buffers, pending connections, and queued operations SHALL be bounded. Cancellation SHALL wake the task, close or restore owned transport state exactly once, and preserve any bytes already committed to the host.

#### Scenario: A task waits for a connection
- **WHEN** `accept` has no pending connection
- **THEN** the calling task suspends and unrelated runnable tasks continue

#### Scenario: A pending read is cancelled
- **WHEN** structured cancellation reaches a task awaiting a socket read
- **THEN** the task resumes with a typed cancellation outcome and its owned stream follows resource cleanup rules

### Requirement: TCP streams preserve byte and backpressure semantics

A TCP stream SHALL preserve ordered bytes, distinguish peer EOF from transport errors, report partial writes, and provide a bounded write-all operation with backpressure. Explicit shutdown and idempotent close SHALL be available, and managed streams SHALL close through `using` on every exit path.

An owner-task TCP stream SHALL expose its numeric remote socket address through
`peer_address()` without performing DNS resolution or changing the stream's
authorization scope.

#### Scenario: A partial write occurs
- **WHEN** the host accepts fewer bytes than a write request contains
- **THEN** the operation reports the accepted byte count and does not claim the remaining bytes were written

#### Scenario: A complete write waits for capacity
- **WHEN** the bounded output queue is full during `write_all`
- **THEN** the task suspends until capacity or a typed failure is available
- **AND** memory use remains within the configured transport bound

#### Scenario: Read a TCP stream's peer address
- **WHEN** the owning task calls `TCPStream.peer_address()` on a connected or accepted stream
- **THEN** it receives the numeric remote socket address
- **AND** the query performs no DNS lookup and grants no additional network authority

### Requirement: TLS authenticates peers with secure defaults

TLS client and server streams SHALL use authenticated TLS 1.3 by default. Clients SHALL verify certificate chains, hostnames, and SNI; TLS 1.2 SHALL require explicit interoperability configuration; obsolete versions and insecure verification bypasses SHALL NOT be enabled by defaults. Server identity material SHALL be explicit and validated.

#### Scenario: A certificate hostname does not match
- **WHEN** a client connects to a host whose certificate identity does not match the requested server name
- **THEN** the handshake fails with a typed TLS error and no application bytes are sent

### Requirement: Native transport support matches Zirk's declared target set

The HTTP transport foundation SHALL compile and execute with equivalent public semantics on macOS aarch64, Linux x86_64, Linux aarch64, and Windows x86_64. Platform-specific readiness mechanisms MAY differ, but unsupported behavior SHALL fail explicitly rather than silently falling back to blocking execution on the scheduler thread.

#### Scenario: Run the transport conformance suite on a supported target
- **WHEN** the transport conformance suite runs natively on any declared target
- **THEN** it verifies connect/listen, EOF, partial and complete writes, cancellation, TLS validation, resource cleanup, and permission denial on that host
