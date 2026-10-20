# `std.http`

`std.http` provides an HTTP client and server built on [`std.net`](11-std-net.md).
It includes immutable URLs, validated headers, typed methods and status codes,
streaming bodies, connection pooling, cookies, redirects, middleware, and
Server-Sent Events (SSE), and bounded WebSocket client/server previews.
HTTP operations support HTTP/1.1 and HTTP/2 through one application API.

All waiting operations suspend cooperative branches, body flow applies backpressure, disconnects
cancel their structured work, and network permissions are revalidated at every
dynamic destination. Framework routing, dependency injection, templating, and ORM
behavior belong in packages built on this module.

> **Implementation status:** this page is the normative public contract. The
> current implementation may support only a subset. Missing functionality must be
> reported explicitly and must never weaken parsing, TLS, permissions, limits, or
> cancellation behavior.
>
> The current native preview provides `URL.parse`, one-shot `HTTP.get` and
> `HTTP.head`, JSON `HTTP.query` plus `HTTP.query_with_content_type` for an
> explicit validated query media type, UTF-8 text `HTTP.post`, JSON
> `HTTP.post_json`, `HTTP.post_json_value<T>` for values implementing the
> static `HTTPJsonSerializable<T>` contract, opaque byte `HTTP.post_bytes`, and
> `HTTP.get_stream` for a
> task-owned response body. `HTTP.request_stream` streams a one-pass body for
> any `HTTPMethod`, and `HTTP.request_stream_named` accepts RFC method tokens
> for extension methods; `HTTP.post_stream` is the POST convenience wrapper.
> Their producer has type `Fn(UInt64) => Array<Byte>` and receives a zero-based chunk index,
> returns an empty array at EOF, and each non-empty fragment is copied under a
> 64 KiB per-fragment and 16 MiB total limit. On HTTP/1.1 the runtime uses
> chunked transfer coding. The producer runs on its owning Zirk task and a
> single-slot queue applies backpressure. `request_stream_result` and
> `request_stream_named_result` accept `Result<Array<Byte>,
> HTTPStreamProducerError>`: `Cancelled` returns `HTTPError.Cancelled`, while
> `Failed` returns `HTTPError.Transport`. A producer error aborts the upload and
> closes any connection already opened. The one-shot stream helpers send once;
> reusable-client retry and redirect APIs replay streams only when explicitly
> enabled and require deterministic output by chunk index. `HTTP.request` sends the standard method enum except
> `HTTPMethod.Connect`; `HTTP.request_named` sends validated extension method
> tokens except the exact `CONNECT` token. These ordinary request APIs return
> `HTTPError.UnsupportedMethod` for CONNECT before permission lookup or
> connection setup. The explicit
> `HTTP.get_cached` and `HTTP.request_cached` methods provide a bounded
> process-local cache partitioned by approved project/manifest, with freshness,
> `Vary`, `Age`, and conditional validator handling for GET/HEAD/QUERY; cache
> state is not yet owned by `HTTPClient`. The current reusable-client preview
> supports `HTTPClient.create(max_origins = 8)`,
> `create_with_trust_roots(roots, max_origins = 8)`,
> `create_with_proxy_and_trust_roots(proxy, roots, max_origins = 8)`,
> `request(method, url, body,
> headers)`, `request_named(token, url, body, headers)` for validated extension
> methods, `request_following(method, url, body, headers, policy,
> max_redirects = 5)`, `request_body(method, url, HTTPBody)`,
> `request_body_following(method, url, HTTPBody, policy, max_redirects = 5)`,
> opt-in `enable_cookies()`, and `close()`. Redirect handling is opt-in through
> `HTTPRedirectPolicy.Never` (the default when callers use ordinary request
> methods), `SameOrigin`, or `Follow`. Each hop uses the same client and
> rechecks the destination origin grant. Cross-origin redirects strip known
> credential headers and fields marked with `add_sensitive`/`set_sensitive`
> while preserving ordinary headers.
> It keeps at most one
> HTTP/1.1 or HTTP/2 connection per origin in a bounded LRU pool.
> `HTTPClientOptions.create_with_protocol(protocol, ...)` accepts
> `HTTPProtocolPreference.Auto`, `HTTP1`, `HTTP2`, or `HTTP3`. `Auto` currently
> negotiates HTTP/2 or HTTP/1.1 through TLS ALPN; it does not try HTTP/3 until
> safe pre-commit fallback and independent-peer conformance are complete.
> Explicit `HTTP3` uses the native QUIC/H3 client when the Zirk runtime is
> built with its `http3` feature. It requires direct HTTPS, uses TLS 1.3 and
> `h3` ALPN, disables 0-RTT, and currently rejects configured proxies.
> `HTTPResponse.protocol()` reports `HTTP1`, `HTTP2`, `HTTP3`, or `Unknown`.
> Integration tests verify HTTP/1.1 and HTTP/2 reporting against independent
> peers. A client may be shared by tasks in the same
> approved executor and project/manifest identity. HTTP/1.1 requests use a
> cooperative FIFO queue; HTTP/2 allows up to eight simultaneous streams on
> one connection. Every request checks its exact network grant. `close()` waits
> for admitted operations. If cancellation interrupts `close()` before the
> drain completes, the client remains open and may be closed again. Executor
> shutdown closes any remaining client handles. HTTPS uses
> platform trust roots by default. `create_with_trust_roots` accepts 1–64 DER
> certificates (64 KiB each, 1 MiB aggregate) and replaces platform roots for
> that client; invalid roots fail before networking. A signed-approval
> compiled-Zirk loopback E2E verifies two requests authenticate and reuse one
> TLS connection with a custom root. A signed-approval compiled-Zirk E2E also
> verifies two simultaneous HTTP/2 streams reach the server before either
> response is released, over the same authenticated TLS connection.
> The client also exposes opt-in bounded
> retries for replayable safe methods on transport/timeout failures.
> Configurable `HTTPClientOptions` carries `connect_timeout_ms`,
> `response_headers_timeout_ms`, `response_idle_timeout_ms`, `max_origins`,
> and the protocol preference.
> Each timeout is validated from 1 ms through one hour, and the origin pool is
> bounded from 1 through 32 entries. The options object is accepted by direct,
> custom-trust-root, proxy, and combined proxy/custom-root client factories.
> TLS identity validation always uses the URL host for SNI and certificate-name verification with either platform roots or the explicitly supplied DER roots. The public API intentionally exposes no custom verifier or hostname override; mutual TLS client credentials are not yet public. The implemented HTTP scope passes full-workspace native CI on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64 (run 39359107965, compiler revision b28daa7). Generic response-producer backpressure is tested for HTTP/1.1 and HTTP/2 on Unix and HTTP/2 on Windows; the Windows HTTP/1.1 slow-consumer fixture remains Unix-gated. Broader standard-library capabilities remain staged.
> `HTTPServer.bind`
> applies the exact listener grant, and `HTTPFormData.parse(request)` parses
> bounded `application/x-www-form-urlencoded` bodies with repeated-value access.
> `HTTPResponse.header_values(name)` preserves repeated values for buffered and streamed responses, and
> `read_chunk(max_bytes)` returns byte chunks, EOF, or a
> typed error and caps each chunk at 64 KiB; call `close()` to abort an unread
> body. Task completion also closes abandoned streams. `HTTPResponse` exposes
> `status_code()`, one-shot `bytes(): Result<Array<Byte>, HTTPError>`, strict
> UTF-8 `text()` decoding, and `header_values(name)` for case-insensitive
> lookup that preserves repeated
> values. `HTTPRequest` exposes parsed `method()`, `target()`, `body()`, and
> `header_values(name)`; request-header lookup preserves repeated values in
> wire order. `HTTPServer.bind(endpoint)` returns a listener after the exact
> listen-grant check and maps TCP failures to `HTTPError`. The native preview's
> `HTTPServer.serve_once(stream, handler)`
> handles one accepted TCP stream; `HTTPServer.serve_next(listener, handler)`
> accepts and serves one connection, while
> `HTTPServer.serve_many(listener, max_requests, handler)` accepts and serves
> up to a fixed number of connections sequentially and returns the completed
> count. `HTTPServer.serve_many_and_close` provides the same finite scope and
> closes its listener after the bound or on a serve error. `HTTPServer.bind_tls(endpoint, chain, key)` creates an explicitly
> identified TLS listener with HTTP ALPN, and `serve_tls_once(stream, handler)`
> serves one request on an accepted stream. It uses HTTP/2 when ALPN selects
> `h2` and otherwise falls back to HTTP/1.1; TLS/network permissions remain
> enforced by `TLSListener`. `HTTPRouter.serve_forever(listener)` and
> `serve_tls_forever(listener)` accept successive connections until their
> owner task is cancelled. They process one accepted connection at a time;
> each HTTP/2 connection may still multiplex up to eight request streams.
> Cancellation releases the listener. `HTTPRouter.route(method, pattern, handler)` registers bounded,
> method-aware routes with named `:parameter` segments; duplicate and
> equal-specificity ambiguous routes are rejected, and matched parameters are
> percent-decoded. `HTTPRouteParameters.get(name)` returns the optional value
> for a named parameter; `name(index)` and `value(index)` provide ordered
> access. `HTTPRouter.serve(listener, max_requests)` uses the same finite
> connection bound; HTTP/1.1 connections are handled sequentially and
> negotiated HTTP/2 requests use up to eight independently scheduled handler
> tasks per connection. Separate concurrent connection admission and an
> unbounded long-lived listener loop are not available in the preview. A router
> returns 404 when no route path matches and 405 when the path matches only
> other methods. Both server entry points require the program's explicit listen
> permission. Handler cancellation is cooperative at task safe points; CPU-only
> code is not preempted.
> Canceling the owning `Job` while `serve_many` is waiting in `accept()` stops
> admission and releases the listener when the task exits. A compiled-Zirk
> HTTP/1.1 E2Es cancel a job while a route handler is suspended in
> `Timer.sleep`; they verify no late response and listener release on macOS
> aarch64 and Linux ARM64. A separate signed compiled-Zirk E2E on macOS and
> Linux ARM64 now cancels middleware at the same safe point and verifies no
> late response and listener release. A bounded
> handler deadline now requests cooperative cancellation after 30 seconds.
> It takes effect when the handler reaches a task cancellation safe point,
> such as a timer or I/O wait; CPU-only handlers are not preempted and may
> exceed that interval. Disconnect-triggered cancellation propagates an
> HTTP/1.1 peer disconnect to a cooperatively suspended handler in a native
> runtime loopback test. The finite HTTP/2 server scope gives graceful shutdown
> five seconds to drain; after that it closes remaining streams and returns
> `HTTPError.Timeout`. Each admitted HTTP/2 stream runs its route callback in
> an independently cancelable Zirk task on the owning executor, with at most
> eight active handler tasks. A compiled-Zirk E2E sends two requests over one
> TLS/HTTP/2 connection and verifies both sleeping handlers overlap on macOS
> aarch64 and Linux ARM64. Stream
> disconnect cancellation is covered by a separate runtime loopback test.
> A 405 response includes a deduplicated, stable-order `Allow` field for the
> methods registered on the matching route paths.
> `HTTPResponse.method_not_allowed(patterns, methods, path)` builds the same
> response for a manual dispatcher.
> `HTTPResponse.try_from_bytes(status, body)` checks the HTTP status range
> (100–599) and 16 MiB body bound before constructing a response.
> `HTTPResponse.try_from_text` and `.try_from_json` apply the same checks while
> adding the matching text or JSON content type. JSON syntax and serialization
> remain the caller's responsibility.
> Server handlers can construct UTF-8 responses with
> `HTTPResponse.from_text(status, text)` and pre-serialized JSON responses with
> `HTTPResponse.from_json(status, json)`. These set the corresponding
> `Content-Type` header; JSON serialization and validation remain the caller's
> responsibility.
> Typed response decoding is available as `decode_json<T>(response)` for types
> that implement `HTTPJsonParsable<T>` with a static `parse_json(String)`;
> this generic top-level function uses compile-time specialization. A parser
> failure maps to `HTTPError.InvalidJson`, while invalid response UTF-8 remains
> `HTTPError.InvalidText`.
> `HTTPHeaders.create()` provides a validated in-memory collection with
> case-insensitive `get`/`get_all`, ordered repeated values, and bounded
> `add`/`set`. Use `add_sensitive` or `set_sensitive` for custom credential
> fields; they are redacted in runtime diagnostics and removed on cross-origin
> redirects. `HTTP.request(method, url, body, headers)` sends an explicit
> method, bounded byte body, and validated request headers; its compiled-Zirk
> E2E verifies PATCH, a custom header, and the exact body on the wire.
> `HTTPResponse.try_with_headers(status, body, headers)` attaches
> the validated collection to a bounded server response and returns typed
> status/body errors. The native server validates the encoded header block
> again before sending it. Protocol-owned framing and Host headers cannot be
> supplied through `HTTPHeaders`.
> `HTTPEventStream.connect(url)` and `next_event()` provide a demand-driven
> SSE client. Each event exposes its type, multiline data, persistent
> `Last-Event-ID`, and optional retry delay. The native stream validates the
> response media type, bounds parser state, and closes the response on EOF,
> explicit close, task cancellation, or parsing/transport failure.
> `HTTPServerEvent.encode(data, event_type?, id?, retry_ms?)` builds one
> bounded UTF-8 event frame for an `HTTPResponse.from_stream` producer. It
> preserves multiline data and optional reconnection metadata, treats an
> empty event name as the default `message` event, and emits an empty `id:`
> field to reset the reconnect cursor. Event names and IDs reject line
> injection; data fields, event lines, and encoded frames are bounded. A
> compiled-Zirk E2E verifies framing, retry values, empty-ID reset, and
> injection rejection. Use `HTTPResponse.from_sse_stream(producer,
> heartbeat_ms?)` to create an event-stream response with an optional
> bounded heartbeat interval.
> Server responses can stream bounded chunks with
> `HTTPResponse.from_stream(status, headers, producer)`. The callback runs on
> the server task, receives a zero-based `UInt64` chunk index, and returns an
> `Array<Byte>`; an empty array ends the response. HTTP/1.1 and HTTP/2 transport
> paths apply bounded buffering and backpressure. `connect_with_reconnect`
> enables demand-driven SSE reconnection.
> `HTTP.get_stream_with_headers(url, headers)` opens a streaming GET with
> validated caller headers. `HTTPEventStream.reconnect(url, last_event_id)`
> uses this path to open a new stream and sends
> the validated `Last-Event-ID` request header. Alternatively,
> `HTTPEventStream.connect_with_reconnect(url)` maintains a demand-driven
> stream using the latest valid server `retry:` delay (3 seconds initially)
> and the persistent non-empty `Last-Event-ID`. It retries initial and later
> transport/timeout failures; values above 2,147,483,647 milliseconds saturate
> to that limit. Waits use cancellation-safe `Timer.sleep`;
> permission, cancellation,
> protocol, and other errors are returned without retry. A `204 No Content`
> response closes the stream without reconnecting. Call `close()` to stop.
> Buffered response bodies remain subject to the runtime body limit.
> `HTTP.post_json(url, json)` validates the JSON document before networking,
> sends `Content-Type: application/json`, and reports malformed JSON as
> `HTTPError.InvalidJson`.
> `HTTP.post_json_value<T>(url, value)` calls the type's static
> `HTTPJsonSerializable<T>.to_json(value)` method, then uses the same local
> JSON validation and transport path. Implementations own field naming and
> escaping; a serialization error maps to `HTTPError.InvalidJson`.
> `HTTP.get_following(url, max_redirects = 5)` follows at most 20 hops and only
> 301, 302, 303, 307,
> and 308 responses, resolves one `Location` header per hop, rejects malformed
> or ambiguous locations, and rechecks the exact network grant through each
> ordinary `HTTP.get` call. Both it and `HTTPClient.get_following` use iterative
> loops, so the 20-hop maximum does not grow the Zirk task stack. Its
> approved-project E2E covers a relative redirect,
> a cross-origin grant denial, and a redirect loop limit on macOS.
> `HTTPClient.request_following_with_history` and
> `request_body_following_with_history` return `HTTPRedirectResult`, which
> keeps the final `HTTPResponse` and a bounded list of followed hops. Each hop
> records the method, status, and normalized destination; user information is
> removed, every query string is replaced with `?redacted`, fragments are
> discarded, and each stored destination is capped at 8 KiB. A compiled-Zirk E2E verifies
> a `POST` to `303` transition, cross-origin credential stripping, and that the
> original query value is absent from printed and JSON history. `history_json()`
> returns a bounded JSON array with string delimiters escaped.
> These paths have approved-project runtime evidence on macOS;
> the remaining client/server surface below is the target contract, not a claim
> that those APIs are already usable.

