## Context

The first delivery established the native DNS/TCP/TLS substrate and HTTP/1.1 plus HTTP/2 client/server contract across macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. The remaining work adds datagram and local transports, interface metadata, richer TLS, HTTP/3 over QUIC, browser-hosted HTTP, and lifecycle/client gaps recorded in the current specs and handbook. These additions cross runtime resource ownership, permission fingerprints, package provenance, HTTP protocol code, browser host injection, documentation, and the existing native CI matrix.

The runtime uses cooperative tasks with a fixed 128 KiB stack. Network waits must suspend tasks rather than block scheduler threads, and cancellation is cooperative at I/O and safe points. HTTP/3 brings a UDP-based multiplexed transport and must preserve the first delivery's origin, TLS identity, body, redirect, cookie, and typed-error guarantees. Browser execution is a separate host environment and must remain hermetic unless its embedding caller injects a narrowly scoped capability.

## Goals / Non-Goals

### Goals

- Complete the supported native `std.net` surface with bounded UDP, local IPC streams, interface snapshots, and explicit TLS configuration/mutual authentication.
- Deliver interoperable HTTP/3 over QUIC v1 for native clients and servers, plus the standardized WebSocket bootstrap over HTTP/3.
- Preserve existing HTTP methods, including RFC 10008 QUERY, permission and redirect semantics, and typed results across HTTP/1.1, HTTP/2, and HTTP/3.
- Make long-running server connection admission concurrent but bounded, expose owner-controlled graceful shutdown, and isolate cancellation by request/stream.
- Add browser HTTP only through a per-execution host capability with an origin allowlist and browser-enforced CORS/TLS policy.
- Close explicit bounded client gaps for stacked content codings, caller-owned cookie stores, and redacted redirect history.
- Apply new transport authority as narrow permission scopes and keep secret redaction at all implemented value-output sinks.
- Verify target runtime behavior independently from compilation and publish only evidence-backed status.

### Non-Goals

- WebTransport, HTTP Datagrams, raw packet sockets, packet capture, interface selection, or routing-table mutation.
- Browser TCP/UDP/TLS sockets, local IPC, interface inspection, or listening servers.
- TLS certificate-verification bypass, public TLS key logging, implicit trust-root downloads, or implicit credential discovery.
- QUIC 0-RTT by default, unsafe automatic request replay, or fallback after a request may have been committed unless its replay contract permits it.
- Persistent cookie storage or built-in cookie encryption. A future persistence API must be designed with the filesystem and crypto contracts.
- Hard preemption of CPU-only Zirk handlers; shutdown deadlines remain cooperative.
- Claiming that the entire standard library is complete. This change covers only the listed networking, HTTP, browser capability, and redaction gaps.

## Decisions

### Inspectability and host-binding inventory

The boundary follows the existing `std.io` pattern, with an explicit preference for moving portable behavior into Zirk source:

| Area | Must remain visible in `.zrk` | Host implementation allowed |
| --- | --- | --- |
| `std.io` reference pattern | Public signatures, `print`/`println` composition and value formatting | Process stdin/stdout/stderr reads and writes, OS flush |
| `std.net` existing and new | Public address/configuration/resource types, argument validation, typed outcomes, grant-facing policy, ownership wrappers | DNS resolver calls, TCP/UDP/local socket syscalls and readiness, interface enumeration, TLS cryptographic engine and certificate verification |
| `std.http` existing and new | Public request/response/body/options types, header and URL validation where portable, redirects, sensitive-header handling, cookies, routing, middleware, SSE framing, WebSocket message rules, error conversion | Bounded HTTP wire engine where implemented by audited dependencies, QUIC state machine, protocol framing/compression engine only where not reasonably maintainable in Zirk, socket I/O |
| Browser HTTP | Per-run capability contract, origin policy representation, request validation, typed result mapping | Host-provided Fetch call, browser TLS/CORS enforcement and cancellation bridge |
| `std.environment` | Public permission-aware API, typed parsing, absence/default semantics, `SecretString` wrapper/redaction policy | Reading process environment values and platform sink serialization primitives |

