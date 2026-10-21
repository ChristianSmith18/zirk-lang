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

## Current compiler-owned host binding inventory

The LLVM runtime registry currently contains 119 distinct `zirk_environment_*`, `zirk_network_*`, and `zirk_http_*` symbols. The exact registered names are listed below by subsystem. This includes utility and resource-lifecycle calls as well as transport calls; a symbol name alone does not imply that its operation is a direct OS syscall.

This symbol list is not yet a completed per-binding security/ownership review.
The active OpenSpec task `1.0.1` requires each binding to document its host or
protocol-engine role, input bounds, permission checks, cancellation behavior,
and resource ownership. The current HTTP registry includes portable helper
operations that still need this review and may need to move into the bundled
Zirk sources: form parsing/encoding, header encoding, route validation and
matching, URL normalization/resolution, response text decoding, and cached
request policy. Until that task is complete, the external binding boundary
must not be described as limited to OS operations and audited protocol engines.

### Environment (4)

- `zirk_environment_contains`
- `zirk_environment_get`
- `zirk_environment_get_secret`
- `zirk_environment_list_names`

### Networking (58)

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

### HTTP (57)

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
- `zirk_http_form_encode`
- `zirk_http_form_parse`
- `zirk_http_form_values`
- `zirk_http_get`
- `zirk_http_get_cached`
- `zirk_http_get_stream`
- `zirk_http_head`
- `zirk_http_headers_encode`
- `zirk_http_multipart_encode`
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
- `zirk_http_route_allow_header`
- `zirk_http_route_match`
- `zirk_http_route_pattern_conflict`
- `zirk_http_route_pattern_validate`
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