## Imports and naming

Namespace and direct imports expose the same symbols:

```zirk
import { HTTP, HTTPClient, HTTPHeaders, HTTPMethod, URL } from std.http;
import { stderr, stdout } from std.io;

fn main(): Void {
    match URL.parse("https://example.com") {
        Ok(url) => {
            match HTTP.get(url) {
                Ok(response) => {
                    stdout.println(response.status_code());
                    _ = response.close();
                }
                Error(error) => stderr.println(error);
            }
            mut empty_body: Array<Byte> = [];
            match HTTPClient.create(8) {
                Ok(client) => {
                    match client.request(HTTPMethod.Get, url, empty_body, HTTPHeaders.create()) {
                        Ok(response) => {
                            stdout.println(response.status_code());
                            _ = response.close();
                            stdout.println("response closed");
                        }
                        Error(error) => stderr.println(error);
                    }
                    _ = client.close();
                    stdout.println("client closed");
                }
                Error(error) => stderr.println(error);
            }
        }
        Error(error) => stderr.println(error);
    }
}
```

HTTP-family acronyms remain uppercase: `HTTP`, `HTTPS`, `URL`, `URI`, `SSE`,
`WebSocket`, `HTTPClient`, `HTTPRequest`, and `HTTPResponse`.

## Methods, including `QUERY`