The local socket ABI is limited to `connect`, `bind`, `accept`, bounded
`read`/`write`/`write_all`, half-close, close, and closed-state checks. Each
operation is bound to `std.net::local` provenance, checks the distinct
`network.local_connect` or `network.local_listen` grant before host effects,
and uses owner-task handles capped by the runtime's network resource ceiling.
Reads and writes retain the TCP byte bounds; cancellation is observed through
the task-aware network wait path. Unix uses filesystem-domain sockets and an
inode-recorded cleanup anchored to the already-open parent directory. Windows
uses local named pipes and does not apply filesystem stale-endpoint deletion.
The only native boolean on `bind_native` selects the source-declared explicit
stale socket policy; endpoint validation, enum policy, errors, and resource
wrapping remain in the `.zrk` source.

Current evidence in `crates/zirk-sema/src/stdlib.rs` confirms that `std.io`, `std.net`, and `std.http` use a closed, fully qualified binding registry. The official `.zrk` files provide ordinary declarations; some are executable Zirk compositions while private `*_native` implementation leaves are associated with Rust runtime functions. The second delivery SHALL keep the same provenance boundary and must not expand host binding merely because a Rust implementation is convenient. For each new binding, its registry key, reason for host execution, validation and authorization boundary, limits, cancellation behavior, and resource owner SHALL be included in code review and tests. Pure parsing/validation should be implemented in Zirk unless profiling or a documented language limitation proves that unsuitable.

### 1. Use reviewed, maintained protocol implementations behind host bindings

Implement QUIC, HTTP/3, QPACK, and HTTP/3 WebSocket framing through maintained Rust dependencies behind Zirk-owned bounded transport adapters. Do not implement QUIC cryptography or QPACK from scratch. Before selecting versions, review primary-source release/security notes, transitive dependency licenses, MSRV, platform support, and binary size. Accept MIT, Apache-2.0, BSD, or ISC licenses; document and review any exception before implementation. Pin compatible versions and add protocol-level tests against independent peers.

**Why:** QUIC and HTTP/3 have complex cryptographic, congestion, stream, and compression behavior. A reviewed implementation reduces protocol risk while keeping the public API and policy under Zirk control.

### 1a. Keep the standard library inspectable and Zirk-first

Treat bundled `.zrk` source as the reviewable implementation of the public library. Public types and signatures, URL/header/body validation, permission-facing policy, redirects, cookie rules, routing, middleware, SSE framing, WebSocket message policy, error mapping, and cross-protocol orchestration SHALL remain ordinary Zirk code whenever they do not require direct host I/O or a maintained protocol engine. Runtime bindings SHALL be limited to operating-system operations and the audited QUIC/TLS/HTTP protocol engines that cannot reasonably be implemented in Zirk today. Each binding SHALL have a fully qualified official-source identity, a documented contract and resource limits, and matching native tests; same-named external declarations SHALL never receive host behavior. No source-level `native`, `intrinsic`, or ABI marker is introduced.

**Why:** Users should be able to inspect how the library makes decisions and how APIs compose, as they can with the Zirk portions of `std.io` and `std.http`. Platform syscalls, TLS cryptography, and the QUIC state machine still need host implementations, but they should not hide the public API or security policy.

### 2. Keep existing Zirk APIs and make protocol choice explicit and safe

Preserve existing `HTTPClient`, `HTTPBody`, redirect, cookie, proxy, timeout, and typed error contracts. Add an extensible validated `HTTPClientOptions` protocol policy with `Auto`, `HTTP1`, `HTTP2`, and `HTTP3` selection. `Auto` prefers the highest mutually supported version. Fallback is allowed only before application bytes are committed or when the body and method are explicitly replayable. Keep 0-RTT disabled by default; if ever exposed, require an explicit safe replay policy and reject unsafe/non-replayable requests. Record the negotiated version in response metadata.

**Why:** Applications need predictable rollout and debugging without weakening permission or retry guarantees.

### 3. Revalidate every QUIC address and path

Resolve and authorize each candidate address before the first UDP datagram. Require TLS 1.3 and `h3` ALPN. Revalidate QUIC migration candidates against the original normalized origin grant before application data can use a new path. Never let a DNS answer, peer-provided address, or path-validation packet enlarge authority. HTTP/3 fallback uses the same origin grant and TLS hostname identity.

**Why:** QUIC's UDP transport and connection migration must not bypass the first delivery's DNS-rebinding and origin-boundary guarantees.

### 4. Make new network permissions disjoint

