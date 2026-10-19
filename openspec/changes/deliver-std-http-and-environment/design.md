## Context

See `proposal.md` for motivation and scope. The normative public contracts already live in `docs/handbook/04-standard-library/11-std-net.md`, `12-std-http.md`, and `18-std-environment.md`; this change delivers them and records actual implementation status separately from those contracts.

The current loader bundles only `std.io` by registering its package root and embedding its source files. `ADR-021` requires ordinary Zirk package surfaces and permits compiler-owned host bindings only for fully qualified declarations from official standard-library sources. The runtime is a single-threaded cooperative stackful-coroutine executor with no I/O reactor; synchronous host I/O currently blocks. The runtime has no HTTP, DNS, TCP, or TLS dependency. The supported native triples are macOS aarch64, Linux x86_64, Linux aarch64, and Windows x86_64. The browser interpreter is tooling and is explicitly outside native-target equivalence claims.

## Goals / Non-Goals

**Goals:**
- Preserve the existing documented public APIs, typed outcomes, permissions, resource ownership, limits, and cancellation contract.
- Deliver the entire documented HTTP surface and the full read-only environment surface on all four supported native triples.
- Make HTTP waits suspend Zirk tasks without blocking the scheduler thread; keep each Zirk handler on the runtime-owned executor.
- Bundle `std.net`, `std.http`, and `std.environment` as ordinary source packages while keeping host operations behind provenance-checked compiler/runtime bindings.
- Treat RFC 10008 and its registered `QUERY`/`Accept-Query` semantics as the normative QUERY baseline.

**Non-Goals:**
- Implement every API documented by `std.net`: UDP, local sockets, interface enumeration, and unrelated DNS record families can ship separately. This change builds only the DNS, TCP, and TLS support required by HTTP.
- Add HTTP/3 or QUIC; the HTTP contract explicitly leaves that to a separate package.
- Add browser networking, a WebAssembly Zirk target, or automatic browser fetch behavior.
- Add mutation APIs for the process environment.
- Add a web framework, dependency injection, templating, ORM, or application-specific routing conventions.

## Decisions

### Keep the source/runtime trust boundary

Publish each API as ordinary Zirk source under its stable logical package root. Extend the existing package-root and embedded-source mechanism for development and installed toolchains. Add only the fully qualified host operations that cannot be expressed in Zirk to the compiler-owned registry; do not add source-level `native`, `intrinsic`, or ABI annotations. This follows `ADR-021` and lets the API evolve without making third-party name collisions privileged.

**Alternative considered:** implement all APIs in Rust and expose them as compiler intrinsics. Rejected because it bypasses the public package model, enlarges compiler coupling, and prevents the ordinary-source contract from being checked.

### Use the existing contracts as the compatibility baseline

Treat the HTTP, network, and environment handbook chapters as normative input, then encode user-visible behavior in the new capability specs. Do not silently simplify typed errors, strict URL/header parsing, permission boundaries, limits, retries, or cleanup to get an early demo. Status pages and public examples may report only the subset with native runtime and cross-target evidence.

**Alternative considered:** invent a smaller replacement API for the first version. Rejected because the user selected the complete existing HTTP contract and the repository already promises these signatures and security properties.

### Keep HTTP client configuration and handler failures explicit

Use one validated, extensible `HTTPClientOptions` value for client timeouts and
future client policy instead of adding a separate timeout argument to each
constructor. A route or middleware handler that returns a structured error is
converted to a generic HTTP 500 response; the server continues accepting later
requests. Error details are not exposed in the response. This keeps application
failure handling local to one request while preserving the server lifecycle.

