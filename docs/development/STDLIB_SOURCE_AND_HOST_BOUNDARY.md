# Standard library source and host boundary

This note describes what an installed Zirk user can inspect in source and where host-dependent behavior runs. It is an implementation snapshot, not a promise that every portable algorithm has already moved into Zirk.

## What ships with the compiler

The public package sources live under `stdlib/std/{io,environment,net,http}`. Imports such as `std.http` resolve to compiler-owned standard package roots in `crates/zirk-cli/src/modules.rs`; `standard_source` uses `include_str!` for the same `.zrk` sources so browser and native package graphs can share them. The installed compiler therefore needs no separate standard-library install. In the current packaging model, those source files are embedded in the compiler and are not automatically copied out as editable `.zrk` files beside the installed executable.

## Responsibility split

| Layer | Source of truth | Responsibility |
| --- | --- | --- |
| Public API and portable policy | `stdlib/std/**/*.zrk` | Public types and methods; validation and policy that are expressed in Zirk; permission-facing APIs and error shapes. |
| Closed compiler binding registry | `crates/zirk-sema/src/stdlib.rs` and `crates/zirk-codegen-llvm/src/runtime.rs` | Recognizes official standard declarations and maps compiler-owned operations to ABI symbols. Source code cannot declare an arbitrary native symbol to gain host authority. |
| Host/runtime operations | `crates/zirk-runtime/src/environment.rs`, `network_api.rs`, `network_transport.rs`, `http_api.rs`, `http_runtime.rs`, and `http3_transport.rs` | Process environment access, OS sockets and interface queries, TLS, protocol engines, and bounded async I/O. |

This is the same visibility model as `std.io`: users can review the Zirk-facing declarations, while OS calls and compiler-private bindings stay in the trusted runtime. The source bodies of `_native` declarations are placeholders; their actual behavior comes from the closed compiler binding, not those fallback return expressions.

Long-lived HTTP servers expose `HTTPServerControl` and validated `HTTPServerOptions` in bundled Zirk. The control can be shared across structured tasks and requests shutdown idempotently. TCP and TLS accept adapters wait on that scoped capability; HTTP/3 also uses it to stop admission and drain bounded handler tasks. `HTTPRouter` provides controlled TCP, TLS, and HTTP/3 entry points. The listening socket remains task-owned and is created by the task that serves it.

## Current compiler-owned host binding inventory

The LLVM runtime registry currently contains 116 distinct `zirk_environment_*`, `zirk_network_*`, and `zirk_http_*` symbols. The exact registered names are listed below by subsystem. This includes utility and resource-lifecycle calls as well as transport calls; a symbol name alone does not imply that its operation is a direct OS syscall.

The 116-symbol inventory is covered by the audit matrix below. Each symbol is
classified by exact name or family, and each family records its host/protocol
role, input bounds, permission checks, cancellation behavior, and resource
ownership. The host boundary is limited to OS access, the HTTP protocol engine,
and validated compiler-owned transport operations; portable construction and
policy code remains in bundled Zirk where that is a good fit. URL-encoded form
serialization, repeated-field selection, route-pattern validation and conflict
detection, multipart encoding, and `HTTPHeaders.encode_for_transport` now run
in bundled Zirk. The host still validates encoded bytes at the transport
boundary. Parsing and URL normalization stay in the bounded HTTP protocol
engine because they require strict percent-encoding, UTF-8, and URI semantics;
they do not grant host authority or perform OS access.
Same-method route conflict detection and `Allow`-header policy are now
implemented in `HTTPRouteMatcher` as bundled Zirk code; their compiler-owned
ABI symbols and lowering paths have been removed. The multipart request
encoder now also runs in `HTTPMultipartBuilder.encode` as bundled Zirk source;
its dedicated compiler IR instruction and runtime ABI have been removed.
Multipart parsing and temporary upload-file access remain audited host
operations with explicit body/chunk ceilings and task-owned cleanup.

### Binding audit: environment (4 of 116)

All four environment leaves have synchronous, bounded host contracts. Their
implementations are in `crates/zirk-runtime/src/environment.rs`, with shared
validation and authorization rules in `crates/zirk-runtime-core/src/environment.rs`.
They do not suspend, so there is no in-flight cancellation state or OS handle
to release. Process storage itself remains outside Zirk's zeroization
guarantee.

