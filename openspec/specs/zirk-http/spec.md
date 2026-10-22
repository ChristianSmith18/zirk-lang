# zirk-http Specification

## Purpose

Defines Zirk's bundled HTTP client and server APIs, including interoperable HTTP/1.1 and HTTP/2, bounded streaming, secure protocol transitions, and the standardized QUERY method.

## Requirements

### Requirement: The standard package exposes the documented HTTP client and server

`std.http` SHALL publish the documented URL, method, status, header, request, response, client, server, routing, middleware, cookie, SSE, and WebSocket APIs through ordinary Zirk imports. One-shot and reusable client APIs SHALL return typed results, and server APIs SHALL expose structured request handling without requiring a third-party Zirk package.

#### Scenario: Import and issue a request
- **WHEN** a program imports `HTTPClient`, `HTTPMethod`, and `URL` from `std.http` and sends a request
- **THEN** the request is compiled against the bundled public API and returns `Ok(HTTPResponse)` or `Error(HTTPError)`

#### Scenario: Bind and route a server
- **WHEN** an authorized program binds an `HTTPServer` and registers a route
- **THEN** it accepts requests through the documented handler API and produces a typed response

### Requirement: HTTP QUERY follows RFC 10008

The client and server SHALL implement the HTTP `QUERY` method as specified by [RFC 10008](https://www.rfc-editor.org/rfc/rfc10008.html). QUERY SHALL be safe and idempotent, its request content SHALL describe the query operation, and every QUERY request SHALL carry exactly one valid, concrete, consistent `Content-Type`, including requests with empty content, as RFC 10008 requires. Wildcard media ranges such as `application/*` and `*/*` SHALL be rejected as `Content-Type`; they do not identify a representation format. The server SHALL expose supported query media types through `Accept-Query` when configured. Automatic retry SHALL be allowed only where request replayability and RFC semantics permit it; QUERY SHALL NOT be treated as permission to mutate target-resource state.

#### Scenario: Send a query with structured content
- **WHEN** a client sends QUERY with a body and matching content type
- **THEN** the server receives that body as query content and returns its query result
- **AND** the request is represented as safe and idempotent for policy decisions

#### Scenario: Reject missing query content type
- **WHEN** a server receives QUERY with or without content and no valid consistent `Content-Type`
- **THEN** it rejects the request with the protocol-appropriate client error before invoking the route handler

#### Scenario: Reject wildcard query content types
- **WHEN** a client or server validates a QUERY with a wildcard `Content-Type` media range
- **THEN** it returns a typed validation error or protocol-appropriate 4xx response before transport or route dispatch

#### Scenario: Discover accepted query formats
- **WHEN** a resource advertises its supported query formats
- **THEN** the response exposes them through `Accept-Query` and the client can inspect the field without guessing a format

#### Scenario: Validate Accept-Query media ranges
- **WHEN** a response advertises `Accept-Query` using RFC 9651 Structured Fields
- **THEN** the validator accepts concrete media ranges, `type/*`, and `*/*`
- **AND** it rejects unsupported wildcard forms such as `*/json`

#### Scenario: Follow a QUERY redirect
- **WHEN** a caller opts into bounded QUERY redirect following
- **THEN** a `303 See Other` resolves its single `Location` against the current URL and continues with GET
- **AND** `301`, `302`, `307`, and `308` preserve QUERY, its content type, and its buffered request content
- **AND** every hop revalidates the destination URL, effective network grant, and TLS identity

### Requirement: URLs and headers are validated without losing significant data

URL parsing and immutable builders SHALL preserve significant escaping, distinguish URI components, reject malformed or forbidden credentials, and resolve relative references according to the documented URI rules. Header names and values SHALL be validated against HTTP field syntax, preserve permitted repeated fields, and reject control characters and invalid framing combinations before transmission or handler dispatch.

#### Scenario: A malformed URL is parsed
- **WHEN** a caller parses an invalid URL or a URL forbidden by the selected credential policy
- **THEN** parsing returns a typed URL error without opening a connection

#### Scenario: Conflicting message framing is received
- **WHEN** an HTTP parser detects ambiguous or conflicting message framing
- **THEN** it rejects the request or response before exposing the body to application code

### Requirement: Clients implement bounded requests and response consumption

HTTP clients SHALL support the documented standard and extension methods, headers, typed bodies, reusable clients, connection pooling, cookies, redirect policies, and response-body consumption. Ordinary request APIs SHALL return `HTTPError.UnsupportedMethod` for `CONNECT` before permission lookup or transport setup because a successful CONNECT response switches the transport into tunnel mode and cannot be represented by `HTTPResponse`. The dedicated proxy tunnel and WebSocket APIs SHALL own their CONNECT flows.

#### Scenario: An ordinary request attempts CONNECT
- **WHEN** a caller sends exact method token `CONNECT` through a buffered, cached, secret-header, reusable-client, or streaming request API
- **THEN** the call returns `HTTPError.UnsupportedMethod` before permission lookup or network I/O
- **AND** dedicated proxy and WebSocket tunnel APIs retain their managed CONNECT behavior

#### Scenario: A named request uses an invalid method token
- **WHEN** a caller supplies an extension method that is not a valid HTTP token
- **THEN** buffered, reusable-client, and streaming request APIs return `HTTPError.InvalidArgument`
- **AND** they do so before permission lookup, producer invocation, or network I/O

Pool entries SHALL be bounded and scoped to the correct origin and TLS identity. Request and response bodies SHALL support bounded streaming and backpressure. Response bodies SHALL be consumed or closed exactly once.

#### Scenario: Reject invalid response chunk bounds without consuming the body
- **WHEN** a caller requests a response chunk with a size of zero or greater than 65,536 bytes
- **THEN** the call returns `HTTPStreamChunk.Error(HTTPError.InvalidArgument)`
- **AND** the response stream position remains unchanged

#### Scenario: Reject invalid temporary upload ranges without reading the file
- **WHEN** a caller requests a temporary upload chunk with a size of zero or greater than 65,536 bytes, or an offset beyond the upload size
- **THEN** the call returns `HTTPStreamChunk.Error(HTTPError.InvalidArgument)`
- **AND** the temporary upload file position and contents remain unchanged

One-shot and reusable-client typed-body APIs SHALL accept one closed
`HTTPBody` value with exactly one of `Text`, `Json`, `Bytes`, `Form`,
`Multipart`, or `Stream`. The selected variant SHALL determine the encoding and
framing behavior; a request SHALL NOT combine multiple body representations.
Convenience methods SHALL delegate to the same typed body path. The lower-level
byte-buffer request APIs SHALL represent only `HTTPBody.Bytes` semantics and
may accept explicit headers; they SHALL NOT perform another body encoding.
Streaming body headers and producer SHALL be represented together so framing
metadata cannot be detached from its producer.

The opt-in response cache SHALL include all request `Content-*` metadata in
its base identity in addition to method, target, and content bytes. It SHALL
honor response `Vary` fields and SHALL bypass cache lookup, storage, and
revalidation whenever any request field is marked sensitive.

#### Scenario: QUERY cache identity includes content metadata
- **WHEN** two QUERY requests have the same target and body but different `Content-Encoding` or `Content-Type`
- **THEN** the cache treats them as different requests

#### Scenario: A sensitive request bypasses the cache
- **WHEN** a safe request contains a field marked sensitive, including a custom credential name
- **THEN** the client sends the request normally without reading from or writing to the response cache

#### Scenario: Send one selected body representation
- **WHEN** a caller sends a request with one `HTTPBody` variant
- **THEN** the client encodes or streams only that representation
- **AND** it applies that variant's content type and framing rules

#### Scenario: Request body has one representation by construction
- **WHEN** a caller constructs an HTTP request body
- **THEN** the closed `HTTPBody` type permits exactly one representation
- **AND** the caller cannot supply text, JSON, form, multipart, bytes, and stream bodies as competing fields on that API

#### Scenario: Send an explicit byte buffer with custom headers
- **WHEN** a caller uses a lower-level byte-buffer request API
- **THEN** the payload is treated as the `Bytes` representation
- **AND** the supplied headers do not cause the runtime to encode a second body representation

#### Scenario: A repeated invalid cookie Path uses the request default
- **WHEN** an opted-in client receives a cookie with a missing, empty, or non-absolute `Path` attribute
- **THEN** it uses the request URL's default cookie path for that attribute
- **AND** the last repeated `Path` attribute determines the stored path, even when that attribute falls back to the default
- **AND** the cookie is not sent to paths outside that scope

#### Scenario: An oversized cookie scope attribute is rejected
- **WHEN** a `Path` or `Domain` attribute exceeds the per-attribute limit
- **THEN** the cookie is rejected with a typed size error
- **AND** an earlier, broader `Path` or `Domain` value is not retained as a fallback

#### Scenario: The final nonempty cookie Domain controls storage
- **WHEN** an opted-in client receives repeated `Domain` attributes
- **THEN** it validates the last nonempty value after parsing the attributes
- **AND** an invalid earlier value does not override a later valid value
- **AND** an invalid final value prevents the cookie from being stored

#### Scenario: Cookie Domain attributes use canonicalized IDNA host names
- **WHEN** an opted-in client receives a Unicode `Domain` attribute that canonicalizes to the request host's punycode form
- **THEN** it stores the cookie using the canonical ASCII domain
- **AND** it applies normal domain-match and public-suffix checks to that canonical value
- **AND** it rejects percent-escaped URL authority syntax instead of decoding it as a cookie domain

The explicit secret-header request sink SHALL accept `SecretString` without exposing its value as an ordinary Zirk `String`, require HTTPS and an exact approved origin, reject a duplicate field name, and never follow redirects.

HTTP proxy routing SHALL be explicit per client. The runtime SHALL authorize both the proxy origin and each destination origin independently before connecting or reusing a connection. HTTP requests SHALL use absolute-form targets through the proxy. HTTPS requests SHALL use CONNECT, validate the proxy TLS identity when the proxy uses HTTPS, and then validate TLS against the destination identity. Basic proxy credentials SHALL be supplied through `SecretString`, never URL user information; credentials SHALL be rejected for non-loopback cleartext HTTP proxies, and SHALL be sent only after the HTTPS proxy identity has been validated. Caller-supplied trust roots SHALL validate each HTTPS peer by its own hostname.

#### Scenario: An HTTP request uses an approved proxy
- **WHEN** a client sends an HTTP request through an explicitly configured proxy
- **THEN** it sends the absolute target URI and authorizes both the proxy and destination origins

#### Scenario: An HTTPS request uses a proxy tunnel
- **WHEN** a client sends an HTTPS request through an explicitly configured proxy
- **THEN** it establishes CONNECT and validates the tunneled TLS connection against the destination host

#### Scenario: A remote cleartext proxy is configured with credentials
- **WHEN** a caller supplies Basic credentials for a non-loopback HTTP proxy
- **THEN** client configuration fails before network I/O

#### Scenario: Authenticate a remote HTTPS proxy
- **WHEN** a caller configures Basic credentials for a remote HTTPS proxy
- **THEN** the runtime validates the proxy TLS identity before sending credentials
- **AND** it authorizes the proxy origin separately from the destination
- **AND** for HTTPS destinations, it validates a second TLS identity after CONNECT
- **AND** proxy authorization is never forwarded through the destination tunnel

#### Scenario: A response body is streamed
- **WHEN** a caller consumes a response body incrementally
- **THEN** the client applies backpressure and does not buffer beyond configured limits
- **AND** cancellation or early scope exit closes the underlying response resource once

#### Scenario: Redirect crosses an origin
- **WHEN** a redirect targets a different origin
- **THEN** the client revalidates network permission before connecting
- **AND** it removes `Authorization`, `Proxy-Authorization`, `Cookie`, `Cookie2`, `Set-Cookie`, `Api-Key`, `X-Api-Key`, `X-Auth-Token`, `X-Access-Token`, and `X-Secret-Key` headers while preserving ordinary headers
- **AND** it also removes custom fields explicitly marked with `HTTPHeaders.add_sensitive` or `HTTPHeaders.set_sensitive`

Reusable-client redirect following SHALL be explicit. `HTTPRedirectPolicy.Never`
is the default for ordinary requests; callers may select `SameOrigin` or
`Follow` for bounded redirect-aware requests. Every hop SHALL validate exactly
one `Location`, resolve it against the current URL, recheck the effective
network grant, and validate the destination TLS identity. A `303` SHALL switch
to GET except for HEAD and discard the request body and its content metadata.
The `301`, `302`, `307`, and `308` statuses SHALL preserve the method and
selected `HTTPBody` representation. Stream producers SHALL restart at chunk
index zero on replay and callers SHALL keep their output deterministic for each
index. Redirect loops and limits beyond 20 hops SHALL return
`HTTPError.RedirectLimit`; ambiguous or missing locations SHALL fail with a
typed error. `SameOrigin` SHALL return a cross-origin redirect response without
following it.

#### Scenario: Apply an explicit redirect policy to typed bodies
- **WHEN** a caller follows a redirect with a reusable client and one `HTTPBody` variant
- **THEN** `Never` performs only the initial request
- **AND** `SameOrigin` follows only targets with the same normalized scheme and authority
- **AND** `Follow` may follow cross-origin targets only after the target grant is revalidated
- **AND** a replayed `Stream` producer restarts at chunk index zero

#### Scenario: An unsafe request fails after transmission
- **WHEN** the connection fails after a non-idempotent request may have reached the server
- **THEN** the client does not retry it automatically

### Requirement: HTTP/1.1 and HTTP/2 are interoperable and bounded

Clients and servers SHALL implement the documented HTTP/1.1 and HTTP/2 behavior, including protocol negotiation where TLS is used, connection reuse, flow control, concurrent streams, and typed protocol errors. A peer's malformed or excessive input SHALL be rejected within configured header, body, frame, and concurrency limits. The implementation SHALL NOT silently claim HTTP/2 when only HTTP/1.1 was negotiated.

#### Scenario: Negotiate HTTP/2 over TLS
- **WHEN** client and server both advertise HTTP/2 through ALPN
- **THEN** they exchange requests and responses using HTTP/2 with bounded stream and connection flow control

#### Scenario: Peer exceeds a configured message limit
- **WHEN** a request or response exceeds a configured protocol limit
- **THEN** the operation returns a typed limit/protocol error and does not buffer the excess body

### Requirement: Servers enforce routing, resource, and lifecycle boundaries

Servers SHALL support the documented method-aware routes and request parameters, build validated responses, apply ordered middleware, and bound connection admission, request parsing, handler work, and response streaming. Listener authority SHALL be checked before bind. Cancellation and shutdown SHALL stop admission, cancel or join owned request work, and close connection resources deterministically.

#### Scenario: A request targets an unknown route or method
- **WHEN** no route matches the request path and method
- **THEN** the server returns the documented not-found or method-not-allowed response without invoking an unrelated handler

#### Scenario: A handler throws while serving a request
- **WHEN** a Zirk route or middleware handler throws an uncaught `Throwable`
- **THEN** the server sends a bounded `500 Internal Server Error` response with no exception details
- **AND** it completes that request's cleanup and continues serving other requests in the finite server scope
- **AND** a sibling HTTP/2 stream remains independently serviceable

#### Scenario: A handler returns a structured error
- **WHEN** a route or middleware registered through a result-returning handler API returns `Error`
- **THEN** the server sends a bounded `500 Internal Server Error` response with no error details
- **AND** it completes that request's cleanup and continues serving other requests in the finite server scope
- **AND** a sibling HTTP/2 stream remains independently serviceable

#### Scenario: Server shutdown begins
- **WHEN** a managed server is cancelled or shut down
- **THEN** it stops accepting new work, cleans up active request resources, and completes within the configured graceful-shutdown bound

#### Scenario: A long-lived server receives successive connections
- **WHEN** an application runs `HTTPRouter.serve_forever` or `serve_tls_forever`
- **THEN** it accepts and serves successive connections with at most one accepted connection active at a time
- **AND** cancelling the owner task stops admission and releases the listener

### Requirement: Multipart form parsing is bounded and binary safe

Servers SHALL parse `multipart/form-data` only when one valid content type and boundary are present. Parsing SHALL preserve part order, names, valid optional media metadata, and payload bytes; it SHALL reject malformed or duplicate part media headers and bound total body bytes, part count, boundary length, and per-part header bytes. A submitted filename SHALL remain untrusted metadata and SHALL NOT be interpreted as a filesystem path. Filename-bearing parts SHALL expose an opaque task-owned temporary upload resource with bounded offset reads, explicit idempotent close, and task-exit cleanup.

#### Scenario: A multipart upload contains binary data and a path-like filename
- **WHEN** an approved route parses a bounded multipart request
- **THEN** it receives the exact binary bytes and original filename metadata in wire order
- **AND** no file is created from the filename

#### Scenario: A managed multipart upload is closed with its task
- **WHEN** a route reads a filename-bearing part through its temporary upload resource
- **THEN** each read is bounded and uses an explicit offset
- **AND** explicit close removes the temporary resource idempotently
- **AND** task exit removes any remaining temporary resources

#### Scenario: Multipart framing or limits are invalid
- **WHEN** a body has malformed framing, invalid disposition metadata, or exceeds a configured bound
- **THEN** parsing returns a typed protocol or limit error without exposing partial parts

### Requirement: Multipart request bodies are bounded and unambiguous

The client SHALL provide a bounded in-memory multipart/form-data builder that preserves binary payloads and part order. It SHALL validate and safely quote names, filenames, media types, and the caller-supplied boundary; reject a boundary that appears as a delimiter in a part payload; and enforce the body and part-count limits before sending. The builder SHALL NOT create files or infer paths from filenames.

#### Scenario: Encode binary multipart parts
- **WHEN** a client builds parts with a valid non-colliding boundary
- **THEN** it emits the original binary payloads in order with validated disposition and media headers
- **AND** the HTTP helper sends the matching `Content-Type` through the normal origin-permission check

#### Scenario: Reject a colliding boundary or an excessive body
- **WHEN** the boundary occurs as a valid part delimiter or the encoded body exceeds its bound
- **THEN** encoding returns a typed protocol or limit error before any network request is opened

### Requirement: SSE and WebSocket preserve their protocol contracts

The server SHALL support bounded Server-Sent Events with ordered event framing, heartbeat policy, cancellation, and backpressure. WebSocket upgrades SHALL validate the handshake and origin policy, enforce frame/message limits, preserve control-frame and close-handshake semantics, and integrate stream cancellation and resource cleanup.

#### Scenario: Observe SSE heartbeat comments
- **WHEN** a client reads through SSE comment lines before receiving an event
- **THEN** `HTTPEventStream.heartbeat_count()` reports the cumulative number of comment lines observed
- **AND** heartbeat comments do not become application events

#### Scenario: Encode an SSE event for a streaming response
- **WHEN** `HTTPServerEvent.encode` receives multiline data and optional event, ID, or retry metadata
- **THEN** it returns one bounded UTF-8 event frame with fields in protocol order
- **AND** an empty event name uses the default `message` event while an empty ID resets the reconnect cursor
- **AND** line injection and configured line, event, and encoded-frame limits return typed errors

#### Scenario: Reconnect an opted-in SSE client
- **WHEN** `HTTPEventStream.connect_with_reconnect(url)` encounters an initial or subsequent transport/timeout failure, or observes EOF
- **THEN** the client waits using the latest valid SSE `retry` value, initially three seconds
- **AND** it sends the persistent non-empty `Last-Event-ID` value on the next request
- **AND** its timer wait remains cancellation-safe
- **AND** retry delays above 2,147,483,647 milliseconds are saturated to that limit
- **AND** a `204 No Content` response closes the stream without reconnecting
- **AND** permission, cancellation, protocol, and other typed failures are not retried

#### Scenario: Combine a proxy with caller-supplied TLS trust roots
- **WHEN** an HTTPS client uses an explicit proxy and caller-supplied DER trust roots
- **THEN** the runtime establishes the proxy tunnel under the proxy grant
- **AND** validates each HTTPS peer against the caller-supplied roots and its own hostname
- **AND** keeps proxy credentials out of the tunneled destination request

#### Scenario: A server produces a bounded streaming response
- **WHEN** an HTTP handler returns `HTTPResponse.from_stream` with a chunk producer
- **THEN** the producer runs on its owning Zirk task and an empty chunk ends the response
- **AND** HTTP/1.1 emits chunks incrementally while HTTP/2 applies bounded flow control
- **AND** EOF, producer failure, disconnect, or task cancellation releases the producer capture root

#### Scenario: A server emits configurable SSE heartbeats
- **WHEN** an HTTP handler returns `HTTPResponse.from_sse_stream` with a valid optional `heartbeat_ms`
- **THEN** the response uses the `text/event-stream` media type and emits bounded comment frames during producer inactivity at the configured interval
- **AND** omitting `heartbeat_ms` disables automatic heartbeat frames
- **AND** intervals below one second or above one hour return a typed error before producer registration
- **AND** producer completion, disconnect, or cancellation stops the heartbeat task without detaching work

#### Scenario: An SSE consumer disconnects
- **WHEN** a peer disconnects while an event stream is active
- **THEN** the server cancels the producer branch and releases its bounded queue and connection resources

#### Scenario: A WebSocket message exceeds its limit
- **WHEN** an incoming fragmented message exceeds the configured maximum
- **THEN** the server rejects or closes the connection with the documented protocol outcome without assembling an unbounded message

### Requirement: Security policy is rechecked across every HTTP transition

HTTP operations SHALL preserve network permission, TLS identity, URL validation, header confidentiality, and resource-limit guarantees across redirects, retries, proxies, DNS changes, connection reuse, cookies, SSE, and WebSocket upgrades. Permission denial and cancellation SHALL remain distinguishable in typed outcomes and SHALL NOT be converted into a successful fallback.

#### Scenario: A redirect resolves outside the permitted address scope
- **WHEN** an otherwise allowed redirect host resolves to a disallowed address
- **THEN** the runtime denies the connection before opening it and returns a typed permission error

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

### Requirement: HTTP success absence is distinct from HTTP failure
Public HTTP APIs SHALL NOT use `Result<T?, HTTPError>` to represent both a successful absence and an operation failure. Successful domain states such as a direct response without an indirect URL or a completed SSE stream SHALL be represented by explicit non-null domain outcomes inside `Result`; HTTP failures SHALL remain in `Error(HTTPError)`.

#### Scenario: A QUERY response has no indirect result URL
- **WHEN** `HTTPResponse.indirect_result_url(base)` inspects a response that is not a 303 response
- **THEN** it returns `Ok(HTTPIndirectResult.Direct)`

#### Scenario: A QUERY response provides an indirect result URL
- **WHEN** `HTTPResponse.indirect_result_url(base)` resolves exactly one valid Location value
- **THEN** it returns `Ok(HTTPIndirectResult.Redirect(url))`

#### Scenario: An indirect result response is malformed
- **WHEN** the Location metadata is missing, ambiguous, or invalid for a 303 response
- **THEN** it returns `Error(HTTPError)` rather than a successful absent URL

#### Scenario: An SSE stream reaches its end
- **WHEN** `HTTPEventStream.next_event()` reaches clean end-of-stream
- **THEN** it returns `Ok(HTTPEventPoll.End)`

#### Scenario: An SSE stream produces an event
- **WHEN** `HTTPEventStream.next_event()` reads a complete event
- **THEN** it returns `Ok(HTTPEventPoll.Event(event))`

#### Scenario: An SSE stream encounters a transport or protocol failure
- **WHEN** `HTTPEventStream.next_event()` cannot continue because of a typed failure
- **THEN** it returns `Error(HTTPError)` distinct from `HTTPEventPoll.End`