Represent request content with one closed `HTTPBody` enum: `Text`, `Json`,
`Bytes`, `Form`, `Multipart`, or `Stream`. One-shot and reusable-client body
request APIs accept exactly one variant. This makes the selected encoding and
ownership model explicit at the call site and prevents combining multiple body
representations in one request. `Stream` keeps its framing headers and producer
together; buffered variants own or copy their input according to their public
value semantics. Keep convenience methods such as `post_json` as thin wrappers
over this representation rather than as separate transport implementations.
For the existing lower-level byte-buffer APIs that also accept explicit
headers, the buffer is the `Bytes` representation; those APIs do not perform
text, JSON, form, or multipart encoding. Preserve them for callers that need
custom headers while making the body encoding unambiguous.

For dependency licensing, the general allowlist is limited to permissive
licenses (0BSD, Apache-2.0, BSD-3-Clause, ISC, and MIT). Any required license
outside that set must be reviewed and scoped to the specific dependency rather
than accepted globally. The current `Unicode-3.0` exceptions are package-scoped
to ICU4X/IDNA and `unicode-ident`; the embedded Mozilla Public Suffix List
remains tracked separately under its MPL-2.0 notice.

### Bridge protocol futures to the existing cooperative executor

Keep Zirk task and handler execution on the owning Zirk executor thread. Drive network readiness through a native reactor and adapt protocol-library futures/wakers so a pending socket operation parks the current stackful Zirk task and wakes it when readiness or completion arrives. Bound reactor registrations, pending accepts, connections, streams, request queues, and body buffers. No networking worker may invoke generated Zirk code or access its task-local GC roots directly.

Before committing to the full protocol stack, build a focused vertical spike on macOS aarch64 and compile/run it on the other native targets. The spike must prove that an HTTP client request and a server handler can suspend/resume through the reactor, preserve GC roots across suspension, and cancel/close exactly once. It is a gate: if a direct readiness/future bridge cannot meet those invariants, revise this design before implementing public APIs.

**Alternatives considered:**
- Blocking sockets on the scheduler thread. Rejected because one slow peer would stall all runnable Zirk tasks and violate the runtime I/O contract.
- One OS thread per connection or request. Rejected because resource growth would be unbounded and server capacity would depend on thread count.
- A separate general-purpose async runtime that calls Zirk handlers on its worker threads. Rejected because current task stacks and GC-root chains are executor-owned and cannot be entered from arbitrary threads.
- A maintained Rust HTTP/TLS implementation with a narrow readiness adapter. Preferred over writing parsers, TLS, HTTP/2, or WebSocket framing from scratch; exact crates and crypto backend are selected only after license, target, maintenance, security-advisory, binary-size, and feature review.

**Implementation spike:** Tokio is the first reactor candidate, enabled with only
`io-util`, `net`, `rt-multi-thread`, and `time` in `zirk-runtime`. Its runtime
uses one named I/O-driver worker; the Zirk executor manually polls each
protocol future on the owning coroutine stack. A task-specific waker publishes
only the executor task id, and the scheduler resumes it at a safe boundary.
This avoids one thread per connection and prevents the Tokio worker from
running Zirk code. The macOS aarch64 loopback test verifies a Zirk-owned client
and server task, unrelated task progress during pending I/O, GC reachability
of a suspended task's shadow-frame root, and future drop on cancellation. This
includes a real Hyper HTTP/1.1 loopback request and
response, but is still not a complete server/transport gate. Current validation is `cargo test -p
zirk-runtime --lib` (372 passed, 3 ignored), strict runtime Clippy, and
`cargo check -p zirk-runtime --lib --target ...` for Linux x86_64, Linux
aarch64, and Windows x86_64, in addition to native macOS aarch64 execution.
The candidate dependency/advisory audit and target compile gate are complete;
native Linux/Windows execution and final linked application-size measurements
remain open.

**Protocol-stack decision:** HTTP/2 and TLS RFCs do not prescribe an
implementation stack size. Zirk keeps the runtime contract of 128 KiB for
spawned tasks (and 8 MiB for the root task). TLS client and server handshakes,
plus the HTTP/2 client handshake and connection state machine, run as native
Rust work on the existing single-thread Tokio reactor worker. HTTP/2 response
metadata and body chunks capped at 64 KiB return through a bounded two-item
channel. A cancellation-aware task owner aborts worker work if its Zirk caller
is cancelled. No generated Zirk handler or task-local GC state runs on that
worker. HTTP/1.1 protocol futures and server handler futures continue to run
on their owning Zirk tasks. This keeps per-task stack memory stable while
containing the deepest library call paths; focused macOS tests verify worker
ownership and a spawned 128 KiB task completing HTTPS/HTTP/2.