| Binding | Host operation | Permission and bounds | Secret/resource handling |
| --- | --- | --- | --- |
| `zirk_environment_get` | Reads one process variable by exact name. | Checks the authenticated exact-name read grant before lookup; rejects invalid names, invalid UTF-8, and values above 1 MiB. | Returns one managed Zirk string; temporary decoded host bytes are zeroized. |
| `zirk_environment_get_secret` | Reads one process variable by exact name. | Checks the authenticated secret-name grant before lookup; uses the same name and 1 MiB value limits. | Zeroizes the temporary Rust `String` after copying into the managed Zirk heap. The Zirk value is still available to program code and must be redacted by output sinks. |
| `zirk_environment_contains` | Checks presence without returning the variable value. | Checks the ordinary exact-name read grant before lookup; validates host value encoding and the 1 MiB limit. | Zeroizes the temporary OS value and returns only a Boolean. |
| `zirk_environment_list_names` | Enumerates process environment names. | Requires explicit list authority or at least one exact read/secret scope; returns only authorized names and filters `ZIRK_RUNTIME_*`. | Discards and zeroizes enumerated values; does not return values or retain an OS resource. |

Relevant evidence includes `denied_lookup_never_inspects_the_environment_snapshot`,
`denied_listing_never_inspects_the_environment_snapshot`, and the environment
ABI tests in `crates/zirk-runtime/src/environment.rs`.

### Binding audit matrix (116 of 116)

The exact names in the three inventories below are partitioned by these audit
groups; each listed symbol is covered once. Limits named here are enforced by
the runtime or shared runtime-core validators before an OS effect or managed
allocation. `current_task()` is the authority for task-owned resources; no
global registry lock is held while a network operation suspends.