Add distinct manifest scopes for UDP destinations, UDP bind addresses, local-endpoint connect, local-endpoint listen, and interface inspection. UDP send destinations use a normalized bounded address/port set, and UDP bind addresses are separate from TCP/HTTP listen addresses. Broadcast and multicast require separate explicit authority. Local endpoints use canonical paths/names and connect/listen remain distinct. Interface snapshots require `network.inspect.interfaces` and confer no connect, bind, or route-selection authority. Include these scopes in the existing signed approval fingerprints and phase model.

**Why:** A single broad “network” grant would violate the least-authority design already used by the first delivery.

### 5. Keep browser networking an injected fetch capability

Expose an embedding-host capability per run, with an origin allowlist, request/response size bounds, cancellation, and typed browser-policy failures. Use browser-managed fetch, TLS, CORS, and credentials. Do not infer support from native compilation, and do not expose native socket abstractions in browser builds. Default browser execution continues to issue no requests.

Browser `fetch` is asynchronous, while the current interpreter loop and
`BrowserApi.run` contract are synchronous. Do not block a browser worker or
hide a synchronous-XHR fallback behind the existing API. First add a resumable
interpreter execution state that can suspend at one bounded HTTP request while
retaining its frames, heap, instruction budget, and trace state, then resume
with a typed response or failure. Expose that path as an additive asynchronous
browser operation; keep `check`, `format`, and synchronous `run` default-deny.
The host capability is passed for one asynchronous invocation, contains the
effective normalized origin allowlist and hard resource limits, uses native
browser `fetch`, and is dropped with completion, failure, or cancellation.
Only buffered request/response operations that the browser can expose safely
are eligible initially; streaming bodies and WebSockets remain explicit
unsupported capabilities until the same suspension and cancellation contract
covers their lifecycles.

**Why:** The embedding page controls browser authority and policy. Zirk can scope requests but cannot bypass browser security or select the underlying HTTP version.

### 6. Bound server concurrency and lifecycle at every queue

Configure independent maximums for accepted connections, concurrent QUIC handshakes, concurrent requests, handler tasks, per-connection streams, and queued response bytes. Admission uses bounded waiting or an explicit rejection result. The HTTP/3 listener cancels pending handshakes as soon as its request limit is reached. Each request owns its task and transport resources; a disconnect cancels only that request's work. Owner shutdown stops admission, drains for a configured grace period, requests cooperative cancellation, and closes TCP/TLS/UDP/QUIC resources exactly once. Preserve the fixed 128 KiB stack and keep protocol state on bounded heap allocations managed by the host runtime.

**Why:** Raising the global stack is unnecessary and would multiply per-task memory; protocol state belongs in explicitly limited connection/stream buffers.

### 7. Make secondary HTTP features opt-in and memory bounded

Support stacked content codings only from a documented allowlist and enforce encoded bytes, decoded bytes, expansion ratio, coding depth, and cancellation during decompression. A custom cookie store is caller-owned and in-memory for this change; default behavior stays opt-in and no implicit filesystem persistence is added. Redirect history is opt-in or bounded by the configured redirect limit and stores only normalized method/status/destination metadata. All history rendering passes through the secret-redaction boundary.

**Why:** These features close documented usability gaps without introducing hidden disk writes or unbounded decompression/history retention.

### 8. Keep secret redaction as a runtime boundary

Route existing formatter and structured-output paths through one redaction contract. This change adds tests at each currently implemented output sink. Since debugger, heap snapshot, and test-report sinks do not yet exist, specify a release gate: each future sink must add executable recursive redaction coverage before it can serialize runtime values.

**Why:** `SecretString` must remain safe through nested values and errors; documentation alone is not an enforcement boundary.

### 9. Separate protocol conformance evidence by target and mode

Run native tests on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64 against independent HTTP/1.1, HTTP/2, and HTTP/3 peers where applicable. Keep browser-host tests separate and identify browser CORS/runtime limitations. Status tables distinguish implemented source, compile evidence, per-target native runtime evidence, and browser-host evidence. Update the companion website only after compiler docs are committed and sync instructions are satisfied.

**Why:** A successful compile or mocked transport does not establish interoperable network behavior.

## Risks / Trade-offs