The runtime waiter registry is bounded to the executor's 4,096 active-task
limit. It deduplicates each `(executor, task)` registration and wakeup,
unregisters canceled waiters, and reports capacity exhaustion so the owning
network future is dropped instead of busy-polling. Runtime tests cover queue
saturation, duplicate notification, cancellation cleanup, timeout wakeup,
loopback readiness, task fairness, and GC roots across a pending socket.

**Dependency decision and audit (2026-10-15):** use Tokio 1.53.3 with only
`io-util`, `net`, `rt-multi-thread`, and `time` enabled; its resolved reactor
uses Mio 1.2.4 and socket2 0.6.5. The protocol graph is Hyper 1.12.0 with
`client`, `server`, `http1`, and `http2`, plus hyper-util 0.1.21 with
`client-legacy`, `server-auto`, `http1`, `http2`, and `tokio`; use `http`
1.5.0, `http-body-util` 0.1.5, and bytes 1.12.1 for values and bodies. TLS
uses tokio-rustls 0.26.6 with stable rustls 0.23.46, disabling default
features and selecting the `ring` crypto provider; `rustls-native-certs` 0.8.5
loads system trust roots. TLS 1.2 support is compiled for explicit
configuration, but runtime defaults are TLS 1.3. Rustls Brotli certificate
decompression stays disabled. WebSockets use tokio-tungstenite 0.30.0 with
default features disabled and only `connect`, `handshake`, and
`rustls-tls-native-roots` enabled. h2 0.4.20 is resolved by Hyper. The native
server now implements HTTP/2 extended CONNECT for WebSockets; a compiled-Zirk
E2E verifies the handshake and message exchange against an independent Hyper
peer on macOS aarch64. The WebSocket client negotiates HTTP/2 over TLS through
ALPN and uses RFC 8441 extended CONNECT only after the peer advertises
`SETTINGS_ENABLE_CONNECT_PROTOCOL`; otherwise it does not send an extended
CONNECT request. Cleartext WebSocket connections remain HTTP/1.1-only. This
single-target proof does not establish cross-target interoperability.

Incremental client response decoding adds `async-compression` 0.4.51 with
default features disabled and only `tokio`, `brotli`, `gzip`, and `zlib`
enabled. It implements the HTTP `br`, `gzip`, and zlib-wrapped `deflate`
content codings. The decoder runs on the Tokio reactor behind a two-item output
queue, caps each read at 64 KiB, limits compressed and expanded input to 16 MiB,
and limits expanded output to 16 MiB while rejecting expansion above 1000:1.
Server encoders run on Tokio tasks so codec state does not use the 128 KiB Zirk
task stack. Buffered response encoders read and emit at most 64 KiB per
iteration, yield cooperatively, cap encoded output at 16 MiB, and abort with
the owning request. Streaming encoders use bounded input/output channels and
preserve generated producer execution on its owner task. Request-body
decoding and explicit CPU deadlines remain outside the implemented slice; the
dependency addition still requires the final security, license-policy, and
linked-size review in task 7.2.