| Exact binding subset in inventory | Host/protocol role and bounds | Permission, cancellation, and ownership review |
| --- | --- | --- |
| `zirk_environment_*` (4 exact names) | Process environment lookup/listing; names, encoding, and values are validated, with values capped at 1 MiB. | Exact-name read/secret grants or explicit listing authority are checked before returning data. Operations are synchronous; values are zeroized in temporary host storage; no OS handle is retained. |
| `zirk_network_ip_address_parse`, `zirk_network_tcp_origin_normalize` | Pure address/endpoint normalization; no socket operation. Input strings are constrained by managed Zirk heap limits and parser validation. | No network permission needed; synchronous and resource-free. |
| `zirk_network_dns_resolve` | Bounded DNS resolver; returns at most 32 canonical candidates and retains at most 256 cache entries. | Revalidates the exact origin grant for resolved candidates before connect; resolver waits use network timeouts and task cancellation; no socket or resolver handle escapes. |
| `zirk_network_interfaces_snapshot` | OS interface enumeration, filtered into at most 256 immutable records with names capped at 256 bytes. | Requires independent `network.inspect.interfaces` authority before enumeration; synchronous snapshot owns no handles and does not confer connect/listen rights. |
| `zirk_network_tcp_connect`, `zirk_network_tcp_listen`, `zirk_network_tcp_accept`, `zirk_network_tcp_accept_until_shutdown` | TCP socket creation/admission; endpoint parsing and transport timeouts apply. TCP listener/stream registries are capped at 1,024 handles each. | Connect/listen grants are checked before socket calls. Suspendable connect/accept is cancellation-aware; listeners and accepted streams are task-owned, owner-checked, and closed on task exit. Controlled accept observes only a scoped HTTP server control. |
| Remaining `zirk_network_tcp_*` operations | Bounded read/write, shutdown, close, closed-state, and peer-address operations on an existing stream/listener. Reads/writes are capped at 1 MiB per call. | Handle lookup checks current owner before temporarily taking the stream; suspension does not hold registry mutexes. Close is idempotent and task exit drops all remaining resources. Peer metadata reveals only the connected peer address. |
| `zirk_network_tls_connect*`, `zirk_network_tls_upgrade_tcp` | Authenticated TLS client handshake with validated trust roots, ALPN, protocol bounds, and optional client identity; upgrade consumes the owner task's existing TCP stream. DER chains/keys and protocol lists are capped in transport constructors. | Exact connect grant is checked before socket effects. Handshakes suspend cooperatively and cancellation drops the in-flight socket; successful streams remain owner-checked. No verification bypass or public key logging exists. |
| `zirk_network_tls_listen*`, `zirk_network_tls_accept*` | TLS listener construction and bounded accept; server identity and optional client trust roots are parsed before bind. | Explicit listen grant is checked before bind; TLS 1.2 requires opt-in, client authentication is opt-in, and peer metadata is exposed only after validation. Listener/stream ownership, controlled accept, cancellation, and task-exit cleanup match TCP. |
| Remaining `zirk_network_tls_*` operations | Bounded TLS read/write/shutdown/close, listener state, peer address, negotiated ALPN, and validated peer-certificate digest. Reads/writes are capped at 1 MiB per call. | Current-task owner checks protect every handle; reads/writes suspend without retaining registry locks, and close/task exit releases TLS state and underlying TCP resources. |
| `zirk_network_udp_connect`, `zirk_network_udp_bind`, `zirk_network_udp_send*`, `zirk_network_udp_receive` | UDP creation and datagram operations; payloads are bounded to IPv4/IPv6 protocol maxima (65,507/65,527 bytes); receive reports truncation instead of silently returning partial datagrams. | Connected destinations and bound addresses use distinct exact grants; broadcast/multicast require separate configuration and membership grants. Operations suspend cancellably; sockets and memberships are owner-task resources removed at task exit. |
| Remaining `zirk_network_udp_*` operations | Close, state, local address, and multicast membership lifecycle. | Owner validation precedes every change; membership is removed on leave, close, cancellation, or task exit. The 1,024-handle ceiling bounds retained socket state. |
| `zirk_network_local_connect`, `zirk_network_local_bind`, `zirk_network_local_accept` | Local stream/endpoint creation and bounded accept; canonical endpoint validation and stale-path policy are enforced by `local_transport`. | Connect and listen grants are distinct and checked before touching the endpoint. Creation does not follow symlinks; cleanup removes only the inode created by the listener. Streams/listeners are owner-task resources with cancellation-safe accept and deterministic task-exit close. |
| Remaining `zirk_network_local_*` operations | Bounded read/write/shutdown/close and closed-state checks. | Owner checks run before I/O; no global lock is held across suspension. Listener cleanup uses inode identity checks; stream close does not delete another listener's path. |
| `zirk_http_url_normalize`, `zirk_http_url_resolve`, `zirk_http_route_match` | Pure URL/router parsing; validation rejects malformed inputs and route parameters; these operations do not open sockets or invoke callbacks. | No ambient permission is read or granted. Inputs are managed strings; parsers return typed failures and allocate managed results. Route matching remains in the audited protocol engine until the full percent-decoding contract is expressible in Zirk. |
| `zirk_http_form_parse`, `zirk_http_multipart_parse`, `zirk_http_response_text`, `zirk_http_response_header_values` | Bounded format decoders over managed request/response values; form parsing caps at 1 MiB/256 fields, multipart at 1 MiB/256 parts/16 KiB part headers, response bodies at 16 MiB, and headers at 64 KiB/256 entries. | No network authority is acquired. Body/header sizes and counts are checked before output allocation; parsing is synchronous and retains no resource. |
| `zirk_http_upload_read`, `zirk_http_upload_close` | Temporary-file access for multipart uploads; reads are capped at 64 KiB and offsets are checked against file size. | Upload handles are capped at 256 and tied to the creating task. Close/task exit deletes the owned temporary file; foreign-task reads/closes fail without consuming the resource. |
| `zirk_http_client_create*`, `zirk_http_client_enable_cookies`, `zirk_http_client_clear_cookies`, `zirk_http_client_close` | Client/pool construction and lifecycle; at most 256 clients and 32 origins per client, with validated timeouts, protocol, proxy, TLS, cookie, and request-slot limits. | Construction has no network side effect. Requests revalidate grants even on reuse. Client handles are executor/project/manifest scoped; cookie state is caller-owned and memory-only; close/task exit drops sessions. |
| `zirk_http_get`, `zirk_http_get_cached`, `zirk_http_head`, `zirk_http_query`, `zirk_http_query_with_content_type`, `zirk_http_post`, `zirk_http_post_bytes`, `zirk_http_post_json`, `zirk_http_request`, `zirk_http_request_cached`, `zirk_http_request_secret`, `zirk_http_client_request` | HTTP request dispatch through HTTP/1.1, HTTP/2, or optional HTTP/3. Header count/bytes are capped at 256/64 KiB; buffered body and response are 16 MiB; redirects, coding depth, concurrency, and phase timeouts are bounded. | Each redirect hop and proxy target is authorized before connect/send; redirects strip sensitive headers across origins. Awaited I/O suspends and cancellation tears down pending requests. Client/response stream handles have explicit owner/lifetime cleanup. Secret variants use redaction-aware paths. |
| `zirk_http_get_stream`, `zirk_http_request_stream`, `zirk_http_client_request_stream`, `zirk_http_response_to_stream`, `zirk_http_stream_read_all`, `zirk_http_stream_read_chunk`, `zirk_http_stream_sse_heartbeat_count`, `zirk_http_stream_sse_next`, `zirk_http_stream_close` | Single-consumption streaming request/response, SSE, and bounded chunks; buffered conversion enforces the 16 MiB body maximum, streaming upload/response chunks are capped at 64 KiB, and streaming is demand-driven. | Stream handles are capped at 1,024 and owner-task scoped. Cancellation/drop closes transport and releases producer captures; read/close reject foreign owners and close is idempotent. |
| `zirk_http_websocket_*` | HTTP/1.1 upgrade, HTTP/2 RFC 8441, and optional HTTP/3 RFC 9220 engines; frame/message and control payload limits apply. | Exact origin grant, Origin policy, subprotocol validation, TLS peer authentication, and task ownership are checked before returning a handle. Send/receive are bounded and cancellable; close/task exit shuts down driver and socket. |
| `zirk_http_server_*` (serve, WebSocket serve, HTTP/3 serve, control) | Bounded listeners, routes, connections, handlers, requests, handshakes, streams, queued response bytes, and producer chunks. Handler exceptions become HTTP 500; options are validated before admission. | Listen permission is checked before bind. Controls are executor/project/manifest scoped and idempotent; shutdown wakes admission, drains to deadline, then cooperatively cancels child handlers. Listener remains owned by its creating task. |
| `zirk_http_server_producer_register` | Registers one managed response producer; at most 1,024 producer handles and 64 KiB per chunk. | Producer capture belongs to the serving task; each chunk is checked before transport write, and cancellation/handler exit releases its registration and roots. |

