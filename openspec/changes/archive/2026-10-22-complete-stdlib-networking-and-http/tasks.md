## 1. Permission and API contracts

- [x] 1.0 Keep public APIs and portable policy in official `.zrk` sources, inventory the compiler-owned symbols, and verify no source-level native/ABI markers are added.
- [x] 1.0.1 Audit every host binding for its OS/protocol-engine role, input bounds, permission checks, cancellation behavior, and resource ownership; move portable HTTP helpers out of the host ABI unless they are part of an audited protocol engine.
- [x] 1.1 Add manifest parsing, normalization, display, fingerprints, signed approval, and denial diagnostics for UDP send/bind, local endpoint connect/listen, and interface-inspection scopes; verify existing TCP/HTTP grants do not imply the new scopes.
- [x] 1.2.1 Implement the bounded task-aware native UDP transport for connected and bound sockets, with destination/listen authorization and no silent receive truncation; verify connected send/receive, unconnected receive/reply, denial, and limit outcomes.
- [x] 1.2.2 Add owner-task UDP handle lifecycle and compiler-owned ABI/IR lowering for socket construction, send, receive, close, and closed-state checks; verify wrong-owner, cancellation, and deterministic task-exit cleanup.
- [x] 1.2.3 Publish the ordinary Zirk `UDPSocket` and datagram result APIs, including canonical address handling and typed errors; verify standard import/provenance and source-level behavior.
- [x] 1.2.4 Run UDP packet-boundary, permission, cancellation, and cleanup conformance on every supported native target and publish target-specific evidence.
- [x] 1.3 Add explicit broadcast and multicast configuration and grants, including membership cleanup; verify unauthorized membership/send is denied before socket effects.
- [x] 1.4 Add local IPC stream/listener APIs with canonical endpoint validation, distinct connect/listen grants, symlink-safe cleanup, and explicit stale-endpoint policy; verify race-safe close behavior on macOS, Linux, and Windows.
- [x] 1.5 Add bounded immutable interface snapshots and `network.inspect.interfaces`; verify denial reveals no records and inspection grants no bind/connect rights.
- [x] 1.6.1 Add client-side TLS configuration for trust roots, protocol bounds, ALPN, and explicit client identities; verify TLS 1.3 default, TLS 1.2 opt-in, malformed identity rejection, and mutual authentication through the bundled Zirk API.
- [x] 1.6.2 Add opt-in server-side client-certificate authentication and expose only validated peer identity metadata.
- [x] 1.6.3 Allow an owner task to wrap its existing TCP stream with TLS without duplicating or detaching resource ownership.
- [x] 1.6.4 Verify the extended TLS behavior on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; confirm verification bypasses and public key logging remain unavailable.

## 2. Native HTTP/3 and server lifecycle

- [x] 2.1 Select and pin maintained QUIC, HTTP/3, QPACK, and WebSocket-over-HTTP/3 dependencies after documenting license, security, MSRV, target-support, and release-binary-size review; verify the allowlist and CI builds for all four native targets.
- [x] 2.2 Implement the QUIC adapter with async task suspension, cancellation-safe ownership, bounded buffers, TLS 1.3/`h3` ALPN, exact-origin address checks, migration revalidation, and 0-RTT disabled by default; verify each boundary with instrumented peers.
- [x] 2.3 Add validated HTTP protocol selection (`Auto`, `HTTP1`, `HTTP2`, `HTTP3`) and negotiated-version metadata to `HTTPClientOptions`; verify fallback never replays a possibly committed non-replayable request.
- [x] 2.4 Implement HTTP/3 client request/response framing, QPACK limits, stream/connection flow control, typed protocol errors, cancellation, proxy policy, redirects, cookies, QUERY, and existing `HTTPBody` variants; verify against independent HTTP/3 peers.
- [x] 2.5 Implement HTTP/3 server listener and routing with bounded connections, streams, handlers, and response queues; verify listener grants, malformed peer rejection, request isolation, and resource cleanup.
- [x] 2.6 Implement shareable `HTTPServerControl.request_shutdown()` and validated `HTTPServerOptions` across TCP/TLS/HTTP/3, with wakeable admission, bounded drain, cooperative handler cancellation, and deterministic resource release; verify idempotence, restart, and shutdown without changing the 128 KiB stack.
- [x] 2.7 Implement WebSocket over HTTP/3 extended CONNECT with origin/subprotocol checks, bounded messages, close semantics, cancellation, and permitted protocol fallback; verify RFC 9220 interoperability and rejection of unsupported peers.
- [x] 2.8 Run independent-peer conformance and adversarial tests for HTTP/1.1, HTTP/2, and HTTP/3 on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; publish per-target runtime evidence and unresolved platform limits. Linux x86_64, Linux ARM64, and macOS ARM64 pass the complete conformance/workspace/independent-peer steps in run 39739088146; Windows passes all three in run 39702042337 at `634179a` (same production sources as `2c06c67`, which changes only this regression test), and the changed Windows regression test passes in focused run 39748669844. The redundant full Windows rerun in 39739088146 remains active unusually long and is not counted as evidence.