The earlier dependency snapshot from 2026-10-16 counted 175 registry packages
and 189 lockfile crates; it is historical, not the current graph. The fresh
2026-10-17 inventory at the end of this document covers 205 registry packages
and 219 lockfile crates. License expressions include permissive alternatives
and `Unicode-3.0`; the included Mozilla Public Suffix List data is MPL-2.0.
The repository now enforces a crate-level allowlist in `deny.toml` for 0BSD, Apache-2.0, BSD-3-Clause, ISC, and MIT. `Unicode-3.0` is handled through exact-version per-crate exceptions for the current ICU4X/IDNA and `unicode-ident` dependencies, rather than a global allowance; the current graph does not require a global Zlib allowance. CI installs cargo-deny 0.20.2 and runs the license check on Linux x86_64. Version 0.18.4 cannot parse the current RustSec advisory database's CVSS 4.0 entries; 0.20.2 passes the complete `cargo-deny check` and the exact CI license command. New crate licenses fail closed. The bundled Mozilla Public Suffix List data remains separately identified as MPL-2.0 because cargo-deny evaluates crate license expressions rather than embedded data notices.
The current lockfile retains the audited Hyper, h2, Tokio, and rustls
dependency families. Any h2 0.4 resolution must stay at or above 0.4.16
(RUSTSEC-2026-0258); rustls 0.23 must stay at or above 0.23.46
(RUSTSEC-2026-0285, -0346, -0347, -0348, and -0349).

The in-memory cookie substrate uses `psl2` 0.1.32 with default features
disabled: the bundled Mozilla Public Suffix List is consulted without IDNA
re-normalization (the validated URL host is already ASCII/punycode) or the
optional fast-lookup table. This keeps its PSL check allocation-free and avoids
the extra lookup index in the linked binary. The crate code is MIT/Apache-2.0;
its re-encoded Mozilla PSL dataset is MPL-2.0 and must be retained in the
dependency/license inventory. Cookie values are bounded, zeroized on removal
and never included in debug output; generated Cookie header material is
zeroizing and redacted as well. The jar is integrated with the public reusable
`HTTPClient` behind an explicit opt-in; a signed-approval compiled-Zirk test
verifies the cookie round-trip over a reused connection.

The bounded cookie jar now supports up to 3,000 entries (50 per domain) and
1 MiB of tracked storage, matching the current cookie specification's
recommended general-purpose client capacity while retaining a deterministic
memory ceiling. It ignores an equal-host public-suffix Domain attribute and
stores the cookie host-only, compares security prefixes case-insensitively,
requires explicit `Path=/` for `__Host-`, caps cookie age to 400 days, and
rejects insecure-origin overlays of existing Secure cookies. These rules are
verified in runtime-core tests and apply to the jar used by `HTTPClient`.

The exact candidate feature graph passed `cargo check` for macOS aarch64,
Linux aarch64, Linux x86_64, and Windows x86_64 MSVC. The Linux checks ran in
Docker (`aarch64` natively and `x86_64` under emulation); Windows used
`cargo-xwin`. These are target compilation checks, not native runtime tests.
For an optimized macOS arm64 Rust reference executable, on-disk sizes were
644,224 bytes for the Tokio reactor alone, 2,297,952 bytes for a Hyper HTTP/1
client/server reference, and 3,660,832 bytes for the candidate with HTTP/2
server types, TLS configuration, and a WebSocket handshake reference linked.
This measures a small `cargo build --release` probe whose futures are
constructed and dropped, not a Zirk application or an executed TLS/WebSocket
exchange; use it only as an early dependency-cost comparison. The dependency
gate is complete, while native transport execution and final application-size
measurement remain later validation tasks.

### Keep the generated-code boundary bounded and cancellation-safe

