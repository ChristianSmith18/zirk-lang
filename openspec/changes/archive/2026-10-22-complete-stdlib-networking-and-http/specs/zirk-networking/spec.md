## ADDED Requirements

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
