# `std.http`

`std.http` provides an HTTP client and server built on [`std.net`](11-std-net.md).
It includes immutable URLs, validated headers, typed methods and status codes,
streaming bodies, connection pooling, cookies, redirects, middleware, Server-Sent
Events (SSE), and WebSocket upgrades. It supports HTTP/1.1 and HTTP/2 through one
application API.

All waiting operations suspend tasks, body flow applies backpressure, disconnects
cancel their structured work, and network permissions are revalidated at every
dynamic destination. Framework routing, dependency injection, templating, and ORM
behavior belong in packages built on this module.

> **Implementation status:** this page is the normative public contract. The
> current implementation may support only a subset. Missing functionality must be
> reported explicitly and must never weaken parsing, TLS, permissions, limits, or
> cancellation behavior.

## Imports and naming

Namespace and direct imports expose the same symbols:

```zirk
import { HTTP } from std.http;
import { HTTPClient, HTTPMethod, URL } from std.http;

mut first = HTTP.get(URL("https://example.com"));
mut second = HTTPClient.create().get(URL("https://example.com"));
```

HTTP-family acronyms remain uppercase: `HTTP`, `HTTPS`, `URL`, `URI`, `SSE`,
`WebSocket`, `HTTPClient`, `HTTPRequest`, and `HTTPResponse`.

## Methods, including `QUERY`

`HTTPMethod` includes the standard methods `Get`, `Head`, `Post`, `Put`,
`Patch`, `Delete`, `Options`, `Connect`, `Trace`, and `Query`. A generic
`request` API accepts any registered or extension method, while dedicated
methods provide the ordinary spelling:

```zirk
client.get(url:)
client.post(url:, json: command)
client.query(url:, json: filter)
client.request(HTTPMethod.Put, url:, json: replacement)
```

`QUERY` follows RFC 10008. It asks the target resource to process enclosed
query content without changing resource state. It is safe, idempotent, and
cacheable; unlike `GET`, its content has defined semantics, and unlike `POST`,
it may be retried automatically when the retry contract permits. A QUERY with
content requires a matching `Content-Type`. Servers may advertise accepted
formats through `Accept-Query`.

```zirk
mut filter = {
    "published_after": "2026-01-01",
    "tags": ["zirk", "language"],
};

match await HTTP.query(
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
escaping. Parsing rejects malformed syntax and credentials where the selected
policy forbids them. It distinguishes scheme, host, port, path segments, query,
and fragment and resolves relative references according to the URI standard.

```zirk
match URL.parse("https://api.example.com/users?page=2") {
    Ok(url) => {
        mut next = url.with_query("page", "3");
        stdout.println(next.origin);
    }
    Error(error) => stderr.println(error);
}
```

Builders such as `with_query`, `without_query`, `with_path`, and
`resolve(relative)` produce a new URL. Query encoding is explicit and never
confused with route parameters. `URL.origin` contains the normalized scheme,
host, and effective port used by permissions, cookie policy, pooling, and
redirect validation.

## One-shot requests

`HTTP.get`, `HTTP.query`, and the other namespace methods create one logical
request with secure defaults:

```zirk
mut url = URL("https://api.example.com/profile");

match await HTTP.get(url:, timeout: 10s) {
    Ok(response) => {
        using response {
            match await response.json<Profile>() {
                Ok(profile) => stdout.println(profile.name);
                Error(error) => stderr.println(error);
            }
        }
    }
    Error(error) => stderr.println(error);
}
```

One-shot calls do not retain cookies between requests. Implementations may use a
shared safe transport pool internally, but authentication, cookie, proxy, and
other caller-visible state never leaks between unrelated calls.

## Reusable clients

`HTTPClient.create` makes persistent policy explicit:

```zirk
mut client = HTTPClient.create(
    timeout: 10s,
    redirects: RedirectPolicy.safe(),
    cookies: CookieJar(),
);