This review found no path that accepts unbounded host data, bypasses an
effective permission grant, holds a global handle lock while awaiting I/O, or
leaves a task-owned socket/upload/stream alive after task termination. The
helper parsers remain private bounded protocol-engine operations; no helper is
a user-selectable native symbol. Evidence is in focused tests in
`crates/zirk-runtime/src/{environment,network_api,http_api,http3_transport}.rs`,
compiler-owned registry tests in `crates/zirk-codegen-llvm`, and compiled-Zirk
package tests in `crates/zirk-cli/tests/end_to_end.rs`.

### Environment (4)

- `zirk_environment_contains`
- `zirk_environment_get`
- `zirk_environment_get_secret`
- `zirk_environment_list_names`

### Networking (60)

- `zirk_network_dns_resolve`
- `zirk_network_interfaces_snapshot`
- `zirk_network_ip_address_parse`
- `zirk_network_local_accept`
- `zirk_network_local_bind`
- `zirk_network_local_close_listener`
- `zirk_network_local_close_stream`
- `zirk_network_local_connect`
- `zirk_network_local_listener_is_closed`
- `zirk_network_local_read`
- `zirk_network_local_shutdown`
- `zirk_network_local_stream_is_closed`
- `zirk_network_local_write`
- `zirk_network_local_write_all`
- `zirk_network_tcp_accept`
- `zirk_network_tcp_accept_until_shutdown`
- `zirk_network_tcp_close`
- `zirk_network_tcp_close_listener`
- `zirk_network_tcp_connect`
- `zirk_network_tcp_is_closed`
- `zirk_network_tcp_listen`
- `zirk_network_tcp_listener_is_closed`
- `zirk_network_tcp_origin_normalize`
- `zirk_network_tcp_peer_address`
- `zirk_network_tcp_read`
- `zirk_network_tcp_shutdown`
- `zirk_network_tcp_write`
- `zirk_network_tcp_write_all`
- `zirk_network_tls_accept`
- `zirk_network_tls_accept_until_shutdown`
- `zirk_network_tls_close`
- `zirk_network_tls_close_listener`
- `zirk_network_tls_connect`
- `zirk_network_tls_connect_with_alpn`
- `zirk_network_tls_connect_with_client_identity`
- `zirk_network_tls_connect_with_roots`
- `zirk_network_tls_is_closed`
- `zirk_network_tls_listen`
- `zirk_network_tls_listen_http`
- `zirk_network_tls_listen_with_client_auth`
- `zirk_network_tls_listener_is_closed`
- `zirk_network_tls_negotiated_alpn`
- `zirk_network_tls_peer_address`
- `zirk_network_tls_peer_certificate_sha256`
- `zirk_network_tls_read`
- `zirk_network_tls_shutdown`
- `zirk_network_tls_upgrade_tcp`
- `zirk_network_tls_write_all`
- `zirk_network_udp_bind`
- `zirk_network_udp_close`
- `zirk_network_udp_connect`
- `zirk_network_udp_is_closed`
- `zirk_network_udp_join_multicast`
- `zirk_network_udp_leave_multicast`
- `zirk_network_udp_local_address`
- `zirk_network_udp_receive`
- `zirk_network_udp_send`
- `zirk_network_udp_send_broadcast`
- `zirk_network_udp_send_multicast`
- `zirk_network_udp_send_to`