## 3. Remaining HTTP client and browser capability

- [x] 3.1 Implement supported stacked content-coding decoding with explicit coding allowlist, depth, encoded/decoded size, expansion-ratio, and cancellation bounds; verify malformed and decompression-bomb cases stop with typed errors.
- [x] 3.2 Add caller-owned, opt-in in-memory cookie-store API with domain/path/secure/expiry isolation and no implicit disk persistence; verify store ownership, cross-origin isolation, and secret-safe diagnostics.
- [x] 3.3 Add bounded optional redirect history with normalized metadata and recursive redaction of credentials, cookie values, sensitive headers, and configured secret query values; verify formatting and structured serialization sinks.
- [x] 3.4.1 Add resumable interpreter execution for asynchronous browser HTTP host effects; preserve frames, heap, instruction and trace budgets across suspension, reject stale or duplicate resumes, and verify cancellation releases pending request state without restarting `main`.
- [x] 3.4 Add browser-host HTTP capability injection per execution with origin allowlist, bounded request/response bodies, cancellation, and typed CORS/browser errors; verify no request occurs by default or after capability scope ends.
- [x] 3.5 Ensure browser builds reject or report TCP, UDP, TLS-socket, local IPC, interface inspection, HTTP/3 host-control, and listening-server operations as explicit unsupported capabilities; verify browser package tests independently of native tests.

## 4. Environment redaction and cross-surface integration

- [x] 4.1 Route every currently implemented formatter and structured output sink through recursive `SecretString` redaction; add sink-specific tests for nested collections, captured values, and errors.
- [x] 4.2 Add an implementation gate and maintainer checklist requiring executable redaction coverage for future debugger, snapshot, and test-report sinks; verify no nonexistent sink is reported as implemented.
- [x] 4.3 Verify official package provenance, standard import resolution, and browser/native target diagnostics for every new `std.net` and `std.http` API; run focused compiler, sema, runtime, and package tests.
- [x] 4.4 Update compiler handbook/API references, examples, limitations, and status from implementation evidence; clearly separate native target execution, compilation, and browser-host evidence.
- [x] 4.5 Commit compiler documentation, run `./scripts/sync-website-content.sh`, review the companion website diff, and validate both repositories; use `--audit-date YYYY-MM-DD` only if the project-status audit date is intentionally updated. Compiler docs were committed in `d760edd`; website sync generated and validated 372 handbook documents, the companion diff is limited to its four generated provenance/status files, `yarn test` passed (80 content tests and 187 Angular tests), and `yarn build` prerendered 378 routes and verified 372 sitemap routes.
- [x] 4.6 Run the complete supported native CI matrix and browser-package verification; record dependency licenses, target-by-target results, known limitations, and release binary-size changes before declaring the change complete. The per-target passing CI evidence is split across 39739088146 and 39702042337 as documented for task 2.8; browser Fetch passes in 39739088146. HTTP/3 remains opt-in; the measured macOS ARM64 executable increase is 1,189,184 bytes (18.0%).