- QUIC dependencies may increase compile time and binary size, or have target-specific limitations; select only after the dependency and license audit and measure release artifacts per target.
- HTTP/3's UDP path and migration increase the number of permission and DNS-rebinding boundaries; missing a revalidation point can create a network-policy bypass.
- More concurrent handlers multiply heap use; every admission, stream, queue, body, and compression limit must compose into an understandable total bound.
- Platform local sockets differ substantially, especially on Windows; use a typed capability error where the operating system cannot provide equivalent semantics rather than silently emulating a weaker transport.
- Browser fetch may hide response details or reject requests due to CORS; expose those as browser transport outcomes and avoid promising native equivalence.
- Stacked decompression can consume CPU even under a decoded-size cap; decompression must remain cancellable and bounded by layers and ratio.
- Cookie stores and redirect history can contain personal data; stores remain caller-owned and non-persistent, and history avoids credentials and secret values.
- The fixed stack may expose overly large protocol call frames; keep protocol machinery in host-managed heap state and include stack-sensitive native tests.
- A large host binding could turn `.zrk` APIs into opaque wrappers; review the boundary against the Zirk-first rule and require a concrete host/platform or audited-protocol-engine reason for each binding.
- The total implementation is broad. Land it in dependency order and do not mark the capability complete until every required target gate has evidence.

## Migration Plan

1. Add scopes and source APIs with typed unsupported errors where a backend is not yet present; preserve all existing APIs and permission fingerprints' fail-closed behavior.
2. Implement the native transport extensions and HTTP/3 behind explicit protocol selection, then enable `Auto` only after the target conformance matrix passes.
3. Add browser fetch capability injection in the browser host package with default-deny behavior.
4. Add bounded client completion features and sink redaction tests.
5. Update compiler docs and status from measured evidence, commit compiler docs, synchronize the companion website, review the website diff, and validate both repositories.

Rollback consists of disabling HTTP/3 auto-selection and browser capability injection while retaining HTTP/1.1/HTTP/2 APIs. New permissions fail closed and can be removed from project manifests independently.

## Validation Evidence (In Progress)

The focused UDP, local IPC, and native interface-enumeration runtime suites
pass on macOS ARM64 and have now been rerun on both Linux architectures. The
HTTP/3 adapter suite also passes on those three targets. On macOS, the bundled
`std.net::interfaces` API also passes approved and denied end-to-end tests. The
UDP suite covers
datagram boundaries, destination authority, size rejection, task ownership,
cancellation, and handle cleanup. The local IPC suite covers stream round
trips, denial before endpoint creation, stale endpoint handling,
replacement-symlink preservation, and cleanup anchored to the original parent
directory. The interface-enumeration suite covers bounded records, normalized
metadata, applicable IPv6 link-local scope IDs, and deterministic ordering.
The policy suite also confirms that inspection-only authority does not grant
connect or listen access.

| Target | Evidence | Result |
| --- | --- | --- |
| macOS ARM64 | Fresh `cargo test -q -p zirk-runtime --features http3 --lib -- --test-threads=1`; focused compiled-Zirk UDP, local IPC, and interface E2E filters; focused HTTP/3 adapter filter | Full runtime suite: 528 passed, 3 ignored; HTTP/3 adapter suite: 10 passed; UDP E2E: 2 passed; local IPC E2E: 2 passed; interface E2E: 2 passed |
| Linux ARM64 | Fresh `cargo test -q -p zirk-runtime --features http3 --lib -- --test-threads=1` in the native ARM64 Docker image | Full runtime suite: 528 passed, 3 ignored; this includes UDP, local IPC, interface, TLS, and HTTP/3 adapter coverage |
| Linux x86_64 | Fresh CI networking filters in `rust:1.94.1-bookworm` with `--platform=linux/amd64` | UDP: 6; local IPC: 5; interfaces: 2; TLS: 9; HTTP/3 adapter: 10; network policy: 14 passed. Full runtime suite and independent-peer script still require native x86_64 CI: the full-suite attempt under Docker Desktop terminated in Rosetta with `mmap_anonymous_rw_guard_page mprotect failed` |
| Windows x86_64 | Fresh `cargo xwin check -p zirk-runtime --tests --features http3 --target x86_64-pc-windows-msvc` | Runtime, Windows named-pipe test code, and current HTTP/3 adapter/test code compile; named-pipe and HTTP/3 runtime tests still require a Windows runner |
| Browser/WASM | `cargo check -p zirk-browser --target wasm32-unknown-unknown`; `cargo test -p zirk-interpreter`; `cargo test -p zirk-browser`; `scripts/test-browser-http-fetch.sh` | WASM package compiled; 65 interpreter tests and 41 browser tests passed, 1 browser test is ignored. The suspension test confirms output and trace sequence continue in the same interpreter; the WASM/Node Fetch adapter passed method/body/header forwarding, JSON POST and QUERY validation, default-deny and origin checks, request/response limits, CORS error mapping, and cancellation. Browser preflight tests cover every new UDP and local IPC instruction plus interface, DNS, TLS, TCP, and HTTP request/listener host boundaries with capability-specific diagnostics. |