### HTTP (52)

- `zirk_http_client_clear_cookies`
- `zirk_http_client_close`
- `zirk_http_client_create`
- `zirk_http_client_create_with_options`
- `zirk_http_client_create_with_protocol_options`
- `zirk_http_client_create_with_proxy`
- `zirk_http_client_create_with_proxy_and_trust_roots`
- `zirk_http_client_create_with_trust_roots`
- `zirk_http_client_enable_cookies`
- `zirk_http_client_request`
- `zirk_http_client_request_stream`
- `zirk_http_form_parse`
- `zirk_http_get`
- `zirk_http_get_cached`
- `zirk_http_get_stream`
- `zirk_http_head`
- `zirk_http_multipart_parse`
- `zirk_http_post`
- `zirk_http_post_bytes`
- `zirk_http_post_json`
- `zirk_http_query`
- `zirk_http_query_with_content_type`
- `zirk_http_request`
- `zirk_http_request_cached`
- `zirk_http_request_secret`
- `zirk_http_request_stream`
- `zirk_http_response_header_values`
- `zirk_http_response_text`
- `zirk_http_response_to_stream`
- `zirk_http_route_match`
- `zirk_http_server_control_create`
- `zirk_http_server_control_request_shutdown`
- `zirk_http_server_producer_register`
- `zirk_http_server_serve_http3`
- `zirk_http_server_serve_once`
- `zirk_http_server_websocket_serve_once`
- `zirk_http_server_websocket_serve_http3_once`
- `zirk_http_stream_close`
- `zirk_http_stream_read_all`
- `zirk_http_stream_read_chunk`
- `zirk_http_stream_sse_heartbeat_count`
- `zirk_http_stream_sse_next`
- `zirk_http_upload_close`
- `zirk_http_upload_read`
- `zirk_http_url_normalize`
- `zirk_http_url_resolve`
- `zirk_http_websocket_close`
- `zirk_http_websocket_close_with`
- `zirk_http_websocket_connect`
- `zirk_http_websocket_connect_with_trust_roots`
- `zirk_http_websocket_receive`
- `zirk_http_websocket_send`

## Architectural status and remaining work

The API is inspectable as bundled Zirk source, but the implementation is not yet predominantly Zirk for HTTP and networking. Rust currently owns HTTP client/server orchestration and protocol framing, TLS/QUIC engines, transport resource registries, and some parsing/encoding helpers. Moving portable policy out of those bindings is still part of the active standard-library completion work; a source declaration should not be presented as proof that its implementation is written in Zirk.

Browser execution supports buffered outbound HTTP through the additive asynchronous `BrowserApi.runWithFetch` operation. The host injects a Fetch function, normalized origin allowlist, request/response byte ceilings, and optional abort signal for that invocation; the interpreter suspends at a request and resumes the same execution after Fetch completes. Existing synchronous `check`, `format`, and `run` stay default-deny. Browser tests exercise the WASM-to-Node Fetch adapter, while browser-enforced CORS against live origins remains a separate runtime evidence requirement. TCP, UDP, TLS streams, local IPC, interface inspection, HTTP/3 host control, listeners, streaming bodies, and WebSocket remain unavailable to browser execution.

To make the embedded standard library directly inspectable after installation, a future installer can optionally materialize the exact versioned `.zrk` source tree beside the compiler and report its source revision. The compiler should continue resolving imports from its verified bundle so users do not have to install or configure a separate library directory.