`HTTPMethod` includes the standard methods `Get`, `Head`, `Post`, `Put`,
`Patch`, `Delete`, `Options`, `Connect`, `Trace`, and `Query`. The generic
`request` API accepts standard methods other than `CONNECT`, plus extension
methods; dedicated methods provide the ordinary spelling. `CONNECT` changes
the connection into a bidirectional tunnel after a successful response, so it
is rejected by ordinary request methods before network I/O. The proxy and
WebSocket APIs manage their own CONNECT tunnels. `HTTPMethod.is_safe` and
`HTTPMethod.is_idempotent` expose each method’s standard safety and idempotency
classification; QUERY is true for both, while POST is false for both.
`HTTP.method_name(method)` returns the exact uppercase wire token, including
`"QUERY"` for `HTTPMethod.Query`. `HTTP.request_named(method, url, body,
headers)` accepts an additional RFC method token such as `"M-SEARCH"`; the
runtime rejects malformed tokens and `CONNECT` before permission lookup or
connection setup. An invalid method token returns
`HTTPError.InvalidArgument`; `CONNECT` returns
`HTTPError.UnsupportedMethod` because ordinary requests do not create tunnels.

```zirk
client.get(url:)
client.post(url:, json: command)
client.query(url:, json: filter)
client.request(HTTPMethod.Put, url:, json: replacement)
HTTP.request(HTTPMethod.Patch, url, body_bytes, headers)
HTTP.request_named("M-SEARCH", url, body_bytes, headers)
```

`QUERY` follows RFC 10008. It asks the target resource to process enclosed
query content without changing resource state. It is safe, idempotent, and
cacheable; unlike `GET`, its content has defined semantics, and unlike `POST`,
it may be retried automatically when the retry contract permits. Every QUERY
request, including one with an empty body, requires one valid, concrete
`Content-Type`; wildcard media ranges such as `application/*` and `*/*` are
rejected because they do not identify the query content format. `HTTP.query`
sends JSON by default;
`HTTP.query_with_content_type(url, body, content_type)` lets the caller choose a
validated query media type without content sniffing. Servers may advertise
accepted formats through `Accept-Query`. Native clients validate received
`Accept-Query` fields as RFC 9651 Structured Field lists and preserve their raw
values through `HTTPResponse.header_values("accept-query")`. The field accepts
the RFC's `type/*` and `*/*` wildcards; a QUERY request's `Content-Type` still
must name a concrete format. A server can
advertise formats by adding a validated `Accept-Query` field to the
`HTTPHeaders` passed to `HTTPResponse.try_with_headers`; the compiled-Zirk
server E2E verifies the field reaches the wire. Typed format matching and
automatic query-format negotiation are left to application policy; RFC 10008
defines capability discovery through `Accept-Query`, not a client selection
algorithm.
When a QUERY response is `303 See Other`, `HTTPResponse.indirect_result_url(base)`
validates its single `Location` value and resolves it against the original URL.
It returns `null` for a direct result. To follow redirects explicitly,
`HTTP.query_following(url, body, max_redirects)` and
`HTTP.query_following_with_content_type(url, body, content_type, max_redirects)`
provide a bounded loop. A `303` switches to GET; `301`, `302`, `307`, and `308`
preserve QUERY and replay its buffered content. Each hop rechecks URL validity,
network permission, and TLS identity. The default limit is five redirects and
the maximum is twenty.

```zirk
mut filter = {
    "published_after": "2026-01-01",
    "tags": ["zirk", "language"],
};

match HTTP.query(
    URL("https://api.example.com/articles"),
    json: filter,
    timeout: 10s,
) {
    Ok(response) => stdout.println(response.status);
    Error(error) => stderr.println(error);
}
```

Libraries and applications must not treat QUERY as permission to mutate state.
Servers that implement a query endpoint are responsible for preserving its safe
semantics. Automatic retries remain bounded and occur only before a response is
observed or where protocol semantics prove them safe.

## URLs

`URL` is immutable, validated, and normalized without losing significant
escaping. The current preview exposes `URL.parse`, `URL.resolve`, and
`as_string()`. Parsing rejects malformed syntax and credentials where the
selected policy forbids them; resolution follows URI reference rules.

```zirk
match URL.parse("https://api.example.com/users?page=2") {
    Ok(url) => match URL.resolve(url, "?page=3") {
        Ok(next) => stdout.println(next.as_string());
        Error(error) => stderr.println(error);
    }
    Error(error) => stderr.println(error);
}
```

Component accessors and builders such as `with_query`, `without_query`,
`with_path`, and an `origin` property belong to the target API and are not in
the current preview. The runtime derives the normalized origin internally for
permissions, cookie policy, pooling, and redirect validation.

## One-shot requests

`HTTP.get`, `HTTP.query`, and the other namespace methods create one logical
request with secure defaults:

```zirk
import { HTTP, URL } from std.http;
import { stderr, stdout } from std.io;

fn main(): Void {
    match URL.parse("https://api.example.com/profile") {
        Ok(url) => match HTTP.get(url) {
            Ok(response) => {
                stdout.println(response.status_code());
                _ = response.close();
                stdout.println("response closed");
            }
            Error(error) => stderr.println(error);
        }
        Error(error) => stderr.println(error);
    }
}
```

One-shot calls do not retain cookies between requests. Implementations may use a
shared safe transport pool internally, but authentication, cookie, proxy, and
other caller-visible state never leaks between unrelated calls.

For a secret credential, use the explicit write-only sink:

```zirk
match Env.get_secret("API_TOKEN") {
    Ok(token) => {
        mut empty_body: Array<Byte> = [];
        match HTTP.request_with_secret_header(
            HTTPMethod.Get,
            url,
            empty_body,
            HTTPHeaders.create(),
            "Authorization",
            token,
        ) {
            Ok(response) => handle(response);
            Error(error) => report(error);
        }
    }
    Error(error) => report_secret_error(error);
}
```

The sink requires HTTPS and the exact `network.connect.origins` grant for the
request URL. It rejects an existing field with the same name, never converts
the secret to a Zirk `String`, and does not follow redirects.

## Reusable clients

The native preview exposes a smaller API than the full policy contract below:

```zirk
match HTTPClient.create(8) {
    Ok(client) => {
        _ = client.enable_cookies();
        match client.request(HTTPMethod.Get, url, [], HTTPHeaders.create()) {
            Ok(response) => stdout.println(response.status_code());
            Error(error) => stdout.println(error.to_string());
        }
        _ = client.close();
    }
    Error(error) => stdout.println(error.to_string());
}
```

`create` accepts 1–32 origins. `request_named` accepts a validated HTTP token,
including registered extension methods. A client handle can be shared between
tasks in the same approved project/manifest. Access uses a FIFO cooperative
queue, so waiting callers do not block the Zirk scheduler. HTTP/1.1 requests
remain sequential; one HTTP/2 connection can carry up to eight concurrent
requests. The queue bounds in-flight requests and every request rechecks its
exact origin grant. Call `close()` to release the pool; it waits for active
requests first. Executor shutdown closes any client left open after all sibling
tasks finish. Cookie storage is disabled by default; when enabled it is
bounded, in-memory, and scoped to the client. The connection pool reuses
HTTP/1.1 and HTTP/2 connections selected through TLS ALPN. HTTPS uses the
platform trust store by default. For a private PKI,
`HTTPClient.create_with_trust_roots(roots, max_origins = 8)` accepts a non-empty
array of DER certificates and replaces the platform roots for that client;
invalid roots fail before networking. Signed-approval compiled-Zirk loopback
E2Es verify authentication and connection reuse using an explicit test root
for HTTP/1.1 and HTTP/2. The HTTP/2 E2E runs two shared-client requests at the
same time and holds both server responses until both streams arrive. HTTP/2
framing, connection state, and response decoding run on the native reactor;
cancelling one request resets only its stream. Explicit proxy constructors are
available. `HTTPClientOptions` sets connect, response-header, and response-body
idle timeouts independently; each accepts 1 to 3,600,000 ms, with defaults of
10,000, 30,000, and 30,000 ms. The options compose with direct, proxy, custom
trust-root, and proxy-plus-trust-root clients through
`create_with_options`, `create_with_options_and_proxy`,
`create_with_options_and_trust_roots`, and
`create_with_options_and_proxy_and_trust_roots` respectively. Proxy
credentials and destination/proxy TLS identity remain scoped to the same
client pool while the configured timeout phases apply to its connections.

`HTTPClient.create(max_origins)` creates a reusable client with default timeout
values and returns a typed result. Use `HTTPClientOptions` to configure those
three phases:

```zirk
import { HTTPClient, HTTPClientOptions } from std.http;
import { stderr } from std.io;

fn main(): Void {
    match HTTPClientOptions.create(
        connect_timeout_ms: 5000,
        response_headers_timeout_ms: 12000,
        response_idle_timeout_ms: 20000,
        max_origins: 4,
    ) {
        Ok(options) => match HTTPClient.create_with_options(options) {
            Ok(client) => {
                _ = client.close();
                stderr.println("client closed");
            }
            Error(error) => stderr.println(error);
        }
        Error(error) => stderr.println(error);
    }
}
```

Use `request` to send a request through the client:

```zirk
import { HTTPClient, HTTPHeaders, HTTPMethod, URL } from std.http;
import { stdout } from std.io;

fn main(): Void {
    match HTTPClient.create(8) {
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {
            match URL.parse("https://example.com/") {
                Error(_) => stdout.println("invalid URL");
                Ok(url) => {
                    mut empty: Array<Byte> = [];
                    match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {
                        Ok(response) => stdout.println(response.status_code());
                        Error(error) => stdout.println(error.to_string());
                    }
                }
            }
            _ = client.close();
        }
    }
}
```

A client owns its connection pools, DNS/TLS session state, optional cookie jar,
proxy policy, decompression configuration, and concurrency limits. Pool keys
include origin, proxy, TLS identity/configuration, and negotiated protocol so
authority or credentials cannot cross incompatible connections. Idle and total
connections, streams per HTTP/2 connection, queue time, and lifetime are
bounded. Closing the client cancels queued work and gracefully closes owned
connections.

## Request bodies

One-shot requests and reusable-client requests accept a closed `HTTPBody`
variant. Exactly one body representation is selected when constructing the
value:

```zirk
HTTP.request_body(HTTPMethod.Put, url, HTTPBody.Text(message))
HTTP.request_body(HTTPMethod.Post, url, HTTPBody.Json("true"))
HTTP.request_body(HTTPMethod.Post, url, HTTPBody.Bytes(content))
HTTP.request_body(HTTPMethod.Post, url, HTTPBody.Form(fields))
HTTP.request_body(HTTPMethod.Post, url, HTTPBody.Multipart(parts, "zirk-boundary"))
HTTP.request_body(HTTPMethod.Post, url, HTTPBody.Stream(headers, producer))
client.request_body(HTTPMethod.Post, url, HTTPBody.Json("true"))
```

The variants delegate to the validated text, JSON, bytes, form, multipart, and
stream helpers. `HTTP.request_body` and `HTTP.request_named_body` support
standard and extension methods; `HTTP.post_body` is the POST convenience
wrapper. The lower-level `HTTP.request` and `client.request` APIs accept a byte
buffer and explicit headers; that payload is the `HTTPBody.Bytes` representation
and receives no additional encoding. Buffered JSON is validated before
connecting. The reusable
`client.request_body` supports text, JSON, bytes, form, multipart, and bounded
stream bodies; JSON is validated before connecting or reusing a pooled
connection. Streaming bodies use HTTP/1.1 chunked framing or HTTP/2 DATA frames,
copy each producer chunk into a one-slot backpressure queue, and run producer
callbacks on their owning Zirk tasks. A shared client can upload multiple
HTTP/2 streams concurrently. `HTTPClient.request_body_with_retries` can replay
a stream for an explicitly enabled safe-method retry; it starts the callback
again at index zero, so each index must produce the same bytes on every
attempt. The one-shot `HTTP.request_body` APIs do not retry uploads.

`HTTPBody` is the closed request representation shared by one-shot and reusable
clients. Its six variants make the encoding choice explicit; text, JSON, bytes,
form, multipart, and stream bodies cannot be combined on these APIs. Convenience
methods such as `HTTP.post`, `HTTP.post_json`, `HTTP.post_bytes`, `HTTP.post_form`,
and `HTTP.post_multipart` delegate to those validated paths. Lower-level
`HTTP.request` and `HTTPClient.request` remain byte-buffer APIs for callers that
need to supply explicit headers.

High-level body parameters are mutually exclusive:

```zirk
client.post(url:, json: user)
client.post(url:, text: message)
client.post(url:, bytes: content)
client.post(url:, form: fields)
client.post(url:, stream: source)
```

Passing more than one is a compile-time error when statically visible and a
typed configuration error otherwise. `json` uses the generated `JSONCodec<T>`;
`text` defaults to UTF-8; `form` uses form URL encoding; and `multipart` creates
a caller-supplied multipart boundary. Streaming bodies do not
require their full content in memory and apply backpressure to their producer.

A retryable request body must be replayable. Bytes, text, JSON, multipart, and
form values are replayable. `HTTPClient.request_body_with_retries` also accepts
a deterministic `HTTPBody.Stream` producer and restarts it at chunk index zero
for each attempt. It retries only `Transport` and `Timeout` for GET, HEAD,
OPTIONS, TRACE, and QUERY, up to three times; unsafe methods are rejected before
the request is sent.

## Headers

`HTTPHeaders` is case-insensitive for lookup while preserving valid values and
repeated fields in insertion order. It validates field names, rejects CR, LF,
NUL, and other forbidden bytes, and rejects protocol-owned framing fields such
as `Content-Length`, `Transfer-Encoding`, connection controls, and `Host`.

```zirk
mut headers = HTTPHeaders.create();
_ = headers.add("accept", "application/json");
_ = headers.add("accept", "application/problem+json");
_ = headers.add("set-cookie", "session=first");
_ = headers.add("set-cookie", "theme=dark");

mut content_type = headers.get("content-type");
mut cookies = headers.get_all("set-cookie");
```

`HTTPResponse.try_with_headers(status, body, headers)` attaches this validated
collection to a bounded server response and returns typed status/body errors.
The native server validates the encoded header block again before sending it.
Pass the same collection to `HTTP.request` or `HTTP.request_named` to send
validated custom request headers.

`set` replaces the field's values, `add` appends one, `get` returns an optional
value, and `get_all` returns every value in wire order. The collection limits
itself to 256 fields and 64 KiB of combined name and value bytes. `set-cookie`
values remain separate and are never comma-joined. Use `add_sensitive` or
`set_sensitive` for application-defined credential fields. Their values remain
available to the transport but are redacted in runtime diagnostics and removed
when a redirect crosses origins. Replacing a marked field with ordinary `set`
does not clear its sensitivity mark.

Two low-level helpers are also exposed on `HTTPHeaders`:
`redirect_copy(same_origin, preserve_body)` returns a copy with known or
explicitly marked credentials removed for cross-origin replay and content
metadata removed when the body is discarded. The caller supplies the origin
and body decisions; normal client redirects apply this policy automatically.
`encode_for_transport()` returns the opaque bounded encoding used at the
runtime boundary. It is not an HTTP wire-format header block and should not be
sent or persisted by application code.

The protocol layer owns `Content-Length`, transfer coding, connection controls,
and HTTP/2 pseudo-fields. `HTTP.request` and `HTTP.request_named` send validated
caller headers while keeping those protocol-owned fields unavailable.

## Responses and body consumption

`HTTPResponse.protocol()` reports the negotiated version for HTTP calls and
`Unknown` for locally constructed responses. `HTTPResponse` does not change its
native object layout to carry redirect metadata. Call one of the explicit
`HTTPClient` history methods to receive an `HTTPRedirectResult`; its
`response()` returns the final response and `history()` returns up to 20
`METHOD status destination` entries; each normalized destination is capped at
8 KiB. `history_json()` serializes those redacted entries as a JSON array. The
helper removes URL user information, redacts the entire query
string, protecting unknown credential parameter names as well as known ones,
and discards URL fragments.
These entries contain no request headers or cookies. Final URL metadata and
serialized structured hop objects are not yet exposed. A response body is
consumed exactly once through one of:

The current bundled API exposes typed JSON decoding as the top-level
`decode_json<T>(response)` function. The `response.json<T>()` form below is the
target instance API; the current compiler does not yet support generic instance
methods. The current preview returns a bounded buffered body from
`HTTPResponse.bytes(): Result<Array<Byte>, HTTPError>` or decodes it through
`.text()`. Both consume the same body, so calling either after the other
returns `HTTPError.BodyConsumed`. Incremental reads use `read_chunk(max_bytes)`
on `HTTPResponse`; `HTTP.get_stream()` returns that same public type with a
task-owned network body. Calling `bytes()` or `text()` on a stream-backed
response consumes the remaining body once within the same size limit.
`HTTPResponse.stream()` transfers a buffered response body into the same chunk
reader and returns a stream-backed `HTTPResponse`; it releases the original
managed byte array after transfer. A second body operation returns
`HTTPError.BodyConsumed`.
`read_chunk(max_bytes)` returns `HTTPStreamChunk.Bytes(bytes)`,
`HTTPStreamChunk.End`, or `HTTPStreamChunk.Error(error)`. `max_bytes` must be
between 1 and 65,536; an out-of-range value returns
`HTTPStreamChunk.Error(HTTPError.InvalidArgument)` without consuming response
bytes. The same chunk enum is used by temporary upload range reads.
`HTTPStreamResponse` remains a compatibility alias for `HTTPResponse`; both
buffered and stream-backed responses use the same single-consumption type.

```zirk
response.text()
response.bytes()
response.json<User>()
response.stream()
```

Buffered readers enforce configured size limits. `text` validates the declared
or selected encoding; the current preview validates UTF-8. Typed JSON decoding
is available through the top-level `decode_json<T>(response)` function and
uses strict JSON decoding. `stream` transfers body ownership to a bounded
asynchronous byte stream. A second body access returns
`HTTPError.BodyConsumed`.

Closing a response before consuming its body either drains only within a small,
safe bound or closes/resets the transport, preventing unbounded cleanup work and
pool poisoning.