The compiler/runtime boundary remains the repository's existing static runtime
C ABI (fixed-width scalars, pointers, and out-parameters); Rust types, trait
objects, futures, and `Result` never cross it. Transport APIs exposed to
generated code use `extern "C"`. The owner-task HTTP server callbacks and the
two server entry points that invoke them use `extern "C-unwind"` as a narrow
exception: Zirk's cooperative cancellation is represented as an unwind on the
owning task stack. Generated callback thunks carry unwind metadata, and the
HTTP/2 dispatcher consumes cancellation from an individual handler when it
joins that task, without failing or canceling sibling streams. Cancellation
of the dispatcher itself and other failures still propagate. No callback runs
on the protocol reactor. Any transport exposed to generated code is an opaque
runtime-owned handle with
explicit close, and byte transfers use caller-provided pointer/length pairs
with a bounded maximum and an out-parameter for the transferred length/status.
The runtime validates handles and buffer bounds before access and maps
ordinary transport failures to stable typed statuses. The call is made on the current Zirk
task stack; when its Rust future becomes pending, the bridge parks that task and
resumes it on the same executor before returning. Cancellation drops the
pending operation on the owner stack, unregisters its waiter, and closes only
resources owned by that operation. No borrowed generated buffer or movable GC
pointer may be retained across suspension; values that must survive are copied
into runtime-owned bounded storage or remain in a registered Zirk shadow frame.

For a managed value whose lifetime must outlive one generated function
activation (for example, a server response producer returned from a handler),
the collector now also exposes an explicit global-root handle. Registration
stores the managed object value in a bounded table of at most 4,096 roots; the
collector traces that table, and the runtime must remove the handle on normal
completion, producer failure, peer disconnect, or task cancellation. The
handle retains only the managed capture object: callback dispatch and all
managed access still occur on the owning Zirk task stack. The collector test
forces collection with and without the handle and verifies retention and
release. This does not replace ordinary shadow frames for lexical lifetimes.

This shape matches the existing codegen runtime declarations and opaque-handle
patterns. It is the transport ABI design constraint; the concrete symbols,
handle table, bounds, and shutdown implementation remain implementation work.

The HTTP server handler is a generated-code callback and must obey the same
ownership rule. The runtime may parse a bounded request and construct response
I/O futures, but it must invoke the Zirk handler only while polling the
connection on the owning Zirk task stack. A generated adapter translates the
bounded request/response ABI to the declared Zirk value types and invokes the
handler through the language callable representation. The caller's callable
stays in a registered shadow-frame root for the full server operation, so its
managed capture block remains live across socket suspension. Reactor workers
must never dereference that capture block, invoke its function pointer, retain
request/response pointers, or enter Zirk code. Cancellation drops the pending
connection future on the owner stack before releasing the rooted callable.

This is a required implementation boundary for `HTTPServer.serve_once` and
the long-lived server loop, rather than a callback optimization. The public
route API may build over that primitive, but route handlers, middleware, and
their captures must preserve the same owner-stack dispatch and root lifetime.

Client upload producers follow the same boundary. A Zirk producer callable
emits one bounded chunk at a time on its owning Zirk stack. The runtime copies
each chunk into native-owned bytes and sends it through a single-slot bounded
channel to the HTTP body consumed by the protocol driver. The producer's
callable and captures stay in a registered shadow-frame root until the upload
ends or is canceled. Reactor workers consume channel data only; they never
call the producer or retain managed pointers. Waiting for channel capacity
parks the producer task and gives the protocol driver backpressure. A
one-shot producer is not replayable, so the client must not automatically
retry it after transmission may have started. Retries require a separately
declared replayable producer factory and permission revalidation.

