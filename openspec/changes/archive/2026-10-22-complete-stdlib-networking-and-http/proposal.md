## Why

The first native delivery made permission-checked environment access, DNS, TCP, authenticated TLS, and bounded HTTP/1.1 and HTTP/2 usable across the four native targets. Applications still cannot use the documented UDP, local IPC, interface-inspection, HTTP/3/QUIC, browser HTTP, and multi-connection server capabilities; completing these gaps is the next coherent step for production network applications.

## What Changes

- Add bounded, task-aware UDP sockets, local IPC streams, and permission-scoped network-interface snapshots to `std.net` on supported native targets.
- Extend TLS with explicit configuration and credentials, mutual authentication, negotiated-ALPN inspection, and wrapping of caller-owned TCP streams while preserving authenticated defaults and task ownership.
- Add HTTP/3 over QUIC for clients and servers, including protocol negotiation/fallback, bounded flow control, WebSocket bootstrapping over HTTP/3, and origin/address revalidation across connection establishment and migration.
- Complete HTTP lifecycle behavior for bounded concurrent connection admission, independently cancellable handlers, externally requested graceful shutdown, and cross-protocol conformance for HTTP/1.1, HTTP/2, and HTTP/3.
- Close remaining client behavior gaps with bounded stacked content-coding decode, opt-in caller-owned cookie stores, and bounded redirect history with sensitive values redacted.
- Provide browser HTTP client support only through an explicit host-provided capability and origin policy; keep browser execution free of ambient network authority and report unsupported socket/server operations as typed capability errors.
- Establish one secret-redaction boundary for every implemented formatter and structured output sink, and require future debugger, snapshot, and test-report sinks to use it before they are introduced.
- Update public API chapters, feature status, limitations, examples, and the companion website to distinguish this delivery from the still-incomplete standard library as a whole.
- Keep the library inspectable: public APIs, policy, validation, orchestration, and portable behavior SHALL live in bundled `.zrk` sources; compiler/runtime bindings SHALL be narrow, documented host boundaries for operating-system and protocol-engine work.

## Capabilities

### New Capabilities

- `zirk-browser-networking`: Explicit, bounded HTTP capability injection for browser-hosted Zirk execution, with origin policy and typed unsupported-operation outcomes.

### Modified Capabilities

- `zirk-networking`: Add native UDP, local IPC, interface snapshots, and configurable authenticated TLS behavior.
- `zirk-http`: Add HTTP/3/QUIC, bounded multi-connection server lifecycle, browser HTTP transport, response decoding, cookie stores, and redirect observability.
- `zirk-environment`: Enforce shared redaction behavior for current sinks and future sink integration.
- `zirk-permissions`: Define least-authority grants for datagrams, local endpoints, and interface inspection.
- `zirk-browser-compiler`: Permit only explicitly supplied browser networking capabilities while retaining hermetic defaults.
- `zirk-standard-library`: Publish and document the expanded target-specific networking surface and explicit exclusions.

## Impact

This is a cross-cutting compiler/runtime and documentation change affecting bundled `stdlib/std/net`, `stdlib/std/http`, and `stdlib/std/environment` sources; permission manifests and approval fingerprints; native networking transports and TLS; HTTP client, server, router, SSE, and WebSocket lifecycle; browser interpreter capability dispatch; dependency/license and binary-size review; native CI on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; browser-package verification; and the companion `../zirk-lang-site` repository. The implementation must preserve the first delivery's public APIs and fail-closed permission behavior.