For low-level SSE consumers, `HTTPResponse.read_sse_event_fields()` consumes
one event and returns `[event, data, last_event_id, retry_ms]` as strings; an
empty list marks end-of-stream, and a consumed or non-stream response returns a
typed error. `sse_heartbeat_count()` reports the cumulative comment-heartbeat
count observed by that response. Application code should normally use
`HTTPEventStream.next_event()` and `heartbeat_count()` instead.

## Status codes

`HTTPStatus` is an integer-backed enum. Every standard case carries its actual
wire code, so `HTTPStatus.Ok` corresponds to `200`, not the text `"OK"`.
Unknown extension codes remain representable and are not collapsed into an
unrelated standard case.

```zirk
mut status = response.status_code();
if status == 200 { stdout.println("success"); }
else if status == 404 { stdout.println("missing"); }
else if status >= 500 { stderr.println(status); }
```

The properties `is_informational`, `is_success`, `is_redirect`,
`is_client_error`, and `is_server_error` derive from the numeric class.
Reason phrases are optional presentation metadata and never determine equality
or matching.

## Redirects

Redirects are never followed by ordinary request methods. Callers can opt in
with `HTTPRedirectPolicy.SameOrigin` or `Follow` on
`HTTPClient.request_following` for byte buffers, or `request_body_following`
for any `HTTPBody` variant. The default policy is `Never`; the hop limit
defaults to five and is capped at twenty. Each hop resolves exactly one
`Location`, rechecks the exact network grant, and rejects loops. `SameOrigin`
returns a cross-origin redirect response without following it. Cross-origin
requests remove known credential headers and preserve ordinary headers. A
`303` changes the method to GET, except HEAD, and drops the body and its content
metadata. Other supported redirect statuses preserve method and body. Stream
producers restart at chunk zero when replayed, so they must return the same
chunk for the same index. Response history is not exposed. One-shot
`HTTP.get_following` and QUERY helpers remain available for their respective
methods.

For replayable requests, `HTTPClient.request_with_retries` accepts buffered
bytes and `HTTPClient.request_body_with_retries` accepts any `HTTPBody` with an
explicit retry count from zero to three. Both retry only transport and timeout
failures for GET, HEAD, OPTIONS, TRACE, and QUERY. Asking either API to retry
POST, PUT, PATCH, DELETE, or CONNECT fails before the request is opened. Permission,
protocol, and cancellation errors are returned immediately, and HTTP status
codes do not trigger retries. Explicit proxy configuration is available through
`HTTPProxy` and `HTTPClient.create_with_proxy`. Retry delays use the current
bounded exponential runtime policy; status-based retries and public backoff
configuration are not available.

## Cookies

Cookies are stored in an internal, bounded, in-memory jar owned by a reusable
client. The public API opts in with `enable_cookies()`:

```zirk
match HTTPClient.create() {
    Ok(client) => {
        _ = client.enable_cookies();
        _ = client.close();
    }
    Error(_) => {}
}
```