The runtime now has an owner-task `Http1ReusableConnection` primitive and a
bounded `HttpClientPool`. Each pool supports at most 32 normalized origins and
one reusable HTTP/1.1 connection per origin, applies LRU eviction, retains an
abort-on-drop connection driver, buffers responses only within the existing
body bound, and checks the current network grant before every request. The
pool uses one immutable TLS configuration and may use an explicit per-client
HTTP or HTTPS forward proxy. HTTP requests use absolute-form targets; HTTPS
uses CONNECT followed by destination-authenticated TLS. An HTTPS proxy is
authenticated first, and an HTTPS destination then receives a separate nested
TLS handshake. The proxy and target origins require separate network
authority. Optional Basic credentials are accepted for loopback HTTP proxies
or HTTPS proxies after proxy certificate and hostname validation; remote
cleartext HTTP proxy credentials fail closed. Proxy credentials use
`SecretString` and authorization values are zeroized/redacted where the runtime
owns them. Caller-supplied trust roots validate HTTPS proxy and destination
peers by their independent hostnames.
Optional per-client cookies use the bounded, zeroizing `CookieJar`. Opaque client FFI
handles are capped at 256 and bound to the verified executor, project, and
manifest identity; sibling tasks in that executor may share a handle. Request
admission uses a FIFO cooperative queue, HTTP/1.1 pool access remains
serialized, and an established HTTP/2 connection accepts up to eight
simultaneous streams. Closing waits for active requests, and executor shutdown
removes the handle after its tasks finish. Loopback tests verify cross-task
HTTP/1.1 use, simultaneous HTTP/2 streams over one authenticated connection,
cookie round-trip, cancellation-safe close retry, and rejection after a grant
or identity mismatch. The public `HTTPClient` Zirk type exposes creation,
request, opt-in cookies, and close through compiler-owned bindings and LLVM
lowering. Windows execution and the complete cross-target client policy matrix
remain required; these tests do not establish them.

### Deliver only the `std.net` transport substrate HTTP consumes

Expose the address, DNS, TCP stream/listener, and authenticated TLS APIs required by `std.http`, with typed outcomes and `Resource` cleanup. Keep UDP, local sockets, interface inspection, and transport features unrelated to HTTP out of this change. Make their status explicit so adding this package does not imply the entire `std.net` handbook chapter shipped.

### Implement the complete HTTP contract in coherent layers

Build the protocol in dependency order: URL/header/status values and typed request configuration; client and server HTTP/1.1; body streaming and bounds; HTTP/2 negotiation, multiplexing, flow control, and shutdown; redirect, cookie, proxy, cache, and retry policy; then SSE and WebSocket (including HTTP/2 extended CONNECT). Both client and server support `QUERY`. Server SSE is constructed through `HTTPResponse.from_sse_stream(producer, heartbeat_ms?)`; the optional interval is disabled by default and accepts 1,000–3,600,000 ms. Heartbeats are SSE comments emitted only while the producer is inactive, through the same bounded response channel, and stop with the response producer. A stream or feature is not marked available until its semantics, permissions, limits, cancellation, and native-target evidence pass together.

QUERY follows RFC 10008 rather than a private GET/POST hybrid: safe and idempotent semantics, exactly one valid `Content-Type` on every request, including empty content, `Accept-Query` discovery when configured, and retries only for replayable content and permitted protocol conditions. Cache keys and reuse must account for the request content as required by HTTP caching semantics; query safety does not authorize resource mutation.

**Alternative considered:** implement QUERY as an alias for POST or GET-with-body. Rejected because that would lose its registered safety/idempotence properties and defined content semantics.

### Keep environment reads synchronous and typed

Implement `std.environment` using process-environment access at the runtime boundary. Validate names and platform strings, check the effective signed grant before looking up a value, distinguish absence from permission/decoding/platform/parse failure, and never mutate the parent environment. Typed reads use an explicit compile-time parsing contract, not reflection or guessed constructors. `SecretString` stays redacted and has no general string conversion; only specifically authorized sink APIs may accept it.

The first authorized reveal sink is `HTTP.request_with_secret_header`. It is deliberately write-only, accepts a `SecretString` directly, requires HTTPS and the exact approved network origin, rejects an already populated field with that name, and does not follow redirects. This avoids adding a general-purpose reveal or conversion API. Other sinks remain out of scope until they have a concrete authority model and equivalent non-disclosure guarantees. The environment FFI zeroizes its temporary native `String` after copying into the managed heap. The current collector does not guarantee clearing managed object memory on collection, so `SecretString` guarantees output redaction and sink discipline, not hardened at-rest memory erasure.

**Alternatives considered:** make environment reads ambient globals, return `null` on any failure, or expose raw secret strings. Rejected because they hide permission/error distinctions, make tests and libraries order-dependent, and leak secrets through ordinary formatting.