On 2026-10-20, fresh macOS ARM64 runs of `scripts/test-browser-http-fetch.sh`
and `scripts/test-http3-websocket-public-api.py` passed. The latter used pinned
`aioquic` as an independent peer and verified Zirk HTTP/3 GET and QUERY, both
directions of RFC 9220 WebSocket data and close handling, and rejection of an
unapproved Origin before the Zirk handler ran. This proves those specific
macOS behaviors; the HTTP/3 peer and browser Fetch run are not yet evidence for
the other native runners.

On 2026-10-20, the Linux ARM64 full runtime suite passed from the current
worktree in a native ARM64 container. The exact network-conformance filters
from CI also passed in the Linux x86_64 container: six UDP tests (connected
and bound datagrams, permissions, receive limit, cancellation, and owner
cleanup), five local IPC tests (round trip, distinct scopes, denial, stale
paths, and symlink-safe cleanup), two interface snapshot tests, nine TLS
transport tests, ten HTTP/3 adapter tests, and fourteen network-policy tests.
The additional attempt to run all 531 runtime tests on the emulated x86_64
container terminated with a Rosetta guard-page trap; it is not counted as a
full-suite pass. The H3 adapter peers use the same Quinn/Hyperium stack, so
these results are runtime adapter evidence rather than independent-peer
interoperability evidence.
The `http3_server_dispatches_multiplexed_requests_to_bounded_owner_tasks`
test opens three independent QUIC connections and verifies that two handlers
overlap while a third request is excluded by the server's request limit. The
listener accepts connections concurrently, caps in-flight QUIC handshakes at
16, and cancels pending handshakes when the request limit is reached. A shared
semaphore caps active request bodies, handlers, and responses at four across
the listener. At the existing 16 MiB per-body limit, buffered request and
response vectors are capped at 128 MiB per listener, in addition to Quinn's
transport windows. These loopback peers use the same Quinn/Hyperium stack, so
the suite remains adapter runtime evidence rather than independent
interoperability evidence.
The client pool now retains one HTTP/3 session per normalized origin, bounded
by its configured origin limit. It clones the HTTP/3 sender per request so
concurrent tasks can use independent streams, and retires sessions on
transport/protocol failures or when the origin grant is denied. The focused
client tests prove two concurrent streams share a single connection and that
two sequential requests through one `HttpClientPool` succeed while the server
admits only one connection. Those are same-stack loopback tests, not
independent-peer interoperability evidence.
Earlier broad `tls_` filters passed 21 TLS tests on macOS ARM64 and both Linux
architectures, covering mutual TLS, trust roots, ALPN, explicit TLS 1.2
opt-in, existing-socket upgrade, and handshake timeout behavior. The fresh
CI-specific filter passed nine TLS transport tests on Linux x86_64; the fresh
full runtime suites passed on macOS ARM64 and Linux ARM64.

The Windows runtime source now includes dedicated tests for named-pipe
permission denial before creation and listener release/rebind after closing
the first pipe instance. They are not counted as passing evidence until they
run on a Windows SDK runner. A fresh macOS ARM64 attempt of plain
`cargo check -p zirk-runtime --tests --target x86_64-pc-windows-msvc` stopped
inside `ring` because the host compiler could not find the Windows SDK C header
`assert.h`. Using the installed `cargo-xwin` SDK toolchain, both the runtime
and test code then compiled successfully for Windows x86_64, with and without
the `http3` feature. This is compile evidence only; named-pipe and HTTP/3
runtime behavior still requires the Windows runner. The native CI matrix now has a dedicated
`Networking and HTTP/3 runtime conformance` step that enables the opt-in
`http3` feature and runs UDP, platform local-transport, interface, HTTP/3
adapter, TLS, and network-policy suites on each of its macOS ARM64, Linux
x86_64, Linux ARM64, and Windows x86_64 runners. That workflow change
establishes the repeatable gate; it does not count as Windows runtime evidence
until the job has actually run successfully.