using client {
    mut result = await client.get(URL("https://example.com"));
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

High-level body parameters are mutually exclusive:

```zirk
client.post(url:, json: user)
client.post(url:, text: message)
client.post(url:, bytes: content)
client.post(url:, form: fields)
client.post(url:, multipart: parts)
client.post(url:, stream: source)
```

Passing more than one is a compile-time error when statically visible and a
typed configuration error otherwise. `json` uses the generated `JSONCodec<T>`;
`text` defaults to UTF-8; `form` uses form URL encoding; and `multipart` creates
a boundary that cannot collide with its encoded parts. Streaming bodies do not
require their full content in memory and apply backpressure to their producer.

A retryable request body must be replayable. Bytes, text, JSON, and immutable
form values are replayable; a one-pass stream is not. Retrying a non-replayable
body fails explicitly rather than sending an incomplete request.

## Headers

`HTTPHeaders` is case-insensitive for lookup while preserving valid values and
multiple-field semantics. It validates field names and rejects CR, LF, NUL, and
other forbidden bytes to prevent response splitting and request smuggling.

```zirk
mut headers = HTTPHeaders([
    ("accept", "application/json"),
    ("user-agent", "zirk-example/1.0"),
]);

headers.set("content-type", "application/json");
headers.add("accept", "application/problem+json");

mut content_type = headers.get("content-type");
mut cookies = headers.get_all("set-cookie");
```

`set` replaces the field's values, `add` appends one, `get` returns
`Option<String>`, and `get_all` returns all values in wire order. The collection
also accepts a list or iterable of name/value pairs during construction or
`add_all`. Fields whose grammar forbids combination, especially `Set-Cookie`,
are never incorrectly comma-joined.

Applications cannot directly emit HTTP/1.1 framing fields that conflict with
the selected body. The protocol layer owns `Content-Length`, transfer coding,
HTTP/2 pseudo-fields, connection-specific fields, and header normalization.

## Responses and body consumption

`HTTPResponse` exposes `status`, `headers`, protocol version, final URL, redirect
history, and one body. The body is consumed exactly once through one of:

```zirk
response.text()
response.bytes()
response.json<User>()
response.stream()
```

Buffered readers enforce configured size limits. `text` validates the declared
or selected encoding; `json<T>` uses strict JSON decoding. `stream` transfers
body ownership to a bounded asynchronous byte stream. A second body access
returns `HTTPBodyConsumedError`.

Closing a response before consuming its body either drains only within a small,
safe bound or closes/resets the transport, preventing unbounded cleanup work and
pool poisoning.

## Status codes

`HTTPStatus` is an integer-backed enum. Every standard case carries its actual
wire code, so `HTTPStatus.Ok` corresponds to `200`, not the text `"OK"`.
Unknown extension codes remain representable and are not collapsed into an
unrelated standard case.

```zirk
match response.status {
    HTTPStatus.Ok => stdout.println("success");
    HTTPStatus.NotFound => stdout.println("missing");
    status if status.is_server_error => stderr.println(status.code);
    _ => {}
}
```

The properties `is_informational`, `is_success`, `is_redirect`,
`is_client_error`, and `is_server_error` derive from the numeric class.
Reason phrases are optional presentation metadata and never determine equality
or matching.

## Redirects

Redirect behavior is selected with `RedirectPolicy.none()`, `.safe()`, or
`.limited(count)`. The safe policy applies a small finite limit, detects loops,
and follows only redirects permitted by method semantics. It removes credentials
and sensitive headers when authority changes, validates the new URL, repeats DNS
and network permission checks, and performs fresh TLS identity validation.

Redirect rewriting follows HTTP semantics. In particular, `307` and `308`
preserve method and body; a body is followed only when it is replayable. QUERY
redirects follow RFC 10008, including indirect-result behavior, without being
rewritten into a state-changing method.

## Cookies

`CookieJar` is explicit mutable client state:

```zirk
mut jar = CookieJar();
mut client = HTTPClient.create(cookies: jar);
```

It implements domain, path, expiry, `Secure`, `HttpOnly`, `SameSite`, prefix,
and public-suffix rules. Secure cookies are never sent over plaintext HTTP.
Cross-origin redirects recalculate cookie eligibility. Persistence is opt-in,
encrypted when configured, and subject to filesystem and secret permissions;
the default jar is memory-only and bounded.

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

Form and multipart parsing is streaming and bounded. Uploaded files are managed
resources and temporary files are removed on success, error, or cancellation.
File names are untrusted metadata and are never interpreted as destination
paths automatically.

## Servers and routing

`HTTPServer` is configured explicitly and served as a structured task:

```zirk
mut server = HTTPServer(
    address: SocketAddress("127.0.0.1", 8080),
);

server.route(HTTPMethod.Get, "/users/:id", get_user);
server.route(HTTPMethod.Query, "/users", query_users);

// Equivalent convenience registrations.
server.get("/health", health);
server.query("/users", query_users);

await server.serve();
```

`HTTPMethod.Get` and `HTTPMethod.Query` are enum cases, not language keywords.
The convenience methods register the same route representation. Duplicate or
ambiguous routes are configuration errors; registration order must not silently
change which structurally equivalent route wins.

A handler has the conceptual form:

```zirk
fn get_user(request: HTTPRequest): Result<HTTPResponse, HTTPError> {
    // ...
}
```

Returning `HTTPResponse` directly is lifted into the expected successful result.
Each request handler runs inside the server's structured scope. A disconnect
cancels request work and body streams; shutting down stops accepting requests,
allows a bounded grace period, then cancels remaining handlers. CPU-heavy work
uses `parallel`; the server never creates one thread per request.

## Response construction

Typed constructors set valid content metadata:

```zirk
HTTPResponse.json(user)
HTTPResponse.text("Created", status: HTTPStatus.Created)
HTTPResponse.bytes(content)
HTTPResponse.stream(source)
HTTPResponse.redirect(url)
```

JSON serialization errors, invalid redirects, and streaming failures stay typed.
Headers can be supplied as `HTTPHeaders` or a list of pairs. A response cannot
combine incompatible body, status, or framing configuration.

## Middleware

Middleware may be installed globally or through decorators:

```zirk
server.use(logging);
server.use(authentication);

@Use(logging, authentication)
fn get_user(request: HTTPRequest): Result<HTTPResponse, HTTPError> {
    // ...
}
```

Request traversal follows declaration order; response and error traversal
returns in reverse order. Middleware receives an explicit continuation and must
call it at most once. It can return a response early, transform a response, or
propagate a typed error. Decorator ordering follows the language's canonical
decorator composition rules rather than defining a second HTTP-only model.

## Compression and representation limits

Clients advertise only enabled encodings and decode supported content codings
incrementally. Servers negotiate representations from validated request fields.
Compressed bytes, expanded bytes, expansion ratio, CPU work, and nesting are
bounded to prevent decompression bombs. Integrity and size errors discard the
affected connection when safe reuse cannot be proven.

Automatic compression is disabled for already compressed content and may be
disabled for secrets to reduce compression side channels. Content negotiation
returns a typed not-acceptable or unsupported-media response rather than
guessing an incompatible representation.

## Proxies, caches, and retries

Proxy use is explicit client configuration. Proxy destinations require their own
network authority; tunneled destinations retain destination permission and TLS
validation. Credentials are scoped to the proxy origin and redacted.

Caching is opt-in and follows HTTP cache-control, validators, `Vary`, freshness,
and authorization rules. Cache keys include all representation-selecting state
and never mix authenticated principals. QUERY responses are cacheable according
to RFC 10008 and can identify equivalent resources through `Location` and
`Content-Location`.

Retries are bounded, jittered, cancellation-aware, and restricted to safe or
idempotent methods with replayable bodies. POST is not automatically retried
unless the caller supplies a protocol-specific idempotency contract. `Retry-After`
is honored within the caller's deadline.

## SSE

`SSEResponse` produces a `text/event-stream` response from a bounded stream of
events:

```zirk
mut events = updates.map(fn(update) => SSEEvent(
    event: "update",
    data: JSON.stringify(update),
));

return SSEResponse(events);
```

Event data, identifiers, retry hints, heartbeat behavior, and reconnection
metadata follow the SSE format. Disconnect cancels the producer. Backpressure
prevents a slow peer from creating an unbounded event queue.

## WebSocket

`WebSocket.upgrade(request)` validates the HTTP handshake and transfers the
connection into a message-oriented full-duplex resource:

```zirk
match WebSocket.upgrade(request) {
    Ok(socket) => {
        using socket {
            match await socket.receive() {
                Value(message) => await socket.send(message);
                End => {}
                Error(error) => stderr.println(error);
            }
        }
    }
    Error(error) => return HTTPResponse.text(
        "Invalid WebSocket upgrade",
        status: HTTPStatus.BadRequest,
    );
}
```

Text and binary messages remain distinct. UTF-8, masking, fragmentation,
control frames, origin policy, subprotocols, message sizes, queues, heartbeat,
and closing handshake are validated and bounded. HTTP/2 extended CONNECT is
supported where negotiated without changing the application API.

## Protocol versions

HTTP/1.1 and HTTP/2 share the same request/response API. Negotiation uses ALPN
under TLS and standards-compliant cleartext behavior where explicitly enabled.
HTTP/2 multiplexing, flow control, stream reset, header compression, and
connection shutdown map into the same task, cancellation, and backpressure
contracts.

HTTP/3 is not part of the core module. It ships with QUIC as an official package
that implements the same high-level client/server contracts, allowing its
protocol and deployment cadence to evolve independently.

## Security and permissions

Client operations require `network.connect.origins`; server binds require
`network.listen.addresses`. Every redirect, DNS result, reconnect, proxy hop,
TLS server name, and WebSocket destination is validated before its external
effect. URL user information, authorization, cookies, proxy credentials, and
security tokens are redacted from diagnostics.

The HTTP/1.1 parser rejects ambiguous framing, conflicting content lengths,
invalid transfer coding, obsolete folding, invalid whitespace, and forbidden
field bytes. HTTP/2 validates pseudo-fields, lowercase names, field order, and
connection-specific field exclusions. A malformed message is rejected and the
connection is closed or reset when reuse could permit request smuggling.

Servers enforce independent bounds for start lines, URL length, field count,
field bytes, body bytes, decoded bytes, multipart parts, uploads, routes,
concurrent requests, queued requests, handler duration, and idle connections.
Defaults are safe and configurable downward or upward within platform policy.

## Errors and cancellation

The principal families are `URLError`, `HTTPConfigurationError`,
`HTTPConnectError`, `HTTPProtocolError`, `HTTPHeaderError`, `HTTPBodyError`,
`HTTPBodyConsumedError`, `HTTPRedirectError`, `HTTPCookieError`,
`HTTPTimeoutError`, `HTTPPermissionError`, `HTTPServerError`, `SSEError`, and
`WebSocketError`. They implement `HTTPError` while preserving stable category,
safe endpoint context, operation, and causal network/TLS error.

Timeout configuration distinguishes connect, TLS handshake, response headers,
body inactivity, total request, queue, and server idle timeouts. The shorthand
`timeout:` sets the total request deadline without erasing tighter safety
defaults. Cancellation stops pending DNS/connect work, resets or closes the
affected protocol stream, releases pool capacity, removes temporary resources,
and never leaves detached handlers.

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
