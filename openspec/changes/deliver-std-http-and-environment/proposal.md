## Why

Zirk's handbook already defines substantial contracts for `std.net`, `std.http`, and `std.environment`, but only `std.io` is currently bundled as an importable Zirk package. Applications therefore cannot use the documented HTTP client/server or permission-checked environment APIs. Delivering them now requires closing the native network-runtime gap as well as publishing the packages through the toolchain.

## What Changes

- Deliver the full documented `std.http` client and server surface, including HTTP/1.1 and HTTP/2, TLS, typed requests and responses, bounded streaming, redirects, pooling, cookies, middleware, SSE, and WebSocket upgrades.
- Implement HTTP `QUERY` according to IETF RFC 10008, including its safe and idempotent semantics, request-content media type handling, `Accept-Query` discovery, server routing, and retry/cache constraints.
- Deliver the DNS, TCP, authenticated TLS, and task-aware I/O substrate required by HTTP across every currently supported native target. Do not expand this dependency into unrelated UDP, local-socket, or network-interface APIs in this change.
- Deliver the existing read-only `std.environment` contract, including exact-scope permission checks, typed parsing, absence/fallback behavior, name listing, and redacted `SecretString` access.
- Bundle the ordinary Zirk package sources and compiler-owned host bindings for `std.net`, `std.http`, and `std.environment`; preserve the compiler's provenance boundary and do not add source-level native markers.
- Update implementation status, limitations, handbook examples and indexes, and the companion website only when implementation evidence supports those claims.

## Capabilities

### New Capabilities
- `zirk-networking`: Native DNS, TCP, authenticated TLS, cancellation-safe transport, and the task-aware I/O substrate required by HTTP.
- `zirk-http`: The documented HTTP client/server contract, including RFC 10008 QUERY, HTTP/1.1 and HTTP/2, streaming, SSE, and WebSocket.
- `zirk-environment`: Permission-checked read-only process environment access, typed values, and secret-safe values.

### Modified Capabilities
- `zirk-standard-library`: Publish and bundle the new standard packages using stable logical imports and compiler-owned host bindings.

## Impact

- Affects `stdlib/std/net`, `stdlib/std/http`, `stdlib/std/environment`, `crates/zirk-cli` standard package resolution, `crates/zirk-sema` trusted standard-library registry, `crates/zirk-runtime` and runtime-core I/O/permission integration, native code generation, and the supported-target CI matrix.
- Introduces reviewed Rust transport/protocol/TLS dependencies or an equivalent audited implementation. The design must choose a bounded, cancellation-safe integration with Zirk's current single-threaded cooperative executor; blocking sockets on the executor thread are not acceptable.
- Public documentation, examples, implementation status, limitations, and roadmap are affected. The companion `../zirk-lang-site` must be synchronized after the implementation is committed and target evidence is reviewed.
- The initial target set is the four currently supported native triples: macOS aarch64, Linux x86_64, Linux aarch64, and Windows x86_64. Browser interpretation and WebAssembly remain outside this change.
