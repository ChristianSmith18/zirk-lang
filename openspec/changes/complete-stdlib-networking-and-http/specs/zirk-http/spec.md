## ADDED Requirements

### Requirement: HTTP/3 interoperates over authenticated QUIC

Native `std.http` clients and servers SHALL support HTTP/3 over QUIC version 1, with TLS 1.3, `h3` ALPN, QPACK, independent stream flow control, bounded connection and stream state, and the HTTP semantics required by [RFC 9114](https://www.rfc-editor.org/rfc/rfc9114.html). QUIC transport SHALL follow [RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html). The API SHALL preserve the existing request, response, body, permission, redirect, cookie, cache, proxy, timeout, cancellation, and error contracts across HTTP/1.1, HTTP/2, and HTTP/3. HTTP/3 SHALL use the HTTPS origin identity and exact connect/listen authority; DNS answers and every selected or migrated address SHALL be checked before use. QUIC 0-RTT SHALL be disabled by default and SHALL NOT replay a request that is unsafe or not explicitly replayable. Clients in automatic mode SHALL fall back to HTTP/2 or HTTP/1.1 when QUIC is unavailable, before retrying a request only when the request's replay policy permits it.

#### Scenario: Negotiate HTTP/3 with an approved peer
- **WHEN** a client with exact origin authority connects to a peer advertising HTTP/3
- **THEN** it validates the peer identity and exchanges bounded requests over HTTP/3

#### Scenario: Fall back when UDP or QUIC is unavailable
- **WHEN** automatic protocol selection cannot establish an approved QUIC connection
- **THEN** the client may use HTTP/2 or HTTP/1.1 under the same origin grant and TLS identity rules
- **AND** it does not replay a non-replayable request body

#### Scenario: Reject an unapproved QUIC path
- **WHEN** DNS rebinding or QUIC path migration selects an address outside the effective origin grant
- **THEN** the client rejects that candidate before sending application data to it

#### Scenario: Keep early data disabled for unsafe requests
- **WHEN** a caller requests 0-RTT for a non-idempotent or non-replayable operation
- **THEN** the client rejects early transmission and returns a typed policy error

#### Scenario: Bound a peer's HTTP/3 state
- **WHEN** a peer exceeds configured header, QPACK, stream, connection, datagram, or body limits
- **THEN** the operation terminates with a typed protocol or limit error without unbounded allocation

### Requirement: HTTP servers admit connections concurrently within explicit bounds

HTTP servers SHALL support separately configured limits for accepted connections, active requests, and concurrent handler tasks across HTTP/1.1, HTTP/2, and HTTP/3. Admission queues and response producers SHALL be bounded. Each request task SHALL retain owner-task resource rules and be independently cancellable; a peer disconnect SHALL cancel only its request work. Server shutdown SHALL expose an explicit owner-controlled signal, stop admission, drain admitted requests for a bounded grace period, cancel remaining cooperative work, and release TCP, TLS, UDP, and QUIC resources deterministically. CPU-only handlers remain cooperatively cancellable and SHALL NOT be described as preempted.

The public lifecycle API SHALL provide a shareable `HTTPServerControl` whose
`request_shutdown()` operation is idempotent and an `HTTPServerOptions` value
that validates the independent connection, handshake, request, handler,
per-connection stream, queued-response-byte, and drain-deadline limits.
TCP, TLS, and HTTP/3 serving entry points SHALL accept this common control and
options contract. Requesting shutdown SHALL wake a pending listener wait; it
SHALL NOT require abrupt cancellation of the server task. Repeated requests
SHALL return successfully and SHALL NOT enqueue duplicate shutdown work.

#### Scenario: Serve independent connections concurrently
- **WHEN** multiple clients connect within configured admission limits
- **THEN** their independent requests can make progress concurrently without exceeding the connection or task bounds

#### Scenario: Apply backpressure at admission capacity
- **WHEN** connection or handler capacity is exhausted
- **THEN** the server applies the documented bounded wait or rejection policy without allocating an unbounded queue

#### Scenario: Disconnect one multiplexed request
- **WHEN** one HTTP/2 or HTTP/3 peer stream disconnects while sibling streams remain active
- **THEN** only the disconnected request's cooperatively suspended work is cancelled

#### Scenario: Request graceful shutdown
- **WHEN** the owner signals server shutdown
- **THEN** the server stops accepting new connections, drains within the configured bound, and releases all listener and active-request resources

#### Scenario: A handler exceeds its cooperative deadline
- **WHEN** a handler runs CPU-only code past its deadline
- **THEN** the runtime requests cancellation and reports that completion awaits a cancellation safe point rather than claiming hard preemption

### Requirement: WebSocket supports the negotiated HTTP/3 bootstrap

The existing WebSocket API SHALL support WebSocket over HTTP/3 using the HTTP/3 extended CONNECT mechanism defined by [RFC 9220](https://www.rfc-editor.org/rfc/rfc9220.html) when the peer and runtime negotiate the required capability. Origin, subprotocol, message size, close handshake, network permissions, and task ownership SHALL remain equivalent to the existing HTTP/1.1 and HTTP/2 APIs. Unsupported peers SHALL fail or use an explicitly allowed lower HTTP version without weakening origin checks.

#### Scenario: Establish a WebSocket session over HTTP/3
- **WHEN** a client and server support the standardized HTTP/3 WebSocket bootstrap and the origin is authorized
- **THEN** they exchange the same bounded typed messages and close handshake as other supported transports

#### Scenario: Peer does not support the WebSocket bootstrap
- **WHEN** the peer does not advertise the required HTTP/3 capability
- **THEN** the client returns a typed unsupported-protocol error or uses an explicitly permitted fallback

### Requirement: Browser HTTP requires a caller-provided network capability

Browser-hosted Zirk SHALL support bounded outbound HTTP requests through an explicit embedding-host capability and origin allowlist. The browser transport SHALL use browser-managed request, TLS, CORS, and credential rules; it SHALL NOT claim control over the negotiated HTTP version. The capability SHALL be passed to a specific execution and SHALL NOT become ambient authority for later runs. Browser execution SHALL NOT expose raw TCP, UDP, TLS sockets, local IPC, interface inspection, or listening-server APIs; those operations SHALL return explicit browser-capability errors.

#### Scenario: Execute HTTP with explicit browser authority
- **WHEN** an embedding host supplies a browser-network capability scoped to an allowed HTTPS origin
- **THEN** a Zirk HTTP request to that origin uses the browser transport and returns bounded typed response data

#### Scenario: Execute without a browser capability
- **WHEN** browser code attempts an HTTP request without an injected capability
- **THEN** it receives a typed permission or unsupported-capability error and makes no network request

#### Scenario: Attempt a native socket operation in a browser
- **WHEN** browser code imports or calls a TCP, UDP, TLS-socket, local-socket, interface, or server operation
- **THEN** the toolchain reports the documented target capability error before pretending the operation succeeded

#### Scenario: Browser policy blocks a request
- **WHEN** browser CORS, mixed-content, user-agent, or origin policy prevents a request
- **THEN** the operation returns a typed browser transport failure without bypassing the browser security policy

### Requirement: Cross-protocol behavior is verified against independent peers

The HTTP implementation SHALL verify common request and response semantics, permission checks, limits, cancellation, redirects, streaming backpressure, proxy behavior, and resource cleanup against independent peers for every implemented protocol. The status documentation SHALL state which protocol and target combinations were executed natively and distinguish those results from compilation or emulation evidence.

#### Scenario: Run cross-protocol conformance on native targets
- **WHEN** the native conformance workflow runs on each supported target
- **THEN** it exercises applicable HTTP/1.1, HTTP/2, and HTTP/3 paths against independent peers and records per-target results

#### Scenario: A feature lacks a target execution result
- **WHEN** an API or protocol only compiles but has no native runtime evidence on a declared target
- **THEN** documentation marks that target execution as unverified rather than completed

### Requirement: Response decoding, cookie storage, and redirect history are bounded

HTTP clients SHALL decode supported stacked content codings in wire order under explicit encoded-size, decoded-size, expansion-ratio, coding-depth, and cancellation limits. A client MAY use a caller-owned cookie store whose lifetime and isolation are explicit; the default SHALL remain non-persistent, and the HTTP package SHALL NOT silently write cookies to disk or claim encryption. Redirect-aware responses MAY expose a bounded hop history containing status, method, and normalized destination metadata, but SHALL redact credentials, sensitive headers, cookie values, and secret-bearing query data at every formatter and structured output boundary. History and cookie handling SHALL preserve existing origin grants and redirect policy.

#### Scenario: Decode stacked content codings within bounds
- **WHEN** a response declares multiple supported content codings and the decoded body stays within configured bounds
- **THEN** the client decodes them in reverse application order and exposes the final body
- **AND** it returns a typed limit or unsupported-coding error before exceeding a configured bound

#### Scenario: Keep cookies in a caller-owned store
- **WHEN** a client is configured with an explicit in-memory cookie store
- **THEN** cookies remain scoped by the existing domain, path, secure, expiry, and origin rules
- **AND** closing or dropping the client follows the documented store ownership without persisting values

#### Scenario: Inspect redirect history safely
- **WHEN** a request follows redirects within its configured hop limit
- **THEN** its optional history records the bounded method, status, and destination metadata for each hop
- **AND** rendering or serializing the history never exposes credential headers, cookie values, or secret query values

#### Scenario: Reject excessive coding depth or expansion
- **WHEN** a peer exceeds the configured content-coding depth, decoded-size, or expansion-ratio bound
- **THEN** the client stops decoding and returns a typed limit error without returning a partial success body