This evidence does not complete tasks 1.2.4, 1.4, or 1.5: Windows is pending,
and the public compiler-to-Zirk interface API has end-to-end evidence only on
macOS so far. It does not prove Windows runtime behavior, full HTTP request
conformance, browser fetch behavior, or independent-peer HTTP/3 interoperability.

The HTTP/3 client adapter is selectable through public Zirk
`HTTPClientOptions.create_with_protocol(HTTPProtocolPreference.HTTP3, ...)` and
the protocol preference is preserved by direct, trust-root, and proxy client
factories. An explicit H3 client request uses the shared-client dispatch path
outside the client gate, so network I/O does not serialize unrelated tasks.
`HTTPResponse.protocol()` now reports the negotiated protocol. Compiled-Zirk
integration tests verify HTTP/1.1 and HTTP/2 metadata against independent
peers; a runtime loopback test exercises the HTTP/3 request through
`HttpClientPool` and checks bounded request/response framing and H3 metadata.
The H3 loopback peer uses the same Quinn and Hyperium H3 stack as the client,
so it is not independent-peer interoperability. `Auto` still negotiates only
HTTP/2 or HTTP/1.1; HTTP/3 auto-selection and safe pre-commit fallback remain
pending. The public Zirk `HTTPServer.serve_http3` declaration is wired through
sema, typed IR, verification, LLVM, and the runtime ABI. Builds without the
optional `http3` feature fail closed with a typed protocol error. The
feature-backed listener checks the exact UDP bind grant before opening Quinn,
enforces finite connection/request bounds, and fixes TLS to TLS 1.3, `h3`
ALPN, and disabled early data. Request framing and bounded response streaming
run on the Tokio reactor; the route callback and generated response producer
run on the owning Zirk task. A capacity-one channel carries response chunks,
preserving backpressure and releasing the rooted producer when the peer
disconnects. The loopback test verifies owner task/thread identity, a rooted
producer surviving a collection, two response chunks, framing, and H3 client
metadata. The listener test also establishes QUIC with a peer that disconnects
before sending HTTP/3 control streams, then confirms that the next valid peer
is still served. The test peer uses the same Quinn and Hyperium H3 stack, so it
is not independent-peer interoperability evidence. A single HTTP/3 connection
now dispatches up to four multiplexed request streams to separate Zirk handler
tasks on the owning executor. The listener still admits QUIC connections
sequentially and has no externally requested graceful shutdown; multi-connection
admission, per-stream peer-disconnect cancellation, and full public-API runtime
coverage remain pending.

A public `HTTPWebSocket.connect_http3` client entry point is wired through
sema, IR, LLVM, and the runtime ABI. With the optional `http3` feature, it uses
authenticated TLS 1.3, RFC 9220 extended CONNECT, validates Origin and
subprotocol negotiation, bounds the WebSocket message path, and retains the
H3 driver for the session. A local Quinn/H3 peer test verifies the extended
CONNECT token and bidirectional WebSocket data. This peer shares the candidate
H3 engine and does not establish independent interoperability. The server
transport advertises extended CONNECT and validates the exact path, version,
Origin, and subprotocols before creating a bounded duplex bridge. The public
`HTTPServer.serve_http3_websocket_once` method is wired through official sema,
IR, LLVM, and runtime bindings; it checks the UDP bind grant, uses TLS 1.3/h3,
and retains the listener/bridge driver for the callback lifetime. A compiler
build fixture verifies the public method lowers and links. On 2026-10-20, two
separately compiled Zirk programs were run against the HTTP/3-enabled runtime
on macOS ARM64: the server used `serve_http3_websocket_once`, and the client
used explicit DER trust roots; they negotiated `echo.v1`, exchanged a text
message, and completed the close handshake. The automated loopback test also
exercises the private adapter's endpoint/codec lifecycle. These peers share
the candidate Quinn/H3 stack and do not establish independent interoperability.
The checked-in `scripts/test-http3-websocket-public-api.py` fixture builds the
optional runtime and executes both public Zirk APIs; CI runs it on each native
matrix runner. This remains same-stack evidence because both programs use the
pinned Quinn/H3 engines. HTTP/3 automatic selection and pre-commit fallback
remain unimplemented. Independent-peer interoperability is freshly verified on
macOS ARM64; Linux x86_64, Linux ARM64, and Windows still need independent-peer
CI results. Windows native runtime results also remain pending.
The public
`connect_http3_with_trust_roots` overload now allows bounded DER roots while
retaining TLS 1.3. Task 2.7 therefore remains open.