The jar implements domain, path, expiry, `Secure`, `HttpOnly`, `SameSite`,
prefix, and public-suffix rules. Secure cookies are never sent over plaintext
HTTP. Missing, empty, or non-absolute `Path` attributes use the request URL's
default path, including when a later repeated `Path` attribute is invalid
([RFC 6265, Sections 5.2.4 and 5.3](https://www.rfc-editor.org/rfc/rfc6265)).
Oversized `Path` and `Domain` attributes reject the cookie instead of keeping
an earlier, broader scope.
Repeated `Domain` attributes follow the last nonempty value; the final domain
is validated after parsing, so an invalid earlier value does not override a
later valid one. Domain names are canonicalized with IDNA before matching the
request host, so a Unicode domain attribute matches its equivalent punycode
host. Percent-escaped domain syntax is rejected rather than URL-decoded
([RFC 6265, Sections 5.1.2 and 5.2.3](https://www.rfc-editor.org/rfc/rfc6265)).
Redirect helpers recalculate cookie eligibility for each destination.
Custom jar construction, persistence, and encrypted storage are not part of the
current API.

## Query, route, and form parameters

Missing values use `Option`; malformed typed values use `Result`:

```zirk
mut search = request.query.get("search");

match request.query.parse<Int>("page") {
    Ok(Some(page)) => stdout.println(page);
    Ok(None) => stdout.println("default page");
    Error(error) => stderr.println(error);
}
```

Route parameters use `:name`, consistent with Zirk's established placeholder
syntax. They are accessed with collection indexing:

```zirk
mut id = request.params["id"];
```

`HTTPFormData.create()` builds URL-encoded form bodies. Add each name/value pair
with `add`; duplicate names and insertion order are preserved. `add` returns a
typed error after 256 fields or 1 MiB of raw UTF-8 input. The encoder also
checks the final percent-encoded wire representation against the 1 MiB body
limit. `encode_urlencoded()` returns bytes that can be passed to `HTTP.request`
with the `application/x-www-form-urlencoded` content type. `HTTP.post_form(url,
form)` performs the encoding and supplies that content type automatically.

`HTTPFormData.parse(request)` parses URL-encoded form bodies. It
requires one `application/x-www-form-urlencoded` content type, accepts UTF-8,
decodes `+` and percent escapes strictly, preserves repeated fields in order,
and caps the body at 1 MiB and the field count at 256. Invalid media types,
escapes, and UTF-8 return `HTTPError.Protocol`; excess body size returns
`HTTPError.BodyTooLarge`, and excess fields return `HTTPError.TooManyFields`.

```zirk
mut outgoing = HTTPFormData.create();
_ = outgoing.add("q", "hello world");
_ = outgoing.add("tag", "a+b");
match HTTP.post_form(url, outgoing) {
    Ok(response) => stdout.println(response.status_code());
    Error(error) => report(error);
}
match outgoing.encode_urlencoded() {
    Ok(body) => {
        mut headers = HTTPHeaders.create();
        _ = headers.add("content-type", "application/x-www-form-urlencoded");
        // Pass body and headers to HTTP.request(...).
    }
    Error(error) => report(error);
}

match HTTPFormData.parse(request) {
    Ok(form) => {
        mut first_query = form.get("q");
        mut all_tags = form.get_all("tag");
    }
    Error(error) => report(error);
}
```

`HTTPMultipartData.parse(request)` parses one `multipart/form-data` request
using its `boundary` parameter. It preserves part order and binary payload
bytes. `part_count()` reports the number of parts and `part(index)` returns an
optional `HTTPMultipartPart` with `name()`, optional `filename()` and
`content_type()`, and `bytes()`.
The parser caps the whole body at 1 MiB, the part count at 256, each part's
header block at 16 KiB, and the boundary at 70 bytes. Header framing,
per-part media types, and disposition parameters are validated strictly. A
filename is untrusted metadata; Zirk does not interpret it as a path. Parts with a
filename also receive an opaque task-owned temporary upload resource. Use
`has_temporary_upload()`, `read_temporary_upload(offset, max_bytes)` (up to
64 KiB per read), and `close_temporary_upload()`. A read size outside
1..65,536 bytes or an offset past the part end returns
`HTTPStreamChunk.Error(HTTPError.InvalidArgument)`. The client filename never
selects the temporary path. Closing is idempotent, and any remaining files are
removed when the owning task exits. `bytes()` remains available for compatibility.
Streaming
request producers are available through the bounded `HTTP.request_stream`,
`HTTP.request_stream_named`, `HTTP.request_stream_result`,
`HTTP.request_stream_named_result`, `HTTP.post_stream`, and
`HTTPClient.request_body` with `HTTPBody.Stream` APIs. The result variants expose explicit producer cancellation and failure outcomes; one-pass
producers cannot be retried. A signed-approval compiled-Zirk loopback E2E on
macOS ARM64 and Linux ARM64 Docker verifies that cancelling the owning
structured task during upload aborts the incomplete request without sending
the terminal chunk. Buffered safe requests can use the opt-in
`HTTPClient.request_with_retries` policy.

`HTTPMultipartBuilder.create()` builds a binary-safe multipart body. Add up to
256 parts with `add(name, bytes, filename?, content_type?)`. Call
`encode(boundary)` to obtain bytes or `HTTP.post_multipart(url, builder,
boundary)` to send them with the matching content type. The boundary and part
metadata are validated and quoted safely, and a boundary that collides with
part framing is rejected; each part header is limited to 16 KiB and the final
encoded body to 1 MiB. This builder retains payloads in managed memory and
does not create files or temporary upload resources.

```zirk
mut multipart = HTTPMultipartBuilder.create();
match multipart.add("avatar", image_bytes, "avatar.png", "image/png") {
    Error(error) => report(error);
    Ok(_) => match HTTP.post_multipart(url, multipart, "zirk-upload-2026") {
        Error(error) => report(error);
        Ok(response) => stdout.println(response.status_code());
    }
}
```

```zirk
match HTTPMultipartData.parse(request) {
    Error(error) => report(error);
    Ok(form) => {
        mut index: Int32 = 0;
        while index < form.part_count() {
            match form.part(index) {
                null => { }
                part => {
                    stdout.println(part.name());
                    mut filename = part.filename();
                    mut bytes = part.bytes();
                    stdout.println(bytes.length);
                }
            }
            index = index + 1;
        }
    }
}
```

## Servers and routing

The current bundled preview uses `HTTPRouter` with an explicit listener. A
plain TCP server can serve a bounded number of requests:

```zirk
import { stdout } from std.io;

mut router: HTTPRouter = HTTPRouter();
match HTTPServer.bind("tcp://127.0.0.1:8080") {
    Error(error) => stdout.println(error.to_string());
    Ok(listener) => {
        _ = router.route(HTTPMethod.Get, "/users/:id", get_user);
        _ = router.route(HTTPMethod.Query, "/users", query_users);
        router.serve(listener, 100);
    }
}
```

`HTTPMethod.Get` and `HTTPMethod.Query` are enum cases, not language keywords.
Duplicate or ambiguous routes are configuration errors; registration order
must not silently change which structurally equivalent route wins. The bounded
TLS variant is `router.serve_tls(listener, max_connections)`; it leaves
listener ownership with the caller. `router.serve_and_close(listener,
max_requests)` and `router.serve_tls_and_close(listener, max_connections)` close
their listeners after the configured bound or a serving error. Use
`HTTPServer.bind_tls(endpoint, chain, key)` to create an HTTP-aware TLS
listener; ALPN negotiates HTTP/2 and falls back to HTTP/1.1. `serve_tls` and
`serve_tls_once` handle one request per accepted connection. For HTTP/2-only
operation on an accepted TLS stream, use `router.serve_tls_many(stream,
max_requests)`: it admits the requested number of streams on one connection,
with at most eight active streams at a time, and sends graceful shutdown after
the final response. A peer disconnect while that final graceful close is being
written is treated as a completed disconnect. The connection is allowed five
seconds to drain; if streams remain, the connection is dropped and the call
returns `HTTPError.Timeout`. It requires negotiated `h2`; it returns a protocol error
if TLS selected HTTP/1.1. For long-lived plain or TLS service, use
`router.serve_forever(listener)` or `router.serve_tls_forever(listener)`.
Both admit one connection at a time until the owner task is cancelled; task
cleanup releases the listener.

A handler has the conceptual form:

```zirk
import { HTTPRequest, HTTPResponse, HTTPRouteParameters } from std.http;

fn get_user(request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse {
    return HTTPResponse.from_text(200, "user");
}

fn main(): Void { }
```

Returning `HTTPResponse` directly is lifted into the expected successful result.
Route handlers run on their owning Zirk task. HTTP/2 framing and flow control
run on the native reactor, with bounded request transfers to that task.
HTTP/2 route handlers run in independently cancelable child tasks, bounded to
eight per connection. Disconnect and drain cancellation propagate to those
tasks at cooperative safe points. An uncaught `Throwable` from a route or
middleware becomes a generic `500 Internal Server Error` response; exception
messages and stack traces are not sent to the client. The request is cleaned
up and the finite server continues to its next admitted request. CPU-only
handlers still cannot be preempted before a cooperative safe point.

## Response construction

The current public constructors are:

```zirk
HTTPResponse.from_bytes(200, body)
HTTPResponse.from_text(201, "Created")
HTTPResponse.from_json(200, serialized_json)
HTTPResponse.try_with_headers(200, body, headers)
```

`from_text` and `from_json` set their matching `Content-Type`. The
`try_from_*` constructors validate the status range and buffered-body limit;
JSON serialization and syntax validation remain the caller's responsibility.
Use `try_with_headers` to attach a validated repeated-header collection.

For a streamed server response, use
`HTTPResponse.from_stream(status, headers, producer)`. The producer receives a
zero-based `UInt64` chunk index and returns one `Array<Byte>`; an empty array
marks end-of-stream. Each chunk is capped at 64 KiB. The producer runs on the
owning Zirk task. HTTP/1.1 writes chunks incrementally, and HTTP/2 queues at
most one native chunk while waiting for flow control. This bounds buffering
without imposing a total response-size limit on a stream. Client-side
`HTTPResponse.stream()` is a separate operation that transfers a buffered
one-shot body to the streaming reader.

## Middleware

The bundled preview installs middleware on an `HTTPRouter`:

```zirk
fn logging(request: HTTPRequest, next: HTTPNext): HTTPResponse {
    stdout.println(request.method());
    mut response: HTTPResponse = next.proceed(request);
    return response;
}

_ = router.add_middleware(logging);
```

Middleware runs in registration order on parsed requests, including requests
that later produce 404 or 405. Responses unwind in reverse order. A middleware
can return early or transform the downstream response. `HTTPNext.proceed`
invokes its continuation at most once; later calls return a bounded 500
response and do not run the downstream handler again. A router accepts at most
256 middleware entries; later registrations return
`HTTPRouteError.MiddlewareLimit`. Handler and middleware cancellation are
cooperative and verified at task safe points. Use `router.route_result` for a
route that returns `Result<HTTPResponse, HTTPError>`, or
`router.add_middleware_result` for result-returning middleware. An `Error`
becomes a generic 500 response without exposing the error value; the listener
continues serving later requests. Decorator-based middleware and strict
preemption of CPU-only work remain future work.

`HTTPRouteHandler.invoke(request, values)` calls one stored route callback with
the supplied ordered parameter values. `HTTPMiddleware.invoke(request, next)`
calls one middleware callback directly. Normal applications should register
these with `HTTPRouter`, which owns path matching, parameter decoding,
registration order, and the continuation chain.

## Compression and representation limits

The native client preview advertises `br`, `gzip`, and `deflate` when the
caller does not provide `Accept-Encoding`. It incrementally decodes one
supported `Content-Encoding` layer on a Tokio worker and removes the encoded
representation's `Content-Encoding` and `Content-Length` fields before
returning the response. A caller-supplied `Accept-Encoding` is validated, and a
response using a coding with quality zero fails with `HTTPError.Protocol`.
Compressed input and expanded output are each limited to 16 MiB; expansion is
also limited to 1000:1. Decoded chunks pass through a two-item bounded queue,
and each worker iteration yields after at most 64 KiB of output. Synchronous
codec polling has a cumulative two-second CPU budget per body. Invalid,
unsupported, or stacked codings fail closed.

The server negotiates `br`, `gzip`, and `deflate` for buffered responses up to
16 MiB, compresses on a Tokio worker, adds `Vary: Accept-Encoding`, and drops
the original `ETag` and `Content-Length` before Hyper writes the encoded
representation. It also compresses streaming channel and producer responses
incrementally, with one input fragment and two output fragments queued at a
time. Generated producers continue to run on their owning Zirk task; the codec
runs on Tokio workers. The codec applies a cumulative two-second CPU budget per
body, excluding async waits. It returns an empty 406 response when identity
is forbidden and no supported coding is acceptable. The server incrementally
decodes one supported `Content-Encoding` layer on incoming requests before
dispatching them, with the same compressed, expanded, ratio, and queue bounds.
Invalid or truncated coding data fails with a typed protocol error; byte-limit
and expansion-ratio violations fail with a typed body-limit error. Nested
content codings are not implemented.

## Proxies, caches, and retries

Proxy use is explicit per-client configuration through `HTTPProxy.create(URL)`
and `HTTPClient.create_with_proxy`. Use
`HTTPClient.create_with_proxy_and_trust_roots(proxy, roots)` when a proxied
HTTPS client must use caller-supplied roots; those roots authenticate HTTPS
proxy and destination TLS peers independently by their own hostnames. The
current preview supports HTTP and HTTPS forward proxies: HTTP requests use
absolute-form targets, and HTTPS uses CONNECT followed by destination TLS.
`HTTPProxy.address()` returns the configured proxy URL without exposing its
separately stored credentials.
Both proxy and destination origins require separate network authority and are
checked for every request. Basic proxy credentials may be supplied through
`HTTPProxy.create_authenticated(URL, SecretString)`. Credentials are accepted
for loopback HTTP proxies or any HTTPS proxy; remote cleartext HTTP proxies are
rejected. For HTTPS destinations through an HTTPS proxy, proxy authorization is
sent inside the proxy TLS connection and is not forwarded through the
destination TLS tunnel. Credentials are not accepted in the proxy URL, and
formatting of the stored authorization value is redacted.
An approved compiled-Zirk loopback test verifies authenticated HTTPS-proxy
`CONNECT`, authenticates both TLS peers with explicit roots, and keeps the
proxy credential out of the destination tunnel.

Caching is opt-in through `HTTP.get_cached(url)` or
`HTTP.request_cached(method, url, body, headers)`. The current native preview
keeps a bounded process-local cache partitioned by the approved project and
manifest. It stores complete GET, HEAD, or QUERY responses with explicit
freshness from `Cache-Control: max-age` or a valid `Expires` date (using
`Date` when present and the response receipt time otherwise),
matches `Vary`, updates `Age`, and refuses authenticated/cookie requests,
`Set-Cookie`, request fields marked sensitive, response `no-store`/`no-cache`,
and `Vary: *`. Cache keys include all `Content-*` request metadata; QUERY keys
therefore distinguish identical body bytes sent with different encodings or
media types. Stale entries revalidate with `ETag` or
`Last-Modified`; a `304 Not Modified` refreshes cached metadata while returning
the stored representation. Request `no-cache` also forces validation. A cache
miss with request `Cache-Control: only-if-cached` returns a local `504` without
contacting the origin; malformed cache directives fail closed. Other methods
passed to `request_cached` are sent normally and never stored. Ordinary
`HTTP.get` and `HTTP.request` never read or write this cache.

The preview follows a conservative subset of [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html)
and applies QUERY cache identity from [RFC 10008](https://www.rfc-editor.org/rfc/rfc10008.html).

Retries are opt-in, bounded to four attempts, and restricted to transport or
timeout failures for safe, idempotent methods with replayable buffered bodies.
The wait before each retry uses a client-handle phase over an exponentially
growing window capped at 400 ms. `Timer.sleep` yields cooperatively, so task
cancellation can interrupt the wait. HTTP response statuses, including
`Retry-After`, are not retried by this policy. POST and other unsafe methods
are rejected when a nonzero retry count is requested.

## SSE

The native preview includes a demand-driven client. `next_event()` reads only
until one complete event is available, so consumers control backpressure:

```zirk
match HTTPEventStream.connect(URL("https://example.com/events")) {
    Ok(events) => {
        loop {
            match events.next_event() {
                Ok(event) => match event {
                    Some(value) => stdout.println(value.data());
                    None => break;
                }
                Error(error) => {
                    stderr.println(error);
                    break;
                }
            }
        }
        _ = events.close();
    }
    Error(error) => stderr.println(error);
}
```

`HTTPEvent` exposes `event_type()`, `data()`, `last_event_id()`, and
`retry_ms()`. `HTTPEventStream.heartbeat_count()` reports the cumulative number
of SSE comment lines observed while reading through the latest event. The
parser handles fragmented LF/CRLF input, multiline data, comments (heartbeats),
persistent reconnect IDs, and retry metadata within fixed line and event bounds.
`HTTPEventStream.reconnect(url, last_event_id)` starts a manual request with the
validated `Last-Event-ID` header. `connect_with_reconnect(url)` automatically
reconnects after EOF or transport/timeout errors, waits according to the latest
valid `retry:` field (3 seconds initially), and sends `Last-Event-ID` when it
is non-empty. Initial network failures are retried too. Delays above
2,147,483,647 milliseconds saturate to that limit. Its timer is cancellation-safe; permission and protocol errors
remain visible, and `204 No Content` closes the stream permanently.
`close()` disables future retries. Closing or canceling the
consumer closes its response. For server-side SSE output, use
`HTTPResponse.from_sse_stream(producer, heartbeat_ms?)`. The response sets
`Content-Type: text/event-stream; charset=utf-8`; return one complete UTF-8
SSE frame per chunk and an empty array at EOF. The optional heartbeat interval
defaults to disabled and accepts values from 1,000 to 3,600,000 milliseconds.
The native reactor emits `: keep-alive` comment frames only while the producer
is inactive, through the same bounded channel used for data. Completion,
disconnect, and cancellation stop the timer. The producer remains on its
owning Zirk task; HTTP/2 queues at most one native chunk while waiting for flow
control. Runtime transport tests verify HTTP/1.1 producers stall when a live peer stops
reading for both identity and gzip responses, and that disconnect releases the
producer capture root. A
compiled-Zirk loopback E2E verifies a configured heartbeat between
two events, the disabled default, and rejection of intervals below one
second and above one hour on macOS aarch64 and Linux ARM64 Docker.

## WebSocket

> **Availability:** `HTTPWebSocket` provides bounded HTTP/1.1 and TLS HTTP/2
> extended CONNECT (RFC 8441) clients, plus an opt-in HTTP/3 client using
> RFC 9220 extended CONNECT when the runtime is built with its `http3` feature.
> The server preview supports HTTP/1.1 and TLS HTTP/1.1 or HTTP/2. Supported
> transports provide typed messages, origin/subprotocol policy, and close
> handshakes.

The server accepts one already-authorized TCP or TLS stream and runs its
callback on the task that owns that stream. It accepts upgrades only at the
exact configured path, checks the request Origin against an exact normalized
allowlist (missing Origin is denied by default), and selects the first offered
subprotocol in server preference order. The callback's `HTTPWebSocket` is
task-owned; if it returns without closing the session, the runtime completes a
bounded close handshake. `max_messages` must be between 1 and 1,024.

```zirk
import { HTTPServer, HTTPWebSocket, HTTPWebSocketMessage } from std.http;
import { TCPListener } from std.net::tcp;

fn echo_one(socket: HTTPWebSocket): Void {
    match socket.receive() {
        Ok(message) => {
            match message {
                HTTPWebSocketMessage.Text(value) => {
                    _ = socket.send(HTTPWebSocketMessage.Text(value));
                }
                _ => { }
            }
        }
        Error(_) => { }
    }
}

fn serve_one(listener: TCPListener): Void {
    mut origins: List<String> = List();
    origins.add("https://app.example");
    mut protocols: List<String> = List();
    protocols.add("zirk.echo.v1");
    match listener.accept() {
        Ok(stream) => {
            _ = HTTPServer.serve_websocket_once(
                stream, "/socket", origins, protocols, 8, echo_one,
            );
        }
        Error(_) => { }
    }
}

fn main(): Void { }
```

Use `HTTPServer.serve_tls_websocket_once` for an accepted `TLSStream`. When
TLS negotiates HTTP/2, it accepts RFC 8441 extended CONNECT; otherwise it uses
the HTTP/1.1 upgrade contract. A client sends extended CONNECT only after the
server advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL`. Bind and accept still
require the corresponding `network.listen.addresses` grant.

The client requires an approved `network.connect.origins` grant, just like
other HTTP clients. Its message type preserves the distinction between text,
binary, Ping, Pong, Close, and end of stream:

Incoming Ping frames are exposed to the application and receive an automatic
protocol Pong. Applications should not send a second Pong for the same Ping.

```zirk
import { HTTPError, HTTPWebSocket, HTTPWebSocketMessage, URL } from std.http;
import { stderr, stdout } from std.io;

fn show_message(message: HTTPWebSocketMessage): Void {
    match message {
        HTTPWebSocketMessage.Text(value) => stdout.println(value);
        HTTPWebSocketMessage.Binary(bytes) => stdout.println(bytes.length);
        HTTPWebSocketMessage.Ping(bytes) => stdout.println("ping {bytes.length}");
        HTTPWebSocketMessage.Pong(bytes) => stdout.println("pong {bytes.length}");
        HTTPWebSocketMessage.Close(code, reason) => {
            stdout.println(code);
            stdout.println(reason);
        }
        HTTPWebSocketMessage.End => stdout.println("end");
    }
}

fn receive_message(result: Result<HTTPWebSocketMessage, HTTPError>): Void {
    match result {
        Ok(message) => show_message(message);
        Error(error) => stderr.println(error);
    }
}

fn use_websocket(url: URL): Void {
    match HTTPWebSocket.connect(url) {
        Ok(socket) => {
            _ = socket.send(HTTPWebSocketMessage.Text("hello"));
            receive_message(socket.receive());
            _ = socket.close();
        }
        Error(error) => stderr.println(error);
    }
}

fn main(): Void { }
```

`connect_with_options(url, origin, subprotocols)` optionally sends a normalized
HTTP(S) Origin (or `"null"`) and a bounded list of unique subprotocol tokens;
the server may select only a protocol offered by the client. Origin and
subprotocol validation happens before network access. The client reassembles
fragmented messages and validates UTF-8, masking, control frames, message
limits, and the closing handshake. Both inbound and outbound messages are
limited to 16 MiB, including the aggregate size of fragmented inbound messages.
The compiled-Zirk loopback test verifies that a 16 MiB + 1 outgoing message
returns `BodyTooLarge` without reaching the peer. Runtime loopback tests verify
that the client and server each reject a 16 MiB fragment followed by one
additional byte as `BodyTooLarge`, before delivering the message. Other tests
verify that the server rejects a disallowed Origin before dispatch, chooses the
preferred offered protocol, dispatches a session on its owner task, and closes
cleanly. A raw HTTP/1.1 upgrade missing `Connection: Upgrade` receives 400.
The TLS server E2E verifies an oversized binary message returns
`BodyTooLarge`, and the message-count limit initiates a 1001 close. An
independent Hyper peer verifies TLS negotiation, RFC 8441 extended CONNECT,
subprotocol selection, a text echo, and the WebSocket close handshake. The
native WebSocket client negotiates HTTP/2 over TLS, waits for
`SETTINGS_ENABLE_CONNECT_PROTOCOL`, validates the response and selected
subprotocol, and ties the connection driver to the socket lifecycle. Runtime
loopback tests verify text/binary exchange, automatic Ping/Pong, close cleanup,
and that a peer without the setting receives no extended CONNECT request. The
full-workspace native matrix on 2026-10-20 also passes on Windows x86_64 and
Linux x86_64, in addition to macOS ARM64 and Linux ARM64.
`connect_with_trust_roots(url, roots_der, allow_tls12)` and
`connect_with_options_and_trust_roots(url, origin, subprotocols, roots_der,
allow_tls12)` allow applications to supply bounded DER trust roots instead of
platform roots. A compiled-Zirk macOS loopback E2E verifies TLS certificate
validation with a caller-provided root, ALPN negotiation, RFC 8441 extended
CONNECT, Origin and subprotocol negotiation, text exchange, and close. The
implemented preview passes the native full-workspace matrix on all four
supported targets. Extended hostile HTTP/2 combinations beyond the recorded
fixtures, mutual TLS client authentication, and other explicitly staged
protocol contracts remain outside the current preview.

`close()` sends a normal close handshake. `close_with(code, reason)` validates
the application-selected close code and UTF-8 reason, sends the close frame,
and waits for the peer's close response within the close timeout.

`connect_http3(url, origin, subprotocols)` forces RFC 9220 WebSocket over
HTTP/3. It requires an `https` URL, the same exact-origin network grant as
other HTTP clients, TLS 1.3, and the runtime's optional `http3` feature. The
runtime validates the negotiated subprotocol and keeps the QUIC/H3 driver alive
for the WebSocket session. A runtime loopback peer verifies the extended
CONNECT `:protocol = websocket` handshake and bidirectional frame exchange.
`connect_http3_with_trust_roots(url, origin, subprotocols, roots_der)` replaces
system trust with the provided bounded DER certificate roots. It always uses
TLS 1.3; TLS 1.2 cannot be enabled for HTTP/3. Protocol fallback is not exposed
by these explicit HTTP/3 methods. Servers can expose one RFC 9220 session with
`HTTPServer.serve_http3_websocket_once(endpoint, certificate_chain_der,
private_key_der, path, allowed_origins, protocols, max_messages, handler,
allow_missing_origin, idle_timeout_ms)`. The endpoint uses the explicit UDP
bind grant and TLS 1.3; the handler owns the session on its Zirk task. Path,
Origin, and subprotocol policy are checked before the callback. The compiler
build fixture verifies this method lowers to its runtime ABI. Two separately
compiled Zirk programs have also completed an HTTP/3 WebSocket echo and close
handshake on macOS ARM64 with the `http3` runtime feature. The checked-in
`scripts/test-http3-websocket-public-api.py` reproduces the exchange and tests
both directions against independent `aioquic` 1.7.0 client and server peers;
it also verifies Zirk HTTP/3 GET and QUERY requests against the independent
server, including the query body and content type. CI installs the pinned
test-only peer and runs it on each native matrix target.
The runtime loopback test also covers the server codec, bidirectional messages,
and close lifecycle through the adapter. The independent peer test verifies
that an unapproved Origin receives `403` before the Zirk handler runs.
Independent-peer results are currently verified only on macOS ARM64;
Linux/Windows runtime results and broader adversarial peer cases remain
pending.

## Protocol versions

The current server preview supports HTTP/1.1 and TLS ALPN negotiation of HTTP/2.
`serve_tls_many` handles a caller-bounded number of multiplexed streams on one
accepted HTTP/2 TLS connection and gracefully shuts it down after the final
response. Handlers run in separate Zirk tasks on the same executor, up to the
eight-stream admission limit; a compiled-Zirk loopback test verifies two
handlers overlap on macOS aarch64 and Linux ARM64. Long-lived listener
admission, complete client interoperability, flow-control/reset conformance, and hostile pseudo-field
coverage remain pending.

HTTP/3 client and listener APIs are available only when the native runtime is
built with the optional `http3` feature. An `HTTPClient` retains at most one
HTTP/3 session per configured origin and uses independent streams for
concurrent requests. Focused loopback tests verify concurrent streams share
one connection and sequential requests reuse the same pool connection.
Automatic HTTP/3 negotiation and fallback, externally requested graceful
listener shutdown, per-stream disconnect cancellation, cross-target
independent-peer interoperability, and Windows runtime verification remain in
progress. The finite HTTP/3 listener
admits connections concurrently with at most 16 QUIC handshakes in flight. It
dispatches at most four active request streams across the listener to separate
Zirk handler tasks on the owning executor and cancels pending handshakes when
the configured request limit is reached. At the existing 16 MiB per-body
limit, buffered request and response vectors are capped at 128 MiB per
listener, in addition to Quinn's transport windows.

## Browser execution

The browser facade provides buffered `HTTP.get`, `HTTP.head`, `HTTP.query`,
`HTTP.query_with_content_type`, `HTTP.post`, `HTTP.post_json`,
`HTTP.post_bytes`, and `HTTP.request` through `BrowserApi.runWithFetch`. Each
call receives a Fetch function, an explicit
JSON origin allowlist, request and response byte ceilings, and an optional
`AbortSignal`. The Fetch capability is discarded when that invocation ends;
the synchronous `run` method remains default-deny. Request and response
bodies are bounded, response streams are cancelled when the configured byte
limit is exceeded, redirects are rejected by Fetch, and Fetch/CORS failures
map to `HTTPError.BrowserPolicy`. Buffered response text is decoded as strict
UTF-8. JSON query and POST helpers validate their JSON document before
calling Fetch; `QUERY` and the POST convenience methods preserve their
documented media types. Native sockets, listeners, streaming request helpers,
and WebSocket remain unsupported by this browser host path.

Run the JavaScript/WASM boundary checks with
`scripts/test-browser-http-fetch.sh`. The script builds the browser crate for
`wasm32-unknown-unknown`, generates its Node.js bindings with the matching
`wasm-bindgen` CLI, and checks request forwarding, convenience methods,
response decoding, origin denial, body limits, CORS error mapping, and cancellation. This is a Fetch
adapter test under Node.js; it does not claim to verify browser-enforced CORS
against a live remote origin.

## Security and permissions

Client operations require `network.connect.origins`; server binds require
`network.listen.addresses`. Package requirements are collected from the
application's locked dependency graph and require explicit application
approval; package declarations never grant authority. Every redirect, DNS result, reconnect, proxy hop,
TLS server name, and WebSocket destination is validated before its external
effect. URL user information, authorization, cookies, proxy credentials, and
security tokens are redacted from diagnostics. Core header values mark
`Authorization`, `Proxy-Authorization`, `Cookie`, and `Set-Cookie` as sensitive,
so derived request and response debug output does not include their contents.
Other application-defined secret values should use `SecretString` and its
explicit write-only sinks. The HTTP `SecretString` header sink also marks the
chosen field sensitive even when its name is application-defined, such as
`X-Api-Key`.

The HTTP/1.1 parser rejects ambiguous framing, conflicting content lengths,
invalid transfer coding, obsolete folding, invalid whitespace, and forbidden
field bytes. HTTP/2 validates pseudo-fields, lowercase names, field order, and
connection-specific field exclusions. A raw-frame runtime test confirms that
duplicate `:method`, a recognized pseudo-field after a regular field, and an
unknown pseudo-field are rejected before application dispatch on macOS and
Linux ARM64. Broader HTTP/2 conformance remains pending. A malformed message is rejected and the
connection is closed or reset when reuse could permit request smuggling.

The native server enforces fixed runtime bounds for request lines and URLs,
header count and bytes, buffered and decoded bodies, form and multipart data,
uploads, routes, and active protocol streams. The current API does not expose a
general `HTTPServerOptions` type for changing those limits. Callers can bound
the number of accepted requests or streams with the `max_requests` arguments
on finite serving methods; long-lived sequential serving methods admit one
accepted connection at a time. Handler deadlines are cooperative and can be
exceeded by a handler that does not reach a cancellation point.

## Errors and cancellation

The current bundled API returns the closed `HTTPError` enum for URL,
permission, transport/protocol, body, timeout, cancellation, redirect, and
method failures. `HTTPHeaderError`, `HTTPRouteError`, and
`HTTPStreamProducerError` describe header validation, route registration, and
stream-producer outcomes. The broader target contract may split these into
more specialized error families; those families are not current public types.
HTTP status codes remain ordinary responses rather than transport failures.

`HTTPClientOptions` currently configures bounded connect,
response-header, and response-idle timeouts, independently from the bounded
origin pool. Cancellation stops pending DNS/connect work, resets or closes the
affected protocol stream, releases pool capacity, removes temporary resources,
and never leaves detached handlers.

These client timeout settings are not server limits. The server's protocol
bounds remain fixed runtime policy; only request/stream counts exposed by a
serving method are caller-configurable in the current API.

## Standards baseline

The normative behavior follows [HTTP Semantics](https://www.rfc-editor.org/rfc/rfc9110.html)
and [Caching](https://www.rfc-editor.org/rfc/rfc9111.html),
[HTTP/1.1](https://www.rfc-editor.org/rfc/rfc9112.html),
[HTTP/2](https://www.rfc-editor.org/rfc/rfc9113.html), the
[HTTP QUERY method](https://www.rfc-editor.org/rfc/rfc10008.html), current
cookie rules, the [WebSocket protocol](https://www.rfc-editor.org/rfc/rfc6455.html)
and its HTTP/2 extension, and the
[WHATWG SSE event-stream format](https://html.spec.whatwg.org/multipage/server-sent-events.html).
Where a platform API disagrees, Zirk preserves this public contract or reports
the feature as unsupported.

---

**Previous:** [← std.net](11-std-net.md) · **Next:** [std.json →](13-std-json.md)
