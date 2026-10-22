# zirk-browser-networking Specification

## Purpose

Defines explicit, per-execution outbound HTTP access for browser-hosted Zirk programs while preserving browser security policy and the compiler package's hermetic default.

## Requirements

### Requirement: Browser HTTP authority is explicit, origin-scoped, and ephemeral

An embedding host SHALL create a browser-network capability only from an explicit caller decision and SHALL bind it to one execution, an exact normalized origin allowlist, and bounded request/response limits. A capability SHALL not be forgeable from Zirk source, inherited by a later execution, serialized into source output, or widened by a dependency. The host SHALL use the browser's native fetch transport and enforce its CORS, mixed-content, credential, and TLS rules.

#### Scenario: Authorize one HTTPS origin for one run
- **WHEN** a host grants a capability for one HTTPS origin and starts a Zirk execution
- **THEN** requests to that origin are permitted only during that execution and within configured limits

#### Scenario: Reject a destination outside the allowlist
- **WHEN** Zirk code requests an origin not present in the injected capability
- **THEN** the request returns a typed permission error before invoking the browser fetch API

#### Scenario: Capability expires with its execution
- **WHEN** an execution returns, fails, or is cancelled
- **THEN** its capability and pending response bodies are revoked and cannot be reused by another execution

### Requirement: Browser responses and cancellation are bounded and typed

Browser HTTP requests SHALL preserve the public HTTP request/response result model where browser semantics permit. Request count, headers, bodies, response bytes, redirects, and outstanding streams SHALL have hard package limits. Redirects SHALL be revalidated against the same origin allowlist at each hop where the browser exposes the destination; opaque CORS responses SHALL not be presented as readable successful Zirk responses. Cancellation SHALL abort the browser request and release pending resources.

#### Scenario: Read a bounded browser response
- **WHEN** an allowed request returns a readable response within configured limits
- **THEN** the host converts its visible status, headers, and body to the documented typed response

#### Scenario: Reject an oversized browser response
- **WHEN** a response exceeds the configured byte limit
- **THEN** the body is aborted and the caller receives a typed size error without retaining an unbounded buffer

#### Scenario: Cancel an in-flight request
- **WHEN** structured cancellation reaches a task awaiting browser HTTP
- **THEN** the host aborts the fetch, resumes the owning task with a typed cancellation outcome, and releases the response reader

#### Scenario: Browser CORS makes a response opaque
- **WHEN** browser security policy hides the response status, headers, or body
- **THEN** the operation returns a typed browser-policy error instead of fabricating an HTTP response

### Requirement: Browser HTTP suspends execution without blocking the worker

Browser HTTP SHALL run through an additive asynchronous execution operation. When a reachable Zirk HTTP operation needs network I/O, the interpreter SHALL suspend with its frames, heap, instruction count, and trace intact, then resume with exactly one bounded typed response or failure. It SHALL NOT block the JavaScript event loop, use synchronous XHR, or restart `main` after a response. Existing synchronous `check`, `format`, and `run` operations SHALL remain default-deny and SHALL report a typed capability failure for reachable HTTP effects.

#### Scenario: Resume after browser fetch completes
- **WHEN** an authorized Zirk HTTP request is suspended and browser `fetch` completes
- **THEN** the same interpreter execution resumes at the request result with its original state and remaining instruction budget

#### Scenario: Cancel a suspended browser request
- **WHEN** the host cancels an execution while it awaits browser HTTP
- **THEN** the fetch is aborted, the interpreter returns a typed cancellation outcome, and its capability and response buffers are released

#### Scenario: Keep synchronous browser execution default-deny
- **WHEN** synchronous `run` encounters a reachable HTTP operation
- **THEN** it returns a capability diagnostic and invokes no browser network API

### Requirement: Browser networking never emulates native socket authority

The browser capability SHALL support outbound HTTP only. It SHALL NOT expose TCP, UDP, TLS streams, local IPC, network-interface inspection, server binding, packet access, custom DNS resolution, or control over the negotiated HTTP version. These operations SHALL remain unavailable in browser execution and SHALL be discoverable through the generated compatibility manifest and diagnostics.

#### Scenario: Attempt a server or raw socket operation
- **WHEN** browser code attempts to bind a server or open a raw socket
- **THEN** the compiler or runtime returns a typed unsupported-capability error before network activity

#### Scenario: Inspect the browser capability manifest
- **WHEN** a consumer reads browser package capabilities
- **THEN** it can identify outbound HTTP as explicitly injectable and all native socket operations as unavailable