The local approval flow is explicit: `zirk permissions approve <project>` prints the exact requested scopes and requires the user to type `approve`. The saved HMAC-authenticated record binds the canonical project path, exact `.zkinit` fingerprint, and sorted scopes. `zirk run` checks the saved record and strips inherited grant tokens before launching generated code; the native runtime verifies the token again. Editing the manifest invalidates the approval. The user-owned key and records live under `~/.local/share/zirk/security` on Unix-like systems and `%LOCALAPPDATA%/Zirk/security` on Windows. This prototype protects key material from other local accounts with filesystem permissions; same-account native code remains outside the Zirk sandbox threat boundary.

### Prove targets and behavior at the layer that owns them

Run transport, HTTP, and environment conformance suites on macOS aarch64, Linux x86_64, Linux aarch64, and Windows x86_64. A successful frontend check, cross-target compile, or Rust unit test alone does not establish native socket, TLS, cancellation, server, or permission behavior. Use local loopback fixtures, hostile framing/body inputs, permission-denial cases, cancellation/GC stress, interoperability fixtures, and target-native CI execution. Browser tests may prove only that unsupported I/O fails closed, not that networking is delivered there.

## Risks / Trade-offs

- **[Reactor/future bridge corrupts scheduler ownership or stalls other tasks] →** Make the macOS vertical spike an implementation gate; test wakeups, nested tasks, GC, cancellation, and fairness before public APIs depend on it.
- **[A protocol dependency does not build or behave consistently on one supported target] →** Audit dependency features/crypto backends first, run target CI early, and fail closed for a feature until that target has native evidence.
- **[Adding TLS and HTTP/2 increases compile time and executable size] →** Measure minimal client and server artifacts separately, disable unrelated dependency features, and record the size cost before accepting the dependency set.
- **[Networking expands privilege or leaks credentials across redirects/pools] →** Revalidate grants at every dynamic destination and include origin, proxy, TLS identity, and principal in pool/cache scope; test authorization redaction and redirect stripping.
- **[HTTP parser, compression, multipart, or WebSocket inputs exhaust resources] →** Use independent configurable bounds, adversarial fixtures, fuzz/property tests, and bounded queues before enabling each protocol feature.
- **[Environment generic parsing requires unsupported type-system surface] →** Define the constrained parse contract as a prerequisite and test built-in and user-defined implementations; do not replace it with runtime reflection.
- **[The change's breadth obscures partial delivery] →** Keep completion gates by layer and target, update feature-status evidence per delivered slice, and do not claim full `std.http` until every documented surface is implemented and validated.

## Migration Plan

This is additive: existing source programs and imports remain unchanged. New package roots are bundled with the compiler/runtime distribution. Projects that use network or environment APIs declare and approve the exact corresponding `.zkinit` scopes; existing projects gain no authority automatically. Rollback removes the new package roots, registry entries, and runtime dependencies without changing existing package resolution. Public docs and the companion site are synchronized only after the implementation commit and reviewed native evidence exist.

**Fresh dependency and link-size evidence (2026-10-17):** `cargo audit` scanned 219 locked crate dependencies and exited successfully with no reported advisories. `cargo metadata --locked` reports license metadata for all 205/205 registry packages; the current `deny.toml` allowlist and package-scoped Unicode exceptions pass `cargo-deny 0.20.2 check licenses`, and CI runs that pinned check, and `psl2`'s included Mozilla PSL dataset remains separately identified as MPL-2.0. A current macOS arm64 application-link probe used `CodegenOptimizationLevel::None`: a minimal `std.io` app is 3,622,520 bytes; typed `std.environment` parsing is 4,451,480 bytes; `std.net::address` parsing is 3,672,040 bytes; a TCP client is 6,817,368 bytes; and an HTTPS `std.http` GET client is 17,926,904 bytes. The HTTP executable's `__TEXT` segment is 9,355,264 bytes. These are unstripped, unoptimized native app outputs, not release packages. Linux, Windows, and optimized/stripped linked-size review remains open.