On 2026-10-20, the server-side RFC 9220 adapter loopback test
`http3_websocket_uses_extended_connect_and_bidirectional_data_stream` passed
on macOS ARM64. It verifies an allowed Origin, server-preferred subprotocol,
and bidirectional WebSocket frames through the HTTP/3 stream bridge. The peer
uses the same Quinn/H3 stack and does not establish independent interoperability.

On 2026-10-20, the RFC 9220 client loopback test
`http3_websocket_uses_extended_connect_and_bidirectional_data_stream` passed
with `--features http3` on macOS ARM64, Linux x86_64, and Linux ARM64. Each
local test peer used Quinn and the same H3 engine as the client, so these runs
verify per-target runtime execution and TLS trust-root handling but do not
substitute for independent-peer interoperability.

Plain `cargo check --target x86_64-pc-windows-msvc` cannot use the macOS C
compiler to build `ring` because the Windows SDK headers are absent. The
installed `cargo-xwin` toolchain resolves that boundary: the current
`cargo xwin check -p zirk-runtime --tests --features http3 --target
x86_64-pc-windows-msvc` passes. This establishes Windows compilation of the
runtime and test code only; Windows runtime behavior still requires its native
runner.

## TLS Client Configuration (Partial)

The bundled `TLSStream.connect_with_alpn(endpoint, protocols)` API offers an
ordered ALPN list with system trust roots and TLS 1.3 defaults.
`TLSStream.connect_with_options(endpoint, roots_der, allow_tls12, protocols)`
combines that policy with custom DER trust roots and explicit TLS 1.2 opt-in;
an empty roots array selects the system trust store. The native boundary limits
the ALPN list to 16 unique printable ASCII identifiers, at most 255 bytes each
and 1 KiB total, and validates it before opening a socket. The
`negotiated_alpn()` accessor returns the selected protocol as `String?`.
`TLSStream.connect_with_client_identity` additionally accepts a bounded client
certificate chain and private-key DER. The temporary key copy is zeroized after
rustls constructs the signer. Runtime validation, a compiled-Zirk loopback
handshake with a custom root and negotiated `zirktest/1`, an approved
compiled-Zirk mutual-authentication exchange with negotiated
`zirk-mtls-test/1`, and malformed client-key rejection pass on macOS ARM64.
The mTLS fixture server requires and validates the client certificate, checks
the presented identity, and exchanges application bytes. It uses rustls and is
loopback evidence, not independent TLS implementation interoperability.
The runtime now has a server-config constructor that requires an explicit,
non-empty client CA list and builds rustls' `WebPkiClientVerifier`; the passing
mTLS test exercises this constructor. `TLSListener.bind_with_client_auth` now
exposes that policy in bundled Zirk source, and
`TLSStream.peer_certificate_sha256()` returns only the validated leaf
certificate fingerprint. An approved Zirk-server E2E test verifies rejection
of an unauthenticated client, acceptance of the configured client identity,
the reported fingerprint, and an application-data exchange. A compiled-Zirk
loopback test now upgrades one approved TCP connection, confirms the original
TCP handle is closed after transfer, negotiates ALPN, exchanges application
data, and verifies the server accepted no second connection. Cross-target
verification remains tracked separately in task 1.6.4.

## Redirect History

The bundled Zirk API offers `HTTPClient.request_following_with_history` and
`request_body_following_with_history`. They return an `HTTPRedirectResult`
with the final response and at most 20 followed-hop entries. Each entry
contains the method, redirect status, and URL-resolved destination. URL user
information is removed, the entire query is replaced with `?redacted`,
fragments are discarded, and each destination is capped at 8 KiB. Request
headers and cookie values are never copied into history. The Zirk-side
`history()` and `history_json()` methods expose printable and JSON-array
serialization without changing the native `HTTPResponse` layout. The public
factory validates methods, redirect statuses, and normalized URLs before
retaining supplied entries. Compiled-Zirk E2E on macOS ARM64 verifies
`POST` to `303`, cross-origin credential removal, preservation of the actual
query for the request, query/userinfo/fragment redaction, JSON serialization,
and rejection of an invalid fabricated history entry. The response-only
redirect methods preserve their signatures. This completes task 3.3; final-URL
metadata and typed hop objects are not part of the implemented API.

