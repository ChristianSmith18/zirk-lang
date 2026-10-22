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

### Requirement: UDP preserves datagram boundaries and requires scoped authority

The bundled native `std.net` API SHALL provide task-aware connected and unconnected UDP sockets on every supported native target. Each send SHALL preserve one datagram boundary; each receive SHALL return its complete payload and peer address or a typed size error, and SHALL NOT silently truncate. Binding, connected destinations, and unconnected destinations SHALL require the matching effective network authority before socket effects. Broadcast and multicast SHALL be disabled unless explicitly configured and separately authorized. Socket counts, queued datagrams, datagram sizes, and memberships SHALL be bounded.

#### Scenario: Receive a complete datagram
- **WHEN** an authorized UDP socket receives a datagram within the configured maximum
- **THEN** it returns the complete payload and source address as one value

#### Scenario: Reject a datagram larger than the receive limit
- **WHEN** a datagram exceeds the caller's configured receive maximum
- **THEN** the operation returns a typed size error and does not return truncated bytes as a complete datagram

#### Scenario: Deny an unapproved datagram destination
- **WHEN** a socket sends to a destination outside the effective datagram grant
- **THEN** it returns permission denial before sending any packet

#### Scenario: Require explicit multicast authority
- **WHEN** a program joins a multicast group or enables broadcast without the corresponding explicit authority
- **THEN** the operation fails before changing socket membership or sending a packet

### Requirement: Local IPC sockets have explicit paths, ownership, and permissions

The bundled native `std.net` API SHALL provide task-aware local byte streams with validated platform-specific endpoint names and distinct connect and listen permissions. It SHALL preserve ordered bytes, bounded reads and writes, cancellation, EOF, idempotent close, and owner-task cleanup. A listener SHALL NOT remove a pre-existing filesystem entry unless the caller explicitly selected an authorized stale-endpoint policy; endpoint cleanup SHALL never follow a symlink or escape its authorized path scope.

#### Scenario: Connect to an approved local endpoint
- **WHEN** a program connects to a local endpoint within its effective local-connect grant
- **THEN** it receives an owner-task stream with the same bounded byte and cancellation semantics as TCP

#### Scenario: Outbound network permission does not authorize local listening
- **WHEN** a program has outbound network authority but lacks local-listen authority
- **THEN** binding a local endpoint fails before creating or replacing a host entry

#### Scenario: Cleanup cannot remove an unrelated endpoint
- **WHEN** a listener closes after another process replaces or redirects its endpoint path
- **THEN** cleanup leaves the unrelated filesystem entry untouched

### Requirement: Network interface inspection is bounded and separately authorized

`std.net` SHALL expose an immutable snapshot of interface names, flags, addresses, prefixes, and applicable scope identifiers through an explicit interface-inspection capability. The API SHALL return no process environment, traffic, or packet contents; SHALL bound the number and size of records; and SHALL not imply permission to bind, connect, or select an interface. Snapshot failures and partial platform support SHALL be returned as typed outcomes rather than silently returning an empty list.

#### Scenario: List interfaces with authority
- **WHEN** an application with interface-inspection authority requests a snapshot
- **THEN** it receives a bounded immutable list of the visible interface metadata

#### Scenario: Deny interface discovery without authority
- **WHEN** an application requests interface metadata without the explicit capability
- **THEN** the API returns permission denial without revealing whether interfaces exist

#### Scenario: Interface inspection does not grant network access
- **WHEN** an application uses an address from an interface snapshot without a matching listen or connect grant
- **THEN** the corresponding socket operation remains denied

### Requirement: TLS configuration remains authenticated and explicit

Native TLS APIs SHALL support a validated configuration for protocol bounds, trust roots, ALPN, and optional client identity, and SHALL allow a TLS stream to wrap an already-owned TCP stream without duplicating or detaching its resource ownership. Server-side client-certificate authentication SHALL be opt-in and expose only validated peer identity metadata. TLS 1.3 and hostname verification remain secure defaults; enabling TLS 1.2, custom roots, client certificates, or additional ALPN identifiers SHALL be explicit. Certificate verification bypasses, public key logging, and secret-bearing debug output SHALL remain unavailable.

#### Scenario: Authenticate a client with mutual TLS
- **WHEN** a client presents an explicitly configured certificate and key to a server requiring client authentication
- **THEN** the handshake succeeds only if both server and client identities validate

#### Scenario: Wrap an existing TCP stream
- **WHEN** an owner task upgrades its authorized TCP stream with a valid TLS configuration
- **THEN** TLS uses that stream and preserves its owner, cancellation, and close lifecycle

#### Scenario: Reject an untrusted peer
- **WHEN** a peer certificate or hostname fails validation
- **THEN** the TLS handshake returns a typed error before exposing application bytes

#### Scenario: Keep TLS secrets out of diagnostics
- **WHEN** TLS configuration contains a private key or client credential and an error is formatted
- **THEN** the diagnostic reveals neither credential bytes nor key material

### Requirement: Native datagram and local transports conform on every supported target

UDP, local IPC, interface snapshots, and the extended TLS API SHALL provide equivalent documented semantics on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. Platform-native mechanisms MAY differ. Any unsupported operation SHALL return an explicit typed error, and waiting SHALL suspend the owning Zirk task rather than block a scheduler thread.

#### Scenario: Run native network conformance
- **WHEN** the network conformance suite runs on a declared native target
- **THEN** it verifies datagram boundaries, permission denial, cancellation, local endpoint cleanup, interface error reporting, TLS identity, and owner-task resource release on that host

### Requirement: TLS metadata absence is distinct from TLS failure
Public TLS metadata APIs SHALL NOT use `Result<T?, TLSError>` to conflate a successful absence with a TLS operation failure. They SHALL return a Result whose non-null success payload explicitly represents metadata presence or absence; authentication, transport, and host failures SHALL remain typed `Error(TLSError)` outcomes.

#### Scenario: The TLS peer negotiated an ALPN protocol
- **WHEN** `TLSStream.negotiated_alpn()` has a selected protocol
- **THEN** it returns `Ok(TLSNegotiatedALPN.Negotiated(protocol))`

#### Scenario: The TLS peer selected no ALPN protocol
- **WHEN** the TLS handshake completes without a negotiated ALPN identifier
- **THEN** it returns `Ok(TLSNegotiatedALPN.NotNegotiated)`

#### Scenario: TLS metadata lookup fails
- **WHEN** reading TLS metadata fails because the stream or host operation is invalid
- **THEN** it returns `Error(TLSError)` distinct from absent metadata

#### Scenario: The TLS peer certificate fingerprint is unavailable
- **WHEN** the TLS role has no peer certificate fingerprint metadata
- **THEN** `peer_certificate_sha256()` returns `Ok(TLSPeerCertificateFingerprint.Unavailable)`

#### Scenario: The TLS peer certificate fingerprint is available
- **WHEN** the authenticated peer certificate has a SHA-256 fingerprint
- **THEN** `peer_certificate_sha256()` returns `Ok(TLSPeerCertificateFingerprint.Sha256(fingerprint))`