## HTTP/3 Dependency Evaluation (In Progress)

RFC 9220 support exposed a missing primitive in the pinned `h3` 0.0.8 API:
its public `Protocol` value recognized only `webtransport` and `connect-udp`,
so callers could not encode or decode the standardized `websocket` value.
Zirk keeps the upstream MIT-licensed source under `vendor/h3` and applies a
narrow patch adding that token. `PATCHES.md` records provenance and removal
conditions. The client and server WebSocket paths are now implemented, and the
fresh independent `aioquic` test passes on macOS ARM64. Task 2.7 stays open for
native matrix interoperability and the remaining cross-target evidence.

The current candidate stack is pinned in `crates/zirk-runtime/Cargo.toml`
behind the opt-in `http3` feature:

| Crate | Pinned version | License | Declared MSRV | Evaluation |
| --- | --- | --- | --- | --- |
| [`quinn`](https://github.com/quinn-rs/quinn) | 0.11.12 | MIT OR Apache-2.0 | 1.85 | Pure-Rust QUIC implementation; upstream documents Linux, macOS, and Windows support. It is compatible with this workspace's Rust 1.94 baseline. |
| [`h3`](https://github.com/hyperium/h3) | 0.0.8 | MIT | 1.70 | Generic async HTTP/3 client/server implementation. Upstream explicitly labels it experimental and warns that the API may change; isolate it behind a private adapter and pin exact versions. |
| [`h3-quinn`](https://github.com/hyperium/h3) | 0.0.10 | MIT | 1.70 | Hyperium's Quinn transport adapter; depends on `h3` 0.0.8 and Quinn 0.11.7-compatible API. |

The direct licenses satisfy the repository allowlist. The initial advisory
check identified RUSTSEC-2026-0382 in locked `rustls-webpki` 0.103.15; the
lockfile now selects fixed version 0.103.16. `cargo deny check licenses` and
`cargo deny check advisories` both pass against the updated workspace lockfile.
The current HTTP/3 adapter suite has ten tests and passes on macOS ARM64,
Linux x86_64, and Linux ARM64; the full runtime suite passes on macOS ARM64 and
Linux ARM64. The Windows runtime and test code compile with `cargo-xwin`, but
native Windows execution remains pending. HTTP/3 client/server and RFC 9220
WebSocket APIs are implemented; independent-peer interoperability currently
has fresh macOS ARM64 evidence, with other native targets pending CI. Automatic
protocol selection and safe fallback remain unimplemented. Release
archive size has been measured on macOS ARM64; final executable and other
target sizes remain pending. The experimental upstream status is an explicit
maintenance risk; the `http3` feature stays opt-in until the remaining gates
pass.

### Release archive size measurement (macOS ARM64)

On 2026-10-21, fresh `cargo build -p zirk-runtime --release` and
`cargo build -p zirk-runtime --release --features http3` runs produced
66,437,024-byte and 74,507,712-byte `libzirk_runtime.a` archives respectively.
The opt-in QUIC/H3 stack adds 8,070,688 bytes (7.70 MiB, 12.15%) before final
executable dead-code elimination. The same-day representative executable
measurement is recorded in `docs/development/HTTP3_DEPENDENCY_REVIEW.md`.
Archive size is not a measurement of the installed compiler or a linked Zirk
program. Repeat per release target before deciding whether HTTP/3 should be
enabled by default.

## Interface Enumeration Dependency Review (In Progress)

The runtime currently pins [`if-addrs` 0.15.0](https://docs.rs/crate/if-addrs/0.15.0) for cross-platform enumeration.
Its API provides address prefixes, interface indices, operational state, and
point-to-point metadata, and it declares the permissive `MIT OR BSD-3-Clause`
license. The crate metadata does not declare an MSRV and [docs.rs reports a
failed documentation build](https://docs.rs/crate/if-addrs/0.15.0). Current macOS and Linux ARM64/x86_64 builds pass; the
Windows target build and release-size impact remain unverified. Do not treat
this dependency review as complete until those checks are recorded.
